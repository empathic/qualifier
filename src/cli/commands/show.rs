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
    let qual_path = format!("{}.qual", args.artifact);
    let qual_path = Path::new(&qual_path);

    if !qual_path.exists() {
        return Err(crate::Error::Validation(format!(
            "No .qual file found for '{}'",
            args.artifact
        )));
    }

    let qf = qual_file::parse(qual_path)?;

    // Load graph if available for effective score
    let root = find_project_root(Path::new("."));
    let graph = crate::cli::config::load_graph(args.graph.as_deref(), root.as_deref());
    let all_qual_files = root
        .as_deref()
        .map(qual_file::discover)
        .transpose()?
        .unwrap_or_default();

    let scores = scoring::effective_scores(&graph, &all_qual_files);
    let report = scores
        .get(&args.artifact)
        .cloned()
        .unwrap_or(scoring::ScoreReport {
            raw: scoring::raw_score(&qf.attestations),
            effective: scoring::raw_score(&qf.attestations),
            limiting_path: None,
        });

    if args.format == "json" {
        println!(
            "{}",
            output::show_json(&args.artifact, &report, &qf.attestations)
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

    let active = scoring::filter_superseded(&qf.attestations);
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
