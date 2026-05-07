//! `qualifier diff <ref>` — what changed in the annotation set on this
//! branch.
//!
//! Compares records on `HEAD` against records at a git ref (default `main`,
//! resolved via merge-base unless `--from-tip`). Output is grouped into
//! three buckets, all reckoned by record `id`:
//!
//! - **Added** — records active on `HEAD` whose id is not in `<ref>`.
//!   Annotations only; resolve-kind records are filtered to avoid
//!   double-counting with the closer in *Resolved*.
//! - **Resolved** — records active at `<ref>` that are no longer active on
//!   `HEAD`, with the closer (the head-side record whose `supersedes`
//!   points at it) named when one exists, or `removed` if not.
//! - **Drifted** — records present at *both* refs whose
//!   `body.span.content_hash` no longer matches the file's current
//!   content. Drift on records freshly added on this branch is suppressed.
//!
//! Both human and JSON output are stable; CI gating uses `--fail-on
//! <KIND[,KIND...]>` and `--fail-on-drift`.
//!
//! Backed by [`gix`] in-process — no subprocess spawn per `.qual` file
//! at the ref.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;

use crate::annotation::{Kind, Record};
use crate::cli::span_context;
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

    /// Compare against the tip of `<ref>` rather than its merge-base with HEAD.
    /// The default (merge-base) matches what a PR introduces — records that
    /// landed on `<ref>` after this branch forked are treated as "old", not
    /// "added".
    #[arg(long)]
    pub from_tip: bool,

    /// Exit non-zero if Added contains any record whose kind matches one of
    /// the comma-separated list. Common: `--fail-on blocker` for CI.
    #[arg(long, value_name = "KIND[,KIND...]")]
    pub fail_on: Option<String>,

    /// Exit non-zero if any record drifted.
    #[arg(long)]
    pub fail_on_drift: bool,

    /// Filter to records whose kind matches one of the comma-separated list.
    /// Applies to all three buckets (added, resolved, drifted).
    #[arg(long, value_name = "KIND[,KIND...]")]
    pub kind: Option<String>,

    /// Filter to records authored by this issuer-type (human, ai, tool, unknown).
    #[arg(long, value_name = "TYPE")]
    pub issuer_type: Option<String>,

    /// Print only the affected subjects, one per line, deduplicated. Pipes
    /// cleanly into `xargs qualifier show` and similar.
    #[arg(long)]
    pub subjects_only: bool,

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

/// `--fail-on*` errors are returned *after* the diff body has been printed
/// to stdout — the build log shows what triggered the failure.
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

    let repo = gix::open(&project_root).map_err(|e| {
        crate::Error::Validation(format!(
            "qualifier diff currently supports git only — could not open repository at {}: {e}",
            project_root.display()
        ))
    })?;

    // Validate the ref exists up-front so the user gets a clean error rather
    // than a smear of object-lookup failures.
    let ref_oid = match repo.rev_parse_single(args.r#ref.as_str()) {
        Ok(id) => id.detach(),
        Err(_) => {
            return Err(crate::Error::Validation(format!(
                "git ref '{}' not found",
                args.r#ref
            )));
        }
    };

    // Resolve the effective comparison commit. Default is the merge-base of
    // HEAD with <ref> — this isolates what this branch introduced from
    // anything that landed on <ref> after the branch forked. `--from-tip`
    // opts back into the literal ref.
    let effective_oid: gix::ObjectId = if args.from_tip {
        ref_oid
    } else {
        let head_oid = repo
            .head_id()
            .map_err(|e| crate::Error::Validation(format!("could not resolve HEAD: {e}")))?
            .detach();
        match repo.merge_base(ref_oid, head_oid) {
            Ok(base) => base.detach(),
            Err(_) => {
                eprintln!(
                    "qualifier diff: no merge-base between HEAD and '{}', comparing to ref tip",
                    args.r#ref
                );
                ref_oid
            }
        }
    };

    let new_qual_files = qual_file::discover(&project_root, !args.no_ignore)?;
    let new_records: Vec<Record> = new_qual_files
        .iter()
        .flat_map(|qf| qf.records.iter().cloned())
        .collect();

    let old_records = load_records_at_ref(&repo, effective_oid, &project_root, &new_qual_files)?;

    let mut diff = compute_diff(&old_records, &new_records, &project_root);
    apply_filters(&mut diff, &args)?;

    let header = DiffHeader {
        input_ref: args.r#ref.clone(),
        base: effective_oid.to_string(),
        from_tip: args.from_tip,
    };

    if args.subjects_only {
        print_subjects(&diff);
    } else if args.format == "json" {
        print_json(&header, &diff);
    } else {
        print_human(&header, &diff, &project_root);
    }

    enforce_fail_flags(&args, &diff)?;
    Ok(())
}

