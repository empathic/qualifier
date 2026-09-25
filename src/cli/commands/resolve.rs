use chrono::Utc;
use clap::Args as ClapArgs;
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, Kind, Record};
use crate::cli::provenance;
use crate::cli::targets;
use crate::qual_file;

/// Close reasons accepted by `--reason`; each becomes the tag `reason:<value>`.
pub const CLOSE_REASONS: &[&str] = &["fixed", "wontfix", "duplicate", "invalid", "obsolete"];

/// Add `reason:<reason>` to `tags` unless present. Rejects unknown reasons,
/// a `reason:*` tag that conflicts with `reason`, and more than one
/// `reason:*` tag.
pub(crate) fn with_reason(
    mut tags: Vec<String>,
    reason: Option<&str>,
) -> crate::Result<Vec<String>> {
    if let Some(r) = reason {
        if !CLOSE_REASONS.contains(&r) {
            return Err(crate::Error::Validation(format!(
                "unknown close reason '{r}' (expected one of: {})",
                CLOSE_REASONS.join(", ")
            )));
        }
        let tag = format!("reason:{r}");
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    let reasons: Vec<&str> = tags
        .iter()
        .filter_map(|t| t.strip_prefix("reason:"))
        .collect();
    if reasons.len() > 1 {
        return Err(crate::Error::Validation(format!(
            "a resolve takes one reason; got reason:{}",
            reasons.join(", reason:")
        )));
    }
    if let Some(r) = reasons.first()
        && !CLOSE_REASONS.contains(r)
    {
        return Err(crate::Error::Validation(format!(
            "unknown close reason '{r}' (expected one of: {})",
            CLOSE_REASONS.join(", ")
        )));
    }
    Ok(tags)
}

/// Inputs for a resolve after target resolution.
pub(crate) struct ResolveInput {
    pub message: Option<String>,
    pub reason: Option<String>,
    pub tags: Vec<String>,
    pub issuer: Option<String>,
    pub issuer_type: Option<String>,
    pub r#ref: Option<String>,
}

/// Build a validated `resolve` annotation superseding `target`.
pub(crate) fn build_resolve(target: &Record, input: ResolveInput) -> crate::Result<Annotation> {
    let tags = with_reason(input.tags, input.reason.as_deref())?;
    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: target.subject().to_string(),
        issuer: provenance::issuer(input.issuer.as_deref()),
        issuer_type: provenance::issuer_type(input.issuer_type.as_deref())?,
        created_at: Utc::now(),
        id: String::new(),
        body: AnnotationBody {
            detail: None,
            kind: Kind::Resolve,
            r#ref: input.r#ref,
            references: None,
            span: None,
            suggested_fix: None,
            summary: input.message.unwrap_or_else(|| "Resolved".into()),
            supersedes: Some(target.id().to_string()),
            tags: provenance::with_session_tag(tags),
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

    /// Resolution message (defaults to "Resolved")
    pub message: Option<String>,

    /// Issuer identity URI (defaults to VCS user email with mailto:)
    #[arg(long)]
    pub issuer: Option<String>,

    /// Issuer type (human, ai, tool, unknown)
    #[arg(long)]
    pub issuer_type: Option<String>,

    /// VCS ref to pin (e.g., "git:3aba500")
    #[arg(long, name = "ref")]
    pub r#ref: Option<String>,

    /// Explicit .qual file to write to
    #[arg(long)]
    pub file: Option<String>,

    /// Output format (human, json)
    #[arg(long, default_value = "human")]
    pub format: String,

    /// Classification tags (repeatable)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Allow targeting a superseded or resolved record (annotating history)
    #[arg(long)]
    pub allow_superseded: bool,

    /// Why the record is closed: fixed, wontfix, duplicate, invalid, or
    /// obsolete. Adds the tag `reason:<value>`.
    #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(CLOSE_REASONS.iter().copied()))]
    pub reason: Option<String>,
}

pub fn run(args: Args) -> crate::Result<()> {
    let all_qual_files = targets::discover_project(true)?;
    let target = targets::resolve_target(&args.target, &all_qual_files, args.allow_superseded)?;

    let att = build_resolve(
        &target,
        ResolveInput {
            message: args.message,
            reason: args.reason,
            tags: args.tags,
            issuer: args.issuer,
            issuer_type: args.issuer_type,
            r#ref: args.r#ref,
        },
    )?;

    let root = targets::project_root()?;
    let qual_path = targets::resolve_existing_target_path(
        &root,
        &att.subject,
        args.file.as_deref().map(Path::new),
    )?;
    let record = Record::Annotation(Box::new(att.clone()));
    targets::preflight_supersession(&qual_path, &record)?;
    qual_file::append(qual_path.as_ref(), &record)?;

    if args.format == "json" {
        println!("{}", serde_json::to_string(&record)?);
    } else {
        println!("{} {} {}", att.body.kind, att.subject, att.body.summary);
        println!("  id: {}", att.id);
        println!("  supersedes: {}", targets::short_id(target.id()));
    }

    Ok(())
}
