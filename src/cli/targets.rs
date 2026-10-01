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
        self.normalize(arg, &self.cwd_rel)
    }

    /// Normalize a subject taken from a record envelope. Envelope subjects
    /// are already root-relative, so they are folded against the root,
    /// not the current directory; a subject that leaves the project root
    /// is an error.
    pub(crate) fn stored_subject(&self, subject: &str) -> crate::Result<String> {
        self.normalize(subject, Path::new(""))
    }

    fn normalize(&self, arg: &str, base: &Path) -> crate::Result<String> {
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
            self.strip_root(path).ok_or_else(outside)?
        } else {
            base.join(path)
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

    /// `path` relative to the root. The lexical comparison is tried first;
    /// failing that, both sides are canonicalized, so a path that reaches
    /// the project through a symlink (macOS `/tmp` is `/private/tmp`)
    /// still resolves. A path that does not exist yet is canonicalized
    /// through its longest existing ancestor.
    fn strip_root(&self, path: &Path) -> Option<PathBuf> {
        if let Ok(rel) = path.strip_prefix(&self.root) {
            return Some(rel.to_path_buf());
        }
        let root = self.root.canonicalize().ok()?;
        let mut existing = path;
        let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
        let canonical = loop {
            if let Ok(c) = existing.canonicalize() {
                break c;
            }
            rest.push(existing.file_name()?);
            existing = existing.parent()?;
        };
        let mut full = canonical;
        full.extend(rest.iter().rev());
        full.strip_prefix(&root).ok().map(Path::to_path_buf)
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
    /// `--file` keeps its CWD-relative meaning. Either way, a path outside
    /// the project root is an error. Creates nothing; see [`append`].
    pub(crate) fn write_path(
        &self,
        subject: &str,
        explicit: Option<&Path>,
    ) -> crate::Result<PathBuf> {
        if let Some(p) = explicit {
            self.normalize(&p.to_string_lossy(), &self.cwd_rel)
                .map_err(|_| {
                    crate::Error::Validation(format!(
                        "--file '{}' is outside the project root ({})",
                        p.display(),
                        self.root.display()
                    ))
                })?;
            return Ok(p.to_path_buf());
        }
        let contained = Path::new(subject)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if !contained {
            return Err(crate::Error::Validation(format!(
                "subject '{subject}' is outside the project root ({})",
                self.root.display()
            )));
        }
        let one_to_one = self.root.join(format!("{subject}.qual"));
        if one_to_one.exists() {
            return Ok(one_to_one);
        }
        Ok(match Path::new(subject).parent() {
            Some(parent) if !parent.as_os_str().is_empty() => self.root.join(parent).join(".qual"),
            _ => self.root.join(".qual"),
        })
    }

    /// Fail when discovery would skip `qual_path` (a path from
    /// [`Self::write_path`]) because an ignore file hides it: a record
    /// written there could never be read back. Checks `.gitignore`,
    /// `.ignore` and `.qualignore` in the project root and in every
    /// directory down to the file; a deeper file's rules win. The error
    /// names the rule and the `--no-ignore` escape hatch.
    pub(crate) fn check_not_ignored(&self, qual_path: &Path) -> crate::Result<()> {
        use ignore::Match;
        use ignore::gitignore::GitignoreBuilder;

        let abs = if qual_path.is_absolute() {
            qual_path.to_path_buf()
        } else {
            self.root.join(&self.cwd_rel).join(qual_path)
        };
        let Some(rel) = self.strip_root(&abs) else {
            return Ok(());
        };
        let rel = PathBuf::from(self.normalize(&rel.to_string_lossy(), Path::new(""))?);
        let path = self.root.join(&rel);

        let mut dirs = vec![self.root.clone()];
        if let Some(parent) = rel.parent() {
            let mut dir = self.root.clone();
            for component in parent.components() {
                dir.push(component);
                dirs.push(dir.clone());
            }
        }
        for dir in dirs.iter().rev() {
            let mut builder = GitignoreBuilder::new(dir);
            for name in [".gitignore", ".ignore", ".qualignore"] {
                let file = dir.join(name);
                if file.is_file() {
                    // A malformed line is skipped, as discovery skips it.
                    let _ = builder.add(file);
                }
            }
            let Ok(matcher) = builder.build() else {
                continue;
            };
            match matcher.matched_path_or_any_parents(&path, false) {
                Match::Ignore(glob) => {
                    let from = glob
                        .from()
                        .and_then(|f| f.strip_prefix(&self.root).ok())
                        .map(|f| f.display().to_string())
                        .unwrap_or_else(|| "an ignore file".into());
                    return Err(crate::Error::Validation(format!(
                        "{} is ignored by '{}' in {from}, so no command would read a \
                         record written there; pass --no-ignore to write it anyway",
                        rel.display(),
                        glob.original(),
                    )));
                }
                Match::Whitelist(_) => return Ok(()),
                Match::None => {}
            }
        }
        Ok(())
    }

    /// The existing `.qual` file holding `subject`'s records, if any: the
    /// 1:1 file, else the directory-level file, under the project root.
    pub(crate) fn existing_qual_file(&self, subject: &str) -> Option<PathBuf> {
        let path = self.write_path(subject, None).ok()?;
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

/// Whether `target` can be an ID prefix: non-empty lowercase hex.
/// Anything else (`Makefile`, `src/a.rs`, `a.rs:3`) is a location.
fn looks_like_id_prefix(target: &str) -> bool {
    !target.is_empty()
        && target
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Resolve a target string to a unique, live record. A lowercase-hex
/// target is an ID prefix; when no record ID starts with it, it is tried
/// as a location (a file named `cafe`). Anything else is a `<location>`
/// (subject + optional span). A superseded or resolved record is rejected
/// (see [`ensure_live`]).
pub(crate) fn resolve_target(
    target: &str,
    qual_files: &[QualFile],
    locator: &Locator,
) -> crate::Result<Record> {
    let record = if looks_like_id_prefix(target) {
        let any_id_matches = qual_files
            .iter()
            .flat_map(|qf| qf.records.iter())
            .any(|r| r.id().starts_with(target));
        if any_id_matches {
            resolve_id_prefix(target, qual_files)?
        } else {
            resolve_location_target(target, qual_files, locator).map_err(|_| {
                crate::Error::Validation(format!(
                    "no record found matching prefix '{target}', and no active record \
                     at location '{target}'\nhint: for a file named '{target}', write ./{target}"
                ))
            })?
        }
    } else {
        resolve_location_target(target, qual_files, locator)?
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

    // Advice names the full ID: pointer fields accept nothing shorter.
    if tip.kind() == Some(&Kind::Resolve) {
        let closer = tip.id();
        return Err(crate::Error::Validation(format!(
            "target {} is closed (resolved by {}); reply to {closer} to comment on \
             the closed thread, or record a new record that supersedes {closer} to reopen it",
            short_id(record.id()),
            short_id(closer),
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
        "target {} is superseded by {} ({kind} {summary:?}); target the live record {}",
        short_id(record.id()),
        short_id(tip.id()),
        tip.id(),
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
/// [`ensure_live`]). Returns the target record. Errors are prefixed with
/// `flag`.
fn find_live<'a>(flag: &str, id: &str, qual_files: &'a [QualFile]) -> crate::Result<&'a Record> {
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
    Ok(record)
}

/// Check the pointer fields of a new record: `supersedes` and
/// `references` must each be the full ID of a live record in
/// `qual_files` (see [`find_live`]), and a superseded record must
/// have the new record's subject. Only the new record's own edges are
/// checked, so a bad record already on disk does not block unrelated
/// writes. A new record cannot close a supersession cycle: its ID hashes
/// its pointers, and every pointer names a record that already exists.
/// Errors are prefixed with `flag_prefix` plus the field name (`--` for
/// command-line flags, empty for batch keys).
pub(crate) fn check_pointers(
    record: &Record,
    qual_files: &[QualFile],
    flag_prefix: &str,
) -> crate::Result<()> {
    if let Some(id) = record.supersedes() {
        let flag = format!("{flag_prefix}supersedes");
        let target = find_live(&flag, id, qual_files)?;
        if target.subject() != record.subject() {
            return Err(crate::Error::Validation(format!(
                "{flag}: target {} is on subject '{}', not '{}' \
                 — cross-subject supersession is not allowed",
                short_id(id),
                target.subject(),
                record.subject(),
            )));
        }
    }
    if let Some(id) = record.references() {
        find_live(&format!("{flag_prefix}references"), id, qual_files)?;
    }
    Ok(())
}

pub(crate) fn record_created_at(r: &Record) -> chrono::DateTime<chrono::Utc> {
    match r {
        Record::Annotation(a) => *a.created_at,
        Record::Epoch(e) => *e.created_at,
        Record::Dependency(d) => *d.created_at,
        Record::Unknown(v) => v
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now),
    }
}
