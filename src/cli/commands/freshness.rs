use std::path::Path;

use clap::Args as ClapArgs;

use crate::content_hash::{self, FreshnessStatus};
use crate::qual_file;
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// Only check annotations for this subject
    pub subject: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Ignore .gitignore and .qualignore rules
    #[arg(long)]
    pub no_ignore: bool,
}

struct CheckResult {
    subject: String,
    span_display: String,
    kind: String,
    summary: String,
    status: FreshnessStatus,
}

pub fn run(args: Args) -> crate::Result<()> {
    let root = Path::new(".");
    let qual_files = qual_file::discover(root, !args.no_ignore)?;

    if qual_files.is_empty() {
        if args.format == "json" {
            println!("[]");
        } else {
            println!("No .qual files found.");
        }
        return Ok(());
    }

    // Collect all active annotations with spans that have content_hash
    let all_records: Vec<_> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter())
        .collect();

    let all_owned: Vec<_> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter().cloned())
        .collect();
    let active = scoring::filter_superseded(&all_owned);
    let active_ids: std::collections::HashSet<&str> =
        active.iter().map(|r| r.id()).collect();

    let mut results = Vec::new();

    for record in &all_records {
        // Only active annotations with spans
        if !active_ids.contains(record.id()) {
            continue;
        }

        if let Some(subject_filter) = &args.subject
            && record.subject() != subject_filter
        {
            continue;
        }

        let att = match record.as_annotation() {
            Some(a) => a,
            None => continue,
        };

        let span = match &att.body.span {
            Some(s) => s,
            None => continue,
        };

        if span.content_hash.is_none() {
            continue;
        }

        let span_display = {
            let end = match &span.end {
                Some(e) if e.line != span.start.line => format!(":{}", e.line),
                _ => String::new(),
            };
            format!("{}:{}{}", att.subject, span.start.line, end)
        };

        let status = content_hash::check_freshness(Path::new(&att.subject), span);

        results.push(CheckResult {
            subject: att.subject.clone(),
            span_display,
            kind: att.body.kind.to_string(),
            summary: att.body.summary.clone(),
            status,
        });
    }

    if args.format == "json" {
        print_json(&results);
    } else {
        print_human(&results);
    }

    Ok(())
}

fn print_human(results: &[CheckResult]) {
    if results.is_empty() {
        println!("No annotations with content hashes found.");
        return;
    }

    let mut fresh = 0u32;
    let mut drifted = 0u32;
    let mut missing = 0u32;

    for r in results {
        let (label, _subject_display) = match &r.status {
            FreshnessStatus::Fresh => {
                fresh += 1;
                ("  FRESH  ", &r.span_display)
            }
            FreshnessStatus::Drifted { .. } => {
                drifted += 1;
                ("  DRIFTED", &r.span_display)
            }
            FreshnessStatus::Missing { .. } => {
                missing += 1;
                ("  MISSING", &r.span_display)
            }
            FreshnessStatus::NoHash => unreachable!(),
        };
        println!(
            "{}  {:<30} {:<12} {:?}",
            label, r.span_display, r.kind, r.summary
        );
    }

    let total = results.len();
    println!();
    println!(
        "{total} annotations checked: {fresh} fresh, {drifted} drifted, {missing} missing"
    );
}

fn print_json(results: &[CheckResult]) {
    let entries: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            let (status, detail) = match &r.status {
                FreshnessStatus::Fresh => ("fresh", serde_json::Value::Null),
                FreshnessStatus::Drifted { expected, actual } => (
                    "drifted",
                    serde_json::json!({
                        "expected": expected,
                        "actual": actual,
                    }),
                ),
                FreshnessStatus::Missing { reason } => {
                    ("missing", serde_json::json!({ "reason": reason }))
                }
                FreshnessStatus::NoHash => ("no_hash", serde_json::Value::Null),
            };

            serde_json::json!({
                "subject": r.subject,
                "location": r.span_display,
                "kind": r.kind,
                "summary": r.summary,
                "status": status,
                "detail": detail,
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&entries).unwrap_or_default()
    );
}