fn apply_filters(diff: &mut Diff, args: &Args) -> crate::Result<()> {
    let kinds: Option<Vec<String>> = args.kind.as_ref().map(|s| {
        s.split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect()
    });
    let issuer_type = match &args.issuer_type {
        Some(s) => Some(
            s.parse::<crate::annotation::IssuerType>()
                .map_err(crate::Error::Validation)?,
        ),
        None => None,
    };

    let kind_match = |r: &Record| -> bool {
        match &kinds {
            Some(list) => r
                .kind()
                .map(|k| list.iter().any(|allowed| allowed == &k.to_string()))
                .unwrap_or(false),
            None => true,
        }
    };
    let issuer_match = |r: &Record| -> bool {
        match &issuer_type {
            Some(want) => r.issuer_type() == Some(want),
            None => true,
        }
    };

    diff.added.retain(|r| kind_match(r) && issuer_match(r));
    diff.resolved
        .retain(|e| kind_match(&e.old) && issuer_match(&e.old));
    diff.drifted
        .retain(|d| kind_match(&d.record) && issuer_match(&d.record));
    Ok(())
}

fn print_subjects(diff: &Diff) {
    let mut subjects: Vec<&str> = diff
        .added
        .iter()
        .map(|r| r.subject())
        .chain(diff.resolved.iter().map(|e| e.old.subject()))
        .chain(diff.drifted.iter().map(|d| d.record.subject()))
        .collect();
    subjects.sort();
    subjects.dedup();
    for s in subjects {
        println!("{s}");
    }
}

struct DiffHeader {
    /// The ref the user typed (e.g. "main", "v0.5.0").
    input_ref: String,
    /// The resolved commit-ish used for comparison — the merge-base sha by
    /// default, or `input_ref` itself when `--from-tip` is set.
    base: String,
    from_tip: bool,
}

impl DiffHeader {
    fn human(&self) -> String {
        if self.from_tip {
            format!("Comparing HEAD against {} (tip)", self.input_ref)
        } else if self.base == self.input_ref {
            // No merge-base resolution happened (fallback path).
            format!("Comparing HEAD against {}", self.input_ref)
        } else {
            format!(
                "Comparing HEAD against merge-base of {} ({})",
                self.input_ref,
                short_sha(&self.base),
            )
        }
    }
}

/// Apply --fail-on / --fail-on-drift after the diff has printed. We always
/// surface the diff first so the user sees *what* triggered the failure.
fn enforce_fail_flags(args: &Args, diff: &Diff) -> crate::Result<()> {
    if args.fail_on_drift && !diff.drifted.is_empty() {
        return Err(crate::Error::Validation(format!(
            "diff failed: {} drifted record(s) (--fail-on-drift)",
            diff.drifted.len()
        )));
    }
    if let Some(ref list) = args.fail_on {
        let kinds: Vec<&str> = list
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let matched: Vec<&Record> = diff
            .added
            .iter()
            .filter(|r| {
                r.kind()
                    .map(|k| kinds.contains(&k.to_string().as_str()))
                    .unwrap_or(false)
            })
            .collect();
        if !matched.is_empty() {
            return Err(crate::Error::Validation(format!(
                "diff failed: {} added record(s) match --fail-on {}",
                matched.len(),
                list
            )));
        }
    }
    Ok(())
}

fn short_sha(s: &str) -> &str {
    if s.len() >= 7 { &s[..7] } else { s }
}

