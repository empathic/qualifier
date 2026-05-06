use clap::{Parser, Subcommand};

pub mod commands;
pub mod config;
pub mod output;
pub mod span_context;

// Clap doesn't natively group subcommands into headed sections in the
// parent --help, so we render the Commands block ourselves via a custom
// help_template. If you add, rename, or remove a subcommand, update
// HELP_TEMPLATE to match — the Commands enum below is still the source
// of truth for parsing.
const HELP_TEMPLATE: &str = "\
{about-with-newline}
{usage-heading} {usage}

Record observations:
  record     Record an annotation: `qualifier record <kind> <location> [message]`
  reply      Reply to an existing record (id-prefix or location)
  resolve    Resolve (close) an existing record (id-prefix or location)
  emit       Emit a raw record of any type

Inspect annotations:
  show       Show annotations for an artifact
  ls         List artifacts by kind
  praise     Show who annotated an artifact and why (alias: blame)
  review     Check freshness of annotations against current code

Maintain:
  compact    Compact a .qual file

Other:
  haiku      Print a random qualifier haiku
  help       Print this message or the help of the given subcommand(s)

Run `qualifier <COMMAND> --help` for command-specific options.

Options:
{options}
";

#[derive(Parser)]
#[command(
    name = "qualifier",
    version,
    about = "Deterministic quality annotations for software artifacts",
    help_template = HELP_TEMPLATE
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Self-contained guide for AI coding agents (start here)
    Agents(commands::agents::Args),

    /// Record an annotation: `qualifier record <kind> <location> [message]`
    Record(Box<commands::record::Args>),
    /// Reply to an existing record (id-prefix or location)
    Reply(commands::reply::Args),
    /// Resolve (close) an existing record (id-prefix or location)
    Resolve(commands::resolve::Args),
    /// Emit a raw record of any type: `qualifier emit <type> <subject> --body '<JSON>'`
    Emit(commands::emit::Args),

    /// Show annotations for an artifact
    Show(commands::show::Args),
    /// List artifacts by kind
    Ls(commands::ls::Args),
    /// Show who annotated an artifact and why
    #[command(alias = "blame")]
    Praise(commands::praise::Args),
    /// Check freshness of annotations against current code
    Review(commands::freshness::Args),

    /// Compact a .qual file
    Compact(commands::compact::Args),

    /// Print a random qualifier haiku
    Haiku,
}

pub fn run() {
    // Detect if the user typed "blame" so we can print a hint
    let used_blame_alias = std::env::args().nth(1).is_some_and(|arg| arg == "blame");

    let cli = Cli::parse();

    if used_blame_alias {
        eprintln!(
            "hint: the command is \"praise\" \u{2014} qualifier tracks who helped, not who to blame"
        );
    }

    let result: crate::Result<()> = match cli.command {
        Commands::Agents(args) => commands::agents::run(args),
        Commands::Record(args) => commands::record::run(*args),
        Commands::Reply(args) => commands::reply::run(args),
        Commands::Resolve(args) => commands::resolve::run(args),
        Commands::Emit(args) => commands::emit::run(args),
        Commands::Show(args) => commands::show::run(args),
        Commands::Ls(args) => commands::ls::run(args),
        Commands::Compact(args) => commands::compact::run(args),
        Commands::Haiku => {
            commands::haiku::run();
            Ok(())
        }
        Commands::Praise(args) => commands::praise::run(args),
        Commands::Review(args) => commands::freshness::run(args),
    };

    if let Err(e) = result {
        eprintln!("qualifier: {e}");
        std::process::exit(1);
    }
}
