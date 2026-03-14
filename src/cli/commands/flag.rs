use clap::Args as ClapArgs;

use crate::attestation::Kind;
use super::review::{self, ReviewArgs};

#[derive(ClapArgs)]
pub struct Args {
    #[command(flatten)]
    pub review: ReviewArgs,
}

pub fn run(args: Args) -> crate::Result<()> {
    review::run_review(args.review, Kind::Concern)
}
