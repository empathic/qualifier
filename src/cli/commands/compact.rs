use clap::Args as ClapArgs;
use std::path::Path;

use crate::compact as compact_lib;
use crate::qual_file::{self, find_project_root};
use crate::scoring;

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

pub fn run(args: Args) -> crate::Result<()> {
    if args.all {
        return run_all(&args);
    }

    let artifact = args
        .artifact
        .as_deref()
        .ok_or_else(|| crate::Error::Validation("artifact is required (or use --all)".into()))?;

    let qual_path = qual_file::find_qual_file_for(artifact).ok_or_else(|| {
        crate::Error::Validation(format!(
            "No .qual file found containing attestations for '{artifact}'"
        ))
    })?;

    let qf = qual_file::parse(&qual_path)?;
    compact_one(&qf, args.snapshot, args.dry_run)?;

    Ok(())
}

fn run_all(args: &Args) -> crate::Result<()> {
    let root = find_project_root(Path::new("."));
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let qual_files = qual_file::discover(discover_root)?;

    if qual_files.is_empty() {
        println!("No .qual files found.");
        return Ok(());
    }

    for qf in &qual_files {
        compact_one(qf, args.snapshot, args.dry_run)?;
    }

    Ok(())
}

fn compact_one(qf: &qual_file::QualFile, snapshot: bool, dry_run: bool) -> crate::Result<()> {
    let score_before = scoring::raw_score(&qf.records);

    let (compacted, result) = if snapshot {
        compact_lib::snapshot(qf)
    } else {
        compact_lib::prune(qf)
    };

    // Verify the invariant
    let score_after = scoring::raw_score(&compacted.records);
    if score_before != score_after {
        return Err(crate::Error::Validation(format!(
            "BUG: compaction changed raw score from {} to {} for {}",
            score_before,
            score_after,
            qf.path.display()
        )));
    }

    if result.pruned == 0 {
        println!(
            "  {}: {} records, nothing to compact",
            qf.path.display(),
            result.before
        );
        return Ok(());
    }

    if snapshot {
        println!(
            "  {}: {} -> {} record (epoch, raw score: {})",
            qf.path.display(),
            result.before,
            result.after,
            score_after,
        );
    } else {
        println!(
            "  {}: {} -> {} records ({} superseded, pruned)",
            qf.path.display(),
            result.before,
            result.after,
            result.pruned,
        );
    }

    if !dry_run {
        qual_file::write_all(&qf.path, &compacted.records)?;
    } else {
        println!("  (dry run — no changes written)");
    }

    Ok(())
}
