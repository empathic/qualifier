use clap::Args as ClapArgs;
use std::path::Path;

use crate::attestation::Kind;
use crate::cli::output;
use crate::cli::span_context;
use crate::qual_file::{self, find_project_root};
use crate::scoring;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to show
    pub artifact: String,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Path to the dependency graph file
    #[arg(long)]
    pub graph: Option<String>,

    /// Disable .gitignore and .qualignore filtering
    #[arg(long)]
    pub no_ignore: bool,

    /// Show source context around spans (compiler-diagnostic style)
    #[arg(long)]
    pub pretty: bool,

    /// Show all records, including resolve tombstones
    #[arg(long)]
    pub all: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    let root = find_project_root(Path::new("."));
    let graph = crate::cli::config::load_graph(args.graph.as_deref(), root.as_deref());
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let all_qual_files = qual_file::discover(discover_root, !args.no_ignore)?;

    let records = qual_file::find_records_for(&args.artifact, &all_qual_files);

    if records.is_empty() {
        return Err(crate::Error::Validation(format!(
            "No records found for '{}'",
            args.artifact
        )));
    }

    let scores = scoring::effective_scores(&graph, &all_qual_files);
    let owned_records: Vec<crate::attestation::Record> =
        records.iter().map(|r| (*r).clone()).collect();
    let report = scores
        .get(&args.artifact)
        .cloned()
        .unwrap_or(scoring::ScoreReport {
            raw: scoring::raw_score(&owned_records),
            effective: scoring::raw_score(&owned_records),
            limiting_path: None,
        });

    // Filter records for display: remove superseded, and unless --all, remove resolve tombstones
    let display_records: Vec<crate::attestation::Record> = if args.all {
        owned_records.clone()
    } else {
        let active = scoring::filter_superseded(&owned_records);
        active
            .into_iter()
            .filter(|r| r.kind() != Some(&Kind::Resolve))
            .cloned()
            .collect()
    };

    if args.format == "json" {
        if args.pretty {
            let mut value: serde_json::Value =
                serde_json::from_str(&output::show_json(&args.artifact, &report, &display_records))?;
            if let Some(records_arr) = value["records"].as_array_mut() {
                for rec_val in records_arr.iter_mut() {
                    if let Some(span_val) = rec_val.get("body").and_then(|b| b.get("span"))
                        && !span_val.is_null()
                        && let Some(record) = display_records.iter().find(|r| {
                            rec_val.get("id").and_then(|v| v.as_str()) == Some(r.id())
                        })
                        && let Some(att) = record.as_attestation()
                        && let Some(ref span) = att.body.span
                    {
                        let ctx = span_context::read_span_context(
                            Path::new(&args.artifact),
                            span,
                            span_context::DEFAULT_CONTEXT_LINES,
                        );
                        rec_val["context"] = span_context::to_json(&ctx);
                    }
                }
            }
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            println!(
                "{}",
                output::show_json(&args.artifact, &report, &display_records)
            );
        }
        return Ok(());
    }

    // Human output
    println!();
    println!("  {}", args.artifact);
    println!("  Raw score:       {}", report.raw);
    if let Some(ref path) = report.limiting_path {
        println!(
            "  Effective score: {} (limited by {})",
            report.effective,
            path.join(" -> ")
        );
    } else {
        println!("  Effective score: {}", report.effective);
    }

    // Build threading: group replies under their parent record
    let display_ids: std::collections::HashSet<&str> =
        display_records.iter().map(|r| r.id()).collect();

    // Map from parent ID -> child records (replies)
    let mut children: std::collections::HashMap<&str, Vec<&crate::attestation::Record>> =
        std::collections::HashMap::new();
    let mut roots: Vec<&crate::attestation::Record> = Vec::new();

    for record in &display_records {
        let parent_id = record
            .as_attestation()
            .and_then(|a| a.body.references.as_deref());
        if let Some(pid) = parent_id
            && display_ids.contains(pid)
        {
            children.entry(pid).or_default().push(record);
        } else {
            roots.push(record);
        }
    }

    println!();
    println!("  Records ({}):", display_records.len());
    for (i, record) in roots.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_record(record, "    ", "    ", &args, &children);
    }
    println!();

    Ok(())
}

/// Print a single record, then recursively print its replies with tree lines.
///
/// `line_prefix` is printed before this record's line (includes tree chars).
/// `cont_prefix` is printed before continuation lines (pretty context, child tree).
fn print_record(
    record: &crate::attestation::Record,
    line_prefix: &str,
    cont_prefix: &str,
    args: &Args,
    children: &std::collections::HashMap<&str, Vec<&crate::attestation::Record>>,
) {
    if let Some(att) = record.as_attestation() {
        let date = att.created_at.format("%Y-%m-%d");
        let issuer_short = att
            .issuer
            .strip_prefix("mailto:")
            .and_then(|e| e.split('@').next())
            .unwrap_or(&att.issuer);
        let id_short = &att.id[..8.min(att.id.len())];
        println!(
            "{line_prefix}{} {}  {:?}  {}  {}  {}",
            output::format_score(att.body.score),
            att.body.kind,
            att.body.summary,
            issuer_short,
            date,
            id_short,
        );
        if args.pretty
            && let Some(ref span) = att.body.span
        {
            let ctx = span_context::read_span_context(
                Path::new(&args.artifact),
                span,
                span_context::DEFAULT_CONTEXT_LINES,
            );
            if let Some(ref warning) = ctx.warning {
                println!("{cont_prefix}  note: {warning}");
            }
            let formatted = span_context::format_human(&ctx);
            if !formatted.is_empty() {
                for line in formatted.lines() {
                    println!("{cont_prefix}{line}");
                }
            }
        }
    } else if let Some(epoch) = record.as_epoch() {
        let date = epoch.created_at.format("%Y-%m-%d");
        let id_short = &epoch.id[..8.min(epoch.id.len())];
        println!(
            "{line_prefix}{} epoch  {:?}  {}  {}  {}",
            output::format_score(Some(epoch.body.score)),
            epoch.body.summary,
            epoch.issuer,
            date,
            id_short,
        );
    }

    // Print threaded replies with tree-drawing characters
    if let Some(replies) = children.get(record.id()) {
        for (i, reply) in replies.iter().enumerate() {
            let is_last = i == replies.len() - 1;
            let branch = if is_last { "\u{2514}\u{2500} " } else { "\u{251c}\u{2500} " };
            let continuation = if is_last { "   " } else { "\u{2502}  " };
            let child_line = format!("{cont_prefix}{branch}");
            let child_cont = format!("{cont_prefix}{continuation}");
            print_record(reply, &child_line, &child_cont, args, children);
        }
    }
}
