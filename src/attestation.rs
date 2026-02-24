use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A quality attestation against a software artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Qualified name of the artifact.
    pub artifact: String,

    /// The type of attestation.
    pub kind: Kind,

    /// Signed quality delta, clamped to -100..=100.
    pub score: i32,

    /// Human-readable one-liner.
    pub summary: String,

    /// Extended description (markdown allowed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// Actionable suggestion for improvement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,

    /// Freeform classification tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Who or what created this attestation.
    pub author: String,

    /// When this attestation was created (RFC 3339).
    pub created_at: DateTime<Utc>,

    /// ID of a prior attestation this replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,

    /// IDs of attestations compacted into this epoch (epoch attestations only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch_refs: Option<Vec<String>>,

    /// Content-addressed attestation ID (BLAKE3).
    pub id: String,
}

/// The type of an attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Pass,
    Fail,
    Blocker,
    Concern,
    Praise,
    Suggestion,
    Waiver,
    Epoch,
    #[serde(untagged)]
    Custom(String),
}

/// Generate a deterministic attestation ID by BLAKE3-hashing the canonical
/// serialization with the `id` field set to the empty string.
pub fn generate_id(attestation: &Attestation) -> String {
    let mut att = attestation.clone();
    att.id = String::new();
    let canonical = serde_json::to_string(&att).expect("attestation must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}
