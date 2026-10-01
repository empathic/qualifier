//! Thread assembly: group annotations into conversations.
//!
//! A thread starts at an *origin* annotation. Records join it by
//! `references` (replies) or by `supersedes` (edits and resolutions of a
//! record already in the thread). The *root chain* is the origin plus the
//! non-reply records that supersede it in turn. A chain member no other
//! chain member supersedes is a *tip* (a chain can fork into more than
//! one). If any tip is not a `resolve`, the thread is open and its root is
//! the newest such tip; otherwise it is closed by the newest resolve tip,
//! and the root is whatever non-resolve chain member that resolve targets,
//! falling back to the newest non-resolve chain member.
//!
//! [`Thread::state`] gives every read command one answer to "where does
//! this thread stand" ([`ThreadState`]), and [`ThreadRenderer`] is the one
//! human rendering of a thread, so no command prints a record without its
//! thread's state.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};

use crate::annotation::{IssuerType, Kind, Record};
use crate::compact::filter_superseded;

/// One conversation: a root annotation, its edits, replies, and closure.
#[derive(Debug)]
pub struct Thread<'a> {
    /// ID of the oldest record in the root chain. Stable across edits.
    pub origin: &'a str,
    /// The live head of the root chain. While open, the newest root-chain
    /// tip that is not a `resolve`. Once closed, `closed_by`'s
    /// `supersedes` target when that target is a non-`resolve` chain
    /// member, else the newest non-`resolve` chain member.
    pub root: &'a Record,
    /// The `resolve` record that closed the thread, if any.
    pub closed_by: Option<&'a Record>,
    /// Records outside the root chain (replies and anything hanging from
    /// them), oldest first.
    pub replies: Vec<ThreadEntry<'a>>,
    /// Root-chain members other than `root` and `closed_by`, oldest first.
    pub history: Vec<&'a Record>,
    /// True while any root-chain tip (a chain member no other chain
    /// member supersedes) is not a `resolve`.
    pub open: bool,
    /// Newest `created_at` across every record in the thread.
    pub latest_at: DateTime<Utc>,
}

/// A record in a thread outside the root chain.
#[derive(Debug)]
pub struct ThreadEntry<'a> {
    pub record: &'a Record,
    /// False when another record supersedes this one.
    pub active: bool,
}

/// Group annotation records into threads. Non-annotation records (epochs,
/// dependencies, unknown types) are ignored. Threads are ordered by root
/// subject, then root span start line, then origin ID.
pub fn build_threads(records: &[Record]) -> Vec<Thread<'_>> {
    let anns: Vec<&Record> = records
        .iter()
        .filter(|r| r.as_annotation().is_some())
        .collect();
    let by_id: HashMap<&str, &Record> = anns.iter().map(|r| (r.id(), *r)).collect();
    let active: HashSet<&str> = filter_superseded(records)
        .into_iter()
        .map(|r| r.id())
        .collect();

    let mut groups: HashMap<&str, Vec<&Record>> = HashMap::new();
    for r in &anns {
        groups.entry(origin_of(r, &by_id)).or_default().push(r);
    }

    let mut threads: Vec<Thread<'_>> = groups
        .into_iter()
        .map(|(origin, mut members)| {
            members.sort_by(|a, b| {
                created_at(a)
                    .cmp(&created_at(b))
                    .then_with(|| a.id().cmp(b.id()))
            });
            assemble(origin, members, &by_id, &active)
        })
        .collect();

    threads.sort_by(|a, b| {
        a.root
            .subject()
            .cmp(b.root.subject())
            .then_with(|| span_line(a.root).cmp(&span_line(b.root)))
            .then_with(|| a.origin.cmp(b.origin))
    });
    threads
}

