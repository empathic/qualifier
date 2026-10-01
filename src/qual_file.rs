use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::annotation::Record;

/// A parsed `.qual` file.
#[derive(Debug, Clone)]
pub struct QualFile {
    /// Path to the `.qual` file on disk.
    pub path: PathBuf,
    /// The subject this file describes (path minus `.qual` suffix).
    pub subject: String,
    /// Records in file order (oldest first).
    pub records: Vec<Record>,
}

/// A `.qual` line that could not be parsed as a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIssue {
    /// The `.qual` file.
    pub path: PathBuf,
    /// 1-indexed line number.
    pub line: usize,
    /// Why the line was rejected.
    pub message: String,
}

impl std::fmt::Display for ParseIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.path.display(), self.line, self.message)
    }
}

/// Parse a `.qual` file from disk.
///
/// Skips empty lines and lines starting with `//` (comments).
/// Each non-comment line must be a valid JSON record; the first one that
/// is not fails the parse. Use this before rewriting a file, so no line is
/// ever dropped.
pub fn parse(path: &Path) -> crate::Result<QualFile> {
    let (qual_file, issues) = parse_lenient(path)?;
    match issues.into_iter().next() {
        Some(issue) => Err(crate::Error::Validation(issue.to_string())),
        None => Ok(qual_file),
    }
}

/// Parse a `.qual` file from disk, skipping lines that are not valid
/// records and returning them as issues. Fails only when the file cannot
/// be read.
pub fn parse_lenient(path: &Path) -> crate::Result<(QualFile, Vec<ParseIssue>)> {
    let content = fs::read_to_string(path)?;
    let subject = subject_name(path);
    let mut records = Vec::new();
    let mut issues = Vec::new();

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        match serde_json::from_str::<Record>(trimmed) {
            Ok(record) => records.push(record),
            Err(e) => issues.push(ParseIssue {
                path: path.to_path_buf(),
                line: line_no + 1,
                message: e.to_string(),
            }),
        }
    }

    let qual_file = QualFile {
        path: path.to_path_buf(),
        subject,
        records,
    };
    Ok((qual_file, issues))
}

/// Parse records from a string (for testing or in-memory use).
pub fn parse_str(content: &str) -> crate::Result<Vec<Record>> {
    let mut records = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let record: Record = serde_json::from_str(trimmed)
            .map_err(|e| crate::Error::Validation(format!("line {}: {}", line_no + 1, e)))?;
        records.push(record);
    }
    Ok(records)
}

/// Append a record to a `.qual` file.
///
/// Creates the file if it doesn't exist. Always appends with a trailing newline.
pub fn append(path: &Path, record: &Record) -> crate::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let json = serde_json::to_string(record)?;
    writeln!(file, "{json}")?;
    Ok(())
}

/// Write a complete `.qual` file (used by compaction).
pub fn write_all(path: &Path, records: &[Record]) -> crate::Result<()> {
    let mut file = fs::File::create(path)?;
    for record in records {
        let json = serde_json::to_string(record)?;
        writeln!(file, "{json}")?;
    }
    Ok(())
}

/// Discover all `.qual` files under a root directory.
///
/// Walks the directory tree recursively, collecting every file whose name
/// ends with `.qual`. Respects `.gitignore` and `.qualignore` by default.
/// Pass `respect_ignore: false` to bypass all ignore rules. VCS metadata
/// directories (`.git`, `.hg`, `.jj`, `.pijul`, `_FOSSIL_`, `.svn`) are
/// always skipped; other hidden directories are walked.
///
/// Lines that are not valid records are skipped with a warning on stderr
/// naming `file:line`, so one bad line does not hide every other record.
/// Re-read a file with [`parse`] before rewriting it.
///
/// Returns them sorted by path for determinism.
pub fn discover(root: &Path, respect_ignore: bool) -> crate::Result<Vec<QualFile>> {
    use ignore::WalkBuilder;

    let mut builder = WalkBuilder::new(root);
    builder.hidden(false); // allow hidden files like .qual

    if respect_ignore {
        builder
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            // Apply .gitignore under every VCS, not only inside a git repo.
            .require_git(false)
            .add_custom_ignore_filename(".qualignore");
    } else {
        builder
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .ignore(false);
    }

    // Never descend into VCS metadata directories. Other hidden directories
    // (like `.github`) are walked and subject to the ignore rules above.
    builder.filter_entry(|entry| {
        !(entry.depth() > 0
            && entry.file_type().is_some_and(|ft| ft.is_dir())
            && VCS_MARKERS.iter().any(|m| entry.file_name() == *m))
    });

    let mut qual_files = Vec::new();
    for entry in builder.build() {
        let entry = entry.map_err(|e| crate::Error::Io(std::io::Error::other(e)))?;
        let path = entry.path();
        if path.is_file()
            && (path.extension().and_then(|e| e.to_str()) == Some("qual")
                || path.file_name().and_then(|f| f.to_str()) == Some(".qual"))
        {
            let (qual_file, issues) = parse_lenient(path)?;
            for issue in issues {
                eprintln!("warning: skipping {issue}");
            }
            qual_files.push(qual_file);
        }
    }
    qual_files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(qual_files)
}

