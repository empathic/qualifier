//! Target resolution shared by the write commands: ID prefixes, locations,
//! and the liveness check that keeps writes off superseded records.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use crate::annotation::{self, Kind, Record, Span};
use crate::compact::filter_superseded;
use crate::qual_file::{self, QualFile};

/// The project root discovery walks from: the nearest VCS root above the
/// current directory, or the current directory itself outside a
/// repository. Always absolute, so it stays meaningful regardless of
/// which subdirectory a command is invoked from.
pub(crate) fn project_root() -> crate::Result<PathBuf> {
    // Resolve from an absolute CWD so the upward walk in find_project_root
    // works from any subdirectory — relative-path arithmetic on `.` doesn't
    // traverse up.
    let cwd = std::env::current_dir()?;
    Ok(qual_file::find_project_root(&cwd).unwrap_or(cwd))
}

/// Discover every `.qual` file under the project root (or the current
/// directory outside a repository).
pub(crate) fn discover_project(respect_ignore: bool) -> crate::Result<Vec<QualFile>> {
    qual_file::discover(&project_root()?, respect_ignore)
}

/// Maps location arguments to stored subjects. Arguments are interpreted
/// relative to the current directory; subjects are stored relative to the
/// project root. Every write lands in a `.qual` file under the project root.
pub(crate) struct Locator {
    root: PathBuf,
    /// The current directory relative to `root` (empty at the root).
    cwd_rel: PathBuf,
}

impl Locator {
    /// A locator for the current directory and its project root.
    pub(crate) fn from_cwd() -> crate::Result<Self> {
        let cwd = std::env::current_dir()?;
        let root = qual_file::find_project_root(&cwd).unwrap_or_else(|| cwd.clone());
        let cwd_rel = cwd
            .strip_prefix(&root)
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Ok(Self { root, cwd_rel })
    }

    /// The project root (absolute).
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Normalize a CWD-relative (or absolute) path argument to a
    /// root-relative subject: `.` and `..` are folded, separators become
    /// `/`, and the project root itself is `.`. A path that leaves the
    /// project root is an error. An empty argument is returned unchanged.
    pub(crate) fn subject(&self, arg: &str) -> crate::Result<String> {
        if arg.is_empty() {
            return Ok(String::new());
        }
        let outside = || {
            crate::Error::Validation(format!(
                "location '{arg}' is outside the project root ({})",
                self.root.display()
            ))
        };
        let path = Path::new(arg);
        let joined = if path.is_absolute() {
            path.strip_prefix(&self.root)
                .map_err(|_| outside())?
                .to_path_buf()
        } else {
            self.cwd_rel.join(path)
        };
        let mut parts: Vec<String> = Vec::new();
        for component in joined.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    parts.pop().ok_or_else(outside)?;
                }
                Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
                Component::RootDir | Component::Prefix(_) => return Err(outside()),
            }
        }
        Ok(if parts.is_empty() {
            ".".into()
        } else {
            parts.join("/")
        })
    }

    /// Parse a `path[:start[:end]]` location argument into a root-relative
    /// subject and optional span.
    pub(crate) fn location(&self, location: &str) -> crate::Result<(String, Option<Span>)> {
        let (path, span) = annotation::parse_location(location);
        Ok((self.subject(&path)?, span))
    }

    /// The on-disk path of a root-relative subject.
    pub(crate) fn file(&self, subject: &str) -> PathBuf {
        self.root.join(subject)
    }

    /// The `.qual` file that receives a new record about `subject`: the
    /// existing 1:1 `<subject>.qual`, else the directory-level `.qual`
    /// next to the subject, both under the project root. An explicit
    /// `--file` keeps its CWD-relative meaning. Creates nothing; see
    /// [`append`].
    pub(crate) fn write_path(&self, subject: &str, explicit: Option<&Path>) -> PathBuf {
        if let Some(p) = explicit {
            return p.to_path_buf();
        }
        let one_to_one = self.root.join(format!("{subject}.qual"));
        if one_to_one.exists() {
            return one_to_one;
        }
        match Path::new(subject).parent() {
            Some(parent) if !parent.as_os_str().is_empty() => self.root.join(parent).join(".qual"),
            _ => self.root.join(".qual"),
        }
    }

    /// The existing `.qual` file holding `subject`'s records, if any: the
    /// 1:1 file, else the directory-level file, under the project root.
    pub(crate) fn existing_qual_file(&self, subject: &str) -> Option<PathBuf> {
        let path = self.write_path(subject, None);
        path.exists().then_some(path)
    }
}

