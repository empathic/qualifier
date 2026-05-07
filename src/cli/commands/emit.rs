use chrono::Utc;
use clap::Args as ClapArgs;
use std::io::{self, BufRead};
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, IssuerType, Record};
use crate::cli::commands::record::{detect_issuer, normalize_issuer_uri};
use crate::qual_file;

#[derive(ClapArgs)]
pub struct Args {
    /// Record type. Spec types: `annotation`, `epoch`, `dependency`,
    /// `license`, `security-advisory`, `perf-measurement`. Or any custom URI.
    /// Required unless --stdin is set.
    pub record_type: Option<String>,

    /// Subject — the artifact qualified name. No span is encoded here;
    /// emit is the low-level shape.
    /// Required unless --stdin is set.
    pub subject: Option<String>,

    /// Raw body as a JSON object (e.g., `{"kind":"pass","summary":"ok"}`).
    /// The body is passed through unchanged.
    #[arg(long)]
    pub body: Option<String>,

    /// Issuer identity URI (defaults to VCS user email with mailto:).
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type (human, ai, tool, unknown).
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// Explicit .qual file to write to (overrides layout resolution).
    #[arg(long)]
    pub file: Option<String>,

    /// Read JSONL records from stdin (batch mode). Each line is a complete
    /// record (envelope + body). The `record_type` and `subject` positionals,
    /// when supplied, become defaults applied to lines missing those fields.
    #[arg(long)]
    pub stdin: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    if args.stdin {
        return run_batch(args.record_type.as_deref(), args.subject.as_deref());
    }

    let record_type = args.record_type.as_deref().ok_or_else(|| {
        crate::Error::Validation("<type> is required (or use --stdin for batch mode)".into())
    })?;

    let subject = args.subject.clone().ok_or_else(|| {
        crate::Error::Validation("<subject> is required (or use --stdin for batch mode)".into())
    })?;

    let body_str = args.body.as_deref().ok_or_else(|| {
        crate::Error::Validation(
            "--body '<JSON>' is required (or use --stdin for batch mode)".into(),
        )
    })?;
    let body_value: serde_json::Value = serde_json::from_str(body_str)
        .map_err(|e| crate::Error::Validation(format!("--body must be valid JSON: {e}")))?;

    let issuer = normalize_issuer_uri(
        args.issuer
            .or_else(detect_issuer)
            .unwrap_or_else(|| "mailto:unknown@localhost".into()),
    );

    let issuer_type = match &args.issuer_type {
        Some(s) => Some(s.parse::<IssuerType>().map_err(crate::Error::Validation)?),
        None => None,
    };

    let record = build_record(record_type, &subject, issuer, issuer_type, body_value)?;

    // For annotation type, run validation. For other types, no validation
    // (yet); for unknown types, the body is opaque.
    if let Some(att) = record.as_annotation() {
        let errors = annotation::validate(att);
        if !errors.is_empty() {
            return Err(crate::Error::Validation(errors.join("; ")));
        }
    }

    let qual_path =
        qual_file::resolve_qual_path(record.subject(), args.file.as_deref().map(Path::new))?;

    qual_file::append(&qual_path, &record)?;

    println!(
        "Emitted {} {} {}",
        record.record_type(),
        record.subject(),
        record.id(),
    );

    Ok(())
}

fn build_record(
    record_type: &str,
    subject: &str,
    issuer: String,
    issuer_type: Option<IssuerType>,
    body: serde_json::Value,
) -> crate::Result<Record> {
    let now = Utc::now();
    match record_type {
        "annotation" => {
            // For annotation, validate body conforms to AnnotationBody.
            let body: AnnotationBody = serde_json::from_value(body).map_err(|e| {
                crate::Error::Validation(format!("body does not conform to annotation schema: {e}"))
            })?;
            let att = annotation::finalize(Annotation {
                metabox: "1".into(),
                record_type: "annotation".into(),
                subject: subject.into(),
                issuer,
                issuer_type,
                created_at: now,
                id: String::new(),
                body,
            });
            Ok(Record::Annotation(Box::new(att)))
        }
        _ => {
            // For non-annotation types we round-trip through Unknown so the
            // body is preserved verbatim. This covers built-in types other
            // than annotation (epoch, dependency, license, ...) and any
            // custom URI types.
            let mut envelope = serde_json::Map::new();
            envelope.insert("metabox".into(), serde_json::Value::String("1".into()));
            envelope.insert("type".into(), serde_json::Value::String(record_type.into()));
            envelope.insert("subject".into(), serde_json::Value::String(subject.into()));
            envelope.insert("issuer".into(), serde_json::Value::String(issuer));
            if let Some(it) = issuer_type {
                envelope.insert(
                    "issuer_type".into(),
                    serde_json::Value::String(it.to_string()),
                );
            }
            envelope.insert(
                "created_at".into(),
                serde_json::Value::String(now.to_rfc3339()),
            );
            envelope.insert("id".into(), serde_json::Value::String(String::new()));
            envelope.insert("body".into(), body);

            let value = serde_json::Value::Object(envelope);

            // Round-trip through Record so well-known types (epoch,
            // dependency) deserialize into their typed variants and get
            // their proper IDs computed by finalize_record.
            let r: Record = serde_json::from_value(value)?;
            Ok(annotation::finalize_record(r))
        }
    }
}

fn run_batch(default_type: Option<&str>, default_subject: Option<&str>) -> crate::Result<()> {
    let stdin = io::stdin();
    let mut count = 0;

    for (line_idx, line) in stdin.lock().lines().enumerate() {
        let line_no = line_idx + 1;
        let line = line.map_err(|e| stdin_err(line_no, e.to_string()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        let mut value: serde_json::Value =
            serde_json::from_str(trimmed).map_err(|e| stdin_err(line_no, e.to_string()))?;
        if let Some(obj) = value.as_object_mut() {
            if !obj.contains_key("type")
                && let Some(t) = default_type
            {
                obj.insert("type".into(), serde_json::Value::String(t.into()));
            }
            if !obj.contains_key("subject")
                && let Some(s) = default_subject
            {
                obj.insert("subject".into(), serde_json::Value::String(s.into()));
            }
        }

        let record: Record =
            serde_json::from_value(value).map_err(|e| stdin_err(line_no, e.to_string()))?;
        let record = annotation::finalize_record(record);

        if let Some(att) = record.as_annotation() {
            let errors = annotation::validate(att);
            if !errors.is_empty() {
                return Err(stdin_err(line_no, errors.join("; ")));
            }
        }

        let qual_path = qual_file::resolve_qual_path(record.subject(), None)
            .map_err(|e| stdin_err(line_no, e.to_string()))?;
        qual_file::append(&qual_path, &record).map_err(|e| stdin_err(line_no, e.to_string()))?;

        let id = record.id();
        let id_short = if id.len() >= 8 { &id[..8] } else { id };
        println!(
            "emitted  {:<24} {}  id: {}",
            record.record_type(),
            record.subject(),
            id_short,
        );
        count += 1;
    }

    eprintln!("Emitted {count} records from stdin");
    Ok(())
}

fn stdin_err(line_no: usize, msg: String) -> crate::Error {
    crate::Error::Validation(format!("stdin line {line_no}: {msg}"))
}
