use std::fmt;
use std::path::Path;

use crate::annotation::Span;

/// Why a span's content hash could not be computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanHashError {
    /// The file does not exist.
    NotFound,
    /// The file exists but could not be read.
    Io(String),
    /// The file is not valid UTF-8.
    NotUtf8,
    /// The span starts at line 0 or reaches past the last line.
    OutOfRange { start: u32, end: u32, lines: usize },
    /// The span ends before it starts.
    Reversed { start: u32, end: u32 },
}

impl fmt::Display for SpanHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpanHashError::NotFound => write!(f, "file not found"),
            SpanHashError::Io(msg) => write!(f, "cannot read file: {msg}"),
            SpanHashError::NotUtf8 => write!(f, "file is not valid UTF-8"),
            SpanHashError::OutOfRange { start, end, lines } => write!(
                f,
                "span lines {start}..={end} beyond file length ({lines} lines)"
            ),
            SpanHashError::Reversed { start, end } => {
                write!(f, "span end (line {end}) precedes start (line {start})")
            }
        }
    }
}

impl std::error::Error for SpanHashError {}

/// Compute a BLAKE3 hash of the full lines covered by a span.
///
/// Reads the file at `file_path`, extracts lines `start.line..=end.line`
/// (1-indexed), joins them with `\n`, and returns the hex BLAKE3 hash.
pub fn compute_span_hash(file_path: &Path, span: &Span) -> Result<String, SpanHashError> {
    let start = span.start.line;
    let end = span.end_or_start().line;
    if end < start {
        return Err(SpanHashError::Reversed { start, end });
    }

    let bytes = std::fs::read(file_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => SpanHashError::NotFound,
        _ => SpanHashError::Io(e.to_string()),
    })?;
    let content = String::from_utf8(bytes).map_err(|_| SpanHashError::NotUtf8)?;
    let all_lines: Vec<&str> = content.lines().collect();

    let (first, last) = (start as usize, end as usize);
    if first == 0 || last > all_lines.len() {
        return Err(SpanHashError::OutOfRange {
            start,
            end,
            lines: all_lines.len(),
        });
    }

    let joined = all_lines[first - 1..=last - 1].join("\n");
    Ok(blake3::hash(joined.as_bytes()).to_hex().to_string())
}

/// Freshness status of an annotation's span against current file content.
#[derive(Debug, PartialEq, Eq)]
pub enum FreshnessStatus {
    /// The content hash matches — the code hasn't changed.
    Fresh,
    /// The content hash differs — the code has drifted.
    Drifted { expected: String, actual: String },
    /// The span's lines cannot be hashed; `reason` says why (missing or
    /// unreadable file, non-UTF-8 content, lines out of range, reversed
    /// span).
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
        Ok(actual) if actual == expected => FreshnessStatus::Fresh,
        Ok(actual) => FreshnessStatus::Drifted { expected, actual },
        Err(SpanHashError::NotFound) => FreshnessStatus::Missing {
            reason: format!("file not found: {}", file_path.display()),
        },
        Err(e @ (SpanHashError::Io(_) | SpanHashError::NotUtf8)) => FreshnessStatus::Missing {
            reason: format!("{}: {e}", file_path.display()),
        },
        Err(e) => FreshnessStatus::Missing {
            reason: e.to_string(),
        },
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
        assert_eq!(
            compute_span_hash(Path::new("/no/such/file.rs"), &s),
            Err(SpanHashError::NotFound)
        );
    }

    #[test]
    fn test_compute_beyond_eof() {
        let f = make_file(&["only one line"]);
        let s = span(5, None);
        assert!(matches!(
            compute_span_hash(f.path(), &s),
            Err(SpanHashError::OutOfRange { lines: 1, .. })
        ));
    }

    #[test]
    fn test_compute_end_beyond_eof() {
        let f = make_file(&["line1", "line2"]);
        let s = span(1, Some(10));
        assert!(matches!(
            compute_span_hash(f.path(), &s),
            Err(SpanHashError::OutOfRange { .. })
        ));
    }

    #[test]
    fn test_freshness_fresh() {
        let f = make_file(&["alpha", "beta", "gamma"]);
        let hash = compute_span_hash(f.path(), &span(2, None)).unwrap();
        let s = Span {
            start: Position { line: 2, col: None },
            end: None,
            content_hash: Some(hash),
        };
        assert_eq!(check_freshness(f.path(), &s), FreshnessStatus::Fresh);
    }

    #[test]
    fn test_freshness_drifted() {
        let f = make_file(&["alpha", "beta", "gamma"]);
        let s = Span {
            start: Position { line: 2, col: None },
            end: None,
            content_hash: Some(
                "0000000000000000000000000000000000000000000000000000000000000000".into(),
            ),
        };
        match check_freshness(f.path(), &s) {
            FreshnessStatus::Drifted { expected, actual } => {
                assert_eq!(
                    expected,
                    "0000000000000000000000000000000000000000000000000000000000000000"
                );
                assert_ne!(actual, expected);
            }
            other => panic!("expected Drifted, got {other:?}"),
        }
    }

    #[test]
    fn test_freshness_missing_file() {
        let s = Span {
            start: Position { line: 1, col: None },
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
            start: Position { line: 5, col: None },
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
    fn test_compute_reversed_span_is_none() {
        let f = make_file(&["1", "2", "3", "4", "5", "6"]);
        assert_eq!(
            compute_span_hash(f.path(), &span(5, Some(2))),
            Err(SpanHashError::Reversed { start: 5, end: 2 })
        );
    }

    #[test]
    fn test_freshness_reversed_span() {
        let f = make_file(&["1", "2", "3", "4", "5", "6"]);
        let mut s = span(5, Some(2));
        s.content_hash = Some("abc".into());
        match check_freshness(f.path(), &s) {
            FreshnessStatus::Missing { reason } => {
                assert!(reason.contains("precedes start"), "{reason}");
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn test_freshness_non_utf8_file() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"one\n\xe9\nthree\n").unwrap();
        f.flush().unwrap();
        assert_eq!(
            compute_span_hash(f.path(), &span(2, None)),
            Err(SpanHashError::NotUtf8)
        );
        let mut s = span(2, None);
        s.content_hash = Some("abc".into());
        match check_freshness(f.path(), &s) {
            FreshnessStatus::Missing { reason } => {
                assert!(reason.contains("not valid UTF-8"), "{reason}");
                assert!(!reason.contains("beyond file length"), "{reason}");
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn test_compute_unreadable_path_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        // Reading a directory fails with an error other than NotFound.
        assert!(matches!(
            compute_span_hash(dir.path(), &span(1, None)),
            Err(SpanHashError::Io(_))
        ));
    }

    #[test]
    fn test_freshness_no_hash() {
        let f = make_file(&["alpha"]);
        let s = span(1, None);
        assert_eq!(check_freshness(f.path(), &s), FreshnessStatus::NoHash);
    }
}
