//! `qualifier diff <ref>` — what changed in the annotation set on this
//! branch.
//!
//! Compares records on `HEAD` against records at a git ref (default `main`,
//! resolved via merge-base unless `--from-tip`). Output is grouped into
//! four buckets:
//!
//! - **Added** — records active on `HEAD` whose id is not in `<ref>`.
//!   Annotations only; resolve-kind records are filtered to avoid
//!   double-counting with the closer in *Resolved*.
//! - **Changed** — open threads (matched by thread origin, see
//!   [`crate::threads`]) whose root at `<ref>` was superseded on this
//!   branch by an edit or re-anchor. Listed here instead of under both
//!   *Added* and *Resolved*.
//! - **Resolved** — records active at `<ref>` that are no longer active on
//!   `HEAD`, with the closers (the head-side records whose `supersedes`
//!   points at it) named when any exist, or `removed` if not.
//! - **Drifted** — records present at *both* refs whose
//!   `body.span.content_hash` no longer matches the file's current
//!   content. Drift on records freshly added on this branch is suppressed.
//!
//! Both human and JSON output are stable; CI gating uses `--fail-on
//! <KIND[,KIND...]>` (Added records, plus Changed records whose kind moved
//! into the list) and `--fail-on-drift`.
//!
//! Backed by [`gix`] in-process — no subprocess spawn per `.qual` file
//! at the ref.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::annotation::{Kind, Record};
use crate::cli::output::Format;
use crate::cli::span_context;
use crate::compact::filter_superseded;
use crate::content_hash::{self, FreshnessStatus};
use crate::qual_file;
use crate::threads::build_threads;

#[derive(ClapArgs)]
pub struct Args {
    /// Git ref to diff against. Defaults to `main`.
    #[arg(default_value = "main")]
    pub r#ref: String,

    /// Output format (human, json)
    #[arg(long, value_enum, default_value_t = Format::Human)]
    pub format: Format,

    /// Compare against the tip of `<ref>` rather than its merge-base with HEAD.
    /// The default (merge-base) matches what a PR introduces — records that
    /// landed on `<ref>` after this branch forked are treated as "old", not
    /// "added".
    #[arg(long)]
    pub from_tip: bool,

    /// Exit non-zero if Added contains any record whose kind matches one of
    /// the comma-separated list, or Changed contains a record whose kind
    /// moved into the list (e.g. concern -> blocker). Common: `--fail-on
    /// blocker` for CI.
    #[arg(long, value_name = "KIND[,KIND...]")]
    pub fail_on: Option<String>,

    /// Exit non-zero if any record drifted.
    #[arg(long)]
    pub fail_on_drift: bool,

    /// Filter to records whose kind matches one of the comma-separated list.
    /// Applies to every bucket; a Changed entry matches on either its old or
    /// its new kind.
    #[arg(long, value_name = "KIND[,KIND...]")]
    pub kind: Option<String>,

    /// Filter to records authored by this issuer-type (human, ai, tool, unknown).
    #[arg(long, value_name = "TYPE")]
    pub issuer_type: Option<String>,

    /// Print only the affected subjects, one per line, deduplicated. Pipes
    /// cleanly into `xargs qualifier show` and similar.
    #[arg(long)]
    pub subjects_only: bool,

    /// Disable .gitignore and .qualignore filtering. Without it, the working
    /// tree's ignore rules apply to both sides: .qual files on disk and
    /// .qual files read from <ref>.
    #[arg(long)]
    pub no_ignore: bool,
}

struct Diff {
    added: Vec<Record>,
    changed: Vec<ChangedEntry>,
    resolved: Vec<ResolvedEntry>,
    drifted: Vec<DriftEntry>,
}

/// An open thread whose root changed on this branch.
struct ChangedEntry {
    /// The thread's root at <ref>.
    previous: Record,
    /// The thread's root on this branch, which supersedes `previous`
    /// directly or through intermediate edits.
    record: Record,
}

impl ChangedEntry {
    fn kinds(&self) -> [Option<&Kind>; 2] {
        [self.previous.kind(), self.record.kind()]
    }
}

