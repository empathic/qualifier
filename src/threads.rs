//! Thread assembly: group annotations into conversations.
//!
//! A thread starts at an *origin* annotation. Records join it by
//! `references` (replies) or by `supersedes` (edits and resolutions of a
//! record already in the thread). The *root chain* is the origin plus the
//! non-reply records that supersede it in turn; its live head is the
//! thread's root. A thread is open while that head is not a `resolve`.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};

use crate::annotation::{Kind, Record};
use crate::compact::filter_superseded;

/// One conversation: a root annotation, its edits, replies, and closure.
#[derive(Debug)]
pub struct Thread<'a> {
    /// ID of the oldest record in the root chain. Stable across edits.
    pub origin: &'a str,
    /// The live head of the root chain. For a closed thread, the newest
    /// non-`resolve` record in the root chain.
    pub root: &'a Record,
    /// The `resolve` record that closed the thread, if any.
    pub closed_by: Option<&'a Record>,
    /// Every other record in the thread, oldest first.
    pub replies: Vec<ThreadEntry<'a>>,
    /// True while the root chain's live head is not a `resolve`.
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
    let head = chain
        .iter()
        .rev()
        .copied()
        .find(|r| active.contains(r.id()));
    let (root, closed_by, open) = match head {
        Some(h) if !is_resolve(h) => (h, None, true),
        _ => {
            let root = chain
                .iter()
                .rev()
                .copied()
                .find(|r| !is_resolve(r))
                .unwrap_or(chain[0]);
            (root, head.filter(|h| is_resolve(h)), false)
        }
    };

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
