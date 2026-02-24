use chrono::Utc;
use clap::Args as ClapArgs;
use std::io::{self, BufRead};

use crate::attestation::{self, Attestation, Kind};
use crate::qual_file;

#[derive(ClapArgs)]
pub struct Args {
    /// The artifact to attest
    pub artifact: String,

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

    /// ID of a prior attestation this replaces
    #[arg(long)]
    pub supersedes: Option<String>,

    /// Read JSONL attestations from stdin (batch mode)
    #[arg(long)]
    pub stdin: bool,
}

pub fn run(args: Args) -> crate::Result<()> {
    if args.stdin {
        return run_batch();
    }

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

    let qual_path = format!("{}.qual", args.artifact);

    let att = attestation::finalize(Attestation {
        artifact: args.artifact.clone(),
        kind,
        score,
        summary,
        detail: args.detail,
        suggested_fix: args.suggested_fix,
        tags: args.tags,
        author,
        created_at: Utc::now(),
        supersedes: args.supersedes,
        epoch_refs: None,
        id: String::new(),
    });

    qual_file::append(qual_path.as_ref(), &att)?;
    println!("Attested {} [{}] {}", att.artifact, att.score, att.kind);
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

        let mut att: Attestation = serde_json::from_str(trimmed)?;
        att = attestation::finalize(att);

        let qual_path = format!("{}.qual", att.artifact);
        qual_file::append(qual_path.as_ref(), &att)?;
        count += 1;
    }

    println!("Attested {count} artifacts from stdin");
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
