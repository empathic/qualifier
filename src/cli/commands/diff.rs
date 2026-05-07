//! `qualifier diff <ref>` — show records added, resolved, or drifted on this
//! branch relative to a git ref.
//!
//! Compares the union of records at HEAD with the union of records at the
//! supplied ref (default `main`). "Active" means not superseded — a record
//! that disappears from the active set on this branch is reported as resolved
//! (or removed, if no successor exists).
//!
//! Drift is checked the same way `qualifier review` does: any active span
//! with a `content_hash` is rechecked against the current file content.
//! Drift on records that are *also* present in the ref counts (the
//! interesting case is "I touched the code under an old annotation");
//! drift on freshly added records is suppressed since the user just authored
//! them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args as ClapArgs;

use crate::annotation::{Kind, Record};
use crate::compact::filter_superseded;
use crate::content_hash::{self, FreshnessStatus};
use crate::qual_file;

#[derive(ClapArgs)]
pub struct Args {
    /// Git ref to diff against. Defaults to `main`.
    #[arg(default_value = "main")]
    pub r#ref: String,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Disable .gitignore and .qualignore filtering when discovering current
    /// .qual files (the ref-side enumeration is governed by git itself).
    #[arg(long)]
    pub no_ignore: bool,
}

struct Diff {
    added: Vec<Record>,
    resolved: Vec<ResolvedEntry>,
    drifted: Vec<DriftEntry>,
}

struct ResolvedEntry {
    /// Record from <ref> that is no longer active.
    old: Record,
    /// Record on this branch that supersedes it, if any.
    closer: Option<Record>,
}

struct DriftEntry {
    record: Record,
    expected: String,
    actual: String,
}

