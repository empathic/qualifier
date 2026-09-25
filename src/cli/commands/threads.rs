//! `qualifier threads` — list conversations across the project.

use clap::Args as ClapArgs;
use globset::{GlobBuilder, GlobMatcher};

use crate::annotation::{self, IssuerType, Kind, Record, Span};
use crate::cli::targets::{self, short_id};
use crate::threads::{self, Thread};

#[derive(ClapArgs)]
pub struct Args {
    /// Filter by location: a path, a directory, a glob (`src/**/*.rs`), or
    /// `path:start[:end]` (threads whose root span overlaps). Any match
    /// selects the thread.
    pub locations: Vec<String>,

    /// Include closed threads and superseded replies
    #[arg(long)]
    pub all: bool,

    /// Root kinds to include (comma-separated)
    #[arg(long, value_name = "KIND[,KIND...]")]
    pub kind: Option<String>,

    /// Tag on the root or a live reply; `ns:*` matches a namespace.
    /// Repeatable; every tag must match.
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Issuer type of the root (human, ai, tool, unknown)
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Don't respect .gitignore / .qualignore
    #[arg(long)]
    pub no_ignore: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    let qual_files = targets::discover_project(!args.no_ignore)?;
    let records: Vec<Record> = qual_files.into_iter().flat_map(|qf| qf.records).collect();
    let filter = Filter::from_args(&args)?;

    let selected: Vec<Thread<'_>> = threads::build_threads(&records)
        .into_iter()
        .filter(|t| filter.matches(t))
        .map(|t| if args.all { t } else { live_replies_only(t) })
        .collect();

    if args.format == "json" {
        print_json(&selected)
    } else {
        print_human(&selected, args.all);
        Ok(())
    }
}

struct Filter {
    all: bool,
    locations: Vec<LocationFilter>,
    kinds: Option<Vec<Kind>>,
    tags: Vec<String>,
    issuer_type: Option<IssuerType>,
}

impl Filter {
    fn from_args(args: &Args) -> crate::Result<Self> {
        let locations = args
            .locations
            .iter()
            .map(|l| LocationFilter::parse(l))
            .collect::<crate::Result<Vec<_>>>()?;
        let kinds = args.kind.as_deref().map(|s| {
            s.split(',')
                .map(|k| k.trim().parse::<Kind>().unwrap())
                .collect()
        });
        let issuer_type = args
            .issuer_type
            .as_deref()
            .map(|s| s.parse::<IssuerType>().map_err(crate::Error::Validation))
            .transpose()?;
        Ok(Self {
            all: args.all,
            locations,
            kinds,
            tags: args.tags.clone(),
            issuer_type,
        })
    }

    fn matches(&self, t: &Thread<'_>) -> bool {
        (self.all || t.open)
            && (self.locations.is_empty() || self.locations.iter().any(|l| l.matches(t.root)))
            && self
                .kinds
                .as_ref()
                .is_none_or(|ks| t.root.kind().is_some_and(|k| ks.contains(k)))
            && self
                .tags
                .iter()
                .all(|pat| thread_tags(t).any(|tag| tag_matches(pat, tag)))
            && self
                .issuer_type
                .as_ref()
                .is_none_or(|it| t.root.issuer_type() == Some(it))
    }
}

enum LocationFilter {
    Glob(GlobMatcher),
    Path { subject: String, span: Option<Span> },
}

impl LocationFilter {
    fn parse(s: &str) -> crate::Result<Self> {
        if s.contains(['*', '?', '[']) {
            let glob = GlobBuilder::new(s)
                .literal_separator(true)
                .build()
                .map_err(|e| crate::Error::Validation(format!("invalid glob '{s}': {e}")))?;
            return Ok(Self::Glob(glob.compile_matcher()));
        }
        let (subject, span) = annotation::parse_location(s);
        let subject = subject
            .trim_start_matches("./")
            .trim_end_matches('/')
            .to_string();
        Ok(Self::Path { subject, span })
    }

