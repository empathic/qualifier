use crate::annotation::Record;

/// JSON output for a single artifact show.
pub fn show_json(subject: &str, records: &[Record]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "subject": subject,
        "records": records,
    }))
    .unwrap_or_default()
}