/// Paths from the current working tree are unioned with paths at `<ref>` so
/// that `.qual` files deleted on this branch still surface their old records
/// (otherwise we'd never see records under Resolved/removed for them).
fn load_records_at_ref(
    repo: &gix::Repository,
    commit_oid: gix::ObjectId,
    project_root: &Path,
    new_qual_files: &[qual_file::QualFile],
) -> crate::Result<Vec<Record>> {
    // Map relative-path -> blob oid for every .qual entry at <ref>.
    let qual_blobs_at_ref = enumerate_qual_blobs(repo, commit_oid)?;

    let mut paths: HashSet<PathBuf> = HashSet::new();
    for qf in new_qual_files {
        if let Ok(rel) = qf.path.strip_prefix(project_root) {
            paths.insert(rel.to_path_buf());
        }
    }
    for path in qual_blobs_at_ref.keys() {
        paths.insert(path.clone());
    }

    let mut all = Vec::new();
    for rel in paths {
        let Some(blob_oid) = qual_blobs_at_ref.get(&rel) else {
            continue; // file did not exist at <ref>
        };
        let blob = match repo.find_object(*blob_oid) {
            Ok(o) => o,
            Err(e) => {
                eprintln!(
                    "qualifier diff: cannot read blob for {} at {}: {e}",
                    rel.display(),
                    commit_oid
                );
                continue;
            }
        };
        let data = &blob.data;
        let s = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => {
                eprintln!(
                    "qualifier diff: skipping non-UTF8 blob for {} at {}",
                    rel.display(),
                    commit_oid
                );
                continue;
            }
        };
        match qual_file::parse_str(s) {
            Ok(records) => all.extend(records),
            Err(e) => {
                // Don't abort the diff for one malformed historical line.
                eprintln!(
                    "qualifier diff: skipping {} at {}: {}",
                    rel.display(),
                    commit_oid,
                    e
                );
            }
        }
    }
    Ok(all)
}

fn enumerate_qual_blobs(
    repo: &gix::Repository,
    commit_oid: gix::ObjectId,
) -> crate::Result<HashMap<PathBuf, gix::ObjectId>> {
    let commit = repo.find_commit(commit_oid).map_err(|e| {
        crate::Error::Validation(format!("could not read commit {commit_oid}: {e}"))
    })?;
    let tree = commit.tree().map_err(|e| {
        crate::Error::Validation(format!("could not read tree at {commit_oid}: {e}"))
    })?;

    let mut recorder = gix::traverse::tree::Recorder::default();
    tree.traverse()
        .breadthfirst(&mut recorder)
        .map_err(|e| crate::Error::Validation(format!("tree traversal failed: {e}")))?;

    let mut out = HashMap::new();
    for entry in recorder.records {
        if !entry.mode.is_blob() {
            continue;
        }
        let bytes: &[u8] = entry.filepath.as_ref();
        let Ok(path_str) = std::str::from_utf8(bytes) else {
            continue;
        };
        let path = PathBuf::from(path_str);
        if is_qual_path(&path) {
            out.insert(path, entry.oid);
        }
    }
    Ok(out)
}

fn is_qual_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("qual")
        || path.file_name().and_then(|f| f.to_str()) == Some(".qual")
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

fn print_human(header: &DiffHeader, diff: &Diff, project_root: &Path) {
    if diff.added.is_empty() && diff.resolved.is_empty() && diff.drifted.is_empty() {
        println!("{}: no annotation changes.", header.human());
        return;
    }

    println!();
    println!("{}", header.human());

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
            print_drifted(entry, project_root);
        }
    }
    println!();
}

fn print_added(r: &Record) {
    let Some(att) = r.as_annotation() else { return };
    print_record_row(
        '+',
        &att.body.kind.to_string(),
        &format_location(att),
        &att.body.summary,
        id_prefix(&att.id),
        &[],
    );
}

fn print_resolved(entry: &ResolvedEntry) {
    let kind = entry
        .old
        .kind()
        .map(|k| k.to_string())
        .unwrap_or_else(|| entry.old.record_type().to_string());
    let loc = entry
        .old
        .as_annotation()
        .map(format_location)
        .unwrap_or_else(|| entry.old.subject().to_string());
    let summary = entry
        .old
        .as_annotation()
        .map(|a| a.body.summary.as_str())
        .unwrap_or("");

    let closer_line = match &entry.closer {
        Some(c) => {
            let verb = if c.kind() == Some(&Kind::Resolve) {
                "resolved by"
            } else {
                "superseded by"
            };
            let closer_id = id_prefix(c.id());
            match c.as_annotation().map(|a| a.body.summary.as_str()) {
                Some(s) if !s.is_empty() => format!("{verb} {closer_id}: {s:?}"),
                _ => format!("{verb} {closer_id}"),
            }
        }
        None => "removed (no successor)".into(),
    };

    print_record_row(
        '-',
        &kind,
        &loc,
        summary,
        id_prefix(entry.old.id()),
        &[closer_line],
    );
}