/// Derive the subject name from a `.qual` file path.
///
/// - `src/parser.rs.qual` -> `src/parser.rs`
/// - `src/.qual` -> `src/`
pub(crate) fn subject_name(qual_path: &Path) -> String {
    let s = qual_path.to_string_lossy();
    if let Some(stripped) = s.strip_suffix(".qual") {
        if stripped.ends_with('/') || stripped.ends_with(std::path::MAIN_SEPARATOR) {
            stripped.to_string()
        } else if qual_path.file_name().map(|f| f.to_string_lossy()) == Some(".qual".into()) {
            // Directory-level: `src/.qual` -> `src/`
            qual_path
                .parent()
                .map(|p| format!("{}/", p.display()))
                .unwrap_or_default()
        } else {
            stripped.to_string()
        }
    } else {
        s.to_string()
    }
}

/// VCS metadata entries. Their presence marks a project root, and discovery
/// never descends into them.
const VCS_MARKERS: &[&str] = &[".git", ".hg", ".jj", ".pijul", "_FOSSIL_", ".svn"];

/// Find the project root by searching upward for VCS markers.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        for marker in VCS_MARKERS {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => return None,
        }
    }
}

/// Detect the VCS in use at a given root.
#[cfg_attr(not(feature = "cli"), allow(dead_code))]
pub(crate) fn detect_vcs(root: &Path) -> Option<&'static str> {
    if root.join(".git").exists() {
        Some("git")
    } else if root.join(".hg").exists() {
        Some("hg")
    } else if root.join(".jj").exists() {
        Some("jj")
    } else if root.join(".pijul").exists() {
        Some("pijul")
    } else if root.join("_FOSSIL_").exists() {
        Some("fossil")
    } else if root.join(".svn").exists() {
        Some("svn")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::{self, Annotation, AnnotationBody, Kind};
    use chrono::Utc;
    use std::fs;

    fn make_annotation(subject: &str, kind: Kind, summary: &str) -> Annotation {
        annotation::finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: subject.into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
                .into(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: summary.into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        })
    }

    fn make_record(subject: &str, kind: Kind, summary: &str) -> Record {
        Record::Annotation(Box::new(make_annotation(subject, kind, summary)))
    }

    #[test]
    fn test_subject_name_file() {
        let path = Path::new("src/parser.rs.qual");
        assert_eq!(subject_name(path), "src/parser.rs");
    }

    #[test]
    fn test_subject_name_directory() {
        let path = Path::new("src/.qual");
        assert_eq!(subject_name(path), "src/");
    }

    #[test]
    fn test_parse_and_append() {
        let dir = tempfile::tempdir().unwrap();
        let qual_path = dir.path().join("test.rs.qual");

        let r1 = make_record("test.rs", Kind::Praise, "Good tests");
        let r2 = make_record("test.rs", Kind::Concern, "Missing docs");

        append(&qual_path, &r1).unwrap();
        append(&qual_path, &r2).unwrap();

        let parsed = parse(&qual_path).unwrap();
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(
            parsed.records[0].as_annotation().unwrap().body.summary,
            "Good tests"
        );
        assert_eq!(
            parsed.records[1].as_annotation().unwrap().body.summary,
            "Missing docs"
        );
        assert_eq!(
            parsed.subject,
            qual_path.to_string_lossy().replace(".qual", "")
        );
    }

    #[test]
    fn test_parse_skips_comments_and_blanks() {
        let dir = tempfile::tempdir().unwrap();
        let qual_path = dir.path().join("test.rs.qual");

        let att = make_annotation("test.rs", Kind::Pass, "ok");
        let json = serde_json::to_string(&att).unwrap();

        fs::write(
            &qual_path,
            format!("// This is a comment\n\n{json}\n\n// Another comment\n"),
        )
        .unwrap();

        let parsed = parse(&qual_path).unwrap();
        assert_eq!(parsed.records.len(), 1);
    }

    #[test]
    fn test_discover() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        let r1 = make_record("src/a.rs", Kind::Pass, "ok");
        let r2 = make_record("src/b.rs", Kind::Fail, "bad");

        append(&src.join("a.rs.qual"), &r1).unwrap();
        append(&src.join("b.rs.qual"), &r2).unwrap();

        // Also create a non-qual file that should be ignored
        fs::write(src.join("a.rs"), "fn main() {}").unwrap();

        let found = discover(dir.path(), true).unwrap();
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_discover_skips_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join(".git");
        fs::create_dir_all(&hidden).unwrap();

        let r = make_record("x", Kind::Pass, "ok");
        append(&hidden.join("x.qual"), &r).unwrap();

        let found = discover(dir.path(), true).unwrap();
        assert_eq!(found.len(), 0);
    }

    #[test]
    fn test_discover_includes_non_vcs_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let workflows = dir.path().join(".github/workflows");
        fs::create_dir_all(&workflows).unwrap();

        let r = make_record(".github/workflows/rust.yml", Kind::Concern, "pin actions");
        append(&workflows.join(".qual"), &r).unwrap();

        for vcs in [".hg", ".jj", ".pijul", "_FOSSIL_", ".svn"] {
            let d = dir.path().join(vcs);
            fs::create_dir_all(&d).unwrap();
            append(&d.join("x.qual"), &make_record("x", Kind::Pass, "ok")).unwrap();
        }

        let found = discover(dir.path(), true).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, workflows.join(".qual"));
    }

    #[test]
    fn test_discover_respects_qualignore() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let examples = dir.path().join("examples");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&examples).unwrap();

        let r1 = make_record("src/a.rs", Kind::Pass, "ok");
        let r2 = make_record("examples/demo.rs", Kind::Pass, "ok");

        append(&src.join("a.rs.qual"), &r1).unwrap();
        append(&examples.join("demo.rs.qual"), &r2).unwrap();

        // Without .qualignore: both found
        let found = discover(dir.path(), true).unwrap();
        assert_eq!(found.len(), 2);

        // Add .qualignore excluding examples/
        fs::write(dir.path().join(".qualignore"), "examples/\n").unwrap();

        let found = discover(dir.path(), true).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].path.to_string_lossy().contains("src"));

        // With --no-ignore: both found again
        let found = discover(dir.path(), false).unwrap();
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_write_all() {
        let dir = tempfile::tempdir().unwrap();
        let qual_path = dir.path().join("test.rs.qual");

        let r1 = make_record("test.rs", Kind::Praise, "Good");
        let r2 = make_record("test.rs", Kind::Concern, "Bad");
        let id1 = r1.id().to_string();
        let id2 = r2.id().to_string();

        write_all(&qual_path, &[r1, r2]).unwrap();

        let parsed = parse(&qual_path).unwrap();
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(parsed.records[0].id(), id1);
        assert_eq!(parsed.records[1].id(), id2);
    }

    #[test]
    fn test_find_project_root() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        let sub = dir.path().join("src").join("deep");
        fs::create_dir_all(&sub).unwrap();

        let root = find_project_root(&sub).unwrap();
        assert_eq!(root, dir.path());
    }

    #[test]
    fn test_detect_vcs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_vcs(dir.path()), None);

        fs::create_dir_all(dir.path().join(".git")).unwrap();
        assert_eq!(detect_vcs(dir.path()), Some("git"));
    }
}
