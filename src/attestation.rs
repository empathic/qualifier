use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::fmt;

// ─── Span types ─────────────────────────────────────────────────────────────

/// A position within an artifact (1-indexed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// 1-indexed line number.
    pub line: u32,
    /// 1-indexed column number (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
}

/// A sub-range within an artifact.
///
/// When `end` is absent, the span addresses a single position (end = start).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Start of the range (inclusive).
    pub start: Position,
    /// End of the range (inclusive). Defaults to `start` if absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Position>,
}

impl Span {
    /// Return the end position, defaulting to start if absent.
    pub fn end_or_start(&self) -> &Position {
        self.end.as_ref().unwrap_or(&self.start)
    }

    /// Normalize: materialize end = start if absent.
    pub fn normalize(&mut self) {
        if self.end.is_none() {
            self.end = Some(self.start.clone());
        }
    }
}

/// Parse a span from CLI syntax: "42", "42:58", "42.5:58.80"
pub fn parse_span(s: &str) -> Result<Span, String> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => {
            let start = parse_position(parts[0])?;
            Ok(Span { start, end: None })
        }
        2 => {
            let start = parse_position(parts[0])?;
            let end = parse_position(parts[1])?;
            Ok(Span {
                start,
                end: Some(end),
            })
        }
        _ => Err(format!(
            "invalid span syntax: '{s}' (expected LINE, LINE:LINE, or LINE.COL:LINE.COL)"
        )),
    }
}

fn parse_position(s: &str) -> Result<Position, String> {
    let parts: Vec<&str> = s.split('.').collect();
    match parts.len() {
        1 => {
            let line: u32 = parts[0]
                .parse()
                .map_err(|_| format!("invalid line number: '{}'", parts[0]))?;
            Ok(Position { line, col: None })
        }
        2 => {
            let line: u32 = parts[0]
                .parse()
                .map_err(|_| format!("invalid line number: '{}'", parts[0]))?;
            let col: u32 = parts[1]
                .parse()
                .map_err(|_| format!("invalid column number: '{}'", parts[1]))?;
            Ok(Position {
                line,
                col: Some(col),
            })
        }
        _ => Err(format!(
            "invalid position syntax: '{s}' (expected LINE or LINE.COL)"
        )),
    }
}

// ─── Kind enum ──────────────────────────────────────────────────────────────

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
            Kind::Custom(_) => 0,
        }
    }
}

// ─── AuthorType enum ────────────────────────────────────────────────────────

/// Author classification for attestations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorType {
    Human,
    Ai,
    Tool,
    Unknown,
}

impl fmt::Display for AuthorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthorType::Human => write!(f, "human"),
            AuthorType::Ai => write!(f, "ai"),
            AuthorType::Tool => write!(f, "tool"),
            AuthorType::Unknown => write!(f, "unknown"),
        }
    }
}

impl std::str::FromStr for AuthorType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "human" => Ok(AuthorType::Human),
            "ai" => Ok(AuthorType::Ai),
            "tool" => Ok(AuthorType::Tool),
            "unknown" => Ok(AuthorType::Unknown),
            other => Err(format!("unknown author_type: '{other}'")),
        }
    }
}

// ─── Body structs (fields alphabetical for MCF) ─────────────────────────────

/// Attestation body fields. Field order is alphabetical (MCF canonical form).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttestationBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_type: Option<AuthorType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    pub score: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Epoch body fields. Field order is alphabetical (MCF canonical form).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EpochBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_type: Option<AuthorType>,
    pub refs: Vec<String>,
    pub score: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    pub summary: String,
}

/// Dependency body fields. Field order is alphabetical (MCF canonical form).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyBody {
    pub depends_on: Vec<String>,
}

// ─── Attestation struct ─────────────────────────────────────────────────────

fn default_attestation_type() -> String {
    "attestation".to_string()
}

fn default_metabox() -> String {
    "1".to_string()
}

/// A quality attestation against a software artifact (Metabox envelope).
///
/// **IMPORTANT:** Envelope field order is fixed: metabox, type, subject,
/// author, created_at, id, body. Body fields are alphabetical (MCF).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attestation {
    /// Metabox envelope version. Always "1".
    #[serde(default = "default_metabox")]
    pub metabox: String,

    /// Record type. Always "attestation".
    #[serde(rename = "type", default = "default_attestation_type")]
    pub record_type: String,

    /// Qualified name of the subject (artifact).
    pub subject: String,

    /// Who or what created this attestation.
    pub author: String,

    /// When this attestation was created (RFC 3339).
    pub created_at: DateTime<Utc>,

    /// Content-addressed record ID (BLAKE3).
    pub id: String,

    /// Type-specific payload.
    pub body: AttestationBody,
}

