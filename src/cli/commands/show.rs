use std::collections::HashSet;

use clap::Args as ClapArgs;

use crate::annotation::Record;
use crate::cli::span_context;
use crate::cli::targets;
use crate::compact::filter_superseded;
use crate::threads::{self, Thread, ThreadRenderer};

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to show, relative to the current directory
    pub artifact: String,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Disable .gitignore and .qualignore filtering
    #[arg(long)]
    pub no_ignore: bool,

    /// Show source context around spans (compiler-diagnostic style)
    #[arg(long)]
    pub pretty: bool,

    /// Also show edit history, superseded replies, and superseded records
    #[arg(long)]
    pub all: bool,

    /// Filter records by envelope `type` (e.g. `annotation`, `epoch`,
    /// `dependency`, or a custom type URI). Open per spec §3.5.
    #[arg(long = "type", value_name = "TYPE")]
    pub record_type: Option<String>,
}

pub fn run(args: Args) -> crate::Result<()> {
    let locator = targets::Locator::from_cwd()?;
    let subject = locator.subject(&args.artifact)?;
    let records: Vec<Record> = targets::discover_project(!args.no_ignore)?
        .into_iter()
        .flat_map(|qf| qf.records)
        .collect();

    if !records.iter().any(|r| r.subject() == subject) {
        return Err(crate::Error::Validation(format!(
            "No records found for '{subject}'"
        )));
    }

    let type_matches = |t: &str| args.record_type.as_deref().is_none_or(|f| f == t);

    // Annotations are shown as threads; every other record type on the
    // subject is listed on its own.
    let thread_list: Vec<Thread<'_>> = if type_matches("annotation") {
        threads::threads_touching(&records, &subject, args.all)
    } else {
        Vec::new()
    };
    let live: HashSet<&str> = filter_superseded(&records)
        .into_iter()
        .map(|r| r.id())
        .collect();
    let others: Vec<&Record> = records
        .iter()
        .filter(|r| r.subject() == subject && r.as_annotation().is_none())
        .filter(|r| args.all || live.contains(r.id()))
        .filter(|r| type_matches(r.record_type()))
        .collect();

    if args.format == "json" {
        return print_json(&args, &locator, &subject, &records, &thread_list, &others);
    }

    let continuation = |r: &Record| pretty_context(&locator, r);
    let renderer = ThreadRenderer {
        all: args.all,
        expand_closed: false,
        attribution: true,
        continuation: if args.pretty {
            Some(&continuation)
        } else {
            None
        },
    };

    println!();
    println!("  {subject}");
    let (open, closed): (Vec<&Thread<'_>>, Vec<&Thread<'_>>) =
        thread_list.iter().partition(|t| t.open);
    if !open.is_empty() {
        println!();
        println!("  Open threads ({}):", open.len());
        for (i, t) in open.iter().enumerate() {
            if i > 0 {
                println!();
            }
            for line in renderer.render(t) {
                println!("    {line}");
            }
        }
    }
    if !closed.is_empty() {
        println!();
        println!("  Closed threads ({}):", closed.len());
        for t in &closed {
            for line in renderer.render(t) {
                println!("    {line}");
            }
        }
    }
    let shown_others: Vec<String> = others.iter().filter_map(|r| other_line(r)).collect();
    if !shown_others.is_empty() {
        println!();
        println!("  Other records ({}):", shown_others.len());
        for line in &shown_others {
            println!("    {line}");
        }
    }
    if thread_list.is_empty() && shown_others.is_empty() {
        println!();
        println!("  No matching records.");
    }
    println!();

    Ok(())
}

/// Source context under a span-addressed annotation, for `--pretty`.
fn pretty_context(locator: &targets::Locator, record: &Record) -> Vec<String> {
    let Some(att) = record.as_annotation() else {
        return Vec::new();
    };
    let Some(span) = att.body.span.as_ref() else {
        return Vec::new();
    };
    let ctx = span_context::read_span_context(
        &locator.file(&att.subject),
        &att.subject,
        span,
        span_context::DEFAULT_CONTEXT_LINES,
    );
    let mut lines = Vec::new();
    if let Some(ref warning) = ctx.warning {
        lines.push(format!("note: {warning}"));
    }
    lines.extend(span_context::format_human(&ctx).lines().map(String::from));
    lines
}

/// One line for a non-annotation record, or `None` for records hidden in
/// human output.
fn other_line(record: &Record) -> Option<String> {
    if let Some(epoch) = record.as_epoch() {
        let date = epoch.created_at.format("%Y-%m-%d");
        return Some(format!(
            "[{}] epoch  {:?}  ({}, {date})",
            threads::short_id(&epoch.id),
            epoch.body.summary,
            threads::short_issuer(&epoch.issuer),
        ));
    }
    if matches!(record, Record::Dependency(_)) {
        // Dependency records are graph metadata, not quality signals.
        // `qualifier show <subject>` is for surfacing quality signals
        // (annotations, epochs); skip dependencies in human output.
        // They remain visible in `--format json`.
        return None;
    }
    // Fallback for unknown / extension record types — preserve substrate
    // visibility per spec §2.5 without trying to interpret the body.
    let type_str = record.record_type();
    let type_display = if type_str.is_empty() {
        "<unknown>"
    } else {
        type_str
    };
    Some(format!(
        "[{}] {type_display}",
        threads::short_id(record.id())
    ))
}

/// `{"subject", "records", "threads"}`: `records` holds every record shown
/// (each thread's root, live replies, and closing resolve, plus history and
/// superseded replies under `--all`, then the subject's other records), in
/// file order; `threads` gives each thread's state.
fn print_json(
    args: &Args,
    locator: &targets::Locator,
    subject: &str,
    records: &[Record],
    thread_list: &[Thread<'_>],
    others: &[&Record],
) -> crate::Result<()> {
    let mut ids: HashSet<&str> = others.iter().map(|r| r.id()).collect();
    for t in thread_list {
        if args.all {
            ids.extend(t.records().map(|r| r.id()));
        } else {
            ids.extend(t.live_records().map(|r| r.id()));
        }
    }
    let mut out = Vec::new();
    for record in records.iter().filter(|r| ids.contains(r.id())) {
        let mut value = serde_json::to_value(record)?;
        if args.pretty
            && let Some(att) = record.as_annotation()
            && let Some(span) = att.body.span.as_ref()
        {
            let ctx = span_context::read_span_context(
                &locator.file(&att.subject),
                &att.subject,
                span,
                span_context::DEFAULT_CONTEXT_LINES,
            );
            value["context"] = span_context::to_json(&ctx);
        }
        out.push(value);
    }
    let value = serde_json::json!({
        "subject": subject,
        "records": out,
        "threads": thread_list.iter().map(threads::thread_summary_json).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