/// Append `record` to `path`, creating missing parent directories.
pub(crate) fn append(path: &Path, record: &Record) -> crate::Result<()> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
        && !dir.exists()
    {
        std::fs::create_dir_all(dir)?;
    }
    qual_file::append(path, record)
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
        n => {
            let mut msg = format!("ambiguous prefix '{prefix}' matches {n} records:\n");
            for r in &matches {
                let location = match r.as_annotation().and_then(|a| a.body.span.as_ref()) {
                    Some(s) => format!("{}:{}", r.subject(), s.start.line),
                    None => r.subject().to_string(),
                };
                msg.push_str(&candidate_line(r, &location));
            }
            msg.push_str("hint: use a longer ID prefix");
            Err(crate::Error::Validation(msg))
        }
    }
}

/// One disambiguation-list line: `  [id8] kind <where> "summary"`.
fn candidate_line(r: &Record, place: &str) -> String {
    let kind = r
        .kind()
        .map(|k| k.to_string())
        .unwrap_or_else(|| r.record_type().to_string());
    let summary = r
        .as_annotation()
        .map(|a| a.body.summary.as_str())
        .unwrap_or("");
    format!(
        "  [{}] {kind:<10} {place:<8} {summary:?}\n",
        short_id(r.id())
    )
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

/// Resolve a target string to a unique, live record. Accepts an id-prefix
/// or a `<location>` (subject + optional span). A superseded or resolved
/// record is rejected (see [`ensure_live`]).
pub(crate) fn resolve_target(
    target: &str,
    qual_files: &[QualFile],
    locator: &Locator,
) -> crate::Result<Record> {
    let record = if looks_like_location(target) {
        resolve_location_target(target, qual_files, locator)?
    } else {
        resolve_id_prefix(target, qual_files)?
    };
    ensure_live(&record, qual_files)?;
    Ok(record)
}

/// Fail when `record` has been superseded. The error names the live tip of
/// its supersession chain, or — when the chain ends in a `resolve` —
/// reports the record as closed and names the closing record.
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
        let closer = short_id(tip.id());
        return Err(crate::Error::Validation(format!(
            "target {} is closed (resolved by {closer}); reply to {closer} to comment on \
             the closed thread, or record a new record that supersedes {closer} to reopen it",
            short_id(record.id()),
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
        "target {} is superseded by {} ({kind} {summary:?}); target the live record",
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

/// The newest active record at `location` (a `resolve` is never a
/// candidate). Ties at the newest timestamp are reported with a candidate
/// list.
fn resolve_location_target(
    location: &str,
    qual_files: &[QualFile],
    locator: &Locator,
) -> crate::Result<Record> {
    let (subject, span_filter) = locator.location(location)?;

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
        .filter(|r| r.kind() != Some(&Kind::Resolve))
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
                let line = r
                    .as_annotation()
                    .and_then(|a| a.body.span.as_ref())
                    .map(|s| format!("L{}", s.start.line))
                    .unwrap_or_else(|| "—".into());
                msg.push_str(&candidate_line(r, &line));
            }
            msg.push_str("hint: specify a span (e.g., 'src/foo.rs:42') or use an id-prefix");
            return Err(crate::Error::Validation(msg));
        }
    }

    Ok(candidates[0].clone())
}

/// Check a pointer value (`--supersedes`, `--references`, or the same
/// keys on a batch line): it must be the full ID (64 lowercase hex) of a
/// record in `qual_files`, and that record must be live (see
/// [`ensure_live`]). Returns the ID. Errors are prefixed with `flag`.
pub(crate) fn require_live_id(
    flag: &str,
    id: &str,
    qual_files: &[QualFile],
) -> crate::Result<String> {
    let err = |msg: String| crate::Error::Validation(format!("{flag}: {msg}"));
    let is_full_id = id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !is_full_id {
        return Err(err(format!(
            "'{id}' must be a full record ID (64 lowercase hex characters)"
        )));
    }
    let record = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter())
        .find(|r| r.id() == id)
        .ok_or_else(|| {
            err(format!(
                "no record with ID {} (must be a full record ID of an existing record)",
                short_id(id)
            ))
        })?;
    ensure_live(record, qual_files).map_err(|e| err(e.to_string()))?;
    Ok(id.to_string())
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
