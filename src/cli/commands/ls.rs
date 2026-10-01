use clap::Args as ClapArgs;
use std::collections::BTreeMap;

use crate::annotation::{Kind, Record};
use crate::cli::targets;
use crate::compact::filter_superseded;

#[derive(ClapArgs)]
pub struct Args {
    /// Filter by annotation kind
    #[arg(long)]
    pub kind: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Disable .gitignore and .qualignore filtering
    #[arg(long)]
    pub no_ignore: bool,
}

/// One listed subject: the kinds of its live records, and how many of them
/// match `--kind` (all of them without the flag).
struct Row {
    subject: String,
    kinds: Vec<String>,
    count: usize,
}

pub fn run(args: Args) -> crate::Result<()> {
    let qual_files = targets::discover_project(!args.no_ignore)?;
    let records: Vec<Record> = qual_files.into_iter().flat_map(|qf| qf.records).collect();

    // Group live records by subject. Superseded records and `resolve`
    // records (which close a thread rather than describe the artifact) are
    // left out of both the kinds and the count.
    let mut by_subject: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for record in filter_superseded(&records) {
        if record.kind() == Some(&Kind::Resolve) {
            continue;
        }
        let kind = record
            .kind()
            .map(|k| k.to_string())
            .unwrap_or_else(|| record.record_type().to_string());
        by_subject
            .entry(record.subject().to_string())
            .or_default()
            .push(kind);
    }

    let rows: Vec<Row> = by_subject
        .into_iter()
        .map(|(subject, kinds)| {
            let count = match args.kind.as_deref() {
                Some(k) => kinds.iter().filter(|kind| *kind == k).count(),
                None => kinds.len(),
            };
            Row {
                subject,
                kinds,
                count,
            }
        })
        .filter(|row| row.count > 0)
        .collect();

    if args.format == "json" {
        let entries: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "subject": row.subject,
                    "annotation_count": row.count,
                    "kinds": row.kinds,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if rows.is_empty() {
        println!("No matching artifacts found.");
    } else {
        for row in &rows {
            let noun = if row.count == 1 {
                "annotation"
            } else {
                "annotations"
            };
            println!("  {}  ({} {noun})", row.subject, row.count);
        }
    }

    Ok(())
}
