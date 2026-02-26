use chrono::Utc;
use clap::Args as ClapArgs;
use std::io::{self, BufRead};
use std::path::Path;

use crate::attestation::{self, Attestation, AttestationBody, AuthorType, Kind, Record};
use crate::qual_file;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to attest (required unless --stdin)
    pub artifact: Option<String>,

    /// Attestation kind (pass, fail, blocker, concern, praise, suggestion, waiver)
    #[arg(long)]
    pub kind: Option<String>,

    /// Quality score delta (-100..=100)
    #[arg(long, allow_hyphen_values = true)]
    pub score: Option<i32>,

    /// One-line summary
    #[arg(long)]
    pub summary: Option<String>,

    /// Extended description
    #[arg(long)]
    pub detail: Option<String>,

    /// Suggested fix
    #[arg(long)]
    pub suggested_fix: Option<String>,

    /// Classification tags (repeatable)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Author identity (defaults to VCS user)
    #[arg(long)]
    pub author: Option<String>,

    /// Author type (human, ai, tool, unknown)
    #[arg(long)]
    pub author_type: Option<String>,

    /// Sub-artifact span (e.g., "42", "42:58", "42.5:58.80")
    #[arg(long)]
    pub span: Option<String>,

    /// VCS ref to pin (e.g., "git:3aba500")
    #[arg(long, name = "ref")]
    pub r#ref: Option<String>,

    /// ID of a prior attestation this replaces
    #[arg(long)]
    pub supersedes: Option<String>,

    /// Explicit .qual file to write to (overrides layout resolution)
    #[arg(long)]
    pub file: Option<String>,

    /// Read JSONL attestations from stdin (batch mode)
    #[arg(long)]
    pub stdin: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    if args.stdin {
        return run_batch();
    }

    let subject = match args.artifact {
        Some(a) => a,
        None => {
            return Err(crate::Error::Validation(
                "<artifact> is required (or use --stdin for batch mode)".into(),
            ));
        }
    };

    let kind: Kind = args.kind.as_deref().unwrap_or("concern").parse().unwrap();

    let score = args.score.unwrap_or_else(|| kind.default_score());

    let summary = match args.summary {
        Some(s) => s,
        None => {
            return Err(crate::Error::Validation(
                "--summary is required (or use --stdin for batch mode)".into(),
            ));
        }
    };

    let author = args
        .author
        .or_else(detect_author)
        .unwrap_or_else(|| "unknown".into());

    let author_type = match &args.author_type {
        Some(s) => Some(s.parse::<AuthorType>().map_err(crate::Error::Validation)?),
        None => None,
    };

    let span = match &args.span {
        Some(s) => Some(attestation::parse_span(s).map_err(crate::Error::Validation)?),
        None => None,
    };

    let qual_path = qual_file::resolve_qual_path(&subject, args.file.as_deref().map(Path::new))?;

    let att = attestation::finalize(Attestation {
        metabox: "1".into(),
        record_type: "attestation".into(),
        subject,
        author,
        created_at: Utc::now(),
        id: String::new(),
        body: AttestationBody {
            author_type,
            detail: args.detail,
            kind,
            r#ref: args.r#ref,
            score,
            span,
            suggested_fix: args.suggested_fix,
            summary,
            supersedes: args.supersedes,
            tags: args.tags,
        },
    });

    let errors = attestation::validate(&att);
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
        all.push(Record::Attestation(Box::new(att.clone())));
        attestation::check_supersession_cycles(&all)?;
        attestation::validate_supersession_targets(&all)?;
    }

    qual_file::append(
        qual_path.as_ref(),
        &Record::Attestation(Box::new(att.clone())),
    )?;
    println!(
        "Attested {} [{}] {}",
        att.subject, att.body.score, att.body.kind
    );
    println!("  id: {}", att.id);

    Ok(())
}

fn run_batch() -> crate::Result<()> {
    let stdin = io::stdin();
    let mut count = 0;

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        let record: Record = serde_json::from_str(trimmed)?;
        let record = attestation::finalize_record(record);

        // Validate attestation records
        if let Some(att) = record.as_attestation() {
            let errors = attestation::validate(att);
            if !errors.is_empty() {
                return Err(crate::Error::Validation(errors.join("; ")));
            }
        }

        let qual_path = qual_file::resolve_qual_path(record.subject(), None)?;

        if record.supersedes().is_some() {
            let existing = if qual_path.exists() {
                qual_file::parse(&qual_path)?.records
            } else {
                Vec::new()
            };
            let mut all = existing;
            all.push(record.clone());
            attestation::check_supersession_cycles(&all)?;
            attestation::validate_supersession_targets(&all)?;
        }

        qual_file::append(&qual_path, &record)?;
        count += 1;
    }

    println!("Attested {count} records from stdin");
    Ok(())
}

fn detect_author() -> Option<String> {
    // Try git first
    std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            // Try hg
            std::process::Command::new("hg")
                .args(["config", "ui.username"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            // Fallback: $USER@localhost
            let user = std::env::var("USER").unwrap_or_else(|_| "unknown".into());
            Some(format!("{user}@localhost"))
        })
}
