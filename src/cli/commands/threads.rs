//! `qualifier threads` — list conversations across the project.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use globset::{GlobBuilder, GlobMatcher};

use crate::annotation::{self, IssuerType, Kind, Record, Span};
use crate::cli::targets::{self, short_id};
use crate::threads::{self, Thread};

#[derive(ClapArgs)]
pub struct Args {
    /// Filter by location: a path, a directory, a glob (`src/**/*.rs`), or
    /// `path:start[:end]` (threads whose root span overlaps). Any match
    /// selects the thread.
    pub locations: Vec<String>,

    /// Include closed threads and superseded replies
    #[arg(long)]
    pub all: bool,

    /// Root kinds to include (comma-separated)
    #[arg(long, value_name = "KIND[,KIND...]")]
    pub kind: Option<String>,

    /// Tag on the root or a live reply; `ns:*` matches a namespace.
    /// Repeatable; every tag must match.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Issuer type of the root (human, ai, tool, unknown)
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Don't respect .gitignore / .qualignore
    #[arg(long)]
    pub no_ignore: bool,

    /// Latest `status:*` tag on the thread (root or live reply)
    #[arg(long, value_parser = ["needs-decision", "decided", "deferred"])]
    pub status: Option<String>,

    /// Only threads whose root subject changed between the merge base of
    /// HEAD and REF and the working tree, including untracked files (git only)
    #[arg(long, value_name = "REF")]
    pub changed_since: Option<String>,

    /// Print at most two `qualifier: ` summary lines, for session-start
    /// hooks. Prints nothing when there is nothing to report.
    #[arg(long)]
    pub summary: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    let qual_files = targets::discover_project(!args.no_ignore)?;
    let records: Vec<Record> = qual_files.into_iter().flat_map(|qf| qf.records).collect();
    let all = threads::build_threads(&records);

    if args.summary {
        print_summary(&all, args.changed_since.as_deref());
        return Ok(());
    }

    let mut filter = Filter::from_args(&args)?;
    if let Some(base) = args.changed_since.as_deref() {
        filter.changed = Some(changed_files(&repo_root()?, base)?);
    }

    let selected: Vec<Thread<'_>> = all
        .into_iter()
        .filter(|t| filter.matches(t))
        .map(|t| if args.all { t } else { live_replies_only(t) })
        .collect();

    if args.format == "json" {
        print_json(&selected)
    } else {
        print_human(&selected, args.all);
        Ok(())
    }
}

struct Filter {
    all: bool,
    locations: Vec<LocationFilter>,
    kinds: Option<Vec<Kind>>,
    tags: Vec<String>,
    issuer_type: Option<IssuerType>,
    status: Option<String>,
    changed: Option<HashSet<String>>,
}

impl Filter {
    fn from_args(args: &Args) -> crate::Result<Self> {
        let locations = args
            .locations
            .iter()
            .map(|l| LocationFilter::parse(l))
            .collect::<crate::Result<Vec<_>>>()?;
        let kinds = args.kind.as_deref().map(|s| {
            s.split(',')
                .map(|k| k.trim().parse::<Kind>().unwrap())
                .collect()
        });
        let issuer_type = args
            .issuer_type
            .as_deref()
            .map(|s| s.parse::<IssuerType>().map_err(crate::Error::Validation))
            .transpose()?;
        Ok(Self {
            all: args.all,
            locations,
            kinds,
            tags: args.tags.clone(),
            issuer_type,
            status: args.status.clone(),
            changed: None,
        })
    }

    fn matches(&self, t: &Thread<'_>) -> bool {
        (self.all || t.open)
            && (self.locations.is_empty() || self.locations.iter().any(|l| l.matches(t.root)))
            && self
                .kinds
                .as_ref()
                .is_none_or(|ks| t.root.kind().is_some_and(|k| ks.contains(k)))
            && self
                .tags
                .iter()
                .all(|pat| thread_tags(t).any(|tag| tag_matches(pat, tag)))
            && self
                .issuer_type
                .as_ref()
                .is_none_or(|it| t.root.issuer_type() == Some(it))
            && self
                .status
                .as_deref()
                .is_none_or(|s| latest_status(t) == Some(s))
            && self
                .changed
                .as_ref()
                .is_none_or(|files| touches(files, t.root.subject()))
    }
}

