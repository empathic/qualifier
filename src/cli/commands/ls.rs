use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Only show artifacts scoring below this threshold
    #[arg(long)]
    pub below: Option<i32>,

    /// Filter by attestation kind
    #[arg(long)]
    pub kind: Option<String>,

    /// Show only unqualified artifacts
    #[arg(long)]
    pub unqualified: bool,

    /// Output format
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub fn run(_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
