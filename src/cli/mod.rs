use clap::{Parser, Subcommand};

pub mod commands;
pub mod config;
pub mod output;

#[derive(Parser)]
#[command(
    name = "qualifier",
    version,
    about = "Deterministic quality attestations for software artifacts"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add an attestation to an artifact
    Attest(commands::attest::Args),
    /// Show attestations and scores for an artifact
    Show(commands::show::Args),
    /// Compute and display scores
    Score(commands::score::Args),
    /// List artifacts by score or kind
    Ls(commands::ls::Args),
    /// CI gate: exit non-zero if below threshold
    Check(commands::check::Args),
    /// Compact a .qual file
    Compact(commands::compact::Args),
    /// Visualize the dependency graph
    Graph(commands::graph_cmd::Args),
    /// Initialize qualifier in a repository
    Init,
    /// Show per-line VCS attribution for a .qual file
    Blame(commands::blame::Args),
}

pub fn run() {
    let cli = Cli::parse();

    let result: crate::Result<()> = match cli.command {
        Commands::Attest(args) => commands::attest::run(args),
        Commands::Show(args) => commands::show::run(args),
        Commands::Score(args) => commands::score::run(args),
        Commands::Ls(args) => commands::ls::run(args),
        Commands::Check(args) => commands::check::run(args),
        Commands::Compact(args) => commands::compact::run(args),
        Commands::Graph(args) => commands::graph_cmd::run(args),
        Commands::Init => commands::init::run(),
        Commands::Blame(args) => commands::blame::run(args),
    };

    if let Err(e) = result {
        match &e {
            crate::Error::CheckFailed(msg) => {
                eprintln!("\n{msg}");
                std::process::exit(1);
            }
            _ => {
                eprintln!("qualifier: {e}");
                std::process::exit(1);
            }
        }
    }
}
