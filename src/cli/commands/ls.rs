use clap::Args as ClapArgs;
use std::collections::BTreeMap;
use std::path::Path;

use crate::qual_file::{self, find_project_root};

#[derive(ClapArgs)]
pub struct Args {
    /// Filter by annotation kind
    #[arg(long)]
    pub kind: Option<String>,

    /// Show only unannotated artifacts (no annotations)
    #[arg(long)]
    pub unqualified: bool,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Disable .gitignore and .qualignore filtering
    #[arg(long)]
    pub no_ignore: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    let root = find_project_root(Path::new("."));
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let qual_files = qual_file::discover(discover_root, !args.no_ignore)?;

    // Group records by subject and count by kind.
    let mut by_subject: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for qf in &qual_files {
        for record in &qf.records {
            let kind = record
                .kind()
                .map(|k| k.to_string())
                .unwrap_or_else(|| record.record_type().to_string());
            by_subject
                .entry(record.subject().to_string())
                .or_default()
                .push(kind);
        }
    }

    let rows: Vec<(String, Vec<String>)> = if args.unqualified {
        // Without per-subject discovery we can't fully list "what doesn't exist";
        // approximate by listing nothing. The flag stays as a placeholder.
        Vec::new()
    } else {
        by_subject
            .into_iter()
            .filter(|(_, kinds)| {
                if let Some(ref kind_filter) = args.kind {
                    return kinds.iter().any(|k| k == kind_filter);
                }
                true
            })
            .collect()
    };

    if args.format == "json" {
        let entries: Vec<serde_json::Value> = rows
            .iter()
            .map(|(subject, kinds)| {
                serde_json::json!({
                    "subject": subject,
                    "annotation_count": kinds.len(),
                    "kinds": kinds,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
    } else if rows.is_empty() {
        if args.unqualified {
            println!("(unqualified listing requires a project file index — not implemented)");
        } else {
            println!("No matching artifacts found.");
        }
    } else {
        for (subject, kinds) in &rows {
            println!("  {}  ({} annotations)", subject, kinds.len());
        }
    }

    Ok(())
}
