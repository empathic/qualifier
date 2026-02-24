use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to show
    pub artifact: String,

    /// Output format
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub fn run(_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
