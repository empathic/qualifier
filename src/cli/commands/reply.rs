use chrono::Utc;
use clap::Args as ClapArgs;
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, IssuerType, Kind, Record, Span};
use crate::cli::commands::record::{detect_issuer, normalize_issuer_uri};
use crate::cli::output;
use crate::qual_file;
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// Target — either an id-prefix (≥4 chars) or a `<location>`
    /// (e.g., `src/auth.rs:42`). A location resolves to the most-recent
    /// active record there; ambiguity is reported with a candidate list.
    pub target: String,

    /// One-line reply message
    pub message: String,

    /// Override the default kind (comment)
    #[arg(long)]
    pub kind: Option<String>,

    /// Quality score override (-100..=100)
    #[arg(long, allow_hyphen_values = true)]
    pub score: Option<i32>,

    /// Extended description
    #[arg(long)]
    pub detail: Option<String>,

    /// Suggested fix
    #[arg(long, alias = "fix")]
    pub suggested_fix: Option<String>,

    /// Classification tags (repeatable)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Issuer identity URI (defaults to VCS user email with mailto:)
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type (human, ai, tool, unknown)
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// VCS ref to pin (e.g., "git:3aba500")
    #[arg(long, name = "ref")]
    pub r#ref: Option<String>,

    /// ID of a prior annotation this replaces
    #[arg(long)]
    pub supersedes: Option<String>,

    /// Explicit .qual file to write to
    #[arg(long)]
    pub file: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,
}

/// Resolve a short ID prefix to a unique record across all qual files.
pub(crate) fn resolve_id_prefix(
    prefix: &str,
    qual_files: &[qual_file::QualFile],
) -> crate::Result<Record> {
    if prefix.len() < 4 {
        return Err(crate::Error::Validation(
            "ID prefix must be at least 4 characters".into(),
        ));
    }

    let matches: Vec<&Record> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter())
        .filter(|r| r.id().starts_with(prefix))
        .collect();

    match matches.len() {
        0 => Err(crate::Error::Validation(format!(
            "no record found matching prefix '{prefix}'"
        ))),
        1 => Ok(matches[0].clone()),
        n => Err(crate::Error::Validation(format!(
            "ambiguous prefix '{prefix}' matches {n} records"
        ))),
    }
}

/// Decide whether a target string should be parsed as a `<location>`
/// rather than an id-prefix. Locations either contain a `:` (line/range)
/// or contain a path separator (and are not pure hex).
fn looks_like_location(target: &str) -> bool {
    if target.contains(':') {
        return true;
    }
    if target.contains('/') || target.contains('\\') || target.contains('.') {
        // Path-like and not a pure hex id-prefix.
        let is_hex = target.chars().all(|c| c.is_ascii_hexdigit());
        return !is_hex;
    }
    false
}

/// Resolve a target string to a unique record. Accepts an id-prefix or a
/// `<location>` (subject + optional span).
pub(crate) fn resolve_target(
    target: &str,
    qual_files: &[qual_file::QualFile],
) -> crate::Result<Record> {
    if looks_like_location(target) {
        resolve_location_target(target, qual_files)
    } else {
        resolve_id_prefix(target, qual_files)
    }
}

fn span_overlaps(a: &Span, b: &Span) -> bool {
    let a_start = a.start.line;
    let a_end = a.end.as_ref().unwrap_or(&a.start).line;
    let b_start = b.start.line;
    let b_end = b.end.as_ref().unwrap_or(&b.start).line;
    a_start <= b_end && b_start <= a_end
}

fn resolve_location_target(
    location: &str,
    qual_files: &[qual_file::QualFile],
) -> crate::Result<Record> {
    let (subject, span_filter) = annotation::parse_location(location);

    // Collect all records, filter to active.
    let all: Vec<Record> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter().cloned())
        .collect();
    let active = scoring::filter_superseded(&all);
    let active_ids: std::collections::HashSet<&str> = active.iter().map(|r| r.id()).collect();

    // Filter to records matching subject and (if specified) span overlap.
    let mut candidates: Vec<&Record> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter())
        .filter(|r| r.subject() == subject)
        .filter(|r| active_ids.contains(r.id()))
        .filter(|r| {
            if let Some(ref s) = span_filter {
                match r.as_annotation().and_then(|a| a.body.span.as_ref()) {
                    Some(rs) => span_overlaps(rs, s),
                    None => false,
                }
            } else {
                true
            }
        })
        .collect();

    if candidates.is_empty() {
        return Err(crate::Error::Validation(format!(
            "no active record found at '{location}'"
        )));
    }

    if candidates.len() > 1 {
        // Pick the most-recent by created_at; if multiple share the
        // newest timestamp, surface a disambiguation list.
        candidates.sort_by_key(|b| std::cmp::Reverse(record_created_at(b)));
        let newest_ts = record_created_at(candidates[0]);
        let tied: Vec<&Record> = candidates
            .iter()
            .copied()
            .take_while(|r| record_created_at(r) == newest_ts)
            .collect();
        if tied.len() > 1 {
            let mut msg = format!(
                "ambiguous location '{location}' matches {} active records:\n",
                candidates.len()
            );
            for r in &candidates {
                let kind = r
                    .kind()
                    .map(|k| k.to_string())
                    .unwrap_or_else(|| r.record_type().to_string());
                let line = r
                    .as_annotation()
                    .and_then(|a| a.body.span.as_ref())
                    .map(|s| format!("L{}", s.start.line))
                    .unwrap_or_else(|| "—".into());
                let summary = r
                    .as_annotation()
                    .map(|a| a.body.summary.clone())
                    .unwrap_or_default();
                let id = r.id();
                let prefix = &id[..id.len().min(8)];
                msg.push_str(&format!("  [{prefix}] {kind:<10} {line:<8} {summary:?}\n"));
            }
            msg.push_str("hint: specify a span (e.g., 'src/foo.rs:42') or use an id-prefix");
            return Err(crate::Error::Validation(msg));
        }
    }

    Ok(candidates[0].clone())
}

fn record_created_at(r: &Record) -> chrono::DateTime<chrono::Utc> {
    match r {
        Record::Annotation(a) => a.created_at,
        Record::Epoch(e) => e.created_at,
        Record::Dependency(d) => d.created_at,
        Record::Unknown(v) => v
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now),
    }
}

pub fn run(args: Args) -> crate::Result<()> {
    let root = qual_file::find_project_root(Path::new("."));
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let all_qual_files = qual_file::discover(discover_root, true)?;

    let target = resolve_target(&args.target, &all_qual_files)?;
    let subject = target.subject().to_string();
    let target_id = target.id().to_string();

    let kind: Kind = args.kind.as_deref().unwrap_or("comment").parse().unwrap();
    let score = args.score;

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
            references: Some(target_id),
            score,
            span: None,
            suggested_fix: args.suggested_fix,
            summary: args.message,
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
        println!(
            "{} {} {} {}",
            att.body.kind,
            att.subject,
            output::format_score(att.body.score),
            att.body.summary,
        );
        println!("  id: {}", att.id);
        println!("  re: {}", &att.body.references.as_ref().unwrap()[..8]);
    }

    Ok(())
}