struct ResolvedEntry {
    /// Record from <ref> that is no longer active.
    old: Record,
    /// Records on this branch that supersede it, oldest first. Usually one;
    /// more after merging branches that each closed it.
    closers: Vec<Record>,
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
    let (effective_oid, comparison): (gix::ObjectId, Comparison) = if args.from_tip {
        (ref_oid, Comparison::Tip)
    } else {
        let head_oid = repo
            .head_id()
            .map_err(|e| crate::Error::Validation(format!("could not resolve HEAD: {e}")))?
            .detach();
        match repo.merge_base(ref_oid, head_oid) {
            Ok(base) => (base.detach(), Comparison::MergeBase),
            Err(_) => {
                eprintln!(
                    "qualifier diff: no merge-base between HEAD and '{}', comparing to ref tip",
                    args.r#ref
                );
                (ref_oid, Comparison::FallbackTip)
            }
        }
    };

    let new_qual_files = qual_file::discover(&project_root, !args.no_ignore)?;
    let new_records: Vec<Record> = new_qual_files
        .iter()
        .flat_map(|qf| qf.records.iter().cloned())
        .collect();

    let mut ignore = if args.no_ignore {
        None
    } else {
        Some(WorkTreeIgnore::new(
            &project_root,
            &repo.common_dir().join("info/exclude"),
        ))
    };
    let old_records = load_records_at_ref(&repo, effective_oid, ignore.as_mut())?;

    let kind_filter = parse_kind_list(args.kind.as_deref());
    let fail_on = parse_kind_list(args.fail_on.as_deref());
    for kind in unknown_kinds(
        kind_filter.iter().chain(fail_on.iter()).flatten(),
        &old_records,
        &new_records,
    ) {
        eprintln!("qualifier diff: warning: kind '{kind}' matches no known kind");
    }

    let mut diff = compute_diff(&old_records, &new_records, &project_root);
    apply_filters(&mut diff, kind_filter.as_deref(), &args)?;

    let header = DiffHeader {
        input_ref: args.r#ref.clone(),
        base: effective_oid.to_string(),
        from_tip: args.from_tip,
        comparison,
    };

    if args.subjects_only {
        print_subjects(&diff);
    } else if args.format == Format::Json {
        print_json(&header, &diff);
    } else {
        print_human(&header, &diff, &project_root);
    }

    enforce_fail_flags(&args, fail_on.as_deref(), &diff)?;
    Ok(())
}

/// Split a comma-separated `--kind` / `--fail-on` value, dropping blanks.
fn parse_kind_list(list: Option<&str>) -> Option<Vec<String>> {
    list.map(|s| {
        s.split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect()
    })
}

/// Kinds in `requested` that are neither built in nor carried by any record
/// on either side, in first-seen order. Custom kinds are legal, so these
/// are only warned about: a typo such as `blockers` would otherwise make a
/// `--fail-on` gate pass silently.
fn unknown_kinds<'a>(
    requested: impl Iterator<Item = &'a String>,
    old: &[Record],
    new: &[Record],
) -> Vec<&'a str> {
    const BUILT_IN: &[&str] = &[
        "pass",
        "fail",
        "blocker",
        "concern",
        "comment",
        "resolve",
        "praise",
        "suggestion",
        "waiver",
    ];
    let present: HashSet<String> = old
        .iter()
        .chain(new)
        .filter_map(|r| r.kind().map(|k| k.to_string()))
        .collect();
    let mut out: Vec<&str> = Vec::new();
    for kind in requested {
        if !BUILT_IN.contains(&kind.as_str())
            && !present.contains(kind)
            && !out.contains(&kind.as_str())
        {
            out.push(kind);
        }
    }
    out
}