enum LocationFilter {
    Glob(GlobMatcher),
    Path { subject: String, span: Option<Span> },
}

impl LocationFilter {
    fn parse(s: &str) -> crate::Result<Self> {
        if s.contains(['*', '?', '[']) {
            let glob = GlobBuilder::new(s)
                .literal_separator(true)
                .build()
                .map_err(|e| crate::Error::Validation(format!("invalid glob '{s}': {e}")))?;
            return Ok(Self::Glob(glob.compile_matcher()));
        }
        let (subject, span) = annotation::parse_location(s);
        let subject = subject
            .trim_start_matches("./")
            .trim_end_matches('/')
            .to_string();
        Ok(Self::Path { subject, span })
    }

    fn matches(&self, root: &Record) -> bool {
        match self {
            Self::Glob(m) => m.is_match(root.subject()),
            Self::Path { subject, span } => {
                let rs = root.subject();
                let path_ok = subject.is_empty()
                    || subject == "."
                    || rs == subject
                    || rs.starts_with(&format!("{subject}/"));
                path_ok
                    && span.as_ref().is_none_or(|s| {
                        root.as_annotation()
                            .and_then(|a| a.body.span.as_ref())
                            .is_some_and(|rsp| targets::span_overlaps(rsp, s))
                    })
            }
        }
    }
}

/// Tags on the root and on every live reply.
fn thread_tags<'a>(t: &Thread<'a>) -> impl Iterator<Item = &'a str> {
    std::iter::once(t.root)
        .chain(t.replies.iter().filter(|e| e.active).map(|e| e.record))
        .filter_map(|r| r.as_annotation())
        .flat_map(|a| a.body.tags.iter().map(String::as_str))
}

/// `ns:*` matches any tag starting with `ns:`; anything else matches exactly.
fn tag_matches(pattern: &str, tag: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => tag.starts_with(prefix),
        None => tag == pattern,
    }
}

/// The thread's latest `status:*` value (addressee suffix dropped), by
/// `created_at`, over the root and live replies.
fn latest_status<'a>(t: &Thread<'a>) -> Option<&'a str> {
    std::iter::once(t.root)
        .chain(t.replies.iter().filter(|e| e.active).map(|e| e.record))
        .filter_map(|r| r.as_annotation())
        .filter_map(|a| {
            a.body
                .tags
                .iter()
                .find_map(|tag| tag.strip_prefix("status:"))
                .map(|s| (a.created_at, s))
        })
        .max_by_key(|(at, _)| *at)
        .map(|(_, s)| s.split(':').next().unwrap_or(s))
}

fn touches(files: &HashSet<String>, subject: &str) -> bool {
    files.contains(subject) || files.iter().any(|f| f.starts_with(&format!("{subject}/")))
}

fn repo_root() -> crate::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    crate::qual_file::find_project_root(&cwd)
        .filter(|root| root.join(".git").exists())
        .ok_or_else(|| crate::Error::Validation("--changed-since requires a git repository".into()))
}

