use chrono::Utc;
use clap::Args as ClapArgs;
use std::io::{self, BufRead};
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, IssuerType, Kind, Record};
use crate::content_hash;
use crate::qual_file;

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

    /// Issuer identity URI (defaults to VCS user email with mailto:).
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type (human, ai, tool, unknown).
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// VCS ref to pin (e.g., "git:3aba500").
    #[arg(long, name = "ref")]
    pub r#ref: Option<String>,

    /// Span override (e.g., "42", "42:58", "42.5:58.80"). When provided,
    /// overrides any span parsed from `<location>`.
    #[arg(long)]
    pub span: Option<String>,

    /// ID of a prior annotation this replaces.
    #[arg(long)]
    pub supersedes: Option<String>,

    /// ID of a related annotation (conversational reference).
    #[arg(long)]
    pub references: Option<String>,

    /// Explicit .qual file to write to (overrides layout resolution).
    #[arg(long)]
    pub file: Option<String>,

    /// Read JSONL records from stdin (batch mode). Each line is one of:
    ///
    ///   1. An overrides object: `{"kind":"concern","location":"src/foo.rs:42",
    ///      "message":"...","detail":"...","suggested_fix":"...","tags":["x"],
    ///      "issuer":"mailto:agent@example.com","issuer_type":"ai",
    ///      "ref":"git:abc123","supersedes":"<id>","references":"<id>",
    ///      "span":"42:58"}`
    ///   2. A complete record envelope (forward-compat) — recognized when the
    ///      object has both `subject` and `body` keys.
    ///
    /// Lines starting with `//` and blank lines are ignored. One record per
    /// line is emitted on stdout (id + summary, or full JSON with --format
    /// json). Errors are reported as `stdin line N: <reason>: <input>` and
    /// by default abort the batch on the first failure. Pass
    /// `--continue-on-error` to collect every error and exit with a summary.
    /// See `qualifier agents record` for a worked example.
    #[arg(long)]
    pub stdin: bool,

    /// In --stdin mode: collect all errors and continue past failed lines
    /// instead of aborting on the first. Exit code is non-zero if any line
    /// failed; valid lines are still written.
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
        return run_batch(&args.format, args.continue_on_error, args.dry_run);
    }

    let kind_str = args.kind.as_deref().ok_or_else(|| {
        crate::Error::Validation("<kind> is required (or use --stdin for batch mode)".into())
    })?;
    let kind: Kind = kind_str.parse().unwrap();

    let location = args.location.as_deref().ok_or_else(|| {
        crate::Error::Validation("<location> is required (or use --stdin for batch mode)".into())
    })?;

    let message = args.message.ok_or_else(|| {
        crate::Error::Validation("<message> is required (or use --stdin for batch mode)".into())
    })?;

    let (subject, location_span) = annotation::parse_location(location);

    // --span overrides the location's span.
    let mut span = match &args.span {
        Some(s) => Some(annotation::parse_span(s).map_err(crate::Error::Validation)?),
        None => location_span,
    };

    // Auto-compute content hash for spans
    if let Some(ref mut s) = span
        && let Some(hash) = content_hash::compute_span_hash(Path::new(&subject), s)
    {
        s.content_hash = Some(hash);
    }

    let issuer = normalize_issuer_uri(
        args.issuer
            .or_else(detect_issuer)
            .unwrap_or_else(|| "mailto:unknown@localhost".into()),
    );

    let issuer_type = match &args.issuer_type {
        Some(s) => Some(s.parse::<IssuerType>().map_err(crate::Error::Validation)?),
        None => None,
    };

    let qual_path = qual_file::resolve_qual_path(&subject, args.file.as_deref().map(Path::new))?;

    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject,
        issuer,
        issuer_type,
        created_at: Utc::now(),
        id: String::new(),
        body: AnnotationBody {
            detail: args.detail,
            kind,
            r#ref: args.r#ref,
            references: args.references,
            span,
            suggested_fix: args.suggested_fix,
            summary: message,
            supersedes: args.supersedes.clone(),
            tags: args.tags,
        },
    });

    let errors = annotation::validate(&att);
    if !errors.is_empty() {
        return Err(crate::Error::Validation(errors.join("; ")));
    }

    if att.body.supersedes.is_some() {
        let existing = if qual_path.exists() {
            qual_file::parse(&qual_path)?.records
        } else {
            Vec::new()
        };
        let mut all = existing;
        all.push(Record::Annotation(Box::new(att.clone())));
        annotation::check_supersession_cycles(&all)?;
        annotation::validate_supersession_targets(&all)?;
    }

    let record = Record::Annotation(Box::new(att.clone()));

    qual_file::append(qual_path.as_ref(), &record)?;

    if args.format == "json" {
        println!("{}", serde_json::to_string(&record)?);
    } else {
        let span_str = match &att.body.span {
            Some(span) => {
                let end = match &span.end {
                    Some(e) if e.line != span.start.line => format!(":{}", e.line),
                    _ => String::new(),
                };
                format!(":{}{}", span.start.line, end)
            }
            None => String::new(),
        };
        println!(
            "{} {}{} {}",
            att.body.kind, att.subject, span_str, att.body.summary,
        );
        println!("  id: {}", att.id);
    }

    Ok(())
}