fn apply_filters(diff: &mut Diff, kinds: Option<&[String]>, args: &Args) -> crate::Result<()> {
    let issuer_type = match &args.issuer_type {
        Some(s) => Some(
            s.parse::<crate::annotation::IssuerType>()
                .map_err(crate::Error::Validation)?,
        ),
        None => None,
    };

    let kind_match = |r: &Record| -> bool {
        match kinds {
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
    diff.changed
        .retain(|e| (kind_match(&e.previous) || kind_match(&e.record)) && issuer_match(&e.record));
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
        .chain(diff.changed.iter().map(|e| e.record.subject()))
        .chain(diff.resolved.iter().map(|e| e.old.subject()))
        .chain(diff.drifted.iter().map(|d| d.record.subject()))
        .collect();
    subjects.sort();
    subjects.dedup();
    for s in subjects {
        println!("{s}");
    }
}

/// Which commit `<ref>` was resolved to for the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Comparison {
    /// The merge-base of HEAD and `<ref>` (the default).
    MergeBase,
    /// The tip of `<ref>`, because `--from-tip` was passed.
    Tip,
    /// The tip of `<ref>`, because HEAD and `<ref>` share no merge-base.
    FallbackTip,
}

impl Comparison {
    /// The value of the JSON `comparison` field.
    fn as_str(self) -> &'static str {
        match self {
            Comparison::MergeBase => "merge-base",
            Comparison::Tip => "tip",
            Comparison::FallbackTip => "fallback-tip",
        }
    }
}

struct DiffHeader {
    /// The ref the user typed (e.g. "main", "v0.5.0").
    input_ref: String,
    /// The full SHA of the commit used for comparison.
    base: String,
    from_tip: bool,
    comparison: Comparison,
}

impl DiffHeader {
    fn human(&self) -> String {
        match self.comparison {
            Comparison::Tip => format!("Comparing HEAD against {} (tip)", self.input_ref),
            Comparison::FallbackTip => format!(
                "Comparing HEAD against {} (tip; no merge-base)",
                self.input_ref
            ),
            Comparison::MergeBase => format!(
                "Comparing HEAD against merge-base of {} ({})",
                self.input_ref,
                short_sha(&self.base),
            ),
        }
    }
}

/// Apply --fail-on / --fail-on-drift after the diff has printed. We always
/// surface the diff first so the user sees *what* triggered the failure.
fn enforce_fail_flags(args: &Args, fail_on: Option<&[String]>, diff: &Diff) -> crate::Result<()> {
    if args.fail_on_drift && !diff.drifted.is_empty() {
        return Err(crate::Error::Validation(format!(
            "diff failed: {} drifted record(s) (--fail-on-drift)",
            diff.drifted.len()
        )));
    }
    if let (Some(list), Some(kinds)) = (&args.fail_on, fail_on) {
        let matched: Vec<&Record> = diff
            .added
            .iter()
            .filter(|r| {
                r.kind()
                    .map(|k| kinds.contains(&k.to_string()))
                    .unwrap_or(false)
            })
            .collect();
        let in_list = |k: Option<&Kind>| k.is_some_and(|k| kinds.contains(&k.to_string()));
        let escalated = diff
            .changed
            .iter()
            .filter(|e| {
                let [before, after] = e.kinds();
                in_list(after) && !in_list(before)
            })
            .count();
        if !matched.is_empty() || escalated > 0 {
            let mut parts = Vec::new();
            if !matched.is_empty() {
                parts.push(format!("{} added record(s)", matched.len()));
            }
            if escalated > 0 {
                parts.push(format!("{escalated} changed record(s)"));
            }
            return Err(crate::Error::Validation(format!(
                "diff failed: {} match --fail-on {}",
                parts.join(" and "),
                list
            )));
        }
    }
    Ok(())
}

fn short_sha(s: &str) -> &str {
    if s.len() >= 7 { &s[..7] } else { s }
}

