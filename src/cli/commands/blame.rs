use clap::Args as ClapArgs;
use std::path::Path;
use std::process::Command;

use crate::qual_file::detect_vcs;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to blame
    pub artifact: String,
}

pub fn run(args: Args) -> crate::Result<()> {
    let qual_path = format!("{}.qual", args.artifact);
    let qual_path = Path::new(&qual_path);

    if !qual_path.exists() {
        return Err(crate::Error::Validation(format!(
            "No .qual file found for '{}'",
            args.artifact
        )));
    }

    let vcs = detect_vcs(Path::new("."));

    match vcs {
        Some("git") => {
            let status = Command::new("git")
                .args(["blame", &qual_path.to_string_lossy()])
                .status()?;
            if !status.success() {
                return Err(crate::Error::Validation("git blame failed".into()));
            }
        }
        Some("hg") => {
            let status = Command::new("hg")
                .args(["annotate", &qual_path.to_string_lossy()])
                .status()?;
            if !status.success() {
                return Err(crate::Error::Validation("hg annotate failed".into()));
            }
        }
        Some(vcs) => {
            return Err(crate::Error::Validation(format!(
                "qualifier blame is not supported for {vcs} — \
                 run your VCS blame/annotate command directly on {}",
                qual_path.display()
            )));
        }
        None => {
            return Err(crate::Error::Validation(
                "No VCS detected — qualifier blame requires git or hg".into(),
            ));
        }
    }

    Ok(())
}