fn assemble<'a>(
    origin: &'a str,
    members: Vec<&'a Record>,
    by_id: &HashMap<&'a str, &'a Record>,
    active: &HashSet<&'a str>,
) -> Thread<'a> {
    // Root chain: origin, then non-reply records superseding a chain member.
    let mut chain_ids: HashSet<&str> = HashSet::from([origin]);
    let mut frontier = vec![origin];
    while let Some(id) = frontier.pop() {
        for r in &members {
            if !is_reply(r, by_id) && r.supersedes() == Some(id) && chain_ids.insert(r.id()) {
                frontier.push(r.id());
            }
        }
    }

    let chain: Vec<&Record> = members
        .iter()
        .copied()
        .filter(|r| chain_ids.contains(r.id()))
        .collect();
    let replies: Vec<ThreadEntry<'_>> = members
        .iter()
        .copied()
        .filter(|r| !chain_ids.contains(r.id()))
        .map(|record| ThreadEntry {
            record,
            active: active.contains(record.id()),
        })
        .collect();

    let is_resolve = |r: &Record| r.kind() == Some(&Kind::Resolve);

    // A tip is a chain member no other chain member supersedes. `chain` is
    // sorted oldest first, so among tips, the last matching one is newest.
    let tips: Vec<&Record> = chain
        .iter()
        .copied()
        .filter(|r| {
            !chain
                .iter()
                .any(|other| other.id() != r.id() && other.supersedes() == Some(r.id()))
        })
        .collect();

    let (root, closed_by, open) = match tips.iter().copied().rfind(|r| !is_resolve(r)) {
        Some(root) => (root, None, true),
        None => {
            // Every tip is a resolve: the thread is closed by the newest one.
            let closer = tips.last().copied().unwrap_or(chain[0]);
            let target = closer
                .supersedes()
                .and_then(|id| by_id.get(id).copied())
                .filter(|r| chain_ids.contains(r.id()) && !is_resolve(r));
            let root = target.unwrap_or_else(|| {
                chain
                    .iter()
                    .copied()
                    .rfind(|r| !is_resolve(r))
                    .unwrap_or(chain[0])
            });
            (root, Some(closer), false)
        }
    };

    let history: Vec<&Record> = chain
        .iter()
        .copied()
        .filter(|r| r.id() != root.id() && closed_by.map(|c| c.id()) != Some(r.id()))
        .collect();

    let latest_at = members
        .iter()
        .map(|r| created_at(r))
        .max()
        .unwrap_or(DateTime::<Utc>::MIN_UTC);

    Thread {
        origin,
        root,
        closed_by,
        replies,
        history,
        open,
        latest_at,
    }
}

/// A reply names an existing record in `references`.
fn is_reply(r: &Record, by_id: &HashMap<&str, &Record>) -> bool {
    r.references().is_some_and(|id| by_id.contains_key(id))
}

/// The record `r` hangs from: its `references` target, else its
/// `supersedes` target, when that record exists.
fn parent_of<'a>(r: &'a Record, by_id: &HashMap<&'a str, &'a Record>) -> Option<&'a str> {
    r.references()
        .filter(|id| by_id.contains_key(id))
        .or_else(|| r.supersedes().filter(|id| by_id.contains_key(id)))
}

fn origin_of<'a>(r: &'a Record, by_id: &HashMap<&'a str, &'a Record>) -> &'a str {
    let mut current = r;
    let mut seen: HashSet<&str> = HashSet::new();
    while let Some(pid) = parent_of(current, by_id) {
        if !seen.insert(current.id()) {
            break;
        }
        current = by_id[pid];
    }
    current.id()
}

fn created_at(r: &Record) -> DateTime<Utc> {
    r.as_annotation()
        .map(|a| a.created_at)
        .unwrap_or(DateTime::<Utc>::MIN_UTC)
}

fn span_line(r: &Record) -> u32 {
    r.as_annotation()
        .and_then(|a| a.body.span.as_ref())
        .map_or(0, |s| s.start.line)
}

// ─── Thread state ───────────────────────────────────────────────────────────

/// Where a thread stands. Every read command derives a thread's state from
/// [`Thread::state`] so they agree on what is open, waiting, or settled.
#[derive(Debug, Clone, Copy)]
pub enum ThreadState<'a> {
    /// Open, with no `status:*` tag, or a latest one of `status:deferred`.
    Open,
    /// Open and waiting on a human judgment call: the latest `status:*`
    /// tag is `status:needs-decision`, optionally addressed with
    /// `status:needs-decision:<issuer>`.
    NeedsDecision { addressee: Option<&'a str> },
    /// Open, with a latest `status:*` tag of `status:decided`: the call has
    /// been made and the thread awaits the change that carries it out.
    Decided,
    /// Closed by the `resolve` record `closer`.
    Closed {
        /// The closer's `reason:*` tag value (`fixed`, `wontfix`, ...).
        reason: Option<&'a str>,
        closer: &'a Record,
        /// The thread closed while its latest `status:*` tag (over the
        /// root, live replies, and the closer) was still
        /// `status:needs-decision`: a question was left unanswered.
        pending_question: bool,
    },
}

