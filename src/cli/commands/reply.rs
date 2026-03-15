use chrono::Utc;
use clap::Args as ClapArgs;
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, IssuerType, Kind, Record};
use crate::cli::commands::attest;
use crate::cli::output;
use crate::qual_file;

#[derive(ClapArgs)]
pub struct Args {
    /// ID prefix of the record to reply to (minimum 4 characters)
    pub id_prefix: String,

    /// One-line reply message
    pub message: String,

    /// Override the default kind (comment)
    #[arg(long)]
    pub kind: Option<String>,

    /// Quality score override (-100..=100)
    #[arg(long, allow_hyphen_values = true)]
    pub score: Option<i32>,

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

    /// Explicit .qual file to write to
    #[arg(long)]
    pub file: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,
}

/// Resolve a short ID prefix to a unique record across all qual files.
pub(crate) fn resolve_id_prefix(prefix: &str, qual_files: &[qual_file::QualFile]) -> crate::Result<Record> {
    if prefix.len() < 4 {
        return Err(crate::Error::Validation(
            "ID prefix must be at least 4 characters".into(),
        ));
    }

    let matches: Vec<&Record> = qual_files
        .iter()
        .flat_map(|qf| qf.records.iter())
        .filter(|r| r.id().starts_with(prefix))
        .collect();

    match matches.len() {
        0 => Err(crate::Error::Validation(format!(
            "no record found matching prefix '{prefix}'"
        ))),
        1 => Ok(matches[0].clone()),
        n => Err(crate::Error::Validation(format!(
            "ambiguous prefix '{prefix}' matches {n} records"
        ))),
    }
}

pub fn run(args: Args) -> crate::Result<()> {
    let root = qual_file::find_project_root(Path::new("."));
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let all_qual_files = qual_file::discover(discover_root, true)?;

    let target = resolve_id_prefix(&args.id_prefix, &all_qual_files)?;
    let subject = target.subject().to_string();
    let target_id = target.id().to_string();

    let kind: Kind = args.kind.as_deref().unwrap_or("comment").parse().unwrap();
    let score = args.score;

    let issuer = attest::normalize_issuer_uri(
        args.issuer
            .or_else(attest::detect_issuer)
            .unwrap_or_else(|| "mailto:unknown@localhost".into()),
    );

    let issuer_type = match &args.issuer_type {
        Some(s) => Some(s.parse::<IssuerType>().map_err(crate::Error::Validation)?),
        None => None,
    };

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
            references: Some(target_id),
            score,
            span: None,
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
        println!(
            "{} {} {} {}",
            att.body.kind,
            att.subject,
            output::format_score(att.body.score),
            att.body.summary,
        );
        println!("  id: {}", att.id);
        println!("  re: {}", &att.body.references.as_ref().unwrap()[..8]);
    }

    Ok(())
}