fn run_batch(format: &str, continue_on_error: bool, dry_run: bool) -> crate::Result<()> {
    let stdin = io::stdin();
    let mut recorded = 0usize;
    let mut errors: Vec<BatchError> = Vec::new();

    for (line_idx, line) in stdin.lock().lines().enumerate() {
        let line_no = line_idx + 1;
        let raw = match line {
            Ok(l) => l,
            Err(e) => {
                let be = BatchError {
                    line: line_no,
                    error: format!("io error: {e}"),
                    input: String::new(),
                };
                if continue_on_error {
                    emit_batch_error(&be, format);
                    errors.push(be);
                    continue;
                }
                return Err(be.into_error());
            }
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        match process_one(trimmed, dry_run) {
            Ok(record) => {
                emit_batch_line(&record, format, dry_run)?;
                recorded += 1;
            }
            Err(msg) => {
                let be = BatchError {
                    line: line_no,
                    error: msg,
                    input: trimmed.to_string(),
                };
                if continue_on_error {
                    emit_batch_error(&be, format);
                    errors.push(be);
                    continue;
                }
                return Err(be.into_error());
            }
        }
    }

    let total = recorded + errors.len();
    let suffix = if dry_run { " (dry run, nothing written)" } else { "" };
    if format == "json" {
        // Trailer summary as JSON so consumers parsing stderr line-by-line
        // see a structured terminator rather than a free-form English line.
        let summary = serde_json::json!({
            "summary": {
                "recorded": recorded,
                "failed": errors.len(),
                "total": total,
                "dry_run": dry_run,
            }
        });
        eprintln!("{summary}");
    } else {
        eprintln!(
            "Recorded {recorded} of {total} records from stdin{}{}",
            if errors.is_empty() {
                String::new()
            } else {
                format!(", {} failed", errors.len())
            },
            suffix
        );
    }

    if !errors.is_empty() {
        // Suppress the top-level `qualifier: ...` line under --format json so
        // stderr stays a clean JSONL stream — the per-line error objects and
        // summary already carry every detail a consumer needs.
        if format == "json" {
            std::process::exit(1);
        }
        return Err(crate::Error::Validation(format!(
            "{} of {} stdin records failed (--continue-on-error)",
            errors.len(),
            total
        )));
    }
    Ok(())
}

fn process_one(trimmed: &str, dry_run: bool) -> std::result::Result<Record, String> {
    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;

    let record = if value.get("body").is_some() && value.get("subject").is_some() {
        let r: Record =
            serde_json::from_value(value).map_err(|e| format!("invalid record: {e}"))?;
        annotation::finalize_record(r)
    } else {
        build_record_from_overrides(value).map_err(|e| e.to_string())?
    };

    if let Some(att) = record.as_annotation() {
        let errors = annotation::validate(att);
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
    }

    let qual_path = qual_file::resolve_qual_path(record.subject(), None).map_err(|e| e.to_string())?;

    if record.supersedes().is_some() {
        let existing = if qual_path.exists() {
            qual_file::parse(&qual_path)
                .map_err(|e| e.to_string())?
                .records
        } else {
            Vec::new()
        };
        let mut all = existing;
        all.push(record.clone());
        annotation::check_supersession_cycles(&all).map_err(|e| e.to_string())?;
        annotation::validate_supersession_targets(&all).map_err(|e| e.to_string())?;
    }

    if !dry_run {
        qual_file::append(&qual_path, &record).map_err(|e| e.to_string())?;
    }
    Ok(record)
}

struct BatchError {
    line: usize,
    error: String,
    input: String,
}

impl BatchError {
    /// Includes the offending line content so the user can see what they
    /// sent without re-piping.
    fn into_error(self) -> crate::Error {
        let truncated = truncate_for_display(&self.input, 200);
        crate::Error::Validation(if truncated.is_empty() {
            format!("stdin line {}: {}", self.line, self.error)
        } else {
            format!("stdin line {}: {}: {}", self.line, self.error, truncated)
        })
    }
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
        if dry_run
            && let Some(obj) = v.as_object_mut()
        {
            obj.insert("dry_run".into(), serde_json::Value::Bool(true));
        }
        println!("{}", serde_json::to_string(&v)?);
        return Ok(());
    }

    let verb = if dry_run { "would-record" } else { "recorded   " };
    let id = record.id();
    let id_short = if id.len() >= 8 { &id[..8] } else { id };
    if let Some(att) = record.as_annotation() {
        let span_str = match &att.body.span {
            Some(span) => {
                let end = match &span.end {
                    Some(e) if e.line != span.start.line => format!(":{}", e.line),
                    _ => String::new(),
                };
                format!(":{}{}", span.start.line, end)
            }
            None => String::new(),
        };
        println!(
            "{verb}  {:<10} {}{}  {}  id: {}",
            att.body.kind.to_string(),
            att.subject,
            span_str,
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

fn build_record_from_overrides(value: serde_json::Value) -> crate::Result<Record> {
    let obj = value
        .as_object()
        .ok_or_else(|| crate::Error::Validation("stdin line must be a JSON object".into()))?;

    let kind_str = obj
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("stdin object missing 'kind'".into()))?;
    let kind: Kind = kind_str.parse().unwrap();

    let location = obj
        .get("location")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("stdin object missing 'location'".into()))?;
    let message = obj
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::Validation("stdin object missing 'message'".into()))?;

    let (subject, location_span) = annotation::parse_location(location);

    let mut span = match obj.get("span").and_then(|v| v.as_str()) {
        Some(s) => Some(annotation::parse_span(s).map_err(crate::Error::Validation)?),
        None => location_span,
    };

    if let Some(ref mut s) = span
        && let Some(hash) = content_hash::compute_span_hash(Path::new(&subject), s)
    {
        s.content_hash = Some(hash);
    }

    let issuer = normalize_issuer_uri(
        obj.get("issuer")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(detect_issuer)
            .unwrap_or_else(|| "mailto:unknown@localhost".into()),
    );

    let issuer_type = match obj.get("issuer_type").and_then(|v| v.as_str()) {
        Some(s) => Some(s.parse::<IssuerType>().map_err(crate::Error::Validation)?),
        None => None,
    };

    let detail = obj.get("detail").and_then(|v| v.as_str()).map(String::from);
    let suggested_fix = obj
        .get("suggested_fix")
        .and_then(|v| v.as_str())
        .map(String::from);
    let r#ref = obj.get("ref").and_then(|v| v.as_str()).map(String::from);
    let supersedes = obj
        .get("supersedes")
        .and_then(|v| v.as_str())
        .map(String::from);
    let references = obj
        .get("references")
        .and_then(|v| v.as_str())
        .map(String::from);
    let tags: Vec<String> = obj
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject,
        issuer,
        issuer_type,
        created_at: Utc::now(),
        id: String::new(),
        body: AnnotationBody {
            detail,
            kind,
            r#ref,
            references,
            span,
            suggested_fix,
            summary: message.to_string(),
            supersedes,
            tags,
        },
    });

    Ok(Record::Annotation(Box::new(att)))
}

/// Detect the issuer identity from VCS configuration.
pub fn detect_issuer() -> Option<String> {
    // Try git first
    std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|email| format!("mailto:{email}"))
        .or_else(|| {
            // Try hg
            std::process::Command::new("hg")
                .args(["config", "ui.username"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
                .map(|email| format!("mailto:{email}"))
        })
        .or_else(|| {
            // Fallback: $USER@localhost
            let user = std::env::var("USER").unwrap_or_else(|_| "unknown".into());
            Some(format!("mailto:{user}@localhost"))
        })
}

/// Normalize an issuer value to a URI. Bare emails get `mailto:` prefix;
/// values already containing `:` are assumed to be valid URIs.
pub fn normalize_issuer_uri(issuer: String) -> String {
    if issuer.contains(':') {
        issuer
    } else {
        format!("mailto:{issuer}")
    }
}
