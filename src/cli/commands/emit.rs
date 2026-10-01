use chrono::Utc;
use clap::Args as ClapArgs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use crate::annotation::{self, Annotation, AnnotationBody, IssuerType, Record};
use crate::cli::commands::record::{BatchView, set_subject};
use crate::cli::provenance;
use crate::cli::targets;

#[derive(ClapArgs)]
pub struct Args {
    /// Record type. Spec types: `annotation`, `epoch`, `dependency`,
    /// `license`, `security-advisory`, `perf-measurement`. Or any custom URI.
    /// Required unless --stdin is set.
    pub record_type: Option<String>,

    /// Subject — the artifact path, relative to the current directory and
    /// stored relative to the project root, as `record` does. No span is
    /// encoded here; emit is the low-level shape.
    /// Required unless --stdin is set.
    pub subject: Option<String>,

    /// Raw body as a JSON object (e.g., `{"kind":"pass","summary":"ok"}`).
    /// The body is passed through unchanged.
    #[arg(long)]
    pub body: Option<String>,

    /// Issuer identity URI (defaults to QUALIFIER_ISSUER, then `issuer` in
    /// .qualifier.toml or the user config, then the VCS user email).
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type: human, ai, tool, unknown (defaults to QUALIFIER_ISSUER_TYPE, then detected agent harness).
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// Explicit .qual file to write to (overrides layout resolution).
    #[arg(long)]
    pub file: Option<String>,

    /// Read JSONL records from stdin (batch mode). Each line is a complete
    /// record (envelope + body) whose `subject` is relative to the project
    /// root. The `record_type` and `subject` positionals, when supplied,
    /// become defaults applied to lines missing those fields. Every line is
    /// validated first (including `supersedes`/`references` targets);
    /// nothing is written if any line fails.
    #[arg(long)]
    pub stdin: bool,

    /// Write even when the target `.qual` file is hidden by `.gitignore`,
    /// `.ignore` or `.qualignore` (read commands will skip it), and see
    /// ignored files when checking `supersedes`/`references`.
    #[arg(long)]
    pub no_ignore: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    if args.stdin {
        return run_batch(
            args.record_type.as_deref(),
            args.subject.as_deref(),
            !args.no_ignore,
        );
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

    let issuer = provenance::issuer(args.issuer.as_deref());
    let issuer_type = provenance::issuer_type(args.issuer_type.as_deref())?;

    let locator = targets::Locator::from_cwd()?;
    let subject = locator.subject(&subject)?;
    let record = build_record(record_type, &subject, issuer, issuer_type, body_value)?;
    validate(&record)?;
    if record.supersedes().is_some() || record.references().is_some() {
        targets::check_pointers(&record, &targets::discover_project(!args.no_ignore)?, "")?;
    }

    let qual_path = locator.write_path(record.subject(), args.file.as_deref().map(Path::new))?;
    if !args.no_ignore {
        locator.check_not_ignored(&qual_path)?;
    }
    targets::append(&qual_path, &record)?;

    println!(
        "Emitted {} {} {}",
        record.record_type(),
        record.subject(),
        record.id(),
    );

    Ok(())
}

/// Annotation records must pass [`annotation::validate`]; other record
/// types are not validated (an unknown type's body is opaque).
fn validate(record: &Record) -> crate::Result<()> {
    if let Some(att) = record.as_annotation() {
        let errors = annotation::validate(att);
        if !errors.is_empty() {
            return Err(crate::Error::Validation(errors.join("; ")));
        }
    }
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

/// Plan every line, report every error, and append only when the whole
/// batch is clean.
fn run_batch(
    default_type: Option<&str>,
    default_subject: Option<&str>,
    respect_ignore: bool,
) -> crate::Result<()> {
    let locator = targets::Locator::from_cwd()?;
    // The positional subject is a CWD-relative path like everywhere else.
    let default_subject = default_subject.map(|s| locator.subject(s)).transpose()?;
    let mut view = BatchView::new(targets::discover_project(respect_ignore)?);
    let mut planned: Vec<(Record, PathBuf)> = Vec::new();
    let mut failed = 0usize;

    for (line_idx, line) in io::stdin().lock().lines().enumerate() {
        let line_no = line_idx + 1;
        let planned_line = line.map_err(|e| e.to_string()).and_then(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                return Ok(None);
            }
            plan_line(
                trimmed,
                default_type,
                default_subject.as_deref(),
                &view,
                &locator,
                respect_ignore,
            )
            .map(Some)
        });
        match planned_line {
            Ok(None) => {}
            Ok(Some((record, path))) => {
                view.push(record.clone());
                planned.push((record, path));
            }
            Err(e) => {
                eprintln!("stdin line {line_no}: {e}");
                failed += 1;
            }
        }
    }

    if failed > 0 {
        return Err(crate::Error::Validation(format!(
            "{failed} of {} stdin records failed; nothing written",
            planned.len() + failed
        )));
    }

    for (i, (record, path)) in planned.iter().enumerate() {
        targets::append(path, record).map_err(|e| {
            crate::Error::Validation(format!(
                "wrote {i} of {} records before an I/O error: {e}",
                planned.len()
            ))
        })?;
    }

    // Printed after every write, so a closed stdout cannot cut the batch.
    let mut out = io::stdout().lock();
    for (record, _) in &planned {
        let line = format!(
            "emitted  {:<24} {}  id: {}",
            record.record_type(),
            record.subject(),
            targets::short_id(record.id()),
        );
        match writeln!(out, "{line}") {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => break,
            other => other?,
        }
    }
    drop(out);
    eprintln!("Emitted {} records from stdin", planned.len());
    Ok(())
}

/// Parse, finalize and check one stdin line against `view`. Returns the
/// record and the `.qual` path it will be appended to.
fn plan_line(
    trimmed: &str,
    default_type: Option<&str>,
    default_subject: Option<&str>,
    view: &BatchView,
    locator: &targets::Locator,
    respect_ignore: bool,
) -> Result<(Record, PathBuf), String> {
    let mut value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| e.to_string())?;
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

    let mut record: Record = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let subject = locator
        .stored_subject(record.subject())
        .map_err(|e| e.to_string())?;
    set_subject(&mut record, subject);
    let record = annotation::finalize_record(record);
    validate(&record).map_err(|e| e.to_string())?;
    targets::check_pointers(&record, view.files(), "").map_err(|e| e.to_string())?;
    let path = locator
        .write_path(record.subject(), None)
        .map_err(|e| e.to_string())?;
    if respect_ignore {
        locator
            .check_not_ignored(&path)
            .map_err(|e| e.to_string())?;
    }
    Ok((record, path))
}