impl ThreadState<'_> {
    /// Stable machine name: `open`, `needs-decision`, `decided`, `closed`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::NeedsDecision { .. } => "needs-decision",
            Self::Decided => "decided",
            Self::Closed { .. } => "closed",
        }
    }

    /// JSON form: `{"name": ...}`, plus `addressee` for `needs-decision`
    /// and `reason`, `closed_by` (the closer's ID), and `pending_question`
    /// for `closed`.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Open | Self::Decided => serde_json::json!({ "name": self.name() }),
            Self::NeedsDecision { addressee } => {
                serde_json::json!({ "name": self.name(), "addressee": addressee })
            }
            Self::Closed {
                reason,
                closer,
                pending_question,
            } => serde_json::json!({
                "name": self.name(),
                "reason": reason,
                "closed_by": closer.id(),
                "pending_question": pending_question,
            }),
        }
    }
}

impl<'a> Thread<'a> {
    /// Every record in the thread: root, history, replies (as currently
    /// held), and the closing resolve.
    pub fn records(&self) -> impl Iterator<Item = &'a Record> + '_ {
        std::iter::once(self.root)
            .chain(self.history.iter().copied())
            .chain(self.replies.iter().map(|e| e.record))
            .chain(self.closed_by)
    }

    /// The records that make up the thread as it stands now: the root,
    /// live replies, and the closing resolve. Edit history and superseded
    /// replies are left out.
    pub fn live_records(&self) -> impl Iterator<Item = &'a Record> + '_ {
        std::iter::once(self.root)
            .chain(self.replies.iter().filter(|e| e.active).map(|e| e.record))
            .chain(self.closed_by)
    }

    /// The newest `status:*` tag (by `created_at`) on the root, a live
    /// reply, or the closing resolve, split into its value and optional
    /// addressee: `status:needs-decision:mailto:a@b` gives
    /// `("needs-decision", Some("mailto:a@b"))`.
    pub fn latest_status(&self) -> Option<(&'a str, Option<&'a str>)> {
        self.live_records()
            .filter_map(|r| r.as_annotation())
            .filter_map(|a| {
                a.body
                    .tags
                    .iter()
                    .find_map(|tag| tag.strip_prefix("status:"))
                    .map(|s| (a.created_at, s))
            })
            .max_by_key(|(at, _)| *at)
            .map(|(_, s)| match s.split_once(':') {
                Some((value, addressee)) => (value, Some(addressee)),
                None => (s, None),
            })
    }

    /// The thread without its superseded replies (the default, non-`--all`
    /// view of every read command).
    pub fn without_superseded_replies(self) -> Self {
        Thread {
            replies: self.replies.into_iter().filter(|e| e.active).collect(),
            ..self
        }
    }

    /// The thread's current state.
    pub fn state(&self) -> ThreadState<'a> {
        let status = self.latest_status();
        if let Some(closer) = self.closed_by.filter(|_| !self.open) {
            let reason = closer.as_annotation().and_then(|a| {
                a.body
                    .tags
                    .iter()
                    .find_map(|tag| tag.strip_prefix("reason:"))
            });
            return ThreadState::Closed {
                reason,
                closer,
                pending_question: status.is_some_and(|(s, _)| s == "needs-decision"),
            };
        }
        match status {
            Some(("needs-decision", addressee)) => ThreadState::NeedsDecision { addressee },
            Some(("decided", _)) => ThreadState::Decided,
            _ => ThreadState::Open,
        }
    }
}

/// The threads with any record on `subject` (a thread rooted elsewhere is
/// included when one of its replies is on `subject`): open threads first,
/// each group ordered by root subject, root span start line, then when the
/// thread began. Unless `all`, superseded replies are dropped.
pub fn threads_touching<'a>(records: &'a [Record], subject: &str, all: bool) -> Vec<Thread<'a>> {
    let mut selected: Vec<Thread<'a>> = build_threads(records)
        .into_iter()
        .filter(|t| t.records().any(|r| r.subject() == subject))
        .map(|t| {
            if all {
                t
            } else {
                t.without_superseded_replies()
            }
        })
        .collect();
    let began = |t: &Thread<'a>| {
        t.records()
            .find(|r| r.id() == t.origin)
            .map_or(DateTime::<Utc>::MIN_UTC, created_at)
    };
    selected.sort_by(|a, b| {
        b.open
            .cmp(&a.open)
            .then_with(|| a.root.subject().cmp(b.root.subject()))
            .then_with(|| span_line(a.root).cmp(&span_line(b.root)))
            .then_with(|| began(a).cmp(&began(b)))
            .then_with(|| a.origin.cmp(b.origin))
    });
    selected
}

