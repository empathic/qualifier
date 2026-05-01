use std::path::Path;

use crate::annotation::Span;

/// Number of context lines to show before/after the span.
pub const DEFAULT_CONTEXT_LINES: u32 = 3;

/// A single line of source context.
#[derive(Debug, Clone)]
pub struct ContextLine {
    pub line_number: u32,
    pub text: String,
    pub in_span: bool,
}

/// Source context around a span, with optional warning.
#[derive(Debug, Clone)]
pub struct SpanContext {
    pub path: String,
    pub lines: Vec<ContextLine>,
    pub warning: Option<String>,
}

/// Read source lines around a span. Never errors — captures warnings instead.
pub fn read_span_context(file_path: &Path, span: &Span, context: u32) -> SpanContext {
    let path_str = file_path.display().to_string();

    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            return SpanContext {
                path: path_str.clone(),
                lines: vec![],
                warning: Some(format!("could not read file: {path_str}")),
            };
        }
    };

    let all_lines: Vec<&str> = content.lines().collect();
    let total = all_lines.len() as u32;

    let start_line = span.start.line;
    let end_line = span.end_or_start().line;

    if start_line == 0 || start_line > total {
        return SpanContext {
            path: path_str,
            lines: vec![],
            warning: Some(format!(
                "span start line {start_line} is beyond file length ({total} lines)"
            )),
        };
    }

    let window_start = start_line.saturating_sub(context).max(1);
    let window_end = end_line.saturating_add(context).min(total);

    let mut lines = Vec::new();
    for line_num in window_start..=window_end {
        let idx = (line_num - 1) as usize;
        let text = all_lines.get(idx).unwrap_or(&"").to_string();
        let in_span = line_num >= start_line && line_num <= end_line;
        lines.push(ContextLine {
            line_number: line_num,
            text,
            in_span,
        });
    }

    let warning = if end_line > total {
        Some(format!(
            "span end line {end_line} is beyond file length ({total} lines)"
        ))
    } else {
        None
    };

    SpanContext {
        path: path_str,
        lines,
        warning,
    }
}

/// Format source context in compiler-diagnostic style.
///
/// ```text
///   src/parser.rs:
///     39 |     fn parse(&self, input: &str) -> Result<Ast> {
///     40 |         let tokens = tokenize(input);
///   > 42 |             panic!("empty input");
///     43 |         }
/// ```
pub fn format_human(ctx: &SpanContext) -> String {
    if ctx.lines.is_empty() {
        return String::new();
    }

    let max_line_num = ctx.lines.last().map(|l| l.line_number).unwrap_or(0);
    let gutter_width = max_line_num.to_string().len();

    let mut out = String::new();
    out.push_str(&format!("  {}:\n", ctx.path));

    for line in &ctx.lines {
        let marker = if line.in_span { ">" } else { " " };
        out.push_str(&format!(
            "  {} {:>gutter_width$} | {}\n",
            marker,
            line.line_number,
            line.text,
            gutter_width = gutter_width,
        ));
    }

    out
}

