use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

/// A quality attestation against a software artifact.
///
/// **IMPORTANT:** Field order is canonical — `serde` serializes in declaration
/// order, and this determines attestation IDs. Do not reorder fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Pass => write!(f, "pass"),
            Kind::Fail => write!(f, "fail"),
            Kind::Blocker => write!(f, "blocker"),
            Kind::Concern => write!(f, "concern"),
            Kind::Praise => write!(f, "praise"),
            Kind::Suggestion => write!(f, "suggestion"),
            Kind::Waiver => write!(f, "waiver"),
            Kind::Epoch => write!(f, "epoch"),
            Kind::Custom(s) => write!(f, "{s}"),
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "pass" => Kind::Pass,
            "fail" => Kind::Fail,
            "blocker" => Kind::Blocker,
            "concern" => Kind::Concern,
            "praise" => Kind::Praise,
            "suggestion" => Kind::Suggestion,
            "waiver" => Kind::Waiver,
            "epoch" => Kind::Epoch,
            other => Kind::Custom(other.to_string()),
        })
    }
}

impl Kind {
    /// Recommended default score for each attestation kind.
    pub fn default_score(&self) -> i32 {
        match self {
            Kind::Pass => 20,
            Kind::Fail => -20,
            Kind::Blocker => -50,
            Kind::Concern => -10,
            Kind::Praise => 30,
            Kind::Suggestion => -5,
            Kind::Waiver => 10,
            Kind::Epoch => 0,
            Kind::Custom(_) => 0,
        }
    }
}

/// Zero-copy view of an Attestation for canonical serialization.
/// Field order and `skip_serializing_if` MUST exactly match `Attestation`.
#[derive(Serialize)]
struct CanonicalView<'a> {
    artifact: &'a str,
    kind: &'a Kind,
    score: i32,
    summary: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_fix: &'a Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: &'a Vec<String>,
    author: &'a str,
    created_at: &'a DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supersedes: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch_refs: &'a Option<Vec<String>>,
    id: &'a str,
}

/// Generate a deterministic attestation ID by BLAKE3-hashing the canonical
/// serialization with the `id` field set to the empty string.
pub fn generate_id(attestation: &Attestation) -> String {
    let view = CanonicalView {
        artifact: &attestation.artifact,
        kind: &attestation.kind,
        score: attestation.score,
        summary: &attestation.summary,
        detail: &attestation.detail,
        suggested_fix: &attestation.suggested_fix,
        tags: &attestation.tags,
        author: &attestation.author,
        created_at: &attestation.created_at,
        supersedes: &attestation.supersedes,
        epoch_refs: &attestation.epoch_refs,
        id: "",
    };
    let canonical = serde_json::to_string(&view).expect("attestation must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Validate an attestation, returning all validation errors found.
pub fn validate(attestation: &Attestation) -> Vec<String> {
    let mut errors = Vec::new();

    if attestation.artifact.is_empty() {
        errors.push("artifact must not be empty".into());
    }
    if attestation.summary.is_empty() {
        errors.push("summary must not be empty".into());
    }
    if attestation.author.is_empty() {
        errors.push("author must not be empty".into());
    }
    if attestation.score < -100 || attestation.score > 100 {
        errors.push(format!(
            "score {} is out of range [-100, 100]",
            attestation.score
        ));
    }
    if attestation.id.is_empty() {
        errors.push("id must not be empty".into());
    }

    // Verify content-addressed ID matches
    if !attestation.id.is_empty() {
        let expected = generate_id(attestation);
        if attestation.id != expected {
            errors.push(format!(
                "id mismatch: expected {}, got {}",
                expected, attestation.id
            ));
        }
    }

    // epoch_refs only valid on epoch attestations
    if attestation.epoch_refs.is_some() && attestation.kind != Kind::Epoch {
        errors.push("epoch_refs is only valid on epoch attestations".into());
    }

    // Warn about potentially misspelled custom kinds
    if let Kind::Custom(ref custom) = attestation.kind {
        let known = [
            "pass",
            "fail",
            "blocker",
            "concern",
            "praise",
            "suggestion",
            "waiver",
            "epoch",
        ];
        for k in &known {
            if is_likely_typo(custom, k) {
                errors.push(format!("unknown kind '{}', did you mean '{}'?", custom, k));
                break;
            }
        }
    }

    errors
}

/// Returns true if `a` is likely a typo of `b` (edit distance <= 2, not identical).
fn is_likely_typo(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let len_diff = (a.len() as isize - b.len() as isize).unsigned_abs();
    if len_diff > 2 {
        return false;
    }
    // Levenshtein distance, bounded — abort early if > 2
    let (m, n) = (a.len(), b.len());
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];
    for (j, val) in prev.iter_mut().enumerate().take(n + 1) {
        *val = j;
    }
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n] <= 2
}

