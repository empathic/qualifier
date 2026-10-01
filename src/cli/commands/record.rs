use chrono::Utc;
use clap::Args as ClapArgs;
use serde_json::{Map, Value};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use crate::annotation::{self, Annotation, AnnotationBody, Kind, Record, Span};
use crate::cli::commands::resolve;
use crate::cli::provenance;
use crate::cli::targets;
use crate::content_hash;
use crate::qual_file::QualFile;

#[derive(ClapArgs)]
pub struct Args {
    /// Annotation kind: concern, comment, suggestion, pass, fail, blocker,
    /// praise, waiver, resolve, or any custom string (per spec §2.7.2).
    /// Required unless --stdin is used.
    pub kind: Option<String>,

    /// Subject location with optional span.
    /// Examples: `src/auth.rs`, `src/auth.rs:42`, `src/auth.rs:15:28`.
    /// Required unless --stdin is used.
    pub location: Option<String>,

    /// One-line summary message. Becomes `body.summary`.
    /// Required (in non-interactive mode) unless --stdin is used.
    pub message: Option<String>,

    /// Extended description.
    #[arg(long)]
    pub detail: Option<String>,

    /// Suggested fix.
    #[arg(long, alias = "fix")]
    pub suggested_fix: Option<String>,

    /// Classification tags (repeatable).
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Issuer identity URI (defaults to QUALIFIER_ISSUER, then detected agent harness, then VCS user email).
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type: human, ai, tool, unknown (defaults to QUALIFIER_ISSUER_TYPE, then detected agent harness).
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// VCS ref to pin (e.g., "git:3aba500").
    #[arg(long, name = "ref")]
    pub r#ref: Option<String>,

    /// Span override (e.g., "42", "42:58", "42.5:58.80"). When provided,
    /// overrides any span parsed from `<location>`.
    #[arg(long)]
    pub span: Option<String>,

    /// Full ID of a prior record this replaces. The record must exist and
    /// be live (not superseded, not closed).
    #[arg(long)]
    pub supersedes: Option<String>,

    /// Full ID of a related record (conversational reference). The record
    /// must exist and be live (not superseded, not closed).
    #[arg(long)]
    pub references: Option<String>,

    /// Explicit .qual file to write to (overrides layout resolution). Not
    /// supported with --stdin.
    #[arg(long)]
    pub file: Option<String>,

    /// Read JSONL records from stdin (batch mode). Each line describes one
    /// new record and is one of:
    ///
    ///   1. An overrides object: `{"kind":"concern","location":"src/foo.rs:42",
    ///      "message":"...","detail":"...","suggested_fix":"...","tags":["x"],
    ///      "issuer":"mailto:agent@example.com","issuer_type":"ai",
    ///      "ref":"git:abc123","supersedes":"<id>","references":"<id>",
    ///      "span":"42:58"}`
    ///   2. A complete record envelope (forward-compat) — recognized when the
    ///      object has both `subject` and `body` keys. Its `subject` is
    ///      relative to the project root and must stay inside it.
    ///
    /// `supersedes` and `references` on either form take the full ID of a
    /// live record in the project or on an earlier line of the same batch,
    /// as `--supersedes`/`--references` do. A reply is a line whose
    /// `references` is the target's ID; a resolve is a `kind: "resolve"`
    /// line whose `supersedes` is the target's ID, with at most one
    /// `reason:*` tag.
    ///
    /// Lines starting with `//` and blank lines are ignored. One record per
    /// line is emitted on stdout (id + summary, or full JSON with --format
    /// json). Errors are reported as `stdin line N: <reason>: <input>`.
    /// Without --continue-on-error the batch is all-or-nothing for
    /// parse/validation failures: every line is validated first, and
    /// nothing is written if any line fails that way. This does not cover
    /// I/O failures while writing — those can leave earlier lines written;
    /// the error reports how many. Pass `--continue-on-error` to collect
    /// every parse/validation error, write the lines that succeeded, and
    /// exit with a summary. `--file` is not supported with --stdin.
    /// See `qualifier agents batch` for a worked example.
    #[arg(long)]
    pub stdin: bool,

    /// In --stdin mode: collect all errors and continue past failed lines
    /// instead of treating the batch as all-or-nothing. Exit code is
    /// non-zero if any line failed; valid lines are still written.
    #[arg(long)]
    pub continue_on_error: bool,

