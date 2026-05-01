use clap::Args as ClapArgs;

use super::review::{self, ReviewArgs};
use crate::annotation::Kind;

#[derive(ClapArgs)]
pub struct Args {
    #[command(flatten)]
    pub review: ReviewArgs,
}

pub fn run(args: Args) -> crate::Result<()> {
    review::run_review(args.review, Kind::Pass)
}