/// Check a slice of attestations for supersession cycles.
/// Returns Err with cycle details if a cycle is found.
pub fn check_supersession_cycles(attestations: &[Attestation]) -> crate::Result<()> {
    let id_set: HashSet<&str> = attestations.iter().map(|a| a.id.as_str()).collect();

    for att in attestations {
        if let Some(ref target) = att.supersedes {
            // Walk the chain from this attestation
            let mut visited = HashSet::new();
            visited.insert(att.id.as_str());
            let mut current = target.as_str();

            loop {
                if visited.contains(current) {
                    return Err(crate::Error::Cycle {
                        context: "supersession".into(),
                        detail: format!("cycle detected involving attestation {}", current),
                    });
                }

                // Find the attestation with this ID
                if !id_set.contains(current) {
                    break; // target not in this file — that's fine
                }

                visited.insert(current);

                // Find next link in chain
                match attestations.iter().find(|a| a.id == current) {
                    Some(next) => match &next.supersedes {
                        Some(next_target) => current = next_target.as_str(),
                        None => break,
                    },
                    None => break,
                }
            }
        }
    }

    Ok(())
}

/// Validate that supersession references target the same artifact.
///
/// Returns an error if any cross-artifact supersession is found.
pub fn validate_supersession_targets(attestations: &[Attestation]) -> crate::Result<()> {
    let by_id: std::collections::HashMap<&str, &Attestation> =
        attestations.iter().map(|a| (a.id.as_str(), a)).collect();

    for att in attestations {
        if let Some(ref target_id) = att.supersedes
            && let Some(target) = by_id.get(target_id.as_str())
            && att.artifact != target.artifact
        {
            return Err(crate::Error::Validation(format!(
                "attestation {} (artifact '{}') supersedes {} (artifact '{}') \
                 — cross-artifact supersession is not allowed",
                &att.id[..8],
                att.artifact,
                &target_id[..target_id.len().min(8)],
                target.artifact
            )));
        }
    }
    Ok(())
}

/// Clamp a score to the valid range [-100, 100].
pub fn clamp_score(score: i32) -> i32 {
    score.clamp(-100, 100)
}

