use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashSet};
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
    /// BLAKE3 hash of the spanned lines (full lines, `\n`-joined).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
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
            Ok(Span {
                start,
                end: None,
                content_hash: None,
            })
        }
        2 => {
            let start = parse_position(parts[0])?;
            let end = parse_position(parts[1])?;
            Ok(Span {
                start,
                end: Some(end),
                content_hash: None,
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

/// Parse a location string into a subject and optional span.
///
/// - `"src/parser.rs"` → `("src/parser.rs", None)`
/// - `"src/parser.rs:42"` → `("src/parser.rs", Some(Span{start: line 42}))`
/// - `"src/parser.rs:15:28"` → `("src/parser.rs", Some(Span{start: line 15, end: line 28}))`
pub fn parse_location(s: &str) -> (String, Option<Span>) {
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    match parts.len() {
        3 => {
            if let (Ok(start), Ok(end)) = (parts[1].parse::<u32>(), parts[0].parse::<u32>()) {
                let subject = parts[2].to_string();
                return (
                    subject,
                    Some(Span {
                        start: Position {
                            line: start,
                            col: None,
                        },
                        end: Some(Position {
                            line: end,
                            col: None,
                        }),
                        content_hash: None,
                    }),
                );
            }
            // Not valid numbers — treat whole thing as subject
            (s.to_string(), None)
        }
        2 => {
            if let Ok(line) = parts[0].parse::<u32>() {
                let subject = parts[1].to_string();
                return (
                    subject,
                    Some(Span {
                        start: Position { line, col: None },
                        end: None,
                        content_hash: None,
                    }),
                );
            }
            // Not a valid number — treat whole thing as subject
            (s.to_string(), None)
        }
        _ => (s.to_string(), None),
    }
}

// ─── Kind enum ──────────────────────────────────────────────────────────────

/// The type of an annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Pass,
    Fail,
    Blocker,
    Concern,
    Comment,
    Resolve,
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
            Kind::Comment => write!(f, "comment"),
            Kind::Resolve => write!(f, "resolve"),
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
            "comment" => Kind::Comment,
            "resolve" => Kind::Resolve,
            "praise" => Kind::Praise,
            "suggestion" => Kind::Suggestion,
            "waiver" => Kind::Waiver,
            other => Kind::Custom(other.to_string()),
        })
    }
}

// ─── IssuerType enum ────────────────────────────────────────────────────────

/// Issuer classification for annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuerType {
    Human,
    Ai,
    Tool,
    Unknown,
}

impl fmt::Display for IssuerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssuerType::Human => write!(f, "human"),
            IssuerType::Ai => write!(f, "ai"),
            IssuerType::Tool => write!(f, "tool"),
            IssuerType::Unknown => write!(f, "unknown"),
        }
    }
}

impl std::str::FromStr for IssuerType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "human" => Ok(IssuerType::Human),
            "ai" => Ok(IssuerType::Ai),
            "tool" => Ok(IssuerType::Tool),
            "unknown" => Ok(IssuerType::Unknown),
            other => Err(format!("unknown issuer_type: '{other}'")),
        }
    }
}

// ─── Body structs ───────────────────────────────────────────────────────────
//
// Bodies serialize with their top-level keys in lexicographic order (MCF),
// merging the defined fields with any custom fields in `extra`. Values keep
// their own serialization, so `span` keeps its start, end, content_hash order.

/// Body fields a record carries beyond the ones its type defines, keyed by
/// name. They are preserved on rewrite and included in the record ID.
pub type ExtraFields = BTreeMap<String, serde_json::Value>;

