use chrono::Utc;
use clap::Args as ClapArgs;
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, IssuerType, Kind, Record};
use crate::cli::commands::attest;
use crate::cli::output;
use crate::content_hash;
use crate::qual_file;

#[derive(ClapArgs)]
pub struct ReviewArgs {
    /// File or location (e.g., "src/parser.rs" or "src/parser.rs:42" or "src/parser.rs:15:28")
    pub location: String,

    /// One-line summary / comment text
    pub message: String,

    /// Extended description
    #[arg(long)]
    pub detail: Option<String>,

    /// Suggested fix
    #[arg(long, alias = "fix")]
    pub suggested_fix: Option<String>,

    /// Classification tags (repeatable)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Issuer identity URI (defaults to VCS user email with mailto:)
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type (human, ai, tool, unknown)
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// VCS ref to pin (e.g., "git:3aba500")
    #[arg(long, name = "ref")]
    pub r#ref: Option<String>,

    /// ID of a prior annotation this replaces
    #[arg(long)]
    pub supersedes: Option<String>,

    /// ID of a related annotation (conversational reference, no scoring impact)
    #[arg(long)]
    pub references: Option<String>,

    /// Explicit .qual file to write to
    #[arg(long)]
    pub file: Option<String>,

    /// Quality score override (-100..=100)
    #[arg(long, allow_hyphen_values = true)]
    pub score: Option<i32>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub fn run_review(args: ReviewArgs, kind: Kind) -> crate::Result<()> {
    let (subject, mut span) = annotation::parse_location(&args.location);

    // Auto-compute content hash for spans
    if let Some(ref mut s) = span
        && let Some(hash) = content_hash::compute_span_hash(Path::new(&subject), s)
    {
        s.content_hash = Some(hash);
    }

    let issuer = attest::normalize_issuer_uri(
        args.issuer
            .or_else(attest::detect_issuer)
            .unwrap_or_else(|| "mailto:unknown@localhost".into()),
    );

    let issuer_type = match &args.issuer_type {
        Some(s) => Some(s.parse::<IssuerType>().map_err(crate::Error::Validation)?),
        None => None,
    };

    let score = args.score;

    let qual_path = qual_file::resolve_qual_path(&subject, args.file.as_deref().map(Path::new))?;

    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject,
        issuer,
        issuer_type,
        created_at: Utc::now(),
        id: String::new(),
        body: AnnotationBody {
            detail: args.detail,
            kind,
            r#ref: args.r#ref,
            references: args.references,
            score,
            span,
            suggested_fix: args.suggested_fix,
            summary: args.message,
            supersedes: args.supersedes.clone(),
            tags: args.tags,
        },
    });

    let errors = annotation::validate(&att);
    if !errors.is_empty() {
        return Err(crate::Error::Validation(errors.join("; ")));
    }

    if att.body.supersedes.is_some() {
        let existing = if qual_path.exists() {
            qual_file::parse(&qual_path)?.records
        } else {
            Vec::new()
        };
        let mut all = existing;
        all.push(Record::Annotation(Box::new(att.clone())));
        annotation::check_supersession_cycles(&all)?;
        annotation::validate_supersession_targets(&all)?;
    }

    let record = Record::Annotation(Box::new(att.clone()));

    qual_file::append(qual_path.as_ref(), &record)?;

    if args.format == "json" {
        println!("{}", serde_json::to_string(&record)?);
    } else {
        let span_str = match &att.body.span {
            Some(span) => {
                let end = match &span.end {
                    Some(e) if e.line != span.start.line => format!(":{}", e.line),
                    _ => String::new(),
                };
                format!(":{}{}", span.start.line, end)
            }
            None => String::new(),
        };
        println!(
            "{} {}{} {} {}",
            att.body.kind,
            att.subject,
            span_str,
            output::format_score(att.body.score),
            att.body.summary,
        );
        println!("  id: {}", att.id);
    }

    Ok(())
}