fn git(root: &Path, args: &[&str]) -> crate::Result<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()?;
    if !out.status.success() {
        return Err(crate::Error::Validation(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Files changed between the merge base of HEAD and `base` and the working
/// tree, plus untracked files, relative to the repository root.
fn changed_files(root: &Path, base: &str) -> crate::Result<HashSet<String>> {
    let merge_base = git(root, &["merge-base", "HEAD", base])?.trim().to_string();
    let mut files: HashSet<String> = git(root, &["diff", "--name-only", &merge_base])?
        .lines()
        .map(String::from)
        .collect();
    files.extend(
        git(root, &["ls-files", "--others", "--exclude-standard"])?
            .lines()
            .map(String::from),
    );
    Ok(files)
}

/// The base branch for `--summary`: the explicit ref, else `main`, else `master`.
fn summary_base(root: &Path, explicit: Option<&str>) -> Option<String> {
    if let Some(b) = explicit {
        return Some(b.to_string());
    }
    ["main", "master"]
        .into_iter()
        .find(|b| {
            git(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{b}"),
                ],
            )
            .is_ok()
        })
        .map(String::from)
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// Never fails: git problems degrade to project-wide counts.
fn print_summary(all: &[Thread<'_>], explicit_base: Option<&str>) {
    let open: Vec<&Thread<'_>> = all.iter().filter(|t| t.open).collect();

    // Scope to files changed on this branch; on the base branch itself (or
    // outside git) there is no branch to scope to, so count project-wide.
    let scoped = repo_root().ok().and_then(|root| {
        let base = summary_base(&root, explicit_base)?;
        let branch = git(&root, &["rev-parse", "--abbrev-ref", "HEAD"]).ok()?;
        if branch.trim() == base {
            return None;
        }
        let files = changed_files(&root, &base).ok()?;
        Some((base, files))
    });
    let in_scope = |t: &&Thread<'_>| {
        scoped
            .as_ref()
            .is_none_or(|(_, files)| touches(files, t.root.subject()))
    };
    let count_kind = |kind: Kind| {
        open.iter()
            .filter(|t| in_scope(t))
            .filter(|t| t.root.kind() == Some(&kind))
            .count()
    };
    let blockers = count_kind(Kind::Blocker);
    let concerns = count_kind(Kind::Concern);
    let waiting = open
        .iter()
        .filter(|t| latest_status(t) == Some("needs-decision"))
        .count();

    if blockers + concerns > 0 {
        let scope = scoped
            .as_ref()
            .map(|(base, _)| format!(" on files changed since {base}"))
            .unwrap_or_else(|| " (project-wide)".to_string());
        println!(
            "qualifier: {} and {} open{scope}",
            plural(blockers, "blocker"),
            plural(concerns, "concern"),
        );
    }
    if waiting > 0 {
        println!(
            "qualifier: {} waiting on a decision (qualifier threads --status needs-decision)",
            plural(waiting, "thread"),
        );
    }
}

fn live_replies_only(t: Thread<'_>) -> Thread<'_> {
    Thread {
        replies: t.replies.into_iter().filter(|e| e.active).collect(),
        ..t
    }
}

fn kind_label(r: &Record) -> String {
    r.kind()
        .map(|k| k.to_string())
        .unwrap_or_else(|| r.record_type().to_string())
}

fn summary(r: &Record) -> &str {
    r.as_annotation()
        .map(|a| a.body.summary.as_str())
        .unwrap_or("")
}

fn location(r: &Record) -> String {
    match r.as_annotation().and_then(|a| a.body.span.as_ref()) {
        Some(s) => match &s.end {
            Some(e) if e.line != s.start.line => {
                format!("{}:{}:{}", r.subject(), s.start.line, e.line)
            }
            _ => format!("{}:{}", r.subject(), s.start.line),
        },
        None => r.subject().to_string(),
    }
}

fn print_human(threads: &[Thread<'_>], all: bool) {
    for t in threads {
        let closed = if t.open { "" } else { " (closed)" };
        println!(
            "[{}] {:<10} {}  {}{closed}",
            short_id(t.root.id()),
            kind_label(t.root),
            location(t.root),
            summary(t.root),
        );
        if all {
            for r in &t.history {
                println!(
                    "    [{}] {:<10} {} (superseded)",
                    short_id(r.id()),
                    kind_label(r),
                    summary(r),
                );
            }
        }
        for e in &t.replies {
            let superseded = if e.active { "" } else { " (superseded)" };
            println!(
                "    [{}] {:<10} {}{superseded}",
                short_id(e.record.id()),
                kind_label(e.record),
                summary(e.record),
            );
        }
    }
    let open = threads.iter().filter(|t| t.open).count();
    let n = threads.len();
    eprintln!("{n} thread{} ({open} open)", if n == 1 { "" } else { "s" });
}

fn print_json(threads: &[Thread<'_>]) -> crate::Result<()> {
    let values = threads
        .iter()
        .map(thread_json)
        .collect::<crate::Result<Vec<_>>>()?;
    println!("{}", serde_json::to_string(&values)?);
    Ok(())
}

fn thread_json(t: &Thread<'_>) -> crate::Result<serde_json::Value> {
    let replies = t
        .replies
        .iter()
        .map(|e| -> crate::Result<serde_json::Value> {
            Ok(serde_json::json!({
                "active": e.active,
                "record": serde_json::to_value(e.record)?,
            }))
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let history = t
        .history
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "origin": t.origin,
        "open": t.open,
        "root": serde_json::to_value(t.root)?,
        "closed_by": t.closed_by.map(serde_json::to_value).transpose()?,
        "history": history,
        "replies": replies,
        "latest_at": t.latest_at.to_rfc3339(),
    }))
}
