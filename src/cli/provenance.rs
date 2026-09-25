//! Issuer, issuer-type, and session defaults shared by the write commands.
//!
//! Precedence for each value: explicit flag, then `QUALIFIER_*` variable,
//! then a detected agent harness, then the VCS identity (issuer only).
//! Empty variables count as unset.

use std::sync::OnceLock;

use crate::annotation::IssuerType;

pub const ENV_ISSUER: &str = "QUALIFIER_ISSUER";
pub const ENV_ISSUER_TYPE: &str = "QUALIFIER_ISSUER_TYPE";
pub const ENV_SESSION: &str = "QUALIFIER_SESSION";

/// An agent harness recognizable from the environment it gives tool calls.
struct Harness {
    /// Harness name used in the session tag (`session:<name>:<id>`).
    name: &'static str,
    /// Variable the harness sets in every tool call, and its value.
    marker: (&'static str, &'static str),
    /// Variable holding the harness's session ID.
    session_var: &'static str,
}

const HARNESSES: &[Harness] = &[Harness {
    name: "claude-code",
    marker: ("CLAUDECODE", "1"),
    session_var: "CLAUDE_CODE_SESSION_ID",
}];

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn detected_harness() -> Option<&'static Harness> {
    HARNESSES
        .iter()
        .find(|h| env_nonempty(h.marker.0).as_deref() == Some(h.marker.1))
}

/// The issuer URI for a new record. VCS detection runs at most once per
/// process.
pub fn issuer(explicit: Option<&str>) -> String {
    static DETECTED: OnceLock<String> = OnceLock::new();
    let raw = explicit
        .map(String::from)
        .or_else(|| env_nonempty(ENV_ISSUER))
        .unwrap_or_else(|| DETECTED.get_or_init(detect_issuer).clone());
    normalize_issuer_uri(raw)
}

/// The issuer type for a new record, or `None` when nothing specifies one.
pub fn issuer_type(explicit: Option<&str>) -> crate::Result<Option<IssuerType>> {
    if let Some(s) = explicit {
        return s
            .parse::<IssuerType>()
            .map(Some)
            .map_err(crate::Error::Validation);
    }
    if let Some(s) = env_nonempty(ENV_ISSUER_TYPE) {
        return s
            .parse::<IssuerType>()
            .map(Some)
            .map_err(|e| crate::Error::Validation(format!("${ENV_ISSUER_TYPE}: {e}")));
    }
    Ok(detected_harness().map(|_| IssuerType::Ai))
}

/// The session identifier recorded as the tag `session:<value>`, if any.
pub fn session() -> Option<String> {
    env_nonempty(ENV_SESSION).or_else(|| {
        let harness = detected_harness()?;
        env_nonempty(harness.session_var).map(|id| format!("{}:{id}", harness.name))
    })
}

/// `tags` plus `session:<value>` when a session is known and the tag is
/// not already present.
pub fn with_session_tag(mut tags: Vec<String>) -> Vec<String> {
    if let Some(s) = session() {
        let tag = format!("session:{s}");
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

/// Detect the issuer identity from VCS configuration, falling back to
/// `mailto:$USER@localhost`.
pub fn detect_issuer() -> String {
    let from = |program: &str, args: &[&str]| -> Option<String> {
        std::process::Command::new(program)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .map(|email| format!("mailto:{email}"))
    };
    from("git", &["config", "user.email"])
        .or_else(|| from("hg", &["config", "ui.username"]))
        .unwrap_or_else(|| {
            let user = std::env::var("USER").unwrap_or_else(|_| "unknown".into());
            format!("mailto:{user}@localhost")
        })
}

/// Normalize an issuer value to a URI. Bare emails get `mailto:` prefix;
/// values already containing `:` are assumed to be valid URIs.
pub fn normalize_issuer_uri(issuer: String) -> String {
    if issuer.contains(':') {
        issuer
    } else {
        format!("mailto:{issuer}")
    }
}
