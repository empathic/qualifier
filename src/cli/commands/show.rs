use clap::Args as ClapArgs;
use std::path::Path;

use crate::cli::output;
use crate::qual_file::{self, find_project_root};
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to show
    pub artifact: String,

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
    let all_qual_files = qual_file::discover(discover_root)?;

    let attestations = qual_file::find_attestations_for(&args.artifact, &all_qual_files);

    if attestations.is_empty() {
        return Err(crate::Error::Validation(format!(
            "No attestations found for '{}'",
            args.artifact
        )));
    }

    let scores = scoring::effective_scores(&graph, &all_qual_files);
    let owned_atts: Vec<crate::attestation::Attestation> =
        attestations.iter().map(|a| (*a).clone()).collect();
    let report = scores
        .get(&args.artifact)
        .cloned()
        .unwrap_or(scoring::ScoreReport {
            raw: scoring::raw_score(&owned_atts),
            effective: scoring::raw_score(&owned_atts),
            limiting_path: None,
        });

    if args.format == "json" {
        println!(
            "{}",
            output::show_json(&args.artifact, &report, &owned_atts)
        );
        return Ok(());
    }

    // Human output
    println!();
    println!("  {}", args.artifact);
    println!("  Raw score:       {}", report.raw);
    if let Some(ref path) = report.limiting_path {
        println!(
            "  Effective score: {} (limited by {})",
            report.effective,
            path.join(" -> ")
        );
    } else {
        println!("  Effective score: {}", report.effective);
    }

    let active = scoring::filter_superseded(&owned_atts);
    println!();
    println!("  Attestations ({}):", active.len());
    for att in &active {
        let date = att.created_at.format("%Y-%m-%d");
        let author_short = att.author.split('@').next().unwrap_or(&att.author);
        println!(
            "    {} {}  {:?}  {}  {}",
            output::format_score(att.score),
            att.kind,
            att.summary,
            author_short,
            date,
        );
    }
    println!();

    Ok(())
}
