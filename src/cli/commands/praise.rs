use clap::Args as ClapArgs;
use std::path::Path;

use crate::cli::output;
use crate::qual_file::{self, find_project_root};
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to show attribution for
    pub artifact: String,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Use VCS blame/annotate on the .qual file instead of record-based output
    #[cfg(not(target_os = "emscripten"))]
    #[arg(long)]
    pub vcs: bool,
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
    let root = find_project_root(Path::new("."));
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let all_qual_files = qual_file::discover(discover_root)?;

    let records: Vec<&crate::attestation::Record> =
        qual_file::find_records_for(&args.artifact, &all_qual_files);

    if records.is_empty() {
        return Err(crate::Error::Validation(format!(
            "No records found for '{}'",
            args.artifact
        )));
    }

    let owned: Vec<crate::attestation::Record> = records.iter().map(|r| (*r).clone()).collect();
    let active = scoring::filter_superseded(&owned);

    if args.format == "json" {
        let entries: Vec<serde_json::Value> =
            active.iter().filter_map(|r| record_to_json(r)).collect();
        let output = serde_json::json!({
            "subject": args.artifact,
            "records": entries,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_default()
        );
        return Ok(());
    }

    // Human output
    println!();
    println!("  {} \u{2014} {} records", args.artifact, active.len());
    println!();

    for record in &active {
        if let Some(att) = record.as_attestation() {
            let date = att.created_at.format("%Y-%m-%d");
            let id_short = if att.id.len() >= 8 {
                format!("{}\u{2026}", &att.id[..8])
            } else {
                att.id.clone()
            };

            // Line 1: score + kind + summary
            println!(
                "    {} {:<10} {:?}",
                output::format_score(att.body.score),
                att.body.kind.to_string(),
                att.body.summary,
            );

            // Line 2: author + date + truncated ID + (author_type)
            let author_type_suffix = match &att.body.author_type {
                Some(at) if *at != crate::attestation::AuthorType::Human => {
                    format!("  ({})", at)
                }
                _ => String::new(),
            };
            println!(
                "          {}  {}  {}{}",
                att.author, date, id_short, author_type_suffix,
            );

            // Line 3 (optional): suggested_fix, detail, or span
            if let Some(ref fix) = att.body.suggested_fix {
                println!("          suggested fix: {:?}", fix);
            } else if let Some(ref detail) = att.body.detail {
                println!("          detail: {:?}", detail);
            }
            if let Some(ref span) = att.body.span {
                let end_str = match &span.end {
                    Some(end) => format!(":{}", format_position(end)),
                    None => String::new(),
                };
                println!(
                    "          span: {}{}",
                    format_position(&span.start),
                    end_str,
                );
            }

            println!();
        } else if let Some(epoch) = record.as_epoch() {
            let date = epoch.created_at.format("%Y-%m-%d");
            let id_short = if epoch.id.len() >= 8 {
                format!("{}\u{2026}", &epoch.id[..8])
            } else {
                epoch.id.clone()
            };
            println!(
                "    {} {:<10} {:?}",
                output::format_score(epoch.body.score),
                "epoch",
                epoch.body.summary,
            );
            let author_type_suffix = match &epoch.body.author_type {
                Some(at) if *at != crate::attestation::AuthorType::Human => {
                    format!("  ({})", at)
                }
                _ => String::new(),
            };
            println!(
                "          {}  {}  {}{}",
                epoch.author, date, id_short, author_type_suffix,
            );
            println!();
        }
    }

    Ok(())
}

fn format_position(pos: &crate::attestation::Position) -> String {
    match pos.col {
        Some(col) => format!("{}.{}", pos.line, col),
        None => format!("{}", pos.line),
    }
}

fn record_to_json(record: &crate::attestation::Record) -> Option<serde_json::Value> {
    if let Some(att) = record.as_attestation() {
        let mut entry = serde_json::json!({
            "id": att.id,
            "kind": att.body.kind.to_string(),
            "score": att.body.score,
            "summary": att.body.summary,
            "author": att.author,
            "created_at": att.created_at.to_rfc3339(),
        });
        if let Some(ref at) = att.body.author_type {
            entry["author_type"] = serde_json::json!(at.to_string());
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
            "score": epoch.body.score,
            "summary": epoch.body.summary,
            "author": epoch.author,
            "created_at": epoch.created_at.to_rfc3339(),
        });
        if let Some(ref at) = epoch.body.author_type {
            entry["author_type"] = serde_json::json!(at.to_string());
        }
        Some(entry)
    } else {
        None
    }
}

#[cfg(not(target_os = "emscripten"))]
fn run_vcs(artifact: &str) -> crate::Result<()> {
    use std::process::Command;

    let qual_path = qual_file::find_qual_file_for(artifact).ok_or_else(|| {
        crate::Error::Validation(format!(
            "No .qual file found containing attestations for '{}'",
            artifact
        ))
    })?;

    let vcs = qual_file::detect_vcs(Path::new("."));

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
