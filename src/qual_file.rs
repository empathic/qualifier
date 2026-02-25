use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::attestation::Attestation;

/// A parsed `.qual` file.
#[derive(Debug, Clone)]
pub struct QualFile {
    /// Path to the `.qual` file on disk.
    pub path: PathBuf,
    /// The artifact this file describes (path minus `.qual` suffix).
    pub artifact: String,
    /// Attestations in file order (oldest first).
    pub attestations: Vec<Attestation>,
}

/// Parse a `.qual` file from disk.
///
/// Skips empty lines and lines starting with `//` (comments).
/// Each non-comment line must be a valid JSON attestation.
pub fn parse(path: &Path) -> crate::Result<QualFile> {
    let content = fs::read_to_string(path)?;
    let artifact = artifact_name(path);
    let mut attestations = Vec::new();

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let att: Attestation = serde_json::from_str(trimmed).map_err(|e| {
            crate::Error::Validation(format!("{}:{}: {}", path.display(), line_no + 1, e))
        })?;
        attestations.push(att);
    }

    Ok(QualFile {
        path: path.to_path_buf(),
        artifact,
        attestations,
    })
}

/// Parse attestations from a string (for testing or in-memory use).
pub fn parse_str(content: &str) -> crate::Result<Vec<Attestation>> {
    let mut attestations = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let att: Attestation = serde_json::from_str(trimmed)
            .map_err(|e| crate::Error::Validation(format!("line {}: {}", line_no + 1, e)))?;
        attestations.push(att);
    }
    Ok(attestations)
}

/// Append an attestation to a `.qual` file.
///
/// Creates the file if it doesn't exist. Always appends with a trailing newline.
pub fn append(path: &Path, attestation: &Attestation) -> crate::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let json = serde_json::to_string(attestation)?;
    writeln!(file, "{json}")?;
    Ok(())
}

/// Write a complete `.qual` file (used by compaction).
pub fn write_all(path: &Path, attestations: &[Attestation]) -> crate::Result<()> {
    let mut file = fs::File::create(path)?;
    for att in attestations {
        let json = serde_json::to_string(att)?;
        writeln!(file, "{json}")?;
    }
    Ok(())
}

/// Resolve which `.qual` file should receive an attestation for the given artifact.
///
/// Resolution order:
/// 1. If `explicit_path` is provided, use it unconditionally (`--file` override).
/// 2. If `{artifact}.qual` exists, use it (backwards compat with 1:1 layout).
/// 3. Otherwise, use `{parent_dir}/.qual` (recommended directory-level layout).
///
/// Creates parent directories if needed.
pub fn resolve_qual_path(artifact: &str, explicit_path: Option<&Path>) -> crate::Result<PathBuf> {
    if let Some(p) = explicit_path {
        if let Some(parent) = p.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            fs::create_dir_all(parent)?;
        }
        return Ok(p.to_path_buf());
    }

    // 1. Check for existing 1:1 file
    let one_to_one = PathBuf::from(format!("{artifact}.qual"));
    if one_to_one.exists() {
        return Ok(one_to_one);
    }

    // 2. Default to directory-level .qual
    let artifact_path = Path::new(artifact);
    let parent = artifact_path.parent().unwrap_or(Path::new("."));
    let dir_qual = if parent.as_os_str().is_empty() {
        PathBuf::from(".qual")
    } else {
        parent.join(".qual")
    };

    // Create parent directories if needed
    if let Some(dir) = dir_qual.parent()
        && !dir.as_os_str().is_empty()
        && !dir.exists()
    {
        fs::create_dir_all(dir)?;
    }

    Ok(dir_qual)
}

/// Find all attestations for a given artifact across all discovered `.qual` files.
///
/// Searches by the `artifact` field in the JSON attestations, not by
/// the file path of the `.qual` file. This supports both 1:1 and directory-level
/// layouts transparently.
pub fn find_attestations_for<'a>(
    artifact: &str,
    qual_files: &'a [QualFile],
) -> Vec<&'a Attestation> {
    qual_files
        .iter()
        .flat_map(|qf| qf.attestations.iter())
        .filter(|att| att.artifact == artifact)
        .collect()
}

/// Find which `.qual` file on disk contains attestations for a given artifact.
///
/// Checks for a 1:1 file first (`{artifact}.qual`), then the directory-level
/// file (`{parent}/.qual`). Returns `None` if neither exists.
pub fn find_qual_file_for(artifact: &str) -> Option<PathBuf> {
    let one_to_one = PathBuf::from(format!("{artifact}.qual"));
    if one_to_one.exists() {
        return Some(one_to_one);
    }

    let artifact_path = Path::new(artifact);
    let parent = artifact_path.parent().unwrap_or(Path::new("."));
    let dir_qual = if parent.as_os_str().is_empty() {
        PathBuf::from(".qual")
    } else {
        parent.join(".qual")
    };
    if dir_qual.exists() {
        return Some(dir_qual);
    }

    None
}

