use clap::Args as ClapArgs;
use std::path::Path;

use crate::graph;
use crate::qual_file::find_project_root;

#[derive(ClapArgs)]
pub struct Args {
    /// Output format (dot, json)
    #[arg(long, default_value = "dot")]
    pub format: String,

    /// Path to the dependency graph file
    #[arg(long)]
    pub graph: Option<String>,
}

pub fn run(args: Args) -> crate::Result<()> {
    let graph_path = if let Some(ref path) = args.graph {
        std::path::PathBuf::from(path)
    } else {
        let root =
            find_project_root(Path::new(".")).unwrap_or_else(|| std::path::PathBuf::from("."));
        root.join("qualifier.graph.jsonl")
    };

    if !graph_path.exists() {
        return Err(crate::Error::Validation(format!(
            "Graph file not found: {} (run `qualifier init` first)",
            graph_path.display()
        )));
    }

    let g = graph::load(&graph_path)?;

    match args.format.as_str() {
        "dot" => print!("{}", g.to_dot()),
        "json" => print!("{}", graph::to_jsonl(&g)),
        other => {
            return Err(crate::Error::Validation(format!(
                "Unknown format: {other} (expected dot or json)"
            )));
        }
    }

    Ok(())
}
