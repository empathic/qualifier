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

// ─── Attestation struct ─────────────────────────────────────────────────────

fn default_attestation_type() -> String {
    "attestation".to_string()
}

/// A quality attestation against a software artifact (v3).
///
/// **IMPORTANT:** Field order is canonical — `serde` serializes in declaration
/// order, and this determines record IDs. Do not reorder fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attestation {
    /// Format version. Always 3.
    #[serde(default)]
    pub v: u8,

    /// Record type. Always "attestation".
    #[serde(rename = "type", default = "default_attestation_type")]
    pub record_type: String,

    /// Qualified name of the artifact.
    pub artifact: String,

    /// Sub-artifact range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,

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

    /// Author classification: human, ai, tool, or unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_type: Option<AuthorType>,

    /// When this attestation was created (RFC 3339).
    pub created_at: DateTime<Utc>,

    /// VCS reference pin (e.g. "git:3aba500"). Opaque to qualifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,

    /// ID of a prior attestation this replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,

    /// Content-addressed record ID (BLAKE3).
    pub id: String,
}

// ─── Epoch struct ───────────────────────────────────────────────────────────

/// An epoch record — a compaction summary that replaces a set of records
/// with a single scored record preserving the net score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Epoch {
    #[serde(default)]
    pub v: u8,
    #[serde(rename = "type")]
    pub record_type: String,
    pub artifact: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    pub score: i32,
    pub summary: String,
    pub refs: Vec<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_type: Option<AuthorType>,
    pub created_at: DateTime<Utc>,
    pub id: String,
}

// ─── DependencyRecord struct ────────────────────────────────────────────────

/// A dependency record declaring edges between artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyRecord {
    #[serde(default)]
    pub v: u8,
    #[serde(rename = "type")]
    pub record_type: String,
    pub artifact: String,
    pub depends_on: Vec<String>,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub id: String,
}

// ─── Record enum ────────────────────────────────────────────────────────────

/// A typed qualifier record. Dispatches on the `type` field in JSON.
#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    Attestation(Attestation),
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
                Ok(Record::Attestation(att))
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
    /// Get the artifact name.
    pub fn artifact(&self) -> &str {
        match self {
            Record::Attestation(a) => &a.artifact,
            Record::Epoch(e) => &e.artifact,
            Record::Dependency(d) => &d.artifact,
            Record::Unknown(v) => v.get("artifact").and_then(|v| v.as_str()).unwrap_or(""),
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
            Record::Attestation(a) => Some(a.score),
            Record::Epoch(e) => Some(e.score),
            _ => None,
        }
    }

    /// Get the supersedes ID (attestations only).
    pub fn supersedes(&self) -> Option<&str> {
        match self {
            Record::Attestation(a) => a.supersedes.as_deref(),
            _ => None,
        }
    }

    /// Get the kind (attestations only).
    pub fn kind(&self) -> Option<&Kind> {
        match self {
            Record::Attestation(a) => Some(&a.kind),
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

fn slice_is_empty(s: &[String]) -> bool {
    s.is_empty()
}

// ─── Canonical views (for ID generation) ────────────────────────────────────

/// Zero-copy canonical view for attestation records.
/// Field order MUST match the v3 spec: v, type, artifact, span, kind, score,
/// summary, detail, suggested_fix, tags, author, author_type, created_at,
/// ref, supersedes, id.
#[derive(Serialize)]
struct AttestationCanonicalView<'a> {
    v: u8,
    r#type: &'a str,
    artifact: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: &'a Option<Span>,
    kind: &'a Kind,
    score: i32,
    summary: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_fix: &'a Option<String>,
    #[serde(skip_serializing_if = "slice_is_empty")]
    tags: &'a [String],
    author: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    author_type: &'a Option<AuthorType>,
    created_at: &'a DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#ref: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supersedes: &'a Option<String>,
    id: &'a str,
}

/// Zero-copy canonical view for epoch records.
/// Field order: v, type, artifact, span, score, summary, refs,
/// author, author_type, created_at, id.
#[derive(Serialize)]
struct EpochCanonicalView<'a> {
    v: u8,
    r#type: &'a str,
    artifact: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: &'a Option<Span>,
    score: i32,
    summary: &'a str,
    refs: &'a [String],
    author: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    author_type: &'a Option<AuthorType>,
    created_at: &'a DateTime<Utc>,
    id: &'a str,
}

