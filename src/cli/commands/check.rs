use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Minimum acceptable effective score
    #[arg(long, default_value = "0")]
    pub min_score: i32,
}

pub fn run(_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
