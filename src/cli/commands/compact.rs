use clap::Args as ClapArgs;

use crate::cli::targets;
use crate::compact as compact_lib;
use crate::qual_file;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to compact, relative to the current directory
    /// (required unless --all)
    pub artifact: Option<String>,

    /// Compact all .qual files in the repo
    #[arg(long)]
    pub all: bool,

    /// Collapse to a single epoch annotation
    #[arg(long)]
    pub snapshot: bool,

    /// Preview without writing
    #[arg(long)]
    pub dry_run: bool,

    /// Disable .gitignore and .qualignore filtering
    #[arg(long)]
    pub no_ignore: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    if args.all {
        return run_all(&args);
    }

    let artifact = args
        .artifact
        .as_deref()
        .ok_or_else(|| crate::Error::Validation("artifact is required (or use --all)".into()))?;

    let locator = targets::Locator::from_cwd()?;
    let subject = locator.subject(artifact)?;
    let qual_path = locator.existing_qual_file(&subject).ok_or_else(|| {
        crate::Error::Validation(format!(
            "No .qual file found containing annotations for '{subject}'"
        ))
    })?;

    let qf = qual_file::parse(&qual_path)?;
    compact_one(&qf, args.snapshot, args.dry_run)?;

    Ok(())
}

fn run_all(args: &Args) -> crate::Result<()> {
    let qual_files = targets::discover_project(!args.no_ignore)?;

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
    let (compacted, result) = if snapshot {
        compact_lib::snapshot(qf)
    } else {
        compact_lib::prune(qf)
    };

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
            "  {}: {} -> {} record (epoch)",
            qf.path.display(),
            result.before,
            result.after,
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