/// Zero-copy canonical view for dependency records.
/// Field order: v, type, artifact, depends_on, author, created_at, id.
#[derive(Serialize)]
struct DependencyCanonicalView<'a> {
    v: u8,
    r#type: &'a str,
    artifact: &'a str,
    depends_on: &'a [String],
    author: &'a str,
    created_at: &'a DateTime<Utc>,
    id: &'a str,
}

// ─── ID generation ──────────────────────────────────────────────────────────

/// Generate a deterministic attestation ID by BLAKE3-hashing the canonical
/// serialization with the `id` field set to the empty string.
pub fn generate_id(attestation: &Attestation) -> String {
    let view = AttestationCanonicalView {
        v: attestation.v,
        r#type: "attestation",
        artifact: &attestation.artifact,
        span: &attestation.span,
        kind: &attestation.kind,
        score: attestation.score,
        summary: &attestation.summary,
        detail: &attestation.detail,
        suggested_fix: &attestation.suggested_fix,
        tags: &attestation.tags,
        author: &attestation.author,
        author_type: &attestation.author_type,
        created_at: &attestation.created_at,
        r#ref: &attestation.r#ref,
        supersedes: &attestation.supersedes,
        id: "",
    };
    let canonical = serde_json::to_string(&view).expect("attestation must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Generate a deterministic epoch ID.
pub fn generate_epoch_id(epoch: &Epoch) -> String {
    let view = EpochCanonicalView {
        v: epoch.v,
        r#type: "epoch",
        artifact: &epoch.artifact,
        span: &epoch.span,
        score: epoch.score,
        summary: &epoch.summary,
        refs: &epoch.refs,
        author: &epoch.author,
        author_type: &epoch.author_type,
        created_at: &epoch.created_at,
        id: "",
    };
    let canonical = serde_json::to_string(&view).expect("epoch must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Generate a deterministic dependency record ID.
pub fn generate_dependency_id(dep: &DependencyRecord) -> String {
    let view = DependencyCanonicalView {
        v: dep.v,
        r#type: "dependency",
        artifact: &dep.artifact,
        depends_on: &dep.depends_on,
        author: &dep.author,
        created_at: &dep.created_at,
        id: "",
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

    if attestation.v != 3 {
        errors.push(format!("unsupported format version: {}", attestation.v));
    }
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

    // Warn about 'epoch' used as a kind (it's now a record type)
    if let Kind::Custom(ref custom) = attestation.kind {
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
    if let Some(ref span) = attestation.span {
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

/// Validate that supersession references target the same artifact.
///
/// Returns an error if any cross-artifact supersession is found.
pub fn validate_supersession_targets(records: &[Record]) -> crate::Result<()> {
    let by_id: std::collections::HashMap<&str, &Record> =
        records.iter().map(|r| (r.id(), r)).collect();

    for record in records {
        if let Some(target_id) = record.supersedes()
            && let Some(target) = by_id.get(target_id)
            && record.artifact() != target.artifact()
        {
            return Err(crate::Error::Validation(format!(
                "record {} (artifact '{}') supersedes {} (artifact '{}') \
                 — cross-artifact supersession is not allowed",
                &record.id()[..8.min(record.id().len())],
                record.artifact(),
                &target_id[..target_id.len().min(8)],
                target.artifact()
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
    attestation.score = clamp_score(attestation.score);
    attestation.v = 3;
    attestation.record_type = "attestation".to_string();
    // Normalize span
    if let Some(ref mut span) = attestation.span {
        span.normalize();
    }
    attestation.id = String::new(); // clear for hashing
    attestation.id = generate_id(&attestation);
    attestation
}

/// Build an epoch with a generated ID.
pub fn finalize_epoch(mut epoch: Epoch) -> Epoch {
    epoch.score = clamp_score(epoch.score);
    epoch.v = 3;
    epoch.record_type = "epoch".to_string();
    if let Some(ref mut span) = epoch.span {
        span.normalize();
    }
    epoch.id = String::new();
    epoch.id = generate_epoch_id(&epoch);
    epoch
}

/// Build a record with a generated ID (dispatches by type).
pub fn finalize_record(record: Record) -> Record {
    match record {
        Record::Attestation(a) => Record::Attestation(finalize(a)),
        Record::Epoch(e) => Record::Epoch(finalize_epoch(e)),
        Record::Dependency(mut d) => {
            d.v = 3;
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
            v: 3,
            record_type: "attestation".into(),
            artifact: "src/parser.rs".into(),
            span: None,
            kind: Kind::Concern,
            score: -30,
            summary: "Panics on malformed input".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "alice@example.com".into(),
            author_type: None,
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            r#ref: None,
            supersedes: None,
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
            v: 3,
            record_type: "attestation".into(),
            artifact: String::new(),
            span: None,
            kind: Kind::Pass,
            score: 0,
            summary: String::new(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: String::new(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: None,
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
            v: 3,
            record_type: "attestation".into(),
            artifact: "test".into(),
            span: None,
            kind: Kind::Pass,
            score: 200, // over max
            summary: "good".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "bot".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: None,
            id: "will be replaced".into(),
        };
        let finalized = finalize(att);
        assert_eq!(finalized.score, 100); // clamped
        assert_eq!(finalized.v, 3);
        assert_eq!(finalized.id, generate_id(&finalized)); // valid ID
    }

    #[test]
    fn test_finalize_normalizes_span() {
        let att = Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "test.rs".into(),
            span: Some(Span {
                start: Position {
                    line: 42,
                    col: None,
                },
                end: None,
            }),
            kind: Kind::Concern,
            score: -10,
            summary: "issue".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: None,
            id: String::new(),
        };
        let finalized = finalize(att);
        let span = finalized.span.unwrap();
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
            v: 3,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: None,
            kind: Kind::Concern,
            score: -10,
            summary: "issue".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: now,
            r#ref: None,
            supersedes: None,
            id: String::new(),
        });

        let with_span = finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: Some(Span {
                start: Position {
                    line: 42,
                    col: None,
                },
                end: None,
            }),
            kind: Kind::Concern,
            score: -10,
            summary: "issue".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: now,
            r#ref: None,
            supersedes: None,
            id: String::new(),
        });

        assert_ne!(without_span.id, with_span.id, "span should affect ID");
    }

    #[test]
    fn test_supersession_cycle_detection() {
        let now = Utc::now();
        let a = Record::Attestation(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "a".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: now,
            r#ref: None,
            supersedes: Some("bbb".into()),
            id: "aaa".into(),
        });
        let b = Record::Attestation(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "b".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: now,
            r#ref: None,
            supersedes: Some("aaa".into()),
            id: "bbb".into(),
        });

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
        let a = Record::Attestation(finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "foo.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: None,
            id: String::new(),
        }));
        let a_id = a.id().to_string();
        let b = Record::Attestation(finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "bar.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 20,
            summary: "updated".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: Some(a_id),
            id: String::new(),
        }));
        let result = validate_supersession_targets(&[a, b]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cross-artifact"));
    }

    #[test]
    fn test_same_artifact_supersession_ok() {
        let a = Record::Attestation(finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "foo.rs".into(),
            span: None,
            kind: Kind::Concern,
            score: -10,
            summary: "bad".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: None,
            id: String::new(),
        }));
        let a_id = a.id().to_string();
        let b = Record::Attestation(finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "foo.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 20,
            summary: "fixed".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: Some(a_id),
            id: String::new(),
        }));
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
    fn test_v3_finalize_sets_version() {
        let att = finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "test.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: None,
            id: String::new(),
        });
        assert_eq!(att.v, 3);
        assert_eq!(att.id, generate_id(&att));
    }

    #[test]
    fn test_v3_id_includes_new_fields() {
        let now = DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let base = finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: None,
            created_at: now,
            r#ref: None,
            supersedes: None,
            id: String::new(),
        });

        // Same content but with author_type set
        let with_author_type = finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: Some(AuthorType::Human),
            created_at: now,
            r#ref: None,
            supersedes: None,
            id: String::new(),
        });

        // Same content but with ref set
        let with_ref = finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: None,
            created_at: now,
            r#ref: Some("git:abc123".into()),
            supersedes: None,
            id: String::new(),
        });

        assert_eq!(base.v, 3);
        assert_ne!(base.id, with_author_type.id, "author_type should affect ID");
        assert_ne!(base.id, with_ref.id, "ref should affect ID");
        assert_ne!(with_author_type.id, with_ref.id);
    }

    #[test]
    fn test_validate_unknown_version() {
        let mut att = Attestation {
            v: 2,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: None,
            created_at: Utc::now(),
            r#ref: None,
            supersedes: None,
            id: String::new(),
        };
        att.id = generate_id(&att);
        let errors = validate(&att);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("unsupported format version")),
            "v:2 should fail validation, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_v3_serde_roundtrip() {
        let att = finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: None,
            kind: Kind::Praise,
            score: 30,
            summary: "great".into(),
            detail: None,
            suggested_fix: None,
            tags: vec!["quality".into()],
            author: "alice@example.com".into(),
            author_type: Some(AuthorType::Human),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            r#ref: Some("git:3aba500".into()),
            supersedes: None,
            id: String::new(),
        });

        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains("\"v\":3"));
        assert!(json.contains("\"type\":\"attestation\""));
        assert!(json.contains("\"author_type\":\"human\""));
        assert!(json.contains("\"ref\":\"git:3aba500\""));

        let parsed: Attestation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, att);
    }

    #[test]
    fn test_record_serde_roundtrip() {
        let att = finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: "x.rs".into(),
            span: None,
            kind: Kind::Pass,
            score: 10,
            summary: "ok".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test".into(),
            author_type: None,
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            r#ref: None,
            supersedes: None,
            id: String::new(),
        });
        let record = Record::Attestation(att.clone());
        let json = serde_json::to_string(&record).unwrap();
        let parsed: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), att.id);
        assert!(parsed.as_attestation().is_some());
    }

    #[test]
    fn test_record_type_defaults_to_attestation() {
        // JSON without "type" field should parse as attestation
        let json = r#"{"v":3,"artifact":"x.rs","kind":"pass","score":10,"summary":"ok","author":"test","created_at":"2026-02-24T10:00:00Z","id":"abc"}"#;
        let record: Record = serde_json::from_str(json).unwrap();
        assert!(record.as_attestation().is_some());
    }

    #[test]
    fn test_epoch_record_roundtrip() {
        let epoch = finalize_epoch(Epoch {
            v: 3,
            record_type: "epoch".into(),
            artifact: "x.rs".into(),
            span: None,
            score: 30,
            summary: "Compacted from 3 records".into(),
            refs: vec!["aaa".into(), "bbb".into()],
            author: "qualifier/compact".into(),
            author_type: Some(AuthorType::Tool),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
        });

        let record = Record::Epoch(epoch.clone());
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"type\":\"epoch\""));

        let parsed: Record = serde_json::from_str(&json).unwrap();
        assert!(parsed.as_epoch().is_some());
        assert_eq!(parsed.as_epoch().unwrap().score, 30);
    }

    #[test]
    fn test_unknown_record_type_preserved() {
        let json = r#"{"v":3,"type":"custom-thing","artifact":"x.rs","foo":"bar","author":"test","created_at":"2026-02-24T10:00:00Z","id":"abc"}"#;
        let record: Record = serde_json::from_str(json).unwrap();
        match record {
            Record::Unknown(v) => {
                assert_eq!(v.get("type").unwrap().as_str().unwrap(), "custom-thing");
                assert_eq!(v.get("foo").unwrap().as_str().unwrap(), "bar");
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