/// Discover all `.qual` files under a root directory.
///
/// Walks the directory tree recursively, collecting every file whose name
/// ends with `.qual`. Returns them sorted by path for determinism.
pub fn discover(root: &Path) -> crate::Result<Vec<QualFile>> {
    let mut qual_files = Vec::new();
    walk_dir(root, &mut qual_files)?;
    qual_files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(qual_files)
}

fn walk_dir(dir: &Path, out: &mut Vec<QualFile>) -> crate::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip hidden directories (like .git)
        if path.is_dir() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            walk_dir(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("qual")
            || entry.file_name() == ".qual"
        {
            out.push(parse(&path)?);
        }
    }

    Ok(())
}

/// Derive the artifact name from a `.qual` file path.
///
/// - `src/parser.rs.qual` -> `src/parser.rs`
/// - `src/.qual` -> `src/`
pub fn artifact_name(qual_path: &Path) -> String {
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

/// Find the project root by searching upward for VCS markers or qualifier.graph.jsonl.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    const VCS_MARKERS: &[&str] = &[".git", ".hg", ".jj", ".pijul", "_FOSSIL_", ".svn"];
    const QUALIFIER_MARKER: &str = "qualifier.graph.jsonl";

    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        // Check for qualifier marker first
        if current.join(QUALIFIER_MARKER).exists() {
            return Some(current);
        }
        // Then VCS markers
        for marker in VCS_MARKERS {
            if current.join(marker).exists() {
                return Some(current);
            }
        }
        // Move up
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => return None,
        }
    }
}

