use std::path::Path;

use crate::annotation::Span;

/// Compute a BLAKE3 hash of the full lines covered by a span.
///
/// Reads the file at `file_path`, extracts lines `start.line..=end.line`
/// (1-indexed), joins them with `\n`, and returns the hex BLAKE3 hash.
/// Returns `None` if the file is unreadable or the span extends beyond EOF.
pub fn compute_span_hash(file_path: &Path, span: &Span) -> Option<String> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let all_lines: Vec<&str> = content.lines().collect();

    let start = span.start.line as usize;
    let end = span.end_or_start().line as usize;

    if start == 0 || start > all_lines.len() || end > all_lines.len() {
        return None;
    }

    let selected: Vec<&str> = all_lines[start - 1..=end - 1].to_vec();
    let joined = selected.join("\n");
    Some(blake3::hash(joined.as_bytes()).to_hex().to_string())
}

/// Freshness status of an annotation's span against current file content.
#[derive(Debug, PartialEq, Eq)]
pub enum FreshnessStatus {
    /// The content hash matches — the code hasn't changed.
    Fresh,
    /// The content hash differs — the code has drifted.
    Drifted { expected: String, actual: String },
    /// The file or span lines are missing.
    Missing { reason: String },
    /// The span has no content_hash — freshness cannot be checked.
    NoHash,
}

/// Check whether the code under a span still matches its recorded content_hash.
pub fn check_freshness(file_path: &Path, span: &Span) -> FreshnessStatus {
    let expected = match &span.content_hash {
        Some(h) => h.clone(),
        None => return FreshnessStatus::NoHash,
    };

    match compute_span_hash(file_path, span) {
        Some(actual) => {
            if actual == expected {
                FreshnessStatus::Fresh
            } else {
                FreshnessStatus::Drifted { expected, actual }
            }
        }
        None => {
            if !file_path.exists() {
                FreshnessStatus::Missing {
                    reason: format!("file not found: {}", file_path.display()),
                }
            } else {
                FreshnessStatus::Missing {
                    reason: format!(
                        "span lines {}..={} beyond file length",
                        span.start.line,
                        span.end_or_start().line
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::{Position, Span};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_file(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
        f.flush().unwrap();
        f
    }

    fn span(start: u32, end: Option<u32>) -> Span {
        Span {
            start: Position {
                line: start,
                col: None,
            },
            end: end.map(|line| Position { line, col: None }),
            content_hash: None,
        }
    }

    #[test]
    fn test_compute_single_line() {
        let f = make_file(&["alpha", "beta", "gamma"]);
        let s = span(2, None);
        let hash = compute_span_hash(f.path(), &s).unwrap();

        // Should hash just "beta"
        let expected = blake3::hash(b"beta").to_hex().to_string();
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_compute_range() {
        let f = make_file(&["alpha", "beta", "gamma", "delta"]);
        let s = span(2, Some(3));
        let hash = compute_span_hash(f.path(), &s).unwrap();

        let expected = blake3::hash(b"beta\ngamma").to_hex().to_string();
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_compute_file_not_found() {
        let s = span(1, None);
        assert!(compute_span_hash(Path::new("/no/such/file.rs"), &s).is_none());
    }

    #[test]
    fn test_compute_beyond_eof() {
        let f = make_file(&["only one line"]);
        let s = span(5, None);
        assert!(compute_span_hash(f.path(), &s).is_none());
    }

    #[test]
    fn test_compute_end_beyond_eof() {
        let f = make_file(&["line1", "line2"]);
        let s = span(1, Some(10));
        assert!(compute_span_hash(f.path(), &s).is_none());
    }

    #[test]
    fn test_freshness_fresh() {
        let f = make_file(&["alpha", "beta", "gamma"]);
        let hash = compute_span_hash(f.path(), &span(2, None)).unwrap();
        let s = Span {
            start: Position {
                line: 2,
                col: None,
            },
            end: None,
            content_hash: Some(hash),
        };
        assert_eq!(check_freshness(f.path(), &s), FreshnessStatus::Fresh);
    }

    #[test]
    fn test_freshness_drifted() {
        let f = make_file(&["alpha", "beta", "gamma"]);
        let s = Span {
            start: Position {
                line: 2,
                col: None,
            },
            end: None,
            content_hash: Some("0000000000000000000000000000000000000000000000000000000000000000".into()),
        };
        match check_freshness(f.path(), &s) {
            FreshnessStatus::Drifted { expected, actual } => {
                assert_eq!(expected, "0000000000000000000000000000000000000000000000000000000000000000");
                assert_ne!(actual, expected);
            }
            other => panic!("expected Drifted, got {other:?}"),
        }
    }

    #[test]
    fn test_freshness_missing_file() {
        let s = Span {
            start: Position {
                line: 1,
                col: None,
            },
            end: None,
            content_hash: Some("abc".into()),
        };
        match check_freshness(Path::new("/no/such/file.rs"), &s) {
            FreshnessStatus::Missing { reason } => {
                assert!(reason.contains("file not found"));
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn test_freshness_missing_lines() {
        let f = make_file(&["only one line"]);
        let s = Span {
            start: Position {
                line: 5,
                col: None,
            },
            end: None,
            content_hash: Some("abc".into()),
        };
        match check_freshness(f.path(), &s) {
            FreshnessStatus::Missing { reason } => {
                assert!(reason.contains("beyond file length"));
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn test_freshness_no_hash() {
        let f = make_file(&["alpha"]);
        let s = span(1, None);
        assert_eq!(check_freshness(f.path(), &s), FreshnessStatus::NoHash);
    }
}
