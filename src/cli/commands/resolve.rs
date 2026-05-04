use chrono::Utc;
use clap::Args as ClapArgs;
use std::path::Path;

use crate::annotation::{self, Annotation, AnnotationBody, IssuerType, Kind, Record};
use crate::cli::commands::record::{detect_issuer, normalize_issuer_uri};
use crate::cli::commands::reply;
use crate::qual_file;

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
}

pub fn run(args: Args) -> crate::Result<()> {
    let root = qual_file::find_project_root(Path::new("."));
    let discover_root = root.as_deref().unwrap_or(Path::new("."));
    let all_qual_files = qual_file::discover(discover_root, true)?;

    let target = reply::resolve_target(&args.target, &all_qual_files)?;
    let subject = target.subject().to_string();
    let target_id = target.id().to_string();

    let message = args.message.unwrap_or_else(|| "Resolved".into());

    let issuer = normalize_issuer_uri(
        args.issuer
            .or_else(detect_issuer)
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
            detail: None,
            kind: Kind::Resolve,
            r#ref: args.r#ref,
            references: None,
            span: None,
            suggested_fix: None,
            summary: message,
            supersedes: Some(target_id.clone()),
            tags: args.tags,
        },
    });

    let errors = annotation::validate(&att);
    if !errors.is_empty() {
        return Err(crate::Error::Validation(errors.join("; ")));
    }

    // Check supersession invariants
    let existing = if qual_path.exists() {
        qual_file::parse(&qual_path)?.records
    } else {
        Vec::new()
    };
    let mut all = existing;
    all.push(Record::Annotation(Box::new(att.clone())));
    annotation::check_supersession_cycles(&all)?;
    annotation::validate_supersession_targets(&all)?;

    let record = Record::Annotation(Box::new(att.clone()));

    qual_file::append(qual_path.as_ref(), &record)?;

    if args.format == "json" {
        println!("{}", serde_json::to_string(&record)?);
    } else {
        println!(
            "{} {} {}",
            att.body.kind, att.subject, att.body.summary,
        );
        println!("  id: {}", att.id);
        println!("  supersedes: {}", &target_id[..8]);
    }

    Ok(())
}