pub fn run(args: Args) -> crate::Result<()> {
    // Resolve from an absolute CWD so the upward walk in find_project_root
    // works from any subdirectory — relative-path arithmetic on `.` doesn't
    // traverse up.
    let cwd = std::env::current_dir()?;
    let project_root = qual_file::find_project_root(&cwd).ok_or_else(|| {
        crate::Error::Validation(
            "qualifier diff requires a git repository (no VCS marker found)".into(),
        )
    })?;

    if !project_root.join(".git").exists() {
        return Err(crate::Error::Validation(
            "qualifier diff currently supports git only — no .git found at project root".into(),
        ));
    }

    // Validate the ref exists up-front so the user gets a clean error rather
    // than a smear of `git show` failures.
    if !ref_exists(&project_root, &args.r#ref) {
        return Err(crate::Error::Validation(format!(
            "git ref '{}' not found",
            args.r#ref
        )));
    }

    let new_qual_files = qual_file::discover(&project_root, !args.no_ignore)?;
    let new_records: Vec<Record> = new_qual_files
        .iter()
        .flat_map(|qf| qf.records.iter().cloned())
        .collect();

    let old_records = load_records_at_ref(&project_root, &args.r#ref, &new_qual_files)?;

    let diff = compute_diff(&old_records, &new_records, &project_root);

    if args.format == "json" {
        print_json(&args.r#ref, &diff);
    } else {
        print_human(&args.r#ref, &diff);
    }
    Ok(())
}

/// Enumerate `.qual` paths at the ref via `git ls-tree`, plus the paths that
/// exist on the current working tree, and load records at `<ref>` for the
/// union. Paths that exist only at the ref (deleted on this branch) are
/// included so their records show up as resolved/removed.
fn load_records_at_ref(
    project_root: &Path,
    git_ref: &str,
    new_qual_files: &[qual_file::QualFile],
) -> crate::Result<Vec<Record>> {
    let mut paths: HashSet<PathBuf> = HashSet::new();

    for qf in new_qual_files {
        if let Ok(rel) = qf.path.strip_prefix(project_root) {
            paths.insert(rel.to_path_buf());
        }
    }
    for p in qual_paths_at_ref(project_root, git_ref)? {
        paths.insert(p);
    }

    let mut all = Vec::new();
    for rel in paths {
        let blob = match git_show(project_root, git_ref, &rel) {
            Some(b) => b,
            None => continue, // file did not exist at <ref>
        };
        match qual_file::parse_str(&blob) {
            Ok(records) => all.extend(records),
            Err(e) => {
                // Don't abort the diff for one malformed historical line;
                // surface it as a hint and skip.
                eprintln!(
                    "qualifier diff: skipping {} at {}: {}",
                    rel.display(),
                    git_ref,
                    e
                );
            }
        }
    }
    Ok(all)
}

fn qual_paths_at_ref(project_root: &Path, git_ref: &str) -> crate::Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", git_ref])
        .current_dir(project_root)
        .output()
        .map_err(|e| crate::Error::Validation(format!("git ls-tree failed: {e}")))?;
    if !output.status.success() {
        return Err(crate::Error::Validation(format!(
            "git ls-tree failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    let paths = listing
        .lines()
        .filter(|p| {
            let path = Path::new(p);
            path.extension().and_then(|e| e.to_str()) == Some("qual")
                || path.file_name().and_then(|f| f.to_str()) == Some(".qual")
        })
        .map(PathBuf::from)
        .collect();
    Ok(paths)
}

fn git_show(project_root: &Path, git_ref: &str, rel_path: &Path) -> Option<String> {
    let spec = format!("{git_ref}:{}", rel_path.display());
    let output = Command::new("git")
        .args(["show", &spec])
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn ref_exists(project_root: &Path, git_ref: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", git_ref])
        .current_dir(project_root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn compute_diff(old: &[Record], new: &[Record], project_root: &Path) -> Diff {
    let old_active: Vec<&Record> = filter_superseded(old);
    let new_active: Vec<&Record> = filter_superseded(new);

    let old_ids: HashSet<&str> = old.iter().map(|r| r.id()).collect();
    let old_active_ids: HashSet<&str> = old_active.iter().map(|r| r.id()).collect();
    let new_active_ids: HashSet<&str> = new_active.iter().map(|r| r.id()).collect();

    // Added: active on HEAD, not present at all in <ref>. Annotations only —
    // epoch/dependency are noise here. Resolve-kind records are filtered out
    // because they're surfaced as the closer in the Resolved section already;
    // listing them under Added too would double-count the same event.
    let mut added: Vec<Record> = new_active
        .iter()
        .filter(|r| !old_ids.contains(r.id()))
        .filter(|r| r.as_annotation().is_some())
        .filter(|r| r.kind() != Some(&Kind::Resolve))
        .map(|r| (*r).clone())
        .collect();
    added.sort_by_key(sort_key);

    // Resolved: active at <ref>, not active at HEAD. Find the closer
    // (any new record whose `supersedes` points at the resolved id).
    let supersedes_index: HashMap<&str, &Record> = new
        .iter()
        .filter_map(|r| r.supersedes().map(|s| (s, r)))
        .collect();

    let mut resolved: Vec<ResolvedEntry> = old_active
        .iter()
        .filter(|r| !new_active_ids.contains(r.id()))
        .map(|r| ResolvedEntry {
            old: (*r).clone(),
            closer: supersedes_index.get(r.id()).map(|c| (*c).clone()),
        })
        .collect();
    resolved.sort_by_key(|e| sort_key(&e.old));

    // Drift: active records on HEAD that were also present at <ref>, with a
    // content_hash that no longer matches the file. Limiting to records also
    // present at <ref> means freshly-recorded annotations don't show up
    // (you just wrote them; their span IS the current code).
    let mut drifted: Vec<DriftEntry> = Vec::new();
    for r in &new_active {
        if !old_active_ids.contains(r.id()) {
            continue;
        }
        let att = match r.as_annotation() {
            Some(a) => a,
            None => continue,
        };
        let span = match &att.body.span {
            Some(s) => s,
            None => continue,
        };
        if span.content_hash.is_none() {
            continue;
        }
        let file = project_root.join(&att.subject);
        if let FreshnessStatus::Drifted { expected, actual } =
            content_hash::check_freshness(&file, span)
        {
            drifted.push(DriftEntry {
                record: (*r).clone(),
                expected,
                actual,
            });
        }
    }
    drifted.sort_by_key(|e| sort_key(&e.record));

    Diff {
        added,
        resolved,
        drifted,
    }
}

fn sort_key(r: &Record) -> (String, u32) {
    let line = r
        .as_annotation()
        .and_then(|a| a.body.span.as_ref())
        .map(|s| s.start.line)
        .unwrap_or(0);
    (r.subject().to_string(), line)
}

fn print_human(git_ref: &str, diff: &Diff) {
    if diff.added.is_empty() && diff.resolved.is_empty() && diff.drifted.is_empty() {
        println!("No annotation changes since {git_ref}.");
        return;
    }

    if !diff.added.is_empty() {
        println!();
        println!("Added on this branch ({})", diff.added.len());
        for r in &diff.added {
            print_added(r);
        }
    }

    if !diff.resolved.is_empty() {
        println!();
        println!("Resolved on this branch ({})", diff.resolved.len());
        for entry in &diff.resolved {
            print_resolved(entry);
        }
    }

    if !diff.drifted.is_empty() {
        println!();
        println!("Drifted ({})", diff.drifted.len());
        for entry in &diff.drifted {
            print_drifted(entry);
        }
    }
    println!();
}

fn print_added(r: &Record) {
    let att = match r.as_annotation() {
        Some(a) => a,
        None => return,
    };
    let id_short = id_prefix(&att.id);
    let loc = format_location(att);
    println!(
        "  + {:<10} {:<32}  {}  ({})",
        att.body.kind.to_string(),
        loc,
        att.body.summary,
        id_short
    );
}

fn print_resolved(entry: &ResolvedEntry) {
    let id_short = id_prefix(entry.old.id());
    let loc = entry
        .old
        .as_annotation()
        .map(format_location)
        .unwrap_or_else(|| entry.old.subject().to_string());
    let kind = entry
        .old
        .kind()
        .map(|k| k.to_string())
        .unwrap_or_else(|| entry.old.record_type().to_string());
    let summary = entry
        .old
        .as_annotation()
        .map(|a| a.body.summary.clone())
        .unwrap_or_default();

    let suffix = match &entry.closer {
        Some(c) => {
            let closer_kind = c.kind().map(|k| k.to_string()).unwrap_or_default();
            let closer_id = id_prefix(c.id());
            if closer_kind == Kind::Resolve.to_string() {
                format!(" — resolved by {closer_id}")
            } else {
                format!(" — superseded by {closer_id}")
            }
        }
        None => " — removed (no successor)".into(),
    };
    println!(
        "  - {:<10} {:<32}  {}  ({}){}",
        kind, loc, summary, id_short, suffix
    );
}

fn print_drifted(entry: &DriftEntry) {
    let att = match entry.record.as_annotation() {
        Some(a) => a,
        None => return,
    };
    let id_short = id_prefix(&att.id);
    let loc = format_location(att);
    let exp = id_prefix(&entry.expected);
    let act = id_prefix(&entry.actual);
    println!(
        "  ~ {:<10} {:<32}  span content moved (expected {}, got {})  ({})",
        att.body.kind.to_string(),
        loc,
        exp,
        act,
        id_short
    );
}

fn id_prefix(id: &str) -> &str {
    if id.len() >= 8 { &id[..8] } else { id }
}

fn format_location(att: &crate::annotation::Annotation) -> String {
    match &att.body.span {
        Some(span) => {
            let end = match &span.end {
                Some(e) if e.line != span.start.line => format!(":{}", e.line),
                _ => String::new(),
            };
            format!("{}:{}{}", att.subject, span.start.line, end)
        }
        None => att.subject.clone(),
    }
}

fn print_json(git_ref: &str, diff: &Diff) {
    let added: Vec<_> = diff.added.iter().collect();
    let resolved: Vec<serde_json::Value> = diff
        .resolved
        .iter()
        .map(|e| {
            serde_json::json!({
                "record": e.old,
                "closer": e.closer,
            })
        })
        .collect();
    let drifted: Vec<serde_json::Value> = diff
        .drifted
        .iter()
        .map(|d| {
            serde_json::json!({
                "record": d.record,
                "expected": d.expected,
                "actual": d.actual,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "ref": git_ref,
        "added": added,
        "resolved": resolved,
        "drifted": drifted,
    });
    println!("{}", serde_json::to_string_pretty(&payload).unwrap());
}