    /// In --stdin mode: validate every line but do not write any records.
    #[arg(long)]
    pub dry_run: bool,

    /// Output format (human, json). In --stdin mode controls per-record output.
    /// Under `--format json`, errors are also emitted as JSON objects on stderr.
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub fn run(args: Args) -> crate::Result<()> {
    if args.stdin {
        if args.file.is_some() {
            return Err(crate::Error::Validation(
                "--file is not supported with --stdin".into(),
            ));
        }
        return run_batch(&args.format, args.continue_on_error, args.dry_run);
    }

    let required = |name: &str| {
        crate::Error::Validation(format!(
            "<{name}> is required (or use --stdin for batch mode)"
        ))
    };
    let input = AnnotationInput {
        kind: args.kind.ok_or_else(|| required("kind"))?,
        location: args.location.ok_or_else(|| required("location"))?,
        message: args.message.ok_or_else(|| required("message"))?,
        detail: args.detail,
        suggested_fix: args.suggested_fix,
        tags: args.tags,
        issuer: args.issuer,
        issuer_type: args.issuer_type,
        r#ref: args.r#ref,
        span: args.span,
        supersedes: args.supersedes,
        references: args.references,
    };

    let locator = targets::Locator::from_cwd()?;
    // Pointer checks need the project's records; skip the walk otherwise.
    let files = if input.supersedes.is_some() || input.references.is_some() {
        targets::discover_project(true)?
    } else {
        Vec::new()
    };
    let record = build_annotation(input, &files, &locator, "--")?;
    let qual_path = locator.write_path(record.subject(), args.file.as_deref().map(Path::new))?;
    targets::append(&qual_path, &record)?;

    if args.format == "json" {
        println!("{}", serde_json::to_string(&record)?);
    } else if let Some(att) = record.as_annotation() {
        println!(
            "{} {}{} {}",
            att.body.kind,
            att.subject,
            span_suffix(att.body.span.as_ref()),
            att.body.summary,
        );
        println!("  id: {}", att.id);
    }

    Ok(())
}

/// Inputs for one new annotation, from the command line or from an
/// overrides line on `--stdin`. Pointer fields hold full record IDs.
pub(crate) struct AnnotationInput {
    pub kind: String,
    /// `path[:start[:end]]`, relative to the current directory.
    pub location: String,
    pub message: String,
    pub detail: Option<String>,
    pub suggested_fix: Option<String>,
    pub tags: Vec<String>,
    pub issuer: Option<String>,
    pub issuer_type: Option<String>,
    pub r#ref: Option<String>,
    /// Span override; replaces any span parsed from `location`.
    pub span: Option<String>,
    pub supersedes: Option<String>,
    pub references: Option<String>,
}

/// Build a validated annotation from `input`: resolve the location to a
/// root-relative subject, apply the span override and content hash, fill
/// issuer defaults and the session tag, and check `supersedes` and
/// `references` against `files` (see [`targets::check_pointers`];
/// `flag_prefix` prefixes their names in errors).
pub(crate) fn build_annotation(
    input: AnnotationInput,
    files: &[QualFile],
    locator: &targets::Locator,
    flag_prefix: &str,
) -> crate::Result<Record> {
    let kind: Kind = input.kind.parse().unwrap();
    let (subject, location_span) = locator.location(&input.location)?;

    let mut span = match &input.span {
        Some(s) => Some(annotation::parse_span(s).map_err(crate::Error::Validation)?),
        None => location_span,
    };
    if let Some(ref mut s) = span
        && let Ok(hash) = content_hash::compute_span_hash(&locator.file(&subject), s)
    {
        s.content_hash = Some(hash);
    }

    let issuer = provenance::issuer(input.issuer.as_deref());
    let issuer_type = provenance::issuer_type(input.issuer_type.as_deref())?;
    let tags = resolve::checked_reason_tags(&kind, input.tags)?;

    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject,
        issuer,
        issuer_type,
        created_at: Utc::now().into(),
        id: String::new(),
        body: AnnotationBody {
            detail: input.detail,
            kind,
            r#ref: input.r#ref,
            references: input.references,
            span,
            suggested_fix: input.suggested_fix,
            summary: input.message,
            supersedes: input.supersedes,
            tags: provenance::with_session_tag(tags),
            extra: Default::default(),
        },
    });
    let errors = annotation::validate(&att);
    if !errors.is_empty() {
        return Err(crate::Error::Validation(errors.join("; ")));
    }
    let record = Record::Annotation(Box::new(att));
    targets::check_pointers(&record, files, flag_prefix)?;
    Ok(record)
}

