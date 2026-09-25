use chrono::Utc;
use clap::Args as ClapArgs;
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, Kind, Record};
use crate::cli::provenance;
use crate::cli::targets;
use crate::qual_file;

/// Inputs for a reply after target resolution.
pub(crate) struct ReplyInput {
    pub message: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub suggested_fix: Option<String>,
    pub tags: Vec<String>,
    pub issuer: Option<String>,
    pub issuer_type: Option<String>,
    pub r#ref: Option<String>,
    /// Full ID of the reply this one replaces.
    pub supersedes: Option<String>,
}

/// Build a validated reply annotation to `target`.
pub(crate) fn build_reply(target: &Record, input: ReplyInput) -> crate::Result<Annotation> {
    let kind: Kind = input.kind.as_deref().unwrap_or("comment").parse().unwrap();
    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: target.subject().to_string(),
        issuer: provenance::issuer(input.issuer.as_deref()),
        issuer_type: provenance::issuer_type(input.issuer_type.as_deref())?,
        created_at: Utc::now(),
        id: String::new(),
        body: AnnotationBody {
            detail: input.detail,
            kind,
            r#ref: input.r#ref,
            references: Some(target.id().to_string()),
            span: None,
            suggested_fix: input.suggested_fix,
            summary: input.message,
            supersedes: input.supersedes,
            tags: provenance::with_session_tag(input.tags),
        },
    });
    let errors = annotation::validate(&att);
    if !errors.is_empty() {
        return Err(crate::Error::Validation(errors.join("; ")));
    }
    Ok(att)
}

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

    /// Issuer identity URI (defaults to QUALIFIER_ISSUER, then detected agent harness, then VCS user email)
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type: human, ai, tool, unknown (defaults to QUALIFIER_ISSUER_TYPE, then detected agent harness)
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
    let qual_files = targets::discover_project(true)?;
    let target = targets::resolve_target(&args.target, &qual_files, args.allow_superseded)?;
    let supersedes = args
        .supersedes
        .as_deref()
        .map(|v| targets::resolve_id_flag("--supersedes", v, &qual_files, args.allow_superseded))
        .transpose()?;

    let att = build_reply(
        &target,
        ReplyInput {
            message: args.message,
            kind: args.kind,
            detail: args.detail,
            suggested_fix: args.suggested_fix,
            tags: args.tags,
            issuer: args.issuer,
            issuer_type: args.issuer_type,
            r#ref: args.r#ref,
            supersedes,
        },
    )?;

    let root = targets::project_root()?;
    let qual_path = targets::resolve_existing_target_path(
        &root,
        &att.subject,
        args.file.as_deref().map(Path::new),
    )?;
    let record = Record::Annotation(Box::new(att.clone()));
    if record.supersedes().is_some() {
        targets::preflight_supersession(&qual_path, &record)?;
    }
    qual_file::append(qual_path.as_ref(), &record)?;

    if args.format == "json" {
        println!("{}", serde_json::to_string(&record)?);
    } else {
        println!("{} {} {}", att.body.kind, att.subject, att.body.summary);
        println!("  id: {}", att.id);
        println!("  re: {}", targets::short_id(target.id()));
    }
    Ok(())
}