fn print_drifted(entry: &DriftEntry, project_root: &Path) {
    let Some(att) = entry.record.as_annotation() else {
        return;
    };
    let id_short = id_prefix(&att.id);
    let loc = format_location(att);

    let mut continuations: Vec<String> = Vec::new();
    if let Some(ref span) = att.body.span {
        let ctx = span_context::read_span_context(
            &project_root.join(&att.subject),
            span,
            span_context::DEFAULT_CONTEXT_LINES,
        );
        let formatted = span_context::format_human(&ctx);
        for line in formatted.lines() {
            continuations.push(line.to_string());
        }
    }

    print_record_row(
        '~',
        &att.body.kind.to_string(),
        &loc,
        &att.body.summary,
        id_short,
        &continuations,
    );
}

/// Render one diff row, wrapping to the terminal width. Columns are kept
/// aligned (`marker  KIND  LOC  SUMMARY  (ID)`) when the whole line fits;
/// otherwise the summary and any extra continuations move to indented
/// follow-up lines so the header (KIND + LOC + ID) stays on one line.
///
/// Width is read from `$COLUMNS`, defaulting to 80 when unset.
fn print_record_row(
    marker: char,
    kind: &str,
    location: &str,
    summary: &str,
    id_short: &str,
    extras: &[String],
) {
    const KIND_WIDTH: usize = 10;
    const HEADER_INDENT: &str = "  ";
    const CONTINUATION_INDENT: &str = "      ";
    let width = term_width();

    let id_chunk = format!("({id_short})");
    let single = if summary.is_empty() {
        format!("{HEADER_INDENT}{marker} {kind:<KIND_WIDTH$} {location}  {id_chunk}",)
    } else {
        format!("{HEADER_INDENT}{marker} {kind:<KIND_WIDTH$} {location}  {summary}  {id_chunk}",)
    };

    if extras.is_empty() && display_width(&single) <= width {
        println!("{single}");
        return;
    }

    // Multi-line: header keeps marker+kind+loc+id; summary (if any) and
    // each extra are indented continuations, themselves truncated to width.
    let header = format!("{HEADER_INDENT}{marker} {kind:<KIND_WIDTH$} {location}  {id_chunk}",);
    println!("{}", truncate_to_width(&header, width));

    let cont_budget = width.saturating_sub(display_width(CONTINUATION_INDENT));
    if !summary.is_empty() {
        println!(
            "{CONTINUATION_INDENT}{}",
            truncate_to_width(summary, cont_budget)
        );
    }
    for line in extras {
        println!(
            "{CONTINUATION_INDENT}{}",
            truncate_to_width(line, cont_budget)
        );
    }
}

fn id_prefix(id: &str) -> &str {
    if id.len() >= 8 { &id[..8] } else { id }
}

/// Effective terminal width. Reads `$COLUMNS` (set by most shells when stdout
/// is a TTY); falls back to 80 columns when unset, malformed, or zero.
fn term_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or(80)
}

/// Display width, counted in chars (good enough for ASCII paths and English
/// summaries; non-ASCII may render slightly off in terminals that disagree
/// with us about grapheme width, but never produces output longer than this).
fn display_width(s: &str) -> usize {
    s.chars().count()
}

fn truncate_to_width(s: &str, max: usize) -> String {
    if display_width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let take = max - 1;
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
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

fn print_json(header: &DiffHeader, diff: &Diff) {
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
        "ref": header.input_ref,
        "base": header.base,
        "from_tip": header.from_tip,
        "added": added,
        "resolved": resolved,
        "drifted": drifted,
    });
    println!("{}", serde_json::to_string_pretty(&payload).unwrap());
}