/// `:start` or `:start:end` (lines only) for human output; empty without
/// a span.
fn span_suffix(span: Option<&Span>) -> String {
    match span {
        Some(span) => match &span.end {
            Some(e) if e.line != span.start.line => format!(":{}:{}", span.start.line, e.line),
            _ => format!(":{}", span.start.line),
        },
        None => String::new(),
    }
}

/// The records a batch line can see: everything discovered on disk plus the
/// lines already planned in this batch (the last, synthetic file).
struct BatchView {
    files: Vec<QualFile>,
}

impl BatchView {
    fn new(mut files: Vec<QualFile>) -> Self {
        files.push(QualFile {
            path: PathBuf::from("<stdin>"),
            subject: String::new(),
            records: Vec::new(),
        });
        Self { files }
    }

    fn push(&mut self, record: Record) {
        self.files
            .last_mut()
            .expect("BatchView always holds the pending file")
            .records
            .push(record);
    }

    fn files(&self) -> &[QualFile] {
        &self.files
    }
}

fn run_batch(format: &str, continue_on_error: bool, dry_run: bool) -> crate::Result<()> {
    let locator = targets::Locator::from_cwd()?;
    let mut view = BatchView::new(targets::discover_project(true)?);
    let mut planned: Vec<(Record, PathBuf, usize)> = Vec::new();
    let mut errors: Vec<BatchError> = Vec::new();

    for (line_idx, line) in io::stdin().lock().lines().enumerate() {
        let line_no = line_idx + 1;
        let raw = match line {
            Ok(l) => l,
            Err(e) => {
                errors.push(BatchError {
                    line: line_no,
                    error: format!("io error: {e}"),
                    input: String::new(),
                });
                continue;
            }
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        match plan_one(trimmed, &view, &locator) {
            Ok((record, path)) => {
                view.push(record.clone());
                planned.push((record, path, line_no));
            }
            Err(error) => errors.push(BatchError {
                line: line_no,
                error,
                input: trimmed.to_string(),
            }),
        }
    }

    for be in &errors {
        emit_batch_error(be, format);
    }

    // Without --continue-on-error the batch is all-or-nothing.
    let proceed = errors.is_empty() || continue_on_error;
    let mut recorded = 0usize;
    if proceed {
        for (i, (record, path, line_no)) in planned.iter().enumerate() {
            if !dry_run && let Err(e) = targets::append(path, record) {
                return Err(crate::Error::Validation(format!(
                    "wrote {i} of {} records before an I/O error appending stdin line {line_no}: {e}",
                    planned.len()
                )));
            }
            emit_batch_line(record, format, dry_run)?;
            recorded += 1;
        }
    }

    let total = planned.len() + errors.len();
    let written = proceed && !dry_run;
    let suffix = if dry_run {
        " (dry run, nothing written)"
    } else if !proceed {
        " (nothing written)"
    } else {
        ""
    };
    if format == "json" {
        let summary = serde_json::json!({
            "summary": {
                "recorded": recorded,
                "failed": errors.len(),
                "total": total,
                "dry_run": dry_run,
                "written": written,
            }
        });
        eprintln!("{summary}");
    } else {
        eprintln!(
            "Recorded {recorded} of {total} records from stdin{}{suffix}",
            if errors.is_empty() {
                String::new()
            } else {
                format!(", {} failed", errors.len())
            },
        );
    }

    if !errors.is_empty() {
        // Keep stderr a clean JSONL stream under --format json.
        if format == "json" {
            std::process::exit(1);
        }
        return Err(crate::Error::Validation(if continue_on_error {
            format!(
                "{} of {total} stdin records failed (--continue-on-error)",
                errors.len()
            )
        } else {
            format!(
                "{} of {total} stdin records failed; nothing written",
                errors.len()
            )
        }));
    }
    Ok(())
}

/// Parse and validate one line against `view`. Returns the record and the
/// `.qual` path it will be appended to. Locations on overrides lines are
/// relative to the current directory; every write path is under the
/// project root. Plans only: nothing is created.
fn plan_one(
    trimmed: &str,
    view: &BatchView,
    locator: &targets::Locator,
) -> std::result::Result<(Record, PathBuf), String> {
    let value: Value = serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;

    let record = if value.get("body").is_some() && value.get("subject").is_some() {
        let mut r: Record =
            serde_json::from_value(value).map_err(|e| format!("invalid record: {e}"))?;
        let subject = locator
            .stored_subject(r.subject())
            .map_err(|e| e.to_string())?;
        set_subject(&mut r, subject);
        let r = annotation::finalize_record(r);
        if let Some(att) = r.as_annotation() {
            let errors = annotation::validate(att);
            if !errors.is_empty() {
                return Err(errors.join("; "));
            }
        }
        targets::check_pointers(&r, view.files(), "").map_err(|e| e.to_string())?;
        r
    } else {
        let obj = value
            .as_object()
            .ok_or_else(|| "stdin line must be a JSON object".to_string())?;
        let input = overrides_input(obj).map_err(|e| e.to_string())?;
        build_annotation(input, view.files(), locator, "").map_err(|e| e.to_string())?
    };

    let qual_path = locator
        .write_path(record.subject(), None)
        .map_err(|e| e.to_string())?;
    Ok((record, qual_path))
}

/// Replace the envelope subject of `record` (before its ID is computed).
pub(crate) fn set_subject(record: &mut Record, subject: String) {
    match record {
        Record::Annotation(a) => a.subject = subject,
        Record::Epoch(e) => e.subject = subject,
        Record::Dependency(d) => d.subject = subject,
        Record::Unknown(v) => {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("subject".into(), Value::String(subject));
            }
        }
    }
}

