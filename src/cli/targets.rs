//! Target resolution shared by the write commands: ID prefixes, locations,
//! and the liveness check that keeps writes off superseded records.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::annotation::{self, Kind, Record, Span};
use crate::compact::filter_superseded;
use crate::qual_file::{self, QualFile};

/// Discover every `.qual` file under the project root (or the current
/// directory outside a repository).
pub(crate) fn discover_project(respect_ignore: bool) -> crate::Result<Vec<QualFile>> {
    // Resolve from an absolute CWD so the upward walk in find_project_root
    // works from any subdirectory — relative-path arithmetic on `.` doesn't
    // traverse up.
    let cwd = std::env::current_dir()?;
    let root = qual_file::find_project_root(&cwd);
    let discover_root = root.as_deref().unwrap_or(cwd.as_path());
    qual_file::discover(discover_root, respect_ignore)
}

/// First eight characters of an ID, for messages.
pub(crate) fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

/// Resolve a short ID prefix to a unique record across all qual files.
pub(crate) fn resolve_id_prefix(prefix: &str, qual_files: &[QualFile]) -> crate::Result<Record> {
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
/// `<location>` (subject + optional span). Unless `allow_superseded`, a
/// superseded or resolved record is rejected (see [`ensure_live`]).
pub(crate) fn resolve_target(
    target: &str,
    qual_files: &[QualFile],
    allow_superseded: bool,
) -> crate::Result<Record> {
    let record = if looks_like_location(target) {
        resolve_location_target(target, qual_files)?
    } else {
        resolve_id_prefix(target, qual_files)?
    };
    if !allow_superseded {
        ensure_live(&record, qual_files)?;
    }
    Ok(record)
}

/// Fail when `record` has been superseded. The error names the live tip of
/// its supersession chain, or — when the chain ends in a `resolve` —
/// reports the record as closed.
pub(crate) fn ensure_live(record: &Record, qual_files: &[QualFile]) -> crate::Result<()> {
    // Map each superseded ID to its newest superseder.
    let mut successor: HashMap<&str, &Record> = HashMap::new();
    for r in qual_files.iter().flat_map(|qf| qf.records.iter()) {
        if let Some(target) = r.supersedes() {
            let newer = match successor.get(target) {
                Some(cur) => record_created_at(r) > record_created_at(cur),
                None => true,
            };
            if newer {
                successor.insert(target, r);
            }
        }
    }

    let Some(mut tip) = successor.get(record.id()).copied() else {
        return Ok(());
    };
    let mut seen: HashSet<&str> = HashSet::from([record.id()]);
    while let Some(next) = successor.get(tip.id()).copied() {
        if !seen.insert(tip.id()) {
            break;
        }
        tip = next;
    }

    if tip.kind() == Some(&Kind::Resolve) {
        return Err(crate::Error::Validation(format!(
            "target {} is closed (resolved by {})\n\
             hint: pass --allow-superseded to annotate a closed record",
            short_id(record.id()),
            short_id(tip.id()),
        )));
    }
    let kind = tip
        .kind()
        .map(|k| k.to_string())
        .unwrap_or_else(|| tip.record_type().to_string());
    let summary = tip
        .as_annotation()
        .map(|a| a.body.summary.as_str())
        .unwrap_or("");
    Err(crate::Error::Validation(format!(
        "target {} is superseded by {} ({kind} {summary:?})\n\
         hint: target the live record, or pass --allow-superseded to annotate history",
        short_id(record.id()),
        short_id(tip.id()),
    )))
}

pub(crate) fn span_overlaps(a: &Span, b: &Span) -> bool {
    let a_start = a.start.line;
    let a_end = a.end.as_ref().unwrap_or(&a.start).line;
    let b_start = b.start.line;
    let b_end = b.end.as_ref().unwrap_or(&b.start).line;
    a_start <= b_end && b_start <= a_end
}

fn resolve_location_target(location: &str, qual_files: &[QualFile]) -> crate::Result<Record> {
    let (subject, span_filter) = annotation::parse_location(location);

    // Collect all records, filter to active.
    let all: Vec<Record> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter().cloned())
        .collect();
    let active = filter_superseded(&all);
    let active_ids: HashSet<&str> = active.iter().map(|r| r.id()).collect();

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
                let prefix = short_id(r.id());
                msg.push_str(&format!("  [{prefix}] {kind:<10} {line:<8} {summary:?}\n"));
            }
            msg.push_str("hint: specify a span (e.g., 'src/foo.rs:42') or use an id-prefix");
            return Err(crate::Error::Validation(msg));
        }
    }

    Ok(candidates[0].clone())
}

/// Resolve an ID-valued flag (`--supersedes`, `--references`) to the full
/// ID of a record. Unless `allow_superseded`, the record must be live.
/// Errors are prefixed with `flag`.
pub(crate) fn resolve_id_flag(
    flag: &str,
    value: &str,
    qual_files: &[QualFile],
    allow_superseded: bool,
) -> crate::Result<String> {
    let prefixed = |e: crate::Error| crate::Error::Validation(format!("{flag}: {e}"));
    let record = resolve_id_prefix(value, qual_files).map_err(prefixed)?;
    if !allow_superseded {
        ensure_live(&record, qual_files).map_err(prefixed)?;
    }
    Ok(record.id().to_string())
}

/// Check that adding `record` to `existing` keeps supersession acyclic and
/// same-subject.
pub(crate) fn check_supersession(mut existing: Vec<Record>, record: &Record) -> crate::Result<()> {
    existing.push(record.clone());
    annotation::check_supersession_cycles(&existing)?;
    annotation::validate_supersession_targets(&existing)
}

/// [`check_supersession`] against the records already in `qual_path`.
pub(crate) fn preflight_supersession(qual_path: &Path, record: &Record) -> crate::Result<()> {
    let existing = if qual_path.exists() {
        qual_file::parse(qual_path)?.records
    } else {
        Vec::new()
    };
    check_supersession(existing, record)
}

pub(crate) fn record_created_at(r: &Record) -> chrono::DateTime<chrono::Utc> {
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
