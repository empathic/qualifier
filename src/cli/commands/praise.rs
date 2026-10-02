use std::collections::HashSet;

use clap::Args as ClapArgs;

use crate::cli::output::Format;

use crate::annotation::Record;
use crate::cli::targets;
use crate::compact::filter_superseded;
use crate::qual_file;
use crate::threads::{self, ThreadRenderer};

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to show attribution for, relative to the current directory
    pub artifact: String,

    /// Output format (human, json)
    #[arg(long, value_enum, default_value_t = Format::Human)]
    pub format: Format,

    /// Use VCS blame/annotate on the .qual file instead of record-based output
    #[cfg(not(target_os = "emscripten"))]
    #[arg(long)]
    pub vcs: bool,

    /// Disable .gitignore and .qualignore filtering
    #[arg(long)]
    pub no_ignore: bool,
}

/// Record-based praise output — works everywhere including emscripten.
pub fn run(args: Args) -> crate::Result<()> {
    #[cfg(not(target_os = "emscripten"))]
    if args.vcs {
        return run_vcs(&args.artifact);
    }

    run_records(args)
}

fn run_records(args: Args) -> crate::Result<()> {
    let subject = targets::Locator::from_cwd()?.subject(&args.artifact)?;
    let records: Vec<Record> = targets::discover_project(!args.no_ignore)?
        .into_iter()
        .flat_map(|qf| qf.records)
        .collect();

    // No records is an answer, not an error: JSON output carries empty
    // arrays, human output says so.
    if args.format != Format::Json && !records.iter().any(|r| r.subject() == subject) {
        println!("No records found for '{subject}'.");
        return Ok(());
    }

    // Annotations are attributed thread by thread: each thread's root, live
    // replies, and closing resolve. Live epochs on the subject follow.
    let thread_list = threads::threads_touching(&records, &subject, false);
    let live: HashSet<&str> = filter_superseded(&records)
        .into_iter()
        .map(|r| r.id())
        .collect();
    let epochs: Vec<&Record> = records
        .iter()
        .filter(|r| r.subject() == subject && r.as_epoch().is_some() && live.contains(r.id()))
        .collect();

    if args.format == Format::Json {
        let mut ids: HashSet<&str> = epochs.iter().map(|r| r.id()).collect();
        for t in &thread_list {
            ids.extend(t.live_records().map(|r| r.id()));
        }
        let entries: Vec<serde_json::Value> = records
            .iter()
            .filter(|r| ids.contains(r.id()))
            .filter_map(record_to_json)
            .collect();
        let output = serde_json::json!({
            "subject": subject,
            "records": entries,
            "threads": thread_list
                .iter()
                .map(threads::thread_summary_json)
                .collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let renderer = ThreadRenderer {
        all: false,
        expand_closed: false,
        attribution: true,
        continuation: Some(&detail_lines),
    };
    let open = thread_list.iter().filter(|t| t.open).count();
    let n = thread_list.len();
    println!();
    println!(
        "  {subject} \u{2014} {n} thread{} ({open} open)",
        if n == 1 { "" } else { "s" }
    );
    for t in &thread_list {
        println!();
        for line in renderer.render(t) {
            println!("    {line}");
        }
    }
    for epoch in epochs.iter().filter_map(|r| r.as_epoch()) {
        let date = epoch.created_at.format("%Y-%m-%d");
        let issuer_type = epoch
            .issuer_type
            .as_ref()
            .map(|t| format!(", {t}"))
            .unwrap_or_default();
        println!();
        println!(
            "    [{}] epoch  {:?}  ({}{issuer_type}, {date})",
            threads::short_id(&epoch.id),
            epoch.body.summary,
            threads::short_issuer(&epoch.issuer),
        );
    }
    println!();

    Ok(())
}

/// The suggested fix, else the detail, under a record line.
fn detail_lines(record: &Record) -> Vec<String> {
    let Some(att) = record.as_annotation() else {
        return Vec::new();
    };
    if let Some(ref fix) = att.body.suggested_fix {
        vec![format!("suggested fix: {fix:?}")]
    } else if let Some(ref detail) = att.body.detail {
        vec![format!("detail: {detail:?}")]
    } else {
        Vec::new()
    }
}

fn record_to_json(record: &Record) -> Option<serde_json::Value> {
    if let Some(att) = record.as_annotation() {
        let mut entry = serde_json::json!({
            "id": att.id,
            "kind": att.body.kind.to_string(),
            "summary": att.body.summary,
            "issuer": att.issuer,
            "created_at": att.created_at.to_rfc3339(),
        });
        if let Some(ref at) = att.issuer_type {
            entry["issuer_type"] = serde_json::json!(at.to_string());
        }
        if let Some(ref fix) = att.body.suggested_fix {
            entry["suggested_fix"] = serde_json::json!(fix);
        }
        if let Some(ref detail) = att.body.detail {
            entry["detail"] = serde_json::json!(detail);
        }
        if let Some(ref span) = att.body.span {
            entry["span"] = serde_json::to_value(span).unwrap_or_default();
        }
        Some(entry)
    } else if let Some(epoch) = record.as_epoch() {
        let mut entry = serde_json::json!({
            "id": epoch.id,
            "type": "epoch",
            "summary": epoch.body.summary,
            "issuer": epoch.issuer,
            "created_at": epoch.created_at.to_rfc3339(),
        });
        if let Some(ref at) = epoch.issuer_type {
            entry["issuer_type"] = serde_json::json!(at.to_string());
        }
        Some(entry)
    } else {
        None
    }
}

#[cfg(not(target_os = "emscripten"))]
fn run_vcs(artifact: &str) -> crate::Result<()> {
    use std::process::Command;

    let locator = targets::Locator::from_cwd()?;
    let subject = locator.subject(artifact)?;
    let qual_path = locator.existing_qual_file(&subject).ok_or_else(|| {
        crate::Error::Validation(format!(
            "No .qual file found containing annotations for '{subject}'"
        ))
    })?;

    let vcs = qual_file::detect_vcs(locator.root());

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
                "VCS blame is not supported for {vcs} \u{2014} \
                 run your VCS blame/annotate command directly on {}",
                qual_path.display()
            )));
        }
        None => {
            return Err(crate::Error::Validation(
                "No VCS detected \u{2014} --vcs requires git or hg".into(),
            ));
        }
    }

    Ok(())
}