fn str_field(obj: &Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn tags_field(obj: &Map<String, Value>) -> Vec<String> {
    obj.get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

struct BatchError {
    line: usize,
    error: String,
    input: String,
}

fn truncate_for_display(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let prefix: String = s.chars().take(max).collect();
        format!("{prefix}...")
    }
}

/// Always to stderr so stdout (the success stream) stays clean.
fn emit_batch_error(be: &BatchError, format: &str) {
    if format == "json" {
        let v = serde_json::json!({
            "line": be.line,
            "error": be.error,
            "input": be.input,
        });
        eprintln!("{v}");
    } else {
        let truncated = truncate_for_display(&be.input, 200);
        if truncated.is_empty() {
            eprintln!("stdin line {}: {}", be.line, be.error);
        } else {
            eprintln!("stdin line {}: {}: {}", be.line, be.error, truncated);
        }
    }
}

/// Under `--dry-run`, the human verb is "would-record" so a glance
/// confirms nothing was committed.
fn emit_batch_line(record: &Record, format: &str, dry_run: bool) -> crate::Result<()> {
    if format == "json" {
        let mut v = serde_json::to_value(record)?;
        if dry_run && let Some(obj) = v.as_object_mut() {
            obj.insert("dry_run".into(), serde_json::Value::Bool(true));
        }
        println!("{}", serde_json::to_string(&v)?);
        return Ok(());
    }

    let verb = if dry_run {
        "would-record"
    } else {
        "recorded   "
    };
    let id = record.id();
    let id_short = if id.len() >= 8 { &id[..8] } else { id };
    if let Some(att) = record.as_annotation() {
        println!(
            "{verb}  {:<10} {}{}  {}  id: {}",
            att.body.kind.to_string(),
            att.subject,
            span_suffix(att.body.span.as_ref()),
            att.body.summary,
            id_short,
        );
    } else {
        println!(
            "{verb}  {:<10} {}  id: {}",
            record.record_type(),
            record.subject(),
            id_short,
        );
    }
    Ok(())
}

/// The [`AnnotationInput`] an overrides line describes.
fn overrides_input(obj: &Map<String, Value>) -> crate::Result<AnnotationInput> {
    let required = |key: &str| {
        str_field(obj, key)
            .ok_or_else(|| crate::Error::Validation(format!("stdin object missing '{key}'")))
    };
    Ok(AnnotationInput {
        kind: required("kind")?,
        location: required("location")?,
        message: required("message")?,
        detail: str_field(obj, "detail"),
        suggested_fix: str_field(obj, "suggested_fix"),
        tags: tags_field(obj),
        issuer: str_field(obj, "issuer"),
        issuer_type: str_field(obj, "issuer_type"),
        r#ref: str_field(obj, "ref"),
        span: str_field(obj, "span"),
        supersedes: str_field(obj, "supersedes"),
        references: str_field(obj, "references"),
    })
}