// ─── Epoch struct ───────────────────────────────────────────────────────────

/// An epoch record — a compaction summary that replaces a set of records
/// with a single scored record preserving the net score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Epoch {
    #[serde(default = "default_metabox")]
    pub metabox: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub subject: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub body: EpochBody,
}

// ─── DependencyRecord struct ────────────────────────────────────────────────

/// A dependency record declaring edges between artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyRecord {
    #[serde(default = "default_metabox")]
    pub metabox: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub subject: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub body: DependencyBody,
}

// ─── Record enum ────────────────────────────────────────────────────────────

/// A typed qualifier record. Dispatches on the `type` field in JSON.
#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    Attestation(Box<Attestation>),
    Epoch(Epoch),
    Dependency(DependencyRecord),
    Unknown(serde_json::Value),
}

impl Serialize for Record {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Record::Attestation(a) => a.serialize(serializer),
            Record::Epoch(e) => e.serialize(serializer),
            Record::Dependency(d) => d.serialize(serializer),
            Record::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let record_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("attestation");

        match record_type {
            "attestation" => {
                let att: Attestation =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Record::Attestation(Box::new(att)))
            }
            "epoch" => {
                let epoch: Epoch =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Record::Epoch(epoch))
            }
            "dependency" => {
                let dep: DependencyRecord =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Record::Dependency(dep))
            }
            _ => Ok(Record::Unknown(value)),
        }
    }
}

impl Record {
    /// Get the subject name (formerly artifact).
    pub fn subject(&self) -> &str {
        match self {
            Record::Attestation(a) => &a.subject,
            Record::Epoch(e) => &e.subject,
            Record::Dependency(d) => &d.subject,
            Record::Unknown(v) => v.get("subject").and_then(|v| v.as_str()).unwrap_or(""),
        }
    }

    /// Get the record ID.
    pub fn id(&self) -> &str {
        match self {
            Record::Attestation(a) => &a.id,
            Record::Epoch(e) => &e.id,
            Record::Dependency(d) => &d.id,
            Record::Unknown(v) => v.get("id").and_then(|v| v.as_str()).unwrap_or(""),
        }
    }

    /// Get the score (if this is a scored record type).
    pub fn score(&self) -> Option<i32> {
        match self {
            Record::Attestation(a) => Some(a.body.score),
            Record::Epoch(e) => Some(e.body.score),
            _ => None,
        }
    }

    /// Get the supersedes ID (attestations only).
    pub fn supersedes(&self) -> Option<&str> {
        match self {
            Record::Attestation(a) => a.body.supersedes.as_deref(),
            _ => None,
        }
    }

    /// Get the kind (attestations only).
    pub fn kind(&self) -> Option<&Kind> {
        match self {
            Record::Attestation(a) => Some(&a.body.kind),
            _ => None,
        }
    }

    /// Try to get this as an attestation.
    pub fn as_attestation(&self) -> Option<&Attestation> {
        match self {
            Record::Attestation(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get this as an epoch.
    pub fn as_epoch(&self) -> Option<&Epoch> {
        match self {
            Record::Epoch(e) => Some(e),
            _ => None,
        }
    }

    /// Returns true if this is a scored record type (attestation or epoch).
    pub fn is_scored(&self) -> bool {
        matches!(self, Record::Attestation(_) | Record::Epoch(_))
    }
}

// ─── Canonical views (for ID generation — MCF) ──────────────────────────────

/// Zero-copy canonical view for attestation records (MCF).
/// Envelope fields in fixed order, body handles its own alphabetical ordering.
#[derive(Serialize)]
struct AttestationCanonicalView<'a> {
    metabox: &'a str,
    r#type: &'a str,
    subject: &'a str,
    author: &'a str,
    created_at: &'a DateTime<Utc>,
    id: &'a str,
    body: &'a AttestationBody,
}

/// Zero-copy canonical view for epoch records (MCF).
#[derive(Serialize)]
struct EpochCanonicalView<'a> {
    metabox: &'a str,
    r#type: &'a str,
    subject: &'a str,
    author: &'a str,
    created_at: &'a DateTime<Utc>,
    id: &'a str,
    body: &'a EpochBody,
}

/// Zero-copy canonical view for dependency records (MCF).
#[derive(Serialize)]
struct DependencyCanonicalView<'a> {
    metabox: &'a str,
    r#type: &'a str,
    subject: &'a str,
    author: &'a str,
    created_at: &'a DateTime<Utc>,
    id: &'a str,
    body: &'a DependencyBody,
}