/// Build an attestation with a generated ID. The `id` field on the input is
/// ignored and replaced with the content-addressed hash.
pub fn finalize(mut attestation: Attestation) -> Attestation {
    attestation.score = clamp_score(attestation.score);
    attestation.id = String::new(); // clear for hashing
    attestation.id = generate_id(&attestation);
    attestation
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_attestation() -> Attestation {
        let mut att = Attestation {
            artifact: "src/parser.rs".into(),
            kind: Kind::Concern,
            score: -30,
            summary: "Panics on malformed input".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "alice@example.com".into(),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            supersedes: None,
            epoch_refs: None,
            id: String::new(),
        };
        att.id = generate_id(&att);
        att
    }

    #[test]
    fn test_generate_id_deterministic() {
        let att = sample_attestation();
        let id1 = generate_id(&att);
        let id2 = generate_id(&att);
        assert_eq!(id1, id2);
        assert!(!id1.is_empty());
        assert_eq!(id1.len(), 64); // BLAKE3 hex
    }

    #[test]
    fn test_generate_id_changes_with_content() {
        let att1 = sample_attestation();
        let mut att2 = att1.clone();
        att2.score = -20;
        att2.id = generate_id(&att2);
        assert_ne!(att1.id, att2.id);
    }

    #[test]
    fn test_validate_valid() {
        let att = sample_attestation();
        let errors = validate(&att);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn test_validate_empty_fields() {
        let att = Attestation {
            artifact: String::new(),
            kind: Kind::Pass,
            score: 0,
            summary: String::new(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: String::new(),
            created_at: Utc::now(),
            supersedes: None,
            epoch_refs: None,
            id: String::new(),
        };
        let errors = validate(&att);
        assert!(errors.iter().any(|e| e.contains("artifact")));
        assert!(errors.iter().any(|e| e.contains("summary")));
        assert!(errors.iter().any(|e| e.contains("author")));
        assert!(errors.iter().any(|e| e.contains("id")));
    }

    #[test]
    fn test_validate_score_out_of_range() {
        let mut att = sample_attestation();
        att.score = 200;
        att.id = generate_id(&att);
        let errors = validate(&att);
        assert!(errors.iter().any(|e| e.contains("out of range")));
    }

    #[test]
    fn test_validate_id_mismatch() {
        let mut att = sample_attestation();
        att.id = "deadbeef".repeat(8);
        let errors = validate(&att);
        assert!(errors.iter().any(|e| e.contains("id mismatch")));
    }

    #[test]
    fn test_validate_epoch_refs_on_non_epoch() {
        let mut att = sample_attestation();
        att.epoch_refs = Some(vec!["abc".into()]);
        att.id = generate_id(&att);
        let errors = validate(&att);
        assert!(errors.iter().any(|e| e.contains("epoch_refs")));
    }

    #[test]
    fn test_clamp_score() {
        assert_eq!(clamp_score(50), 50);
        assert_eq!(clamp_score(-200), -100);
        assert_eq!(clamp_score(200), 100);
        assert_eq!(clamp_score(-100), -100);
        assert_eq!(clamp_score(100), 100);
    }

    #[test]
    fn test_finalize() {
        let att = Attestation {
            artifact: "test".into(),
            kind: Kind::Pass,
            score: 200, // over max
            summary: "good".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "bot".into(),
            created_at: Utc::now(),
            supersedes: None,
            epoch_refs: None,
            id: "will be replaced".into(),
        };
        let finalized = finalize(att);
        assert_eq!(finalized.score, 100); // clamped
        assert_eq!(finalized.id, generate_id(&finalized)); // valid ID
    }

    #[test]
    fn test_supersession_cycle_detection() {
        let now = Utc::now();
        let mut a = Attestation {
            artifact: "x".into(),
            kind: Kind::Pass,
            score: 10,
            summary: "a".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            created_at: now,
            supersedes: None,
            epoch_refs: None,
            id: "aaa".into(),
        };
        let mut b = a.clone();
        b.id = "bbb".into();
        b.supersedes = Some("aaa".into());

        // No cycle: a <- b
        assert!(check_supersession_cycles(&[a.clone(), b.clone()]).is_ok());

        // Create cycle: a -> b -> a
        a.supersedes = Some("bbb".into());
        assert!(check_supersession_cycles(&[a, b]).is_err());
    }

    #[test]
    fn test_kind_roundtrip() {
        let kinds = vec![
            Kind::Pass,
            Kind::Fail,
            Kind::Blocker,
            Kind::Concern,
            Kind::Praise,
            Kind::Suggestion,
            Kind::Waiver,
            Kind::Epoch,
        ];
        for kind in &kinds {
            let s = kind.to_string();
            let parsed: Kind = s.parse().unwrap();
            assert_eq!(&parsed, kind);
        }
    }

    #[test]
    fn test_kind_serde_roundtrip() {
        let att = sample_attestation();
        let json = serde_json::to_string(&att).unwrap();
        let parsed: Attestation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kind, att.kind);
    }

    #[test]
    fn test_custom_kind() {
        let kind: Kind = "my_custom_kind".parse().unwrap();
        assert_eq!(kind, Kind::Custom("my_custom_kind".into()));
        assert_eq!(kind.to_string(), "my_custom_kind");
    }

    #[test]
    fn test_kind_default_scores() {
        assert_eq!(Kind::Pass.default_score(), 20);
        assert_eq!(Kind::Fail.default_score(), -20);
        assert_eq!(Kind::Blocker.default_score(), -50);
        assert_eq!(Kind::Concern.default_score(), -10);
        assert_eq!(Kind::Praise.default_score(), 30);
        assert_eq!(Kind::Suggestion.default_score(), -5);
        assert_eq!(Kind::Waiver.default_score(), 10);
        assert_eq!(Kind::Epoch.default_score(), 0);
        assert_eq!(Kind::Custom("foo".into()).default_score(), 0);
    }

    #[test]
    fn test_typo_detection_in_validate() {
        let mut att = sample_attestation();
        att.kind = Kind::Custom("pss".into());
        att.id = generate_id(&att);
        let errors = validate(&att);
        assert!(
            errors.iter().any(|e| e.contains("did you mean 'pass'")),
            "expected typo suggestion, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_no_typo_for_distant_custom_kind() {
        let mut att = sample_attestation();
        att.kind = Kind::Custom("my_custom_lint".into());
        att.id = generate_id(&att);
        let errors = validate(&att);
        assert!(
            !errors.iter().any(|e| e.contains("did you mean")),
            "unexpected typo suggestion for distant custom kind: {:?}",
            errors
        );
    }

    #[test]
    fn test_cross_artifact_supersession_detected() {
        let a = finalize(Attestation {
            artifact: "foo.rs".into(),
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            created_at: Utc::now(),
            supersedes: None,
            epoch_refs: None,
            id: String::new(),
        });
        let b = finalize(Attestation {
            artifact: "bar.rs".into(),
            kind: Kind::Pass,
            score: 20,
            summary: "updated".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            created_at: Utc::now(),
            supersedes: Some(a.id.clone()),
            epoch_refs: None,
            id: String::new(),
        });
        let result = validate_supersession_targets(&[a, b]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cross-artifact"));
    }

    #[test]
    fn test_same_artifact_supersession_ok() {
        let a = finalize(Attestation {
            artifact: "foo.rs".into(),
            kind: Kind::Concern,
            score: -10,
            summary: "bad".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            created_at: Utc::now(),
            supersedes: None,
            epoch_refs: None,
            id: String::new(),
        });
        let b = finalize(Attestation {
            artifact: "foo.rs".into(),
            kind: Kind::Pass,
            score: 20,
            summary: "fixed".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            created_at: Utc::now(),
            supersedes: Some(a.id.clone()),
            epoch_refs: None,
            id: String::new(),
        });
        let result = validate_supersession_targets(&[a, b]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_likely_typo() {
        assert!(is_likely_typo("pss", "pass"));
        assert!(is_likely_typo("pas", "pass"));
        assert!(is_likely_typo("bloker", "blocker"));
        assert!(!is_likely_typo("pass", "pass")); // identical
        assert!(!is_likely_typo("my_custom_lint", "pass")); // too far
    }
}
