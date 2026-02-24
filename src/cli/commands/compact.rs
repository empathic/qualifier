use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to compact (required unless --all)
    pub artifact: Option<String>,

    /// Compact all .qual files in the repo
    #[arg(long)]
    pub all: bool,

    /// Collapse to a single epoch attestation
    #[arg(long)]
    pub snapshot: bool,

    /// Preview without writing
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