/// Read every `.qual` blob in the tree at `commit_oid`, skipping paths that
/// `ignore` matches. Files deleted on this branch are still in that tree,
/// which is how their records reach the Resolved bucket as removed.
fn load_records_at_ref(
    repo: &gix::Repository,
    commit_oid: gix::ObjectId,
    mut ignore: Option<&mut WorkTreeIgnore>,
) -> crate::Result<Vec<Record>> {
    let qual_blobs_at_ref = enumerate_qual_blobs(repo, commit_oid)?;

    let mut all = Vec::new();
    for (rel, blob_oid) in qual_blobs_at_ref {
        if let Some(ig) = ignore.as_deref_mut()
            && ig.is_ignored(&rel)
        {
            continue;
        }
        let blob = match repo.find_object(blob_oid) {
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
) -> crate::Result<BTreeMap<PathBuf, gix::ObjectId>> {
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

    let mut out = BTreeMap::new();
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

/// The working tree's ignore rules, applied to paths read from `<ref>` so
/// both sides of the diff see the same set of `.qual` files.
///
/// Mirrors the precedence `qual_file::discover` gets from the `ignore`
/// crate: `.qualignore`, then `.ignore`, then `.gitignore` (each searched
/// from the deepest directory up), then `.git/info/exclude`, then the
/// global excludes file. A path is ignored when it or any parent directory
/// is, because the walk never descends into an ignored directory.
struct WorkTreeIgnore {
    root: PathBuf,
    exclude: Gitignore,
    global: Gitignore,
    /// Rules read from each directory, keyed by path relative to `root`.
    dirs: HashMap<PathBuf, DirRules>,
}

/// Ignore files in one directory, highest precedence first.
struct DirRules([Gitignore; 3]);

impl WorkTreeIgnore {
    fn new(root: &Path, exclude_file: &Path) -> Self {
        WorkTreeIgnore {
            root: root.to_path_buf(),
            exclude: load_ignore_file(root, exclude_file),
            global: GitignoreBuilder::new(root).build_global().0,
            dirs: HashMap::new(),
        }
    }

    fn is_ignored(&mut self, rel: &Path) -> bool {
        let components: Vec<_> = rel.components().collect();
        let mut prefix = PathBuf::new();
        for (i, c) in components.iter().enumerate() {
            prefix.push(c);
            let is_dir = i + 1 < components.len();
            if self.matched(&prefix, is_dir).is_ignore() {
                return true;
            }
        }
        false
    }

    fn matched(&mut self, rel: &Path, is_dir: bool) -> Match<()> {
        let abs = self.root.join(rel);
        let ancestors: Vec<PathBuf> = rel.ancestors().skip(1).map(Path::to_path_buf).collect();
        for dir in &ancestors {
            if !self.dirs.contains_key(dir) {
                let full = self.root.join(dir);
                let rules = DirRules([
                    load_ignore_file(&full, &full.join(".qualignore")),
                    load_ignore_file(&full, &full.join(".ignore")),
                    load_ignore_file(&full, &full.join(".gitignore")),
                ]);
                self.dirs.insert(dir.clone(), rules);
            }
        }
        for category in 0..3 {
            for dir in &ancestors {
                let m = self.dirs[dir].0[category].matched(&abs, is_dir);
                if !m.is_none() {
                    return m.map(|_| ());
                }
            }
        }
        let m = self.exclude.matched(&abs, is_dir);
        if !m.is_none() {
            return m.map(|_| ());
        }
        self.global.matched(&abs, is_dir).map(|_| ())
    }
}

/// Parse one ignore file rooted at `dir`, or an empty matcher when the file
/// is absent or unreadable.
fn load_ignore_file(dir: &Path, file: &Path) -> Gitignore {
    if !file.is_file() {
        return Gitignore::empty();
    }
    let mut builder = GitignoreBuilder::new(dir);
    builder.add(file);
    builder.build().unwrap_or_else(|_| Gitignore::empty())
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
    let changed = find_changed(old, new, &old_ids);
    let changed_new: HashSet<&str> = changed.iter().map(|e| e.record.id()).collect();
    let changed_old: HashSet<&str> = changed.iter().map(|e| e.previous.id()).collect();

    let mut added: Vec<Record> = new_active
        .iter()
        .filter(|r| !old_ids.contains(r.id()))
        .filter(|r| !changed_new.contains(r.id()))
        .filter(|r| r.as_annotation().is_some())
        .filter(|r| r.kind() != Some(&Kind::Resolve))
        .map(|r| (*r).clone())
        .collect();
    added.sort_by_key(sort_key);

    // Resolved: active at <ref>, not active at HEAD. Find the closers
    // (every new record whose `supersedes` points at the resolved id).
    let mut supersedes_index: HashMap<&str, Vec<&Record>> = HashMap::new();
    for r in new {
        if let Some(target) = r.supersedes() {
            supersedes_index.entry(target).or_default().push(r);
        }
    }
    for closers in supersedes_index.values_mut() {
        closers.sort_by(|a, b| created_at(a).cmp(&created_at(b)).then(a.id().cmp(b.id())));
    }

    let mut resolved: Vec<ResolvedEntry> = old_active
        .iter()
        .filter(|r| !new_active_ids.contains(r.id()))
        .filter(|r| !changed_old.contains(r.id()))
        .map(|r| ResolvedEntry {
            old: (*r).clone(),
            closers: supersedes_index
                .get(r.id())
                .map(|cs| cs.iter().map(|c| (*c).clone()).collect())
                .unwrap_or_default(),
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
        changed,
        resolved,
        drifted,
    }
}

/// Threads open on both sides whose root on this branch is a record not
/// present at <ref>: an edit or re-anchor of the root, matched by thread
/// origin so a chain of several edits still pairs with the <ref>-side root.
fn find_changed(old: &[Record], new: &[Record], old_ids: &HashSet<&str>) -> Vec<ChangedEntry> {
    let old_roots: HashMap<&str, &Record> = build_threads(old)
        .into_iter()
        .filter(|t| t.open)
        .map(|t| (t.origin, t.root))
        .collect();
    let mut changed: Vec<ChangedEntry> = build_threads(new)
        .into_iter()
        .filter(|t| t.open && !old_ids.contains(t.root.id()))
        .filter_map(|t| {
            let previous = old_roots.get(t.origin)?;
            Some(ChangedEntry {
                previous: (*previous).clone(),
                record: t.root.clone(),
            })
        })
        .collect();
    changed.sort_by_key(|e| sort_key(&e.record));
    changed
}

fn created_at(r: &Record) -> Option<chrono::DateTime<chrono::Utc>> {
    r.as_annotation().map(|a| *a.created_at)
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
    if diff.added.is_empty()
        && diff.changed.is_empty()
        && diff.resolved.is_empty()
        && diff.drifted.is_empty()
    {
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

    if !diff.changed.is_empty() {
        println!();
        println!("Changed on this branch ({})", diff.changed.len());
        for entry in &diff.changed {
            print_changed(entry);
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

fn print_changed(entry: &ChangedEntry) {
    let Some(att) = entry.record.as_annotation() else {
        return;
    };
    let was = match entry.previous.as_annotation() {
        Some(prev) => {
            let head = format!(
                "was {} {} ({})",
                prev.body.kind,
                format_location(prev),
                id_prefix(&prev.id)
            );
            if prev.body.summary != att.body.summary && !prev.body.summary.is_empty() {
                format!("{head}: {:?}", prev.body.summary)
            } else {
                head
            }
        }
        None => format!("was {}", id_prefix(entry.previous.id())),
    };
    print_record_row(
        '*',
        &att.body.kind.to_string(),
        &format_location(att),
        &att.body.summary,
        id_prefix(&att.id),
        &[was],
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

    let mut closer_lines: Vec<String> = entry
        .closers
        .iter()
        .map(|c| {
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
        })
        .collect();
    if closer_lines.is_empty() {
        closer_lines.push("removed (no successor)".into());
    }

    print_record_row(
        '-',
        &kind,
        &loc,
        summary,
        id_prefix(entry.old.id()),
        &closer_lines,
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
            &att.subject,
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
    let changed: Vec<serde_json::Value> = diff
        .changed
        .iter()
        .map(|e| {
            serde_json::json!({
                "record": e.record,
                "previous": e.previous,
            })
        })
        .collect();
    let resolved: Vec<serde_json::Value> = diff
        .resolved
        .iter()
        .map(|e| {
            serde_json::json!({
                "record": e.old,
                "closer": e.closers.last(),
                "closers": e.closers,
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
        "comparison": header.comparison.as_str(),
        "added": added,
        "changed": changed,
        "resolved": resolved,
        "drifted": drifted,
    });
    println!("{}", serde_json::to_string_pretty(&payload).unwrap());
}
