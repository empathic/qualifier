use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Output format (dot, json)
    #[arg(long, default_value = "dot")]
    pub format: String,
}

pub fn run(_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
