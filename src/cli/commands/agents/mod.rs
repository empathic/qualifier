use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Topic name (e.g. `record`, `concepts`, `workflows`). Omit to print the orientation page.
    pub topic: Option<String>,
}

const OVERVIEW: &str = include_str!("pages/_overview.md");

pub fn run(args: Args) -> crate::Result<()> {
    match args.topic.as_deref() {
        None => {
            print!("{OVERVIEW}");
            Ok(())
        }
        Some(name) => {
            eprintln!("qualifier agents: no such topic '{name}'. Available: (none yet)");
            std::process::exit(2);
        }
    }
}
