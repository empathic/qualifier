use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

pub mod commands;
pub mod config;
pub mod output;
pub mod provenance;
pub mod span_context;
pub mod targets;

// Clap doesn't natively group subcommands into headed sections in the
// parent --help, so we render the Commands block ourselves via a custom
// help_template. If you add, rename, or remove a subcommand, update
// HELP_TEMPLATE to match — the Commands enum below is still the source
// of truth for parsing.
const HELP_TEMPLATE: &str = "\
{about-with-newline}
{usage-heading} {usage}

If you are an AI coding agent, run `qualifier agents` first — it covers
the conventions and pitfalls you need before recording any annotation.

Initialize:
  init       Bootstrap a project: VCS merge config and agent directives

For AI agents:
  agents     Read this before recording annotations. Self-contained agent guide.

Record observations:
  record     Record an annotation: `qualifier record <kind> <location> [message]`
  reply      Reply to an existing record (id-prefix or location)
  resolve    Resolve (close) an existing record (id-prefix or location)
  emit       Emit a raw record of any type

Inspect annotations:
  show       Show annotations for an artifact
  threads    List conversation threads across the project
  ls         List artifacts by kind
  praise     Show who annotated an artifact and why (alias: blame)
  review     Check freshness of annotations against current code
  diff       Show records added, resolved, or drifted since a git ref

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
    /// Bootstrap a project: VCS merge config and agent directives
    Init(commands::init::Args),

    /// Read this before recording annotations. Self-contained agent guide.
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
    /// List conversation threads across the project
    Threads(commands::threads::Args),
    /// List artifacts by kind
    Ls(commands::ls::Args),
    /// Show who annotated an artifact and why
    #[command(alias = "blame")]
    Praise(commands::praise::Args),
    /// Check freshness of annotations against current code
    Review(commands::freshness::Args),
    /// Show records added, resolved, or drifted since a git ref
    Diff(commands::diff::Args),

    /// Compact a .qual file
    Compact(commands::compact::Args),

    /// Print a random qualifier haiku
    Haiku,
}

pub fn run() {
    // Detect if the user typed "blame" so we can print a hint
    let used_blame_alias = std::env::args().nth(1).is_some_and(|arg| arg == "blame");

    // Load config before parsing: it supplies the `--format` default. A
    // load error is reported after parsing, so `--help` still works.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project_root = crate::qual_file::find_project_root(&cwd).unwrap_or(cwd);
    let loaded = config::load(Some(&project_root));
    let format_default = loaded.as_ref().map_or(output::Format::Human, |c| c.format);
    let cli = parse_with_format_default(format_default);

    if used_blame_alias {
        eprintln!(
            "hint: the command is \"praise\" \u{2014} qualifier tracks who helped, not who to blame"
        );
    }

    // A malformed .qualifier.toml, user config, or QUALIFIER_FORMAT fails
    // before the command runs.
    match loaded {
        Ok(cfg) => config::init(cfg),
        Err(e) => {
            eprintln!("qualifier: {e}");
            std::process::exit(1);
        }
    }

    let result: crate::Result<()> = match cli.command {
        Commands::Init(args) => commands::init::run(args),
        Commands::Agents(args) => commands::agents::run(args),
        Commands::Record(args) => commands::record::run(*args),
        Commands::Reply(args) => commands::reply::run(args),
        Commands::Resolve(args) => commands::resolve::run(args),
        Commands::Emit(args) => commands::emit::run(args),
        Commands::Show(args) => commands::show::run(args),
        Commands::Threads(args) => commands::threads::run(args),
        Commands::Ls(args) => commands::ls::run(args),
        Commands::Compact(args) => commands::compact::run(args),
        Commands::Haiku => {
            commands::haiku::run();
            Ok(())
        }
        Commands::Praise(args) => commands::praise::run(args),
        Commands::Review(args) => commands::freshness::run(args),
        Commands::Diff(args) => commands::diff::run(args),
    };

    match result {
        Ok(()) => {}
        Err(crate::Error::AlreadyReported(code)) => std::process::exit(code),
        Err(e) => {
            eprintln!("qualifier: {e}");
            std::process::exit(1);
        }
    }
}

/// Parse the command line with `format` as the default of every
/// subcommand's `--format` flag (flags still win).
fn parse_with_format_default(format: output::Format) -> Cli {
    let mut cmd = Cli::command();
    if format != output::Format::Human {
        let value = match format {
            output::Format::Human => "human",
            output::Format::Json => "json",
        };
        let names: Vec<String> = cmd
            .get_subcommands()
            .filter(|sc| sc.get_arguments().any(|a| a.get_id() == "format"))
            .map(|sc| sc.get_name().to_string())
            .collect();
        for name in names {
            cmd = cmd.mut_subcommand(name, |sc| sc.mut_arg("format", |a| a.default_value(value)));
        }
    }
    let matches = cmd.get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}
