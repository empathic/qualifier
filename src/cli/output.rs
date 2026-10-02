use serde::{Deserialize, Serialize};

use crate::annotation::Record;

/// Output format for `--format` on every command that has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// Text for people.
    Human,
    /// JSON for programs.
    Json,
}

/// JSON output for a single artifact show.
pub fn show_json(subject: &str, records: &[Record]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "subject": subject,
        "records": records,
    }))
    .unwrap_or_default()
}
