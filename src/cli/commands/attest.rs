use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to attest
    pub artifact: String,

    /// Attestation kind
    #[arg(long)]
    pub kind: String,

    /// Quality score delta (-100..=100)
    #[arg(long)]
    pub score: i32,

    /// One-line summary
    #[arg(long)]
    pub summary: Option<String>,

    /// Suggested fix
    #[arg(long)]
    pub suggested_fix: Option<String>,

    /// Classification tags (repeatable)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Author identity (defaults to VCS user)
    #[arg(long)]
    pub author: Option<String>,
}

pub fn run(_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