    fn matches(&self, root: &Record) -> bool {
        match self {
            Self::Glob(m) => m.is_match(root.subject()),
            Self::Path { subject, span } => {
                let rs = root.subject();
                let path_ok = subject.is_empty()
                    || subject == "."
                    || rs == subject
                    || rs.starts_with(&format!("{subject}/"));
                path_ok
                    && span.as_ref().is_none_or(|s| {
                        root.as_annotation()
                            .and_then(|a| a.body.span.as_ref())
                            .is_some_and(|rsp| targets::span_overlaps(rsp, s))
                    })
            }
        }
    }
}

/// Tags on the root and on every live reply.
fn thread_tags<'a>(t: &Thread<'a>) -> impl Iterator<Item = &'a str> {
    std::iter::once(t.root)
        .chain(t.replies.iter().filter(|e| e.active).map(|e| e.record))
        .filter_map(|r| r.as_annotation())
        .flat_map(|a| a.body.tags.iter().map(String::as_str))
}

/// `ns:*` matches any tag starting with `ns:`; anything else matches exactly.
fn tag_matches(pattern: &str, tag: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => tag.starts_with(prefix),
        None => tag == pattern,
    }
}

fn live_replies_only(t: Thread<'_>) -> Thread<'_> {
    Thread {
        replies: t.replies.into_iter().filter(|e| e.active).collect(),
        ..t
    }
}

fn kind_label(r: &Record) -> String {
    r.kind()
        .map(|k| k.to_string())
        .unwrap_or_else(|| r.record_type().to_string())
}

fn summary(r: &Record) -> &str {
    r.as_annotation()
        .map(|a| a.body.summary.as_str())
        .unwrap_or("")
}

fn location(r: &Record) -> String {
    match r.as_annotation().and_then(|a| a.body.span.as_ref()) {
        Some(s) => match &s.end {
            Some(e) if e.line != s.start.line => {
                format!("{}:{}:{}", r.subject(), s.start.line, e.line)
            }
            _ => format!("{}:{}", r.subject(), s.start.line),
        },
        None => r.subject().to_string(),
    }
}

fn print_human(threads: &[Thread<'_>], all: bool) {
    for t in threads {
        let closed = if t.open { "" } else { " (closed)" };
        println!(
            "[{}] {:<10} {}  {}{closed}",
            short_id(t.root.id()),
            kind_label(t.root),
            location(t.root),
            summary(t.root),
        );
        if all {
            for r in &t.history {
                println!(
                    "    [{}] {:<10} {} (superseded)",
                    short_id(r.id()),
                    kind_label(r),
                    summary(r),
                );
            }
        }
        for e in &t.replies {
            let superseded = if e.active { "" } else { " (superseded)" };
            println!(
                "    [{}] {:<10} {}{superseded}",
                short_id(e.record.id()),
                kind_label(e.record),
                summary(e.record),
            );
        }
    }
    let open = threads.iter().filter(|t| t.open).count();
    let n = threads.len();
    eprintln!("{n} thread{} ({open} open)", if n == 1 { "" } else { "s" });
}

fn print_json(threads: &[Thread<'_>]) -> crate::Result<()> {
    let values = threads
        .iter()
        .map(thread_json)
        .collect::<crate::Result<Vec<_>>>()?;
    println!("{}", serde_json::to_string(&values)?);
    Ok(())
}

fn thread_json(t: &Thread<'_>) -> crate::Result<serde_json::Value> {
    let replies = t
        .replies
        .iter()
        .map(|e| -> crate::Result<serde_json::Value> {
            Ok(serde_json::json!({
                "active": e.active,
                "record": serde_json::to_value(e.record)?,
            }))
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let history = t
        .history
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
        "origin": t.origin,
        "open": t.open,
        "root": serde_json::to_value(t.root)?,
        "closed_by": t.closed_by.map(serde_json::to_value).transpose()?,
        "history": history,
        "replies": replies,
        "latest_at": t.latest_at.to_rfc3339(),
    }))
}