/// One top-level body value, borrowed for serialization.
enum BodyValue<'a> {
    Str(&'a str),
    Kind(&'a Kind),
    Span(&'a Span),
    List(&'a [String]),
    Json(&'a serde_json::Value),
}

impl Serialize for BodyValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            BodyValue::Str(s) => s.serialize(serializer),
            BodyValue::Kind(k) => k.serialize(serializer),
            BodyValue::Span(s) => s.serialize(serializer),
            BodyValue::List(l) => l.serialize(serializer),
            BodyValue::Json(v) => v.serialize(serializer),
        }
    }
}

/// Serialize a body as a map whose keys are in lexicographic order.
///
/// `fields` holds the defined fields that are present; `names` lists every
/// defined field name, so a custom field can never shadow one.
fn serialize_body<'a, S: Serializer>(
    serializer: S,
    names: &[&str],
    mut fields: Vec<(&'a str, BodyValue<'a>)>,
    extra: &'a ExtraFields,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    fields.extend(
        extra
            .iter()
            .filter(|(k, _)| !names.contains(&k.as_str()))
            .map(|(k, v)| (k.as_str(), BodyValue::Json(v))),
    );
    fields.sort_by(|a, b| a.0.cmp(b.0));
    let mut map = serializer.serialize_map(Some(fields.len()))?;
    for (key, value) in &fields {
        map.serialize_entry(key, value)?;
    }
    map.end()
}

/// Annotation body fields.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AnnotationBody {
    #[serde(default)]
    pub detail: Option<String>,
    pub kind: Kind,
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub references: Option<String>,
    #[serde(default)]
    pub span: Option<Span>,
    #[serde(default)]
    pub suggested_fix: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Custom body fields (see [`ExtraFields`]).
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Serialize for AnnotationBody {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        const NAMES: &[&str] = &[
            "detail",
            "kind",
            "ref",
            "references",
            "span",
            "suggested_fix",
            "summary",
            "supersedes",
            "tags",
        ];
        let mut fields = vec![
            ("kind", BodyValue::Kind(&self.kind)),
            ("summary", BodyValue::Str(&self.summary)),
        ];
        let optional = [
            ("detail", &self.detail),
            ("ref", &self.r#ref),
            ("references", &self.references),
            ("suggested_fix", &self.suggested_fix),
            ("supersedes", &self.supersedes),
        ];
        for (name, value) in optional {
            if let Some(v) = value {
                fields.push((name, BodyValue::Str(v)));
            }
        }
        if let Some(span) = &self.span {
            fields.push(("span", BodyValue::Span(span)));
        }
        if !self.tags.is_empty() {
            fields.push(("tags", BodyValue::List(&self.tags)));
        }
        serialize_body(serializer, NAMES, fields, &self.extra)
    }
}

/// Epoch body fields.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct EpochBody {
    pub refs: Vec<String>,
    #[serde(default)]
    pub span: Option<Span>,
    pub summary: String,
    /// Custom body fields (see [`ExtraFields`]).
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Serialize for EpochBody {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut fields = vec![
            ("refs", BodyValue::List(&self.refs)),
            ("summary", BodyValue::Str(&self.summary)),
        ];
        if let Some(span) = &self.span {
            fields.push(("span", BodyValue::Span(span)));
        }
        serialize_body(
            serializer,
            &["refs", "span", "summary"],
            fields,
            &self.extra,
        )
    }
}

/// Dependency body fields.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DependencyBody {
    pub depends_on: Vec<String>,
    /// Custom body fields (see [`ExtraFields`]).
    #[serde(flatten)]
    pub extra: ExtraFields,
}

impl Serialize for DependencyBody {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let fields = vec![("depends_on", BodyValue::List(&self.depends_on))];
        serialize_body(serializer, &["depends_on"], fields, &self.extra)
    }
}

// ─── Annotation struct ──────────────────────────────────────────────────────

fn default_annotation_type() -> String {
    "annotation".to_string()
}

fn default_metabox() -> String {
    "1".to_string()
}

/// A quality annotation against a software artifact (Metabox envelope).
///
/// **IMPORTANT:** Envelope field order is fixed: metabox, type, subject,
/// issuer, issuer_type, created_at, id, body. Body fields are alphabetical (MCF).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Annotation {
    /// Metabox envelope version. Always "1".
    #[serde(default = "default_metabox")]
    pub metabox: String,

    /// Record type. Always "annotation".
    #[serde(rename = "type", default = "default_annotation_type")]
    pub record_type: String,

    /// Qualified name of the subject (artifact).
    pub subject: String,

    /// Who or what created this annotation (URI).
    pub issuer: String,

    /// Issuer classification (human, ai, tool, unknown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_type: Option<IssuerType>,

    /// When this annotation was created (RFC 3339).
    pub created_at: DateTime<Utc>,

    /// Content-addressed record ID (BLAKE3).
    pub id: String,

    /// Type-specific payload.
    pub body: AnnotationBody,
}

