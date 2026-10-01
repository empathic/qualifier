use std::path::PathBuf;

use clap::Args as ClapArgs;

use crate::annotation::Kind;
use crate::cli::targets;
use crate::compact as compact_lib;
use crate::qual_file;
use crate::threads;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to compact, relative to the current directory
    /// (required unless --all). Only this artifact's records are
    /// compacted, in every .qual file that holds them.
    pub artifact: Option<String>,

    /// Compact all .qual files in the repo
    #[arg(long)]
    pub all: bool,

    /// Collapse each artifact's records to a single epoch record
    #[arg(long)]
    pub snapshot: bool,

    /// Snapshot even when it folds open blocker or concern threads
    #[arg(long, requires = "snapshot")]
    pub force: bool,

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

    let mut paths: Vec<PathBuf> = targets::discover_project(!args.no_ignore)?
        .into_iter()
        .filter(|qf| qf.records.iter().any(|r| r.subject() == subject))
        .map(|qf| qf.path)
        .collect();
    if paths.is_empty()
        && let Some(path) = locator.existing_qual_file(&subject)
    {
        paths.push(path);
    }
    if paths.is_empty() {
        return Err(crate::Error::Validation(format!(
            "No .qual file found containing annotations for '{subject}'"
        )));
    }

    let files = paths
        .iter()
        .map(|p| qual_file::parse(p))
        .collect::<crate::Result<Vec<_>>>()?;
    if args.snapshot && !args.force {
        refuse_folding_open_threads(&files, Some(&subject))?;
    }
    for qf in &files {
        compact_one(qf, Some(&subject), args.snapshot, args.dry_run)?;
    }
    Ok(())
}

fn run_all(args: &Args) -> crate::Result<()> {
    let discovered = targets::discover_project(!args.no_ignore)?;

    if discovered.is_empty() {
        println!("No .qual files found.");
        return Ok(());
    }

    // Rewrites re-read each file strictly, so a malformed line fails the
    // command instead of being dropped from the rewritten file.
    let files = discovered
        .iter()
        .map(|qf| qual_file::parse(&qf.path))
        .collect::<crate::Result<Vec<_>>>()?;
    if args.snapshot && !args.force {
        refuse_folding_open_threads(&files, None)?;
    }
    for qf in &files {
        compact_one(qf, None, args.snapshot, args.dry_run)?;
    }

    Ok(())
}

/// Fail when a snapshot would fold an open blocker or concern thread into
/// an epoch.
fn refuse_folding_open_threads(
    files: &[qual_file::QualFile],
    subject: Option<&str>,
) -> crate::Result<()> {
    let records: Vec<_> = files.iter().flat_map(|qf| qf.records.clone()).collect();
    let open: Vec<String> = threads::build_threads(&records)
        .into_iter()
        .filter(|t| t.open && subject.is_none_or(|s| t.root.subject() == s))
        .filter(|t| matches!(t.root.kind(), Some(Kind::Blocker | Kind::Concern)))
        .map(|t| {
            let summary = t
                .root
                .as_annotation()
                .map_or("", |a| a.body.summary.as_str());
            format!(
                "  [{}] {} {}: {}",
                &t.origin[..t.origin.len().min(8)],
                t.root.kind().map(|k| k.to_string()).unwrap_or_default(),
                t.root.subject(),
                summary
            )
        })
        .collect();
    if open.is_empty() {
        return Ok(());
    }
    Err(crate::Error::Validation(format!(
        "--snapshot would fold {} open blocker/concern thread(s) into an epoch:\n{}\n\
         Resolve them first, or pass --force to snapshot anyway.",
        open.len(),
        open.join("\n")
    )))
}

fn compact_one(
    qf: &qual_file::QualFile,
    subject: Option<&str>,
    snapshot: bool,
    dry_run: bool,
) -> crate::Result<()> {
    let (compacted, result) = match (snapshot, subject) {
        (true, Some(s)) => compact_lib::snapshot_subject(qf, s),
        (true, None) => compact_lib::snapshot(qf),
        (false, Some(s)) => compact_lib::prune_subject(qf, s),
        (false, None) => compact_lib::prune(qf),
    };

    if result.pruned == 0 && result.epochs == 0 {
        println!(
            "  {}: {} records, nothing to compact",
            qf.path.display(),
            result.before
        );
        return Ok(());
    }

    if snapshot {
        println!(
            "  {}: {} -> {} records ({} epoch{})",
            qf.path.display(),
            result.before,
            result.after,
            result.epochs,
            if result.epochs == 1 { "" } else { "s" },
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