// ─── ID generation ──────────────────────────────────────────────────────────

/// Generate a deterministic attestation ID by BLAKE3-hashing the canonical
/// serialization with the `id` field set to the empty string.
pub fn generate_id(attestation: &Attestation) -> String {
    let view = AttestationCanonicalView {
        metabox: &attestation.metabox,
        r#type: "attestation",
        subject: &attestation.subject,
        author: &attestation.author,
        created_at: &attestation.created_at,
        id: "",
        body: &attestation.body,
    };
    let canonical = serde_json::to_string(&view).expect("attestation must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Generate a deterministic epoch ID.
pub fn generate_epoch_id(epoch: &Epoch) -> String {
    let view = EpochCanonicalView {
        metabox: &epoch.metabox,
        r#type: "epoch",
        subject: &epoch.subject,
        author: &epoch.author,
        created_at: &epoch.created_at,
        id: "",
        body: &epoch.body,
    };
    let canonical = serde_json::to_string(&view).expect("epoch must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Generate a deterministic dependency record ID.
pub fn generate_dependency_id(dep: &DependencyRecord) -> String {
    let view = DependencyCanonicalView {
        metabox: &dep.metabox,
        r#type: "dependency",
        subject: &dep.subject,
        author: &dep.author,
        created_at: &dep.created_at,
        id: "",
        body: &dep.body,
    };
    let canonical = serde_json::to_string(&view).expect("dependency must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Generate a deterministic ID for any record type.
pub fn generate_record_id(record: &Record) -> String {
    match record {
        Record::Attestation(a) => generate_id(a),
        Record::Epoch(e) => generate_epoch_id(e),
        Record::Dependency(d) => generate_dependency_id(d),
        Record::Unknown(_) => String::new(),
    }
}

// ─── Validation ─────────────────────────────────────────────────────────────

/// Validate an attestation, returning all validation errors found.
pub fn validate(attestation: &Attestation) -> Vec<String> {
    let mut errors = Vec::new();

    if attestation.metabox != "1" {
        errors.push(format!(
            "unsupported metabox version: {:?}",
            attestation.metabox
        ));
    }
    if attestation.subject.is_empty() {
        errors.push("subject must not be empty".into());
    }
    if attestation.body.summary.is_empty() {
        errors.push("summary must not be empty".into());
    }
    if attestation.author.is_empty() {
        errors.push("author must not be empty".into());
    }
    if attestation.body.score < -100 || attestation.body.score > 100 {
        errors.push(format!(
            "score {} is out of range [-100, 100]",
            attestation.body.score
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

    // Warn about 'epoch' used as a kind (it's now a record type)
    if let Kind::Custom(ref custom) = attestation.body.kind {
        if custom == "epoch" {
            errors.push("'epoch' is a record type, not a kind; use type: \"epoch\" instead".into());
        }

        // Warn about potentially misspelled custom kinds
        let known = [
            "pass",
            "fail",
            "blocker",
            "concern",
            "praise",
            "suggestion",
            "waiver",
        ];
        for k in &known {
            if is_likely_typo(custom, k) {
                errors.push(format!("unknown kind '{}', did you mean '{}'?", custom, k));
                break;
            }
        }
    }

    // Validate span
    if let Some(ref span) = attestation.body.span {
        if span.start.line == 0 {
            errors.push("span.start.line must be >= 1 (1-indexed)".into());
        }
        if let Some(ref end) = span.end
            && end.line == 0
        {
            errors.push("span.end.line must be >= 1 (1-indexed)".into());
        }
        if let Some(col) = span.start.col
            && col == 0
        {
            errors.push("span.start.col must be >= 1 (1-indexed)".into());
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

// ─── Supersession ───────────────────────────────────────────────────────────

/// Check a slice of records for supersession cycles.
/// Returns Err with cycle details if a cycle is found.
pub fn check_supersession_cycles(records: &[Record]) -> crate::Result<()> {
    let id_set: HashSet<&str> = records.iter().map(|r| r.id()).collect();

    for record in records {
        if let Some(target) = record.supersedes() {
            // Walk the chain from this record
            let mut visited = HashSet::new();
            visited.insert(record.id());
            let mut current = target;

            loop {
                if visited.contains(current) {
                    return Err(crate::Error::Cycle {
                        context: "supersession".into(),
                        detail: format!("cycle detected involving record {}", current),
                    });
                }

                // Find the record with this ID
                if !id_set.contains(current) {
                    break; // target not in this file — that's fine
                }

                visited.insert(current);

                // Find next link in chain
                match records.iter().find(|r| r.id() == current) {
                    Some(next) => match next.supersedes() {
                        Some(next_target) => current = next_target,
                        None => break,
                    },
                    None => break,
                }
            }
        }
    }

    Ok(())
}

/// Validate that supersession references target the same subject.
///
/// Returns an error if any cross-subject supersession is found.
pub fn validate_supersession_targets(records: &[Record]) -> crate::Result<()> {
    let by_id: std::collections::HashMap<&str, &Record> =
        records.iter().map(|r| (r.id(), r)).collect();

    for record in records {
        if let Some(target_id) = record.supersedes()
            && let Some(target) = by_id.get(target_id)
            && record.subject() != target.subject()
        {
            return Err(crate::Error::Validation(format!(
                "record {} (subject '{}') supersedes {} (subject '{}') \
                 — cross-subject supersession is not allowed",
                &record.id()[..8.min(record.id().len())],
                record.subject(),
                &target_id[..target_id.len().min(8)],
                target.subject()
            )));
        }
    }
    Ok(())
}

// ─── Finalize ───────────────────────────────────────────────────────────────

/// Clamp a score to the valid range [-100, 100].
pub fn clamp_score(score: i32) -> i32 {
    score.clamp(-100, 100)
}

/// Build an attestation with a generated ID. The `id` field on the input is
/// ignored and replaced with the content-addressed hash.
pub fn finalize(mut attestation: Attestation) -> Attestation {
    attestation.body.score = clamp_score(attestation.body.score);
    attestation.metabox = "1".into();
    attestation.record_type = "attestation".to_string();
    // Normalize span
    if let Some(ref mut span) = attestation.body.span {
        span.normalize();
    }
    attestation.id = String::new(); // clear for hashing
    attestation.id = generate_id(&attestation);
    attestation
}

/// Build an epoch with a generated ID.
pub fn finalize_epoch(mut epoch: Epoch) -> Epoch {
    epoch.body.score = clamp_score(epoch.body.score);
    epoch.metabox = "1".into();
    epoch.record_type = "epoch".to_string();
    if let Some(ref mut span) = epoch.body.span {
        span.normalize();
    }
    epoch.id = String::new();
    epoch.id = generate_epoch_id(&epoch);
    epoch
}

/// Build a record with a generated ID (dispatches by type).
pub fn finalize_record(record: Record) -> Record {
    match record {
        Record::Attestation(a) => Record::Attestation(Box::new(finalize(*a))),
        Record::Epoch(e) => Record::Epoch(finalize_epoch(e)),
        Record::Dependency(mut d) => {
            d.metabox = "1".into();
            d.record_type = "dependency".to_string();
            d.id = String::new();
            d.id = generate_dependency_id(&d);
            Record::Dependency(d)
        }
        other => other,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_attestation() -> Attestation {
        let mut att = Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "src/parser.rs".into(),
            author: "alice@example.com".into(),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                score: -30,
                span: None,
                suggested_fix: None,
                summary: "Panics on malformed input".into(),
                supersedes: None,
                tags: vec![],
            },
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
        att2.body.score = -20;
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
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: String::new(),
            author: String::new(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 0,
                span: None,
                suggested_fix: None,
                summary: String::new(),
                supersedes: None,
                tags: vec![],
            },
        };
        let errors = validate(&att);
        assert!(errors.iter().any(|e| e.contains("subject")));
        assert!(errors.iter().any(|e| e.contains("summary")));
        assert!(errors.iter().any(|e| e.contains("author")));
        assert!(errors.iter().any(|e| e.contains("id")));
    }

    #[test]
    fn test_validate_score_out_of_range() {
        let mut att = sample_attestation();
        att.body.score = 200;
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
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "test".into(),
            author: "bot".into(),
            created_at: Utc::now(),
            id: "will be replaced".into(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 200, // over max
                span: None,
                suggested_fix: None,
                summary: "good".into(),
                supersedes: None,
                tags: vec![],
            },
        };
        let finalized = finalize(att);
        assert_eq!(finalized.body.score, 100); // clamped
        assert_eq!(finalized.metabox, "1");
        assert_eq!(finalized.id, generate_id(&finalized)); // valid ID
    }

    #[test]
    fn test_finalize_normalizes_span() {
        let att = Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "test.rs".into(),
            author: "test".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                score: -10,
                span: Some(Span {
                    start: Position {
                        line: 42,
                        col: None,
                    },
                    end: None,
                }),
                suggested_fix: None,
                summary: "issue".into(),
                supersedes: None,
                tags: vec![],
            },
        };
        let finalized = finalize(att);
        let span = finalized.body.span.unwrap();
        assert_eq!(
            span.end,
            Some(Position {
                line: 42,
                col: None
            })
        );
    }

    #[test]
    fn test_span_changes_id() {
        let now = DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let without_span = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "test".into(),
            created_at: now,
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                score: -10,
                span: None,
                suggested_fix: None,
                summary: "issue".into(),
                supersedes: None,
                tags: vec![],
            },
        });

        let with_span = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "test".into(),
            created_at: now,
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                score: -10,
                span: Some(Span {
                    start: Position {
                        line: 42,
                        col: None,
                    },
                    end: None,
                }),
                suggested_fix: None,
                summary: "issue".into(),
                supersedes: None,
                tags: vec![],
            },
        });

        assert_ne!(without_span.id, with_span.id, "span should affect ID");
    }

    #[test]
    fn test_supersession_cycle_detection() {
        let now = Utc::now();
        let a = Record::Attestation(Box::new(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x".into(),
            author: "test".into(),
            created_at: now,
            id: "aaa".into(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "a".into(),
                supersedes: Some("bbb".into()),
                tags: vec![],
            },
        }));
        let b = Record::Attestation(Box::new(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x".into(),
            author: "test".into(),
            created_at: now,
            id: "bbb".into(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "b".into(),
                supersedes: Some("aaa".into()),
                tags: vec![],
            },
        }));

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
        assert_eq!(parsed.body.kind, att.body.kind);
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
        assert_eq!(Kind::Custom("foo".into()).default_score(), 0);
    }

    #[test]
    fn test_typo_detection_in_validate() {
        let mut att = sample_attestation();
        att.body.kind = Kind::Custom("pss".into());
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
        att.body.kind = Kind::Custom("my_custom_lint".into());
        att.id = generate_id(&att);
        let errors = validate(&att);
        assert!(
            !errors.iter().any(|e| e.contains("did you mean")),
            "unexpected typo suggestion for distant custom kind: {:?}",
            errors
        );
    }

    #[test]
    fn test_cross_subject_supersession_detected() {
        let a = Record::Attestation(Box::new(finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "foo.rs".into(),
            author: "test".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
            },
        })));
        let a_id = a.id().to_string();
        let b = Record::Attestation(Box::new(finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "bar.rs".into(),
            author: "test".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 20,
                span: None,
                suggested_fix: None,
                summary: "updated".into(),
                supersedes: Some(a_id),
                tags: vec![],
            },
        })));
        let result = validate_supersession_targets(&[a, b]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cross-subject"));
    }

    #[test]
    fn test_same_subject_supersession_ok() {
        let a = Record::Attestation(Box::new(finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "foo.rs".into(),
            author: "test".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                score: -10,
                span: None,
                suggested_fix: None,
                summary: "bad".into(),
                supersedes: None,
                tags: vec![],
            },
        })));
        let a_id = a.id().to_string();
        let b = Record::Attestation(Box::new(finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "foo.rs".into(),
            author: "test".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 20,
                span: None,
                suggested_fix: None,
                summary: "fixed".into(),
                supersedes: Some(a_id),
                tags: vec![],
            },
        })));
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

    #[test]
    fn test_metabox_finalize_sets_version() {
        let att = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "test.rs".into(),
            author: "test@test.com".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
            },
        });
        assert_eq!(att.metabox, "1");
        assert_eq!(att.id, generate_id(&att));
    }

    #[test]
    fn test_metabox_id_includes_new_fields() {
        let now = DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let base = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "test@test.com".into(),
            created_at: now,
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
            },
        });

        let with_author_type = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "test@test.com".into(),
            created_at: now,
            id: String::new(),
            body: AttestationBody {
                author_type: Some(AuthorType::Human),
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
            },
        });

        let with_ref = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "test@test.com".into(),
            created_at: now,
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: Some("git:abc123".into()),
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
            },
        });

        assert_eq!(base.metabox, "1");
        assert_ne!(base.id, with_author_type.id, "author_type should affect ID");
        assert_ne!(base.id, with_ref.id, "ref should affect ID");
        assert_ne!(with_author_type.id, with_ref.id);
    }

    #[test]
    fn test_validate_unknown_metabox_version() {
        let mut att = Attestation {
            metabox: "99".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "test@test.com".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
            },
        };
        att.id = generate_id(&att);
        let errors = validate(&att);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("unsupported metabox version")),
            "metabox:99 should fail validation, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_metabox_serde_roundtrip() {
        let att = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "alice@example.com".into(),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AttestationBody {
                author_type: Some(AuthorType::Human),
                detail: None,
                kind: Kind::Praise,
                r#ref: Some("git:3aba500".into()),
                score: 30,
                span: None,
                suggested_fix: None,
                summary: "great".into(),
                supersedes: None,
                tags: vec!["quality".into()],
            },
        });

        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains("\"metabox\":\"1\""));
        assert!(json.contains("\"type\":\"attestation\""));
        assert!(json.contains("\"body\""));
        assert!(json.contains("\"author_type\":\"human\""));
        assert!(json.contains("\"ref\":\"git:3aba500\""));

        let parsed: Attestation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, att);
    }

    #[test]
    fn test_record_serde_roundtrip() {
        let att = finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: "x.rs".into(),
            author: "test".into(),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score: 10,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
            },
        });
        let record = Record::Attestation(Box::new(att.clone()));
        let json = serde_json::to_string(&record).unwrap();
        let parsed: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), att.id);
        assert!(parsed.as_attestation().is_some());
    }

    #[test]
    fn test_record_type_defaults_to_attestation() {
        // JSON without "type" field should parse as attestation
        let json = r#"{"metabox":"1","subject":"x.rs","author":"test","created_at":"2026-02-24T10:00:00Z","id":"abc","body":{"kind":"pass","score":10,"summary":"ok"}}"#;
        let record: Record = serde_json::from_str(json).unwrap();
        assert!(record.as_attestation().is_some());
    }

    #[test]
    fn test_epoch_record_roundtrip() {
        let epoch = finalize_epoch(Epoch {
            metabox: "1".into(),
            record_type: "epoch".into(),
            subject: "x.rs".into(),
            author: "qualifier/compact".into(),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: EpochBody {
                author_type: Some(AuthorType::Tool),
                refs: vec!["aaa".into(), "bbb".into()],
                score: 30,
                span: None,
                summary: "Compacted from 3 records".into(),
            },
        });

        let record = Record::Epoch(epoch.clone());
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"type\":\"epoch\""));

        let parsed: Record = serde_json::from_str(&json).unwrap();
        assert!(parsed.as_epoch().is_some());
        assert_eq!(parsed.as_epoch().unwrap().body.score, 30);
    }

    #[test]
    fn test_unknown_record_type_preserved() {
        let json = r#"{"metabox":"1","type":"custom-thing","subject":"x.rs","author":"test","created_at":"2026-02-24T10:00:00Z","id":"abc","body":{"foo":"bar"}}"#;
        let record: Record = serde_json::from_str(json).unwrap();
        match record {
            Record::Unknown(v) => {
                assert_eq!(v.get("type").unwrap().as_str().unwrap(), "custom-thing");
            }
            _ => panic!("expected Unknown record"),
        }
    }

    #[test]
    fn test_parse_span_line_only() {
        let span = parse_span("42").unwrap();
        assert_eq!(
            span.start,
            Position {
                line: 42,
                col: None
            }
        );
        assert_eq!(span.end, None);
    }

    #[test]
    fn test_parse_span_line_range() {
        let span = parse_span("42:58").unwrap();
        assert_eq!(
            span.start,
            Position {
                line: 42,
                col: None
            }
        );
        assert_eq!(
            span.end,
            Some(Position {
                line: 58,
                col: None
            })
        );
    }

    #[test]
    fn test_parse_span_with_columns() {
        let span = parse_span("42.5:58.80").unwrap();
        assert_eq!(
            span.start,
            Position {
                line: 42,
                col: Some(5)
            }
        );
        assert_eq!(
            span.end,
            Some(Position {
                line: 58,
                col: Some(80)
            })
        );
    }

    #[test]
    fn test_author_type_roundtrip() {
        let types = vec![
            AuthorType::Human,
            AuthorType::Ai,
            AuthorType::Tool,
            AuthorType::Unknown,
        ];
        for at in &types {
            let s = at.to_string();
            let parsed: AuthorType = s.parse().unwrap();
            assert_eq!(&parsed, at);
        }
    }
}