// ─── Epoch struct ───────────────────────────────────────────────────────────

/// An epoch record — a compaction summary that replaces a set of records
/// with a single record summarising their net effect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Epoch {
    #[serde(default = "default_metabox")]
    pub metabox: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub subject: String,
    pub issuer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_type: Option<IssuerType>,
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
    pub issuer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_type: Option<IssuerType>,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub body: DependencyBody,
}

// ─── Record enum ────────────────────────────────────────────────────────────

/// A typed qualifier record. Dispatches on the `type` field in JSON.
#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    Annotation(Box<Annotation>),
    Epoch(Epoch),
    Dependency(DependencyRecord),
    Unknown(serde_json::Value),
}

impl Serialize for Record {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Record::Annotation(a) => a.serialize(serializer),
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
            .unwrap_or("annotation");

        match record_type {
            "annotation" => {
                let att: Annotation =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(Record::Annotation(Box::new(att)))
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
            Record::Annotation(a) => &a.subject,
            Record::Epoch(e) => &e.subject,
            Record::Dependency(d) => &d.subject,
            Record::Unknown(v) => v.get("subject").and_then(|v| v.as_str()).unwrap_or(""),
        }
    }

    /// Get the record ID.
    pub fn id(&self) -> &str {
        match self {
            Record::Annotation(a) => &a.id,
            Record::Epoch(e) => &e.id,
            Record::Dependency(d) => &d.id,
            Record::Unknown(v) => v.get("id").and_then(|v| v.as_str()).unwrap_or(""),
        }
    }

    /// Get the supersedes ID (annotations only).
    pub fn supersedes(&self) -> Option<&str> {
        match self {
            Record::Annotation(a) => a.body.supersedes.as_deref(),
            _ => None,
        }
    }

    /// Get the references ID (annotations only).
    pub fn references(&self) -> Option<&str> {
        match self {
            Record::Annotation(a) => a.body.references.as_deref(),
            _ => None,
        }
    }

    /// Get the kind (annotations only).
    pub fn kind(&self) -> Option<&Kind> {
        match self {
            Record::Annotation(a) => Some(&a.body.kind),
            _ => None,
        }
    }

    /// Try to get this as an annotation.
    pub fn as_annotation(&self) -> Option<&Annotation> {
        match self {
            Record::Annotation(a) => Some(a),
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

    /// Get the issuer type classification.
    pub fn issuer_type(&self) -> Option<&IssuerType> {
        match self {
            Record::Annotation(a) => a.issuer_type.as_ref(),
            Record::Epoch(e) => e.issuer_type.as_ref(),
            Record::Dependency(d) => d.issuer_type.as_ref(),
            Record::Unknown(_) => None,
        }
    }

    /// Get the envelope `type` string (e.g. `"annotation"`, `"epoch"`,
    /// `"dependency"`, or any custom URI for `Unknown` records).
    ///
    /// For `Unknown` records, this reads the `type` field from the raw JSON
    /// value; returns `""` if absent.
    pub fn record_type(&self) -> &str {
        match self {
            Record::Annotation(a) => &a.record_type,
            Record::Epoch(e) => &e.record_type,
            Record::Dependency(d) => &d.record_type,
            Record::Unknown(v) => v.get("type").and_then(|v| v.as_str()).unwrap_or(""),
        }
    }
}

// ─── Canonical views (for ID generation — MCF) ──────────────────────────────

/// Zero-copy canonical view for annotation records (MCF).
/// Envelope fields in fixed order, body handles its own alphabetical ordering.
#[derive(Serialize)]
struct AnnotationCanonicalView<'a> {
    metabox: &'a str,
    r#type: &'a str,
    subject: &'a str,
    issuer: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    issuer_type: Option<&'a IssuerType>,
    created_at: &'a DateTime<Utc>,
    id: &'a str,
    body: &'a AnnotationBody,
}

/// Zero-copy canonical view for epoch records (MCF).
#[derive(Serialize)]
struct EpochCanonicalView<'a> {
    metabox: &'a str,
    r#type: &'a str,
    subject: &'a str,
    issuer: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    issuer_type: Option<&'a IssuerType>,
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
    issuer: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    issuer_type: Option<&'a IssuerType>,
    created_at: &'a DateTime<Utc>,
    id: &'a str,
    body: &'a DependencyBody,
}

