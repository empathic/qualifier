use clap::Args as ClapArgs;
use std::path::Path;

use crate::cli::output;
use crate::qual_file::{self, find_project_root};
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// Artifacts to score (all if omitted)
    pub artifacts: Vec<String>,

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

    // Filter to requested artifacts, or show all
    let mut reports: Vec<(String, scoring::ScoreReport)> = if args.artifacts.is_empty() {
        scores.into_iter().collect()
    } else {
        scores
            .into_iter()
            .filter(|(k, _)| args.artifacts.contains(k))
            .collect()
    };

    // Sort by effective score ascending (worst first)
    reports.sort_by_key(|(_, r)| r.effective);

    if args.format == "json" {
        println!("{}", output::scores_json(&reports));
    } else if reports.is_empty() {
        println!("No qualified artifacts found.");
    } else {
        println!("{}", output::score_table(&reports));
    }

    Ok(())
}
