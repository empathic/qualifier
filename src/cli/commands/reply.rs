use chrono::Utc;
use clap::Args as ClapArgs;
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, Kind, Record};
use crate::cli::provenance;
use crate::cli::targets;
use crate::qual_file;

#[derive(ClapArgs)]
pub struct Args {
    /// Target — either an id-prefix (≥4 chars) or a `<location>`
    /// (e.g., `src/auth.rs:42`). A location resolves to the most-recent
    /// active record there; ambiguity is reported with a candidate list.
    pub target: String,

    /// One-line reply message
    pub message: String,

    /// Override the default kind (comment)
    #[arg(long)]
    pub kind: Option<String>,

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

    /// Allow targeting a superseded or resolved record (annotating history)
    #[arg(long)]
    pub allow_superseded: bool,

    /// Explicit .qual file to write to
    #[arg(long)]
    pub file: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,
}

pub fn run(args: Args) -> crate::Result<()> {
    let all_qual_files = targets::discover_project(true)?;

    let target = targets::resolve_target(&args.target, &all_qual_files, args.allow_superseded)?;
    let subject = target.subject().to_string();
    let target_id = target.id().to_string();

    let kind: Kind = args.kind.as_deref().unwrap_or("comment").parse().unwrap();

    let issuer = provenance::issuer(args.issuer.as_deref());
    let issuer_type = provenance::issuer_type(args.issuer_type.as_deref())?;

    let supersedes = args
        .supersedes
        .as_deref()
        .map(|v| {
            targets::resolve_id_flag("--supersedes", v, &all_qual_files, args.allow_superseded)
        })
        .transpose()?;

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
            span: None,
            suggested_fix: args.suggested_fix,
            summary: args.message,
            supersedes,
            tags: provenance::with_session_tag(args.tags),
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
        println!("{} {} {}", att.body.kind, att.subject, att.body.summary,);
        println!("  id: {}", att.id);
        println!("  re: {}", &att.body.references.as_ref().unwrap()[..8]);
    }

    Ok(())
}