// ─── ID generation ──────────────────────────────────────────────────────────

/// Generate a deterministic annotation ID by BLAKE3-hashing the canonical
/// serialization with the `id` field set to the empty string.
pub fn generate_id(annotation: &Annotation) -> String {
    let view = AnnotationCanonicalView {
        metabox: &annotation.metabox,
        r#type: "annotation",
        subject: &annotation.subject,
        issuer: &annotation.issuer,
        issuer_type: annotation.issuer_type.as_ref(),
        created_at: &annotation.created_at,
        id: "",
        body: &annotation.body,
    };
    let canonical = serde_json::to_string(&view).expect("annotation must serialize");
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Generate a deterministic epoch ID.
pub fn generate_epoch_id(epoch: &Epoch) -> String {
    let view = EpochCanonicalView {
        metabox: &epoch.metabox,
        r#type: "epoch",
        subject: &epoch.subject,
        issuer: &epoch.issuer,
        issuer_type: epoch.issuer_type.as_ref(),
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
        issuer: &dep.issuer,
        issuer_type: dep.issuer_type.as_ref(),
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
        Record::Annotation(a) => generate_id(a),
        Record::Epoch(e) => generate_epoch_id(e),
        Record::Dependency(d) => generate_dependency_id(d),
        Record::Unknown(_) => String::new(),
    }
}

// ─── Validation ─────────────────────────────────────────────────────────────

/// Validate an annotation, returning all validation errors found.
pub fn validate(annotation: &Annotation) -> Vec<String> {
    let mut errors = Vec::new();

    if annotation.metabox != "1" {
        errors.push(format!(
            "unsupported metabox version: {:?}",
            annotation.metabox
        ));
    }
    if annotation.subject.is_empty() {
        errors.push("subject must not be empty".into());
    }
    if annotation.body.summary.is_empty() {
        errors.push("summary must not be empty".into());
    }
    if annotation.issuer.is_empty() {
        errors.push("issuer must not be empty".into());
    } else if !annotation.issuer.contains(':') {
        errors.push("issuer must be a URI (e.g. mailto:user@example.com)".into());
    }
    if annotation.id.is_empty() {
        errors.push("id must not be empty".into());
    }

    // Verify content-addressed ID matches
    if !annotation.id.is_empty() {
        let expected = generate_id(annotation);
        if annotation.id != expected {
            errors.push(format!(
                "id mismatch: expected {}, got {}",
                expected, annotation.id
            ));
        }
    }

    // Warn about 'epoch' used as a kind (it's now a record type)
    if let Kind::Custom(ref custom) = annotation.body.kind {
        if custom == "epoch" {
            errors.push("'epoch' is a record type, not a kind; use type: \"epoch\" instead".into());
        }

        // Warn about potentially misspelled custom kinds
        let known = [
            "pass",
            "fail",
            "blocker",
            "concern",
            "comment",
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

    // Self-reference check
    if let Some(ref references) = annotation.body.references
        && !annotation.id.is_empty()
        && references == &annotation.id
    {
        errors.push("references must not point to the record itself".into());
    }

    // Validate span
    if let Some(ref span) = annotation.body.span {
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
        if let Some(ref end) = span.end {
            if end.col == Some(0) {
                errors.push("span.end.col must be >= 1 (1-indexed)".into());
            }
            if end.line < span.start.line {
                errors.push(format!(
                    "span end (line {}) must not precede span start (line {})",
                    end.line, span.start.line
                ));
            } else if end.line == span.start.line
                && let (Some(start_col), Some(end_col)) = (span.start.col, end.col)
                && end_col < start_col
            {
                errors.push(format!(
                    "span end (column {end_col}) must not precede span start (column {start_col}) on line {}",
                    end.line
                ));
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

/// Build an annotation with a generated ID. The `id` field on the input is
/// ignored and replaced with the content-addressed hash.
pub fn finalize(mut annotation: Annotation) -> Annotation {
    annotation.metabox = "1".into();
    annotation.record_type = "annotation".to_string();
    // Normalize span
    if let Some(ref mut span) = annotation.body.span {
        span.normalize();
    }
    annotation.id = String::new(); // clear for hashing
    annotation.id = generate_id(&annotation);
    annotation
}

/// Build an epoch with a generated ID.
pub fn finalize_epoch(mut epoch: Epoch) -> Epoch {
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
        Record::Annotation(a) => Record::Annotation(Box::new(finalize(*a))),
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

    fn sample_annotation() -> Annotation {
        let mut att = Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "src/parser.rs".into(),
            issuer: "mailto:alice@example.com".into(),
            issuer_type: None,
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "Panics on malformed input".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        };
        att.id = generate_id(&att);
        att
    }

    #[test]
    fn test_generate_id_deterministic() {
        let att = sample_annotation();
        let id1 = generate_id(&att);
        let id2 = generate_id(&att);
        assert_eq!(id1, id2);
        assert!(!id1.is_empty());
        assert_eq!(id1.len(), 64); // BLAKE3 hex
    }

    #[test]
    fn test_generate_id_changes_with_content() {
        let att1 = sample_annotation();
        let mut att2 = att1.clone();
        att2.body.summary = "different summary".into();
        att2.id = generate_id(&att2);
        assert_ne!(att1.id, att2.id);
    }

    #[test]
    fn test_validate_valid() {
        let att = sample_annotation();
        let errors = validate(&att);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn test_validate_empty_fields() {
        let att = Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: String::new(),
            issuer: String::new(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: String::new(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        };
        let errors = validate(&att);
        assert!(errors.iter().any(|e| e.contains("subject")));
        assert!(errors.iter().any(|e| e.contains("summary")));
        assert!(errors.iter().any(|e| e.contains("issuer")));
        assert!(errors.iter().any(|e| e.contains("id")));
    }

    #[test]
    fn test_validate_id_mismatch() {
        let mut att = sample_annotation();
        att.id = "deadbeef".repeat(8);
        let errors = validate(&att);
        assert!(errors.iter().any(|e| e.contains("id mismatch")));
    }

    #[test]
    fn test_finalize() {
        let att = Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "test".into(),
            issuer: "mailto:bot@localhost".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: "will be replaced".into(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "good".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        };
        let finalized = finalize(att);
        assert_eq!(finalized.metabox, "1");
        assert_eq!(finalized.id, generate_id(&finalized)); // valid ID
    }

    #[test]
    fn test_finalize_normalizes_span() {
        let att = Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "test.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                references: None,
                span: Some(Span {
                    start: Position {
                        line: 42,
                        col: None,
                    },
                    end: None,
                    content_hash: None,
                }),
                suggested_fix: None,
                summary: "issue".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
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

        let without_span = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "issue".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });

        let with_span = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                references: None,
                span: Some(Span {
                    start: Position {
                        line: 42,
                        col: None,
                    },
                    end: None,
                    content_hash: None,
                }),
                suggested_fix: None,
                summary: "issue".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });

        assert_ne!(without_span.id, with_span.id, "span should affect ID");
    }

    #[test]
    fn test_supersession_cycle_detection() {
        let now = Utc::now();
        let a = Record::Annotation(Box::new(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: "aaa".into(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "a".into(),
                supersedes: Some("bbb".into()),
                tags: vec![],
                extra: Default::default(),
            },
        }));
        let b = Record::Annotation(Box::new(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: "bbb".into(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "b".into(),
                supersedes: Some("aaa".into()),
                tags: vec![],
                extra: Default::default(),
            },
        }));

        assert!(check_supersession_cycles(&[a, b]).is_err());
    }

    fn with_span(start: (u32, Option<u32>), end: (u32, Option<u32>)) -> Annotation {
        let mut att = sample_annotation();
        att.body.span = Some(Span {
            start: Position {
                line: start.0,
                col: start.1,
            },
            end: Some(Position {
                line: end.0,
                col: end.1,
            }),
            content_hash: None,
        });
        att.id = generate_id(&att);
        att
    }

    #[test]
    fn test_validate_rejects_reversed_span() {
        let errors = validate(&with_span((5, None), (2, None)));
        assert!(
            errors.iter().any(|e| e.contains("must not precede")),
            "{errors:?}"
        );
        let errors = validate(&with_span((3, Some(9)), (3, Some(4))));
        assert!(errors.iter().any(|e| e.contains("column 4")), "{errors:?}");
        let errors = validate(&with_span((3, Some(1)), (4, Some(0))));
        assert!(
            errors.iter().any(|e| e.contains("span.end.col")),
            "{errors:?}"
        );
        assert!(validate(&with_span((3, Some(9)), (4, Some(1)))).is_empty());
        assert!(validate(&with_span((3, None), (3, None))).is_empty());
    }

    #[test]
    fn test_kind_roundtrip() {
        let kinds = vec![
            Kind::Pass,
            Kind::Fail,
            Kind::Blocker,
            Kind::Concern,
            Kind::Comment,
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
        let att = sample_annotation();
        let json = serde_json::to_string(&att).unwrap();
        let parsed: Annotation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.body.kind, att.body.kind);
    }

    #[test]
    fn test_custom_kind() {
        let kind: Kind = "my_custom_kind".parse().unwrap();
        assert_eq!(kind, Kind::Custom("my_custom_kind".into()));
        assert_eq!(kind.to_string(), "my_custom_kind");
    }

    #[test]
    fn test_typo_detection_in_validate() {
        let mut att = sample_annotation();
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
        let mut att = sample_annotation();
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
        let a = Record::Annotation(Box::new(finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "foo.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        })));
        let a_id = a.id().to_string();
        let b = Record::Annotation(Box::new(finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "bar.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "updated".into(),
                supersedes: Some(a_id),
                tags: vec![],
                extra: Default::default(),
            },
        })));
        let result = validate_supersession_targets(&[a, b]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cross-subject"));
    }

    #[test]
    fn test_same_subject_supersession_ok() {
        let a = Record::Annotation(Box::new(finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "foo.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Concern,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "bad".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        })));
        let a_id = a.id().to_string();
        let b = Record::Annotation(Box::new(finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "foo.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "fixed".into(),
                supersedes: Some(a_id),
                tags: vec![],
                extra: Default::default(),
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
        let att = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "test.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
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

        let base = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });

        let with_issuer_type = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: Some(IssuerType::Human),
            created_at: now,
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });

        let with_ref = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: Some("git:abc123".into()),
                references: None,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });

        assert_eq!(base.metabox, "1");
        assert_ne!(base.id, with_issuer_type.id, "issuer_type should affect ID");
        assert_ne!(base.id, with_ref.id, "ref should affect ID");
        assert_ne!(with_issuer_type.id, with_ref.id);
    }

    #[test]
    fn test_validate_unknown_metabox_version() {
        let mut att = Annotation {
            metabox: "99".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
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
        let att = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:alice@example.com".into(),
            issuer_type: Some(IssuerType::Human),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Praise,
                r#ref: Some("git:3aba500".into()),
                references: None,
                span: None,
                suggested_fix: None,
                summary: "great".into(),
                supersedes: None,
                tags: vec!["quality".into()],
                extra: Default::default(),
            },
        });

        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains("\"metabox\":\"1\""));
        assert!(json.contains("\"type\":\"annotation\""));
        assert!(json.contains("\"body\""));
        assert!(json.contains("\"issuer_type\":\"human\""));
        assert!(json.contains("\"ref\":\"git:3aba500\""));

        let parsed: Annotation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, att);
    }

    #[test]
    fn test_record_serde_roundtrip() {
        let att = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "ok".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });
        let record = Record::Annotation(Box::new(att.clone()));
        let json = serde_json::to_string(&record).unwrap();
        let parsed: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), att.id);
        assert!(parsed.as_annotation().is_some());
    }

    #[test]
    fn test_record_type_defaults_to_annotation() {
        // JSON without "type" field should parse as annotation
        let json = r#"{"metabox":"1","subject":"x.rs","issuer":"mailto:test@test.com","created_at":"2026-02-24T10:00:00Z","id":"abc","body":{"kind":"pass","summary":"ok"}}"#;
        let record: Record = serde_json::from_str(json).unwrap();
        assert!(record.as_annotation().is_some());
    }

    #[test]
    fn test_epoch_record_roundtrip() {
        let epoch = finalize_epoch(Epoch {
            metabox: "1".into(),
            record_type: "epoch".into(),
            subject: "x.rs".into(),
            issuer: "urn:qualifier:compact".into(),
            issuer_type: Some(IssuerType::Tool),
            created_at: DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: EpochBody {
                refs: vec!["aaa".into(), "bbb".into()],
                span: None,
                summary: "Compacted from 3 records".into(),
                extra: Default::default(),
            },
        });

        let record = Record::Epoch(epoch.clone());
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"type\":\"epoch\""));

        let parsed: Record = serde_json::from_str(&json).unwrap();
        assert!(parsed.as_epoch().is_some());
        assert_eq!(parsed.as_epoch().unwrap().body.refs.len(), 2);
    }

    #[test]
    fn test_unknown_record_type_preserved() {
        let json = r#"{"metabox":"1","type":"custom-thing","subject":"x.rs","issuer":"mailto:test@test.com","created_at":"2026-02-24T10:00:00Z","id":"abc","body":{"foo":"bar"}}"#;
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
    fn test_issuer_type_roundtrip() {
        let types = vec![
            IssuerType::Human,
            IssuerType::Ai,
            IssuerType::Tool,
            IssuerType::Unknown,
        ];
        for at in &types {
            let s = at.to_string();
            let parsed: IssuerType = s.parse().unwrap();
            assert_eq!(&parsed, at);
        }
    }

    #[test]
    fn test_parse_location_no_span() {
        let (subject, span) = parse_location("src/parser.rs");
        assert_eq!(subject, "src/parser.rs");
        assert!(span.is_none());
    }

    #[test]
    fn test_parse_location_single_line() {
        let (subject, span) = parse_location("src/parser.rs:42");
        assert_eq!(subject, "src/parser.rs");
        let span = span.unwrap();
        assert_eq!(span.start.line, 42);
        assert!(span.end.is_none());
    }

    #[test]
    fn test_parse_location_range() {
        let (subject, span) = parse_location("src/parser.rs:15:28");
        assert_eq!(subject, "src/parser.rs");
        let span = span.unwrap();
        assert_eq!(span.start.line, 15);
        assert_eq!(span.end.unwrap().line, 28);
    }

    #[test]
    fn test_parse_location_not_a_number() {
        let (subject, span) = parse_location("src/parser.rs:abc");
        assert_eq!(subject, "src/parser.rs:abc");
        assert!(span.is_none());
    }

    #[test]
    fn test_references_in_canonical_form() {
        let now = DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let without_ref = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Comment,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "note".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });

        let with_ref = finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: now,
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Comment,
                r#ref: None,
                references: Some("deadbeef".into()),
                span: None,
                suggested_fix: None,
                summary: "note".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        });

        assert_ne!(without_ref.id, with_ref.id, "references should affect ID");
    }

    #[test]
    fn test_self_reference_rejected() {
        let mut att = Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: "x.rs".into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: Utc::now(),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind: Kind::Comment,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: "self-ref".into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        };
        att.id = generate_id(&att);
        // Set references to own ID
        att.body.references = Some(att.id.clone());
        // Regenerate ID (references changed the content)
        att.id = generate_id(&att);
        att.body.references = Some(att.id.clone());
        // Now ID and references match
        att.id = generate_id(&att);
        // This is tricky: changing references changes the id, so we need to
        // manually set references == id for the test. Let's use a fixed ID.
        att.id = "abcdef1234567890".into();
        att.body.references = Some("abcdef1234567890".into());

        let errors = validate(&att);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("references must not point to the record itself")),
            "self-reference should be rejected, got: {:?}",
            errors
        );
    }
}
