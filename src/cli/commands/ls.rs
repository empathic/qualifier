use clap::Args as ClapArgs;
use std::collections::HashSet;
use std::path::Path;

use crate::cli::output;
use crate::qual_file::{self, find_project_root};
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// Only show artifacts scoring below this threshold
    #[arg(long)]
    pub below: Option<i32>,

    /// Filter by attestation kind
    #[arg(long)]
    pub kind: Option<String>,

    /// Show only unqualified artifacts (no attestations)
    #[arg(long)]
    pub unqualified: bool,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Path to the dependency graph file
    #[arg(long)]
    pub graph: Option<String>,
}

pub fn run(args: Args) -> crate::Result<()> {
    let root = find_project_root(Path::new("."));
    let graph = crate::cli::config::load_graph(args.graph.as_deref(), root.as_deref());
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let qual_files = qual_file::discover(discover_root)?;

    let scores = scoring::effective_scores(&graph, &qual_files);

    // Build an index of subjects that have records
    let attested: HashSet<String> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter().map(|r| r.subject().to_string()))
        .collect();

    let mut reports: Vec<(String, scoring::ScoreReport)> = scores
        .into_iter()
        .filter(|(subject, report)| {
            if args.unqualified && attested.contains(subject) {
                return false;
            }

            if let Some(threshold) = args.below
                && report.effective >= threshold
            {
                return false;
            }

            if let Some(ref kind_filter) = args.kind {
                let kind_match = qual_files.iter().any(|qf| {
                    qf.records.iter().any(|r| {
                        r.subject() == *subject
                            && r.kind().map(|k| k.to_string()).as_deref() == Some(kind_filter)
                    })
                });
                if !kind_match {
                    return false;
                }
            }

            true
        })
        .collect();

    reports.sort_by_key(|(_, r)| r.effective);

    if args.format == "json" {
        println!("{}", output::scores_json(&reports));
    } else if reports.is_empty() {
        println!("No matching artifacts found.");
    } else {
        println!("{}", output::score_table(&reports));
    }

    Ok(())
}
