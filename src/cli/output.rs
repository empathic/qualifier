use comfy_table::{Cell, CellAlignment, Color, Table};

use crate::attestation::Record;
use crate::scoring::{self, ScoreReport};

/// Format a score for human display: `[+40]` or `[-30]` or `[  0]`.
pub fn format_score(score: i32) -> String {
    if score > 0 {
        format!("[+{}]", score)
    } else if score < 0 {
        format!("[{}]", score)
    } else {
        "[  0]".into()
    }
}

/// Pick a color based on the effective score.
pub fn score_color(score: i32) -> Color {
    if score >= 60 {
        Color::Green
    } else if score >= 0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Build a comfy-table for the `qualifier score` output.
pub fn score_table(reports: &[(String, ScoreReport)]) -> Table {
    let mut table = Table::new();
    table.set_header(vec!["ARTIFACT", "RAW", "EFF", "", "STATUS"]);

    for (artifact, report) in reports {
        let status = scoring::score_status(report);
        let bar = scoring::score_bar(report.effective, 10);

        let status_text = if let Some(ref path) = report.limiting_path {
            format!("limited by {}", path.join(" -> "))
        } else {
            status.to_string()
        };

        let color = score_color(report.effective);

        table.add_row(vec![
            Cell::new(artifact),
            Cell::new(report.raw).set_alignment(CellAlignment::Right),
            Cell::new(report.effective)
                .set_alignment(CellAlignment::Right)
                .fg(color),
            Cell::new(&bar),
            Cell::new(&status_text).fg(color),
        ]);
    }

    table
}

/// JSON output for machine consumption — scores.
pub fn scores_json(reports: &[(String, ScoreReport)]) -> String {
    let entries: Vec<serde_json::Value> = reports
        .iter()
        .map(|(artifact, report)| {
            serde_json::json!({
                "artifact": artifact,
                "raw_score": report.raw,
                "effective_score": report.effective,
                "status": scoring::score_status(report),
                "limiting_path": report.limiting_path,
            })
        })
        .collect();

    serde_json::to_string_pretty(&entries).unwrap_or_default()
}

/// JSON output for a single artifact show.
pub fn show_json(artifact: &str, report: &ScoreReport, records: &[Record]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "artifact": artifact,
        "raw_score": report.raw,
        "effective_score": report.effective,
        "limiting_path": report.limiting_path,
        "records": records,
    }))
    .unwrap_or_default()
}