// ─── Rendering ──────────────────────────────────────────────────────────────

/// The first eight characters of a record ID.
pub fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

/// A short display name for an issuer URI: `mailto:alice@example.com`
/// becomes `alice`; any other URI is returned unchanged.
pub fn short_issuer(issuer: &str) -> &str {
    issuer
        .strip_prefix("mailto:")
        .and_then(|e| e.split('@').next())
        .unwrap_or(issuer)
}

/// The one human renderer for threads, shared by `threads`, `show`, and
/// `praise`. It never prints a record without its thread's state:
///
/// - An open thread prints its root line, carrying the state when it is
///   waiting on or has reached a decision, then one indented line per
///   live reply.
/// - A closed thread prints one line that carries its answer:
///   `[31ef1c78] concern    lib.rs:1  a() rounds wrong — closed (wontfix): Won't change b()`,
///   ending in `— question still pending` when it closed on an unanswered
///   `status:needs-decision`. Truncating output with `head`/`tail` keeps
///   each closed thread's outcome. With [`ThreadRenderer::expand_closed`],
///   its replies and the closing resolve follow that line.
pub struct ThreadRenderer<'f> {
    /// Also print edit history and superseded replies, marked
    /// `(superseded)` (the `--all` view).
    pub all: bool,
    /// Print the replies and closing resolve of closed threads (used when a
    /// thread is asked for by ID).
    pub expand_closed: bool,
    /// Append `(issuer, date)` to each record line, with the issuer type
    /// between them when it is not `human`, and name who closed a closed
    /// thread.
    pub attribution: bool,
    /// Extra lines printed under a record (span context, detail), indented
    /// one step further than the record. Not called for the one-line form
    /// of a closed thread.
    pub continuation: Option<&'f Continuation<'f>>,
}

/// Produces the extra lines printed under one record; see
/// [`ThreadRenderer::continuation`].
pub type Continuation<'f> = dyn Fn(&Record) -> Vec<String> + 'f;

const REPLY_INDENT: &str = "    ";