/// Serialize source context as a JSON value.
pub fn to_json(ctx: &SpanContext) -> serde_json::Value {
    let lines: Vec<serde_json::Value> = ctx
        .lines
        .iter()
        .map(|l| {
            serde_json::json!({
                "line": l.line_number,
                "text": l.text,
                "in_span": l.in_span,
            })
        })
        .collect();

    serde_json::json!({
        "path": ctx.path,
        "lines": lines,
        "warning": ctx.warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::{Position, Span};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn sample_file() -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for i in 1..=20 {
            writeln!(f, "line {i} content").unwrap();
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
    fn test_basic_read() {
        let f = sample_file();
        let s = span(10, None);
        let ctx = read_span_context(f.path(), &s, 2);

        assert!(ctx.warning.is_none());
        assert_eq!(ctx.lines.len(), 5); // lines 8..=12
        assert_eq!(ctx.lines[0].line_number, 8);
        assert!(!ctx.lines[0].in_span);
        assert_eq!(ctx.lines[2].line_number, 10);
        assert!(ctx.lines[2].in_span);
        assert!(!ctx.lines[4].in_span);
    }

    #[test]
    fn test_range_span() {
        let f = sample_file();
        let s = span(5, Some(8));
        let ctx = read_span_context(f.path(), &s, 1);

        assert!(ctx.warning.is_none());
        // lines 4..=9
        assert_eq!(ctx.lines.len(), 6);
        assert!(!ctx.lines[0].in_span); // line 4
        assert!(ctx.lines[1].in_span); // line 5
        assert!(ctx.lines[4].in_span); // line 8
        assert!(!ctx.lines[5].in_span); // line 9
    }

    #[test]
    fn test_file_not_found() {
        let s = span(1, None);
        let ctx = read_span_context(Path::new("/nonexistent/file.rs"), &s, 3);

        assert!(ctx.lines.is_empty());
        assert!(ctx.warning.is_some());
        assert!(ctx.warning.unwrap().contains("could not read"));
    }

    #[test]
    fn test_beyond_eof() {
        let f = sample_file(); // 20 lines
        let s = span(999, None);
        let ctx = read_span_context(f.path(), &s, 3);

        assert!(ctx.lines.is_empty());
        assert!(ctx.warning.is_some());
        assert!(ctx.warning.unwrap().contains("beyond file length"));
    }

    #[test]
    fn test_end_beyond_eof() {
        let f = sample_file(); // 20 lines
        let s = span(18, Some(25));
        let ctx = read_span_context(f.path(), &s, 1);

        // Should still show lines 17..=20, with a warning
        assert!(!ctx.lines.is_empty());
        assert!(ctx.warning.is_some());
        assert!(ctx.warning.unwrap().contains("beyond file length"));
    }

    #[test]
    fn test_file_start_edge() {
        let f = sample_file();
        let s = span(1, None);
        let ctx = read_span_context(f.path(), &s, 3);

        assert_eq!(ctx.lines[0].line_number, 1);
        assert!(ctx.lines[0].in_span);
        // context after: lines 2..=4
        assert_eq!(ctx.lines.len(), 4);
    }

    #[test]
    fn test_file_end_edge() {
        let f = sample_file(); // 20 lines
        let s = span(20, None);
        let ctx = read_span_context(f.path(), &s, 3);

        assert_eq!(ctx.lines.last().unwrap().line_number, 20);
        assert!(ctx.lines.last().unwrap().in_span);
        // context before: lines 17..=19
        assert_eq!(ctx.lines.len(), 4);
    }

    #[test]
    fn test_format_human_output() {
        let f = sample_file();
        let s = span(5, None);
        let ctx = read_span_context(f.path(), &s, 1);
        let output = format_human(&ctx);

        assert!(output.contains("> "));
        assert!(output.contains("line 5 content"));
        // Non-span lines should not have >
        assert!(output.contains("  4 |"));
        assert!(output.contains("  6 |"));
    }

    #[test]
    fn test_format_human_empty() {
        let ctx = SpanContext {
            path: "foo.rs".into(),
            lines: vec![],
            warning: Some("nope".into()),
        };
        assert_eq!(format_human(&ctx), "");
    }

    #[test]
    fn test_to_json_structure() {
        let f = sample_file();
        let s = span(3, None);
        let ctx = read_span_context(f.path(), &s, 1);
        let json = to_json(&ctx);

        assert!(json["lines"].is_array());
        let lines = json["lines"].as_array().unwrap();
        assert!(!lines.is_empty());

        let span_line = lines.iter().find(|l| l["in_span"] == true).unwrap();
        assert_eq!(span_line["line"], 3);
        assert!(json["warning"].is_null());
    }

    #[test]
    fn test_to_json_with_warning() {
        let s = span(1, None);
        let ctx = read_span_context(Path::new("/no/such/file.rs"), &s, 1);
        let json = to_json(&ctx);

        assert!(json["warning"].is_string());
    }
}