/// Detect the VCS in use at a given root.
pub fn detect_vcs(root: &Path) -> Option<&'static str> {
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
    use crate::attestation::{self, Kind};
    use chrono::Utc;
    use std::fs;

    fn make_attestation(artifact: &str, kind: Kind, score: i32, summary: &str) -> Attestation {
        attestation::finalize(Attestation {
            v: 2,
            artifact: artifact.into(),
            kind,
            score,
            summary: summary.into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            r#ref: None,
            supersedes: None,
            epoch_refs: None,
            id: String::new(),
        })
    }

    #[test]
    fn test_artifact_name_file() {
        let path = Path::new("src/parser.rs.qual");
        assert_eq!(artifact_name(path), "src/parser.rs");
    }

    #[test]
    fn test_artifact_name_directory() {
        let path = Path::new("src/.qual");
        assert_eq!(artifact_name(path), "src/");
    }

    #[test]
    fn test_parse_and_append() {
        let dir = tempfile::tempdir().unwrap();
        let qual_path = dir.path().join("test.rs.qual");

        let att1 = make_attestation("test.rs", Kind::Praise, 40, "Good tests");
        let att2 = make_attestation("test.rs", Kind::Concern, -20, "Missing docs");

        append(&qual_path, &att1).unwrap();
        append(&qual_path, &att2).unwrap();

        let parsed = parse(&qual_path).unwrap();
        assert_eq!(parsed.attestations.len(), 2);
        assert_eq!(parsed.attestations[0].summary, "Good tests");
        assert_eq!(parsed.attestations[1].summary, "Missing docs");
        assert_eq!(
            parsed.artifact,
            qual_path.to_string_lossy().replace(".qual", "")
        );
    }

    #[test]
    fn test_parse_skips_comments_and_blanks() {
        let dir = tempfile::tempdir().unwrap();
        let qual_path = dir.path().join("test.rs.qual");

        let att = make_attestation("test.rs", Kind::Pass, 10, "ok");
        let json = serde_json::to_string(&att).unwrap();

        fs::write(
            &qual_path,
            format!("// This is a comment\n\n{json}\n\n// Another comment\n"),
        )
        .unwrap();

        let parsed = parse(&qual_path).unwrap();
        assert_eq!(parsed.attestations.len(), 1);
    }

    #[test]
    fn test_discover() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        let att1 = make_attestation("src/a.rs", Kind::Pass, 10, "ok");
        let att2 = make_attestation("src/b.rs", Kind::Fail, -10, "bad");

        append(&src.join("a.rs.qual"), &att1).unwrap();
        append(&src.join("b.rs.qual"), &att2).unwrap();

        // Also create a non-qual file that should be ignored
        fs::write(src.join("a.rs"), "fn main() {}").unwrap();

        let found = discover(dir.path()).unwrap();
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_discover_skips_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join(".git");
        fs::create_dir_all(&hidden).unwrap();

        let att = make_attestation("x", Kind::Pass, 10, "ok");
        append(&hidden.join("x.qual"), &att).unwrap();

        let found = discover(dir.path()).unwrap();
        assert_eq!(found.len(), 0);
    }

    #[test]
    fn test_write_all() {
        let dir = tempfile::tempdir().unwrap();
        let qual_path = dir.path().join("test.rs.qual");

        let att1 = make_attestation("test.rs", Kind::Praise, 40, "Good");
        let att2 = make_attestation("test.rs", Kind::Concern, -20, "Bad");

        write_all(&qual_path, &[att1.clone(), att2.clone()]).unwrap();

        let parsed = parse(&qual_path).unwrap();
        assert_eq!(parsed.attestations.len(), 2);
        assert_eq!(parsed.attestations[0].id, att1.id);
        assert_eq!(parsed.attestations[1].id, att2.id);
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

    #[test]
    fn test_resolve_qual_path_prefers_existing_1to1() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("foo.rs.qual"), "").unwrap();

        let artifact = dir.path().join("src/foo.rs");
        let path = resolve_qual_path(artifact.to_str().unwrap(), None).unwrap();
        assert_eq!(path, PathBuf::from(format!("{}.qual", artifact.display())));
    }

    #[test]
    fn test_resolve_qual_path_defaults_to_dir_qual() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        // No existing 1:1 file → should resolve to directory .qual
        let artifact = dir.path().join("src/foo.rs");
        let path = resolve_qual_path(artifact.to_str().unwrap(), None).unwrap();
        assert_eq!(path, src.join(".qual"));
    }

    #[test]
    fn test_resolve_qual_path_root_level_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("README.md");
        let path = resolve_qual_path(artifact.to_str().unwrap(), None).unwrap();
        assert_eq!(path, dir.path().join(".qual"));
    }

    #[test]
    fn test_resolve_qual_path_explicit_override() {
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("custom.qual");
        let artifact = dir.path().join("src/foo.rs");
        let path = resolve_qual_path(artifact.to_str().unwrap(), Some(&custom)).unwrap();
        assert_eq!(path, custom);
    }

    #[test]
    fn test_resolve_qual_path_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("src/deep");

        // src/deep/ doesn't exist yet
        let artifact = dir.path().join("src/deep/module.rs");
        let path = resolve_qual_path(artifact.to_str().unwrap(), None).unwrap();
        assert_eq!(path, deep.join(".qual"));
        assert!(deep.exists(), "parent dir should be created");
    }

    #[test]
    fn test_find_attestations_for_across_files() {
        let att_a1 = make_attestation("src/a.rs", Kind::Praise, 40, "good");
        let att_a2 = make_attestation("src/a.rs", Kind::Concern, -10, "meh");
        let att_b = make_attestation("src/b.rs", Kind::Pass, 20, "ok");

        let qfs = vec![
            QualFile {
                path: PathBuf::from("src/.qual"),
                artifact: "src/".into(),
                attestations: vec![att_a1.clone(), att_b.clone()],
            },
            QualFile {
                path: PathBuf::from("src/a.rs.qual"),
                artifact: "src/a.rs".into(),
                attestations: vec![att_a2.clone()],
            },
        ];

        let found = find_attestations_for("src/a.rs", &qfs);
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|a| a.id == att_a1.id));
        assert!(found.iter().any(|a| a.id == att_a2.id));

        let found_b = find_attestations_for("src/b.rs", &qfs);
        assert_eq!(found_b.len(), 1);
        assert_eq!(found_b[0].id, att_b.id);

        let found_none = find_attestations_for("src/c.rs", &qfs);
        assert!(found_none.is_empty());
    }

    #[test]
    fn test_find_qual_file_for_1to1() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("foo.rs.qual"), "").unwrap();

        let artifact = format!("{}/foo.rs", src.display());
        let found = find_qual_file_for(&artifact);
        assert_eq!(found, Some(PathBuf::from(format!("{artifact}.qual"))));
    }

    #[test]
    fn test_find_qual_file_for_dir_qual() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join(".qual"), "").unwrap();

        let artifact = format!("{}/foo.rs", src.display());
        let found = find_qual_file_for(&artifact);
        assert_eq!(found, Some(src.join(".qual")));
    }

    #[test]
    fn test_find_qual_file_for_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = format!("{}/foo.rs", dir.path().join("src").display());
        let found = find_qual_file_for(&artifact);
        assert_eq!(found, None);
    }
}