impl ThreadRenderer<'_> {
    /// Render `t` as lines without trailing newlines. The root line starts
    /// at column 0; replies are indented by four spaces.
    pub fn render(&self, t: &Thread<'_>) -> Vec<String> {
        let state = t.state();
        let mut out = vec![format!(
            "[{}] {:<10} {}  {}{}{}",
            short_id(t.root.id()),
            kind_label(t.root),
            location(t.root),
            summary(t.root),
            self.attribution_suffix(t.root),
            self.state_suffix(&state),
        )];
        let closed = matches!(state, ThreadState::Closed { .. });
        if closed && !self.expand_closed {
            return out;
        }
        self.push_continuation(&mut out, t.root, "");
        if self.all {
            for r in &t.history {
                out.push(format!(
                    "{REPLY_INDENT}[{}] {:<10} {}{} (superseded)",
                    short_id(r.id()),
                    kind_label(r),
                    summary(r),
                    self.attribution_suffix(r),
                ));
            }
        }
        for e in t.replies.iter().filter(|e| self.all || e.active) {
            let superseded = if e.active { "" } else { " (superseded)" };
            out.push(format!(
                "{REPLY_INDENT}[{}] {:<10} {}{}{superseded}",
                short_id(e.record.id()),
                kind_label(e.record),
                summary(e.record),
                self.attribution_suffix(e.record),
            ));
            self.push_continuation(&mut out, e.record, REPLY_INDENT);
        }
        if let ThreadState::Closed { reason, closer, .. } = state {
            let label = match reason {
                Some(r) => format!("resolve ({r})"),
                None => "resolve".to_string(),
            };
            out.push(format!(
                "{REPLY_INDENT}[{}] {:<10} {}{}",
                short_id(closer.id()),
                label,
                summary(closer),
                self.attribution_suffix(closer),
            ));
        }
        out
    }

    fn push_continuation(&self, out: &mut Vec<String>, r: &Record, indent: &str) {
        if let Some(f) = self.continuation {
            out.extend(
                f(r).into_iter()
                    .map(|l| format!("{indent}{REPLY_INDENT}{l}")),
            );
        }
    }

    fn attribution_suffix(&self, r: &Record) -> String {
        if !self.attribution {
            return String::new();
        }
        let Some(a) = r.as_annotation() else {
            return String::new();
        };
        let date = a.created_at.format("%Y-%m-%d");
        match &a.issuer_type {
            Some(t) if *t != IssuerType::Human => {
                format!("  ({}, {t}, {date})", short_issuer(&a.issuer))
            }
            _ => format!("  ({}, {date})", short_issuer(&a.issuer)),
        }
    }

    fn state_suffix(&self, state: &ThreadState<'_>) -> String {
        match state {
            ThreadState::Open => String::new(),
            ThreadState::NeedsDecision { addressee: None } => " — needs decision".into(),
            ThreadState::NeedsDecision { addressee: Some(a) } => {
                format!(" — needs decision from {}", short_issuer(a))
            }
            ThreadState::Decided => " — decided".into(),
            ThreadState::Closed {
                reason,
                closer,
                pending_question,
            } => {
                let reason = reason.map(|r| format!(" ({r})")).unwrap_or_default();
                let by = match closer.as_annotation() {
                    Some(a) if self.attribution => format!(" by {}", short_issuer(&a.issuer)),
                    _ => String::new(),
                };
                let pending = if *pending_question {
                    " — question still pending"
                } else {
                    ""
                };
                format!(" — closed{reason}{by}: {}{pending}", summary(closer))
            }
        }
    }
}

/// A record's kind, or its envelope type for non-annotations.
pub fn kind_label(r: &Record) -> String {
    r.kind()
        .map(|k| k.to_string())
        .unwrap_or_else(|| r.record_type().to_string())
}

fn summary(r: &Record) -> &str {
    r.as_annotation()
        .map(|a| a.body.summary.as_str())
        .unwrap_or("")
}

/// `subject`, `subject:line`, or `subject:start:end` for a record's span.
pub fn location(r: &Record) -> String {
    match r.as_annotation().and_then(|a| a.body.span.as_ref()) {
        Some(s) => match &s.end {
            Some(e) if e.line != s.start.line => {
                format!("{}:{}:{}", r.subject(), s.start.line, e.line)
            }
            _ => format!("{}:{}", r.subject(), s.start.line),
        },
        None => r.subject().to_string(),
    }
}

/// The JSON object for one thread, as `qualifier threads --format json`
/// prints it: `origin`, `open`, `state` ([`ThreadState::to_json`]),
/// `root`, `closed_by`, `history`, `replies` (`{active, record}`), and
/// `latest_at`.
pub fn thread_json(t: &Thread<'_>) -> serde_json::Result<serde_json::Value> {
    let replies = t
        .replies
        .iter()
        .map(|e| -> serde_json::Result<serde_json::Value> {
            Ok(serde_json::json!({
                "active": e.active,
                "record": serde_json::to_value(e.record)?,
            }))
        })
        .collect::<serde_json::Result<Vec<_>>>()?;
    let history = t
        .history
        .iter()
        .map(serde_json::to_value)
        .collect::<serde_json::Result<Vec<_>>>()?;
    Ok(serde_json::json!({
        "origin": t.origin,
        "open": t.open,
        "state": t.state().to_json(),
        "root": serde_json::to_value(t.root)?,
        "closed_by": t.closed_by.map(serde_json::to_value).transpose()?,
        "history": history,
        "replies": replies,
        "latest_at": t.latest_at.to_rfc3339(),
    }))
}

/// A compact per-thread summary for commands whose JSON is a record list
/// (`show`, `praise`): `origin`, the live `root` ID, `state`, and the
/// `closed_by` ID.
pub fn thread_summary_json(t: &Thread<'_>) -> serde_json::Value {
    serde_json::json!({
        "origin": t.origin,
        "root": t.root.id(),
        "state": t.state().to_json(),
        "closed_by": t.closed_by.map(|r| r.id()),
    })
}
