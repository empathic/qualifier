use clap::{Parser, Subcommand};

pub mod commands;
pub mod output;

#[derive(Parser)]
#[command(name = "qualifier", about = "Deterministic quality attestations for software artifacts")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    Graph(commands::graph::Args),
    /// Initialize qualifier in a repository
    Init,
}

pub fn run() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Attest(args) => commands::attest::run(args),
        Commands::Show(args) => commands::show::run(args),
        Commands::Score(args) => commands::score::run(args),
        Commands::Ls(args) => commands::ls::run(args),
        Commands::Check(args) => commands::check::run(args),
        Commands::Compact(args) => commands::compact::run(args),
        Commands::Graph(args) => commands::graph::run(args),
        Commands::Init => commands::init::run(),
    };

    if let Err(e) = result {
        eprintln!("qualifier: {e}");
        std::process::exit(1);
    }
}
