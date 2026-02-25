use clap::Args as ClapArgs;
use std::path::Path;

use crate::qual_file::{self, find_project_root};
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// Minimum acceptable effective score (default: 0)
    #[arg(long, default_value = "0", allow_hyphen_values = true)]
    pub min_score: i32,

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

    let mut failures: Vec<(String, scoring::ScoreReport)> = scores
        .into_iter()
        .filter(|(_, report)| report.effective < args.min_score)
        .collect();

    failures.sort_by_key(|(_, r)| r.effective);

    if failures.is_empty() {
        println!("All artifacts meet minimum score of {}", args.min_score);
        Ok(())
    } else {
        for (artifact, report) in &failures {
            let detail = if let Some(ref path) = report.limiting_path {
                format!(" (limited by {})", path.join(" -> "))
            } else {
                String::new()
            };
            eprintln!(
                "FAIL: {} effective={} raw={}{}",
                artifact, report.effective, report.raw, detail
            );
        }
        Err(crate::Error::CheckFailed(format!(
            "{} artifact(s) below minimum score of {}",
            failures.len(),
            args.min_score
        )))
    }
}
