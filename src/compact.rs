use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::annotation::{self, Epoch, EpochBody, IssuerType, Record};
use crate::qual_file::QualFile;

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactResult {
    /// Number of records before compaction.
    pub before: usize,
    /// Number of records after compaction.
    pub after: usize,
    /// Number of records removed (`before - after`).
    pub pruned: usize,
    /// Number of epoch records the compaction created (snapshot only).
    pub epochs: usize,
}

/// Filter out superseded records, returning only the active ones.
///
/// A record is superseded if any other record's `supersedes` field
/// points to its ID. Only annotations can supersede or be superseded.
/// Non-annotation records always pass through.
pub fn filter_superseded(records: &[Record]) -> Vec<&Record> {
    let superseded_ids: HashSet<&str> = records.iter().filter_map(|r| r.supersedes()).collect();
    records
        .iter()
        .filter(|r| !superseded_ids.contains(r.id()))
        .collect()
}

/// Prune superseded records from every subject in the file.
///
/// See [`prune_subject`] for which superseded records are kept.
pub fn prune(qual_file: &QualFile) -> (QualFile, CompactResult) {
    prune_where(qual_file, |_| true)
}

/// Prune superseded records of one subject; records of other subjects are
/// kept unchanged.
///
/// A superseded record is removed unless removing it would change how the
/// remaining records group into threads: a record that a kept record names
/// in `references` is kept, and so is every record that supersedes a kept
/// record. A resolved thread with replies therefore keeps its root, and
/// its replies never reappear as threads of their own. Records of other
/// types (epochs, dependencies, unknown types) are always kept.
pub fn prune_subject(qual_file: &QualFile, subject: &str) -> (QualFile, CompactResult) {
    prune_where(qual_file, |r| r.subject() == subject)
}

fn prune_where(
    qual_file: &QualFile,
    in_scope: impl Fn(&Record) -> bool,
) -> (QualFile, CompactResult) {
    let records = &qual_file.records;
    let ids: HashSet<&str> = records.iter().map(|r| r.id()).collect();
    let mut successors: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut references: HashMap<&str, Vec<&str>> = HashMap::new();
    for r in records {
        if let Some(target) = r.supersedes() {
            successors.entry(target).or_default().push(r.id());
        }
        if let Some(target) = r.references() {
            references.entry(r.id()).or_default().push(target);
        }
    }

    // Start from the active records plus everything out of scope, then
    // keep whatever those records need to stay in the same threads.
    let active: HashSet<&str> = filter_superseded(records)
        .into_iter()
        .map(|r| r.id())
        .collect();
    let mut keep: HashSet<&str> = records
        .iter()
        .filter(|r| !in_scope(r) || active.contains(r.id()))
        .map(|r| r.id())
        .collect();
    let mut work: Vec<&str> = keep.iter().copied().collect();
    while let Some(id) = work.pop() {
        let referenced = references.get(id).into_iter().flatten();
        let superseding = successors.get(id).into_iter().flatten();
        for &next in referenced.chain(superseding) {
            if ids.contains(next) && keep.insert(next) {
                work.push(next);
            }
        }
    }

    let kept: Vec<Record> = records
        .iter()
        .filter(|r| keep.contains(r.id()))
        .cloned()
        .collect();
    let result = CompactResult {
        before: records.len(),
        after: kept.len(),
        pruned: records.len() - kept.len(),
        epochs: 0,
    };
    let pruned_file = QualFile {
        path: qual_file.path.clone(),
        subject: qual_file.subject.clone(),
        records: kept,
    };
    (pruned_file, result)
}

/// Collapse each subject's annotation and epoch records into one epoch.
///
/// See [`snapshot_subject`].
pub fn snapshot(qual_file: &QualFile) -> (QualFile, CompactResult) {
    snapshot_where(qual_file, |_| true)
}

/// Collapse one subject's annotation and epoch records into one epoch;
/// records of other subjects are kept unchanged.
///
/// Superseded records are pruned first: the epoch's `refs` lists the
/// active (non-superseded) records it replaces, and its summary counts
/// them. The epoch takes the place of the subject's first folded record.
/// A subject whose only record is already an epoch is left alone.
/// Records of other types (dependencies, unknown types) pass through.
pub fn snapshot_subject(qual_file: &QualFile, subject: &str) -> (QualFile, CompactResult) {
    snapshot_where(qual_file, |r| r.subject() == subject)
}

fn snapshot_where(
    qual_file: &QualFile,
    in_scope: impl Fn(&Record) -> bool,
) -> (QualFile, CompactResult) {
    let records = &qual_file.records;
    let foldable =
        |r: &Record| matches!(r, Record::Annotation(_) | Record::Epoch(_)) && in_scope(r);

    let mut folded: HashMap<&str, Vec<&Record>> = HashMap::new();
    for r in records.iter().filter(|r| foldable(r)) {
        folded.entry(r.subject()).or_default().push(r);
    }
    let mut active: HashMap<&str, Vec<&Record>> = HashMap::new();
    for r in filter_superseded(records)
        .into_iter()
        .filter(|r| foldable(r))
    {
        active.entry(r.subject()).or_default().push(r);
    }
    // Leave a subject alone when there is nothing to fold (a supersession
    // cycle leaves no active record) or it is already a single epoch.
    folded.retain(|subject, all| {
        let live = active.get(subject).map_or(0, Vec::len);
        live > 0 && !(all.len() == 1 && all[0].as_epoch().is_some())
    });

    let mut epochs: HashMap<&str, Record> = HashMap::new();
    for &subject in folded.keys() {
        let live = &active[subject];
        let epoch = annotation::finalize_epoch(Epoch {
            metabox: "1".into(),
            record_type: "epoch".into(),
            subject: subject.to_string(),
            issuer: "urn:qualifier:compact".into(),
            issuer_type: Some(IssuerType::Tool),
            created_at: Utc::now(),
            id: String::new(),
            body: EpochBody {
                refs: live.iter().map(|r| r.id().to_string()).collect(),
                span: None,
                summary: format!("Compacted from {} records", live.len()),
                extra: Default::default(),
            },
        });
        epochs.insert(subject, Record::Epoch(epoch));
    }
    let epoch_count = epochs.len();

    let mut out = Vec::with_capacity(records.len());
    for r in records {
        if foldable(r) && folded.contains_key(r.subject()) {
            if let Some(epoch) = epochs.remove(r.subject()) {
                out.push(epoch);
            }
        } else {
            out.push(r.clone());
        }
    }

    let result = CompactResult {
        before: records.len(),
        after: out.len(),
        pruned: records.len() - out.len(),
        epochs: epoch_count,
    };
    let snapshot_file = QualFile {
        path: qual_file.path.clone(),
        subject: qual_file.subject.clone(),
        records: out,
    };
    (snapshot_file, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::{self, Annotation, AnnotationBody, Kind};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_att(subject: &str, kind: Kind, summary: &str) -> Annotation {
        annotation::finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: subject.into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AnnotationBody {
                detail: None,
                kind,
                r#ref: None,
                references: None,
                span: None,
                suggested_fix: None,
                summary: summary.into(),
                supersedes: None,
                tags: vec![],
                extra: Default::default(),
            },
        })
    }

    fn make_record(subject: &str, kind: Kind, summary: &str) -> Record {
        Record::Annotation(Box::new(make_att(subject, kind, summary)))
    }

    fn make_superseding(subject: &str, supersedes_id: &str) -> Record {
        Record::Annotation(Box::new(annotation::finalize(Annotation {
            metabox: "1".into(),
            record_type: "annotation".into(),
            subject: subject.into(),
            issuer: "mailto:test@test.com".into(),
            issuer_type: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
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
                summary: "updated".into(),
                supersedes: Some(supersedes_id.into()),
                tags: vec![],
                extra: Default::default(),
            },
        })))
    }

    fn make_qual_file(records: Vec<Record>) -> QualFile {
        QualFile {
            path: PathBuf::from("test.rs.qual"),
            subject: "test.rs".into(),
            records,
        }
    }

    #[test]
    fn test_prune_no_supersession() {
        let records = vec![
            make_record("test.rs", Kind::Praise, "good"),
            make_record("test.rs", Kind::Concern, "meh"),
        ];
        let qf = make_qual_file(records);
        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 2);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 0);
        assert_eq!(pruned.records.len(), 2);
    }

    #[test]
    fn test_prune_removes_superseded() {
        let original = make_record("test.rs", Kind::Concern, "bad");
        let replacement = make_superseding("test.rs", original.id());
        let unrelated = make_record("test.rs", Kind::Praise, "nice");

        let replacement_id = replacement.id().to_string();
        let unrelated_id = unrelated.id().to_string();

        let qf = make_qual_file(vec![original, replacement, unrelated]);
        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 3);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 1);
        assert!(pruned.records.iter().any(|r| r.id() == replacement_id));
        assert!(pruned.records.iter().any(|r| r.id() == unrelated_id));
    }

    #[test]
    fn test_snapshot_empty() {
        let qf = make_qual_file(vec![]);
        let (snapped, result) = snapshot(&qf);
        assert_eq!(result.before, 0);
        assert_eq!(result.after, 0);
        assert!(snapped.records.is_empty());
    }

    #[test]
    fn test_snapshot_collapses_to_epoch() {
        let records = vec![
            make_record("test.rs", Kind::Praise, "good"),
            make_record("test.rs", Kind::Concern, "meh"),
        ];
        let qf = make_qual_file(records);
        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 2);
        assert_eq!(result.after, 1);
        assert_eq!(result.pruned, 1);

        let epoch = snapped.records[0].as_epoch().unwrap();
        assert_eq!(epoch.issuer, "urn:qualifier:compact");
        assert_eq!(epoch.body.refs.len(), 2);
    }

    #[test]
    fn test_snapshot_with_supersession_chain() {
        let a = make_record("test.rs", Kind::Fail, "terrible");
        let b = make_superseding("test.rs", a.id());
        let c = make_superseding("test.rs", b.id());

        let c_id = c.id().to_string();

        let qf = make_qual_file(vec![a, b, c]);
        let (snapped, result) = snapshot(&qf);
        assert_eq!(snapped.records.len(), 1);
        assert_eq!(result.epochs, 1);
        let epoch = snapped.records[0].as_epoch().unwrap();
        // Superseded records are pruned before the snapshot.
        assert_eq!(epoch.body.refs, vec![c_id]);
        assert_eq!(epoch.body.summary, "Compacted from 1 records");
    }

    fn with_body(subject: &str, kind: Kind, f: impl FnOnce(&mut AnnotationBody)) -> Record {
        let summary = format!("{kind} on {subject}");
        let mut att = make_att(subject, kind, &summary);
        f(&mut att.body);
        Record::Annotation(Box::new(annotation::finalize(att)))
    }

    #[test]
    fn test_prune_keeps_resolved_thread_with_replies() {
        let root = make_record("test.rs", Kind::Concern, "root");
        let root_id = root.id().to_string();
        let reply = with_body("test.rs", Kind::Comment, |b| {
            b.references = Some(root_id.clone())
        });
        let resolve = with_body("test.rs", Kind::Resolve, |b| {
            b.supersedes = Some(root_id.clone())
        });

        let qf = make_qual_file(vec![root, reply, resolve]);
        let (pruned, result) = prune(&qf);
        assert_eq!(result.pruned, 0, "every record is still needed");
        assert_eq!(pruned.records.len(), 3);
        let threads = crate::threads::build_threads(&pruned.records);
        assert_eq!(threads.len(), 1);
        assert!(!threads[0].open);
    }

    #[test]
    fn test_prune_keeps_chain_from_referenced_record_to_tip() {
        // a <- b (edit) <- resolve; a reply names b. Dropping b would
        // reopen the thread, so b and its successors stay; a goes.
        let a = make_record("test.rs", Kind::Concern, "first wording");
        let b = with_body("test.rs", Kind::Concern, |body| {
            body.supersedes = Some(a.id().to_string())
        });
        let reply = with_body("test.rs", Kind::Comment, |body| {
            body.references = Some(b.id().to_string())
        });
        let resolve = with_body("test.rs", Kind::Resolve, |body| {
            body.supersedes = Some(b.id().to_string())
        });
        let a_id = a.id().to_string();

        let qf = make_qual_file(vec![a, b, reply, resolve]);
        let (pruned, result) = prune(&qf);
        assert_eq!(result.pruned, 1);
        assert!(pruned.records.iter().all(|r| r.id() != a_id));
        let threads = crate::threads::build_threads(&pruned.records);
        assert_eq!(threads.len(), 1);
        assert!(!threads[0].open);
        assert_eq!(threads[0].replies.len(), 1);
    }

    #[test]
    fn test_prune_subject_leaves_other_subjects() {
        let a1 = make_record("src/a.rs", Kind::Concern, "issue");
        let a2 = make_superseding("src/a.rs", a1.id());
        let b1 = make_record("src/b.rs", Kind::Concern, "old");
        let b2 = make_superseding("src/b.rs", b1.id());
        let b1_id = b1.id().to_string();

        let qf = make_qual_file(vec![a1, a2, b1, b2]);
        let (pruned, result) = prune_subject(&qf, "src/a.rs");
        assert_eq!(result.pruned, 1);
        assert!(pruned.records.iter().any(|r| r.id() == b1_id));
    }

    #[test]
    fn test_snapshot_subject_leaves_other_subjects() {
        let a1 = make_record("src/a.rs", Kind::Praise, "good");
        let a2 = make_record("src/a.rs", Kind::Praise, "also good");
        let b1 = make_record("src/b.rs", Kind::Blocker, "open");
        let b1_id = b1.id().to_string();

        let qf = make_qual_file(vec![b1, a1, a2]);
        let (snapped, result) = snapshot_subject(&qf, "src/a.rs");
        assert_eq!(result.epochs, 1);
        assert_eq!(snapped.records.len(), 2);
        assert_eq!(snapped.records[0].id(), b1_id, "order is preserved");
        let epoch = snapped.records[1].as_epoch().unwrap();
        assert_eq!(epoch.subject, "src/a.rs");
        assert_eq!(epoch.body.refs.len(), 2);
    }

    #[test]
    fn test_snapshot_leaves_a_lone_epoch_alone() {
        let qf = make_qual_file(vec![make_record("test.rs", Kind::Praise, "good")]);
        let (once, _) = snapshot(&qf);
        let (twice, result) = snapshot(&once);
        assert_eq!(result.epochs, 0);
        assert_eq!(twice.records, once.records);
    }

    #[test]
    fn test_prune_with_dangling_supersedes() {
        let a = make_record("test.rs", Kind::Praise, "good");
        let mut b_att = make_att("test.rs", Kind::Pass, "fixed");
        b_att.body.supersedes = Some("nonexistent_id_12345".into());
        b_att = annotation::finalize(b_att);
        let b = Record::Annotation(Box::new(b_att));

        let qf = make_qual_file(vec![a, b]);
        let (pruned, result) = prune(&qf);
        assert_eq!(result.pruned, 0);
        assert_eq!(pruned.records.len(), 2);
    }

    #[test]
    fn test_prune_multiple_disjoint_chains() {
        let a1 = make_record("test.rs", Kind::Concern, "issue 1");
        let a2 = make_superseding("test.rs", a1.id());
        let b1 = make_record("test.rs", Kind::Concern, "issue 2");
        let b2 = make_superseding("test.rs", b1.id());

        let a2_id = a2.id().to_string();
        let b2_id = b2.id().to_string();

        let qf = make_qual_file(vec![a1, a2, b1, b2]);
        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 4);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 2);
        assert!(pruned.records.iter().any(|r| r.id() == a2_id));
        assert!(pruned.records.iter().any(|r| r.id() == b2_id));
    }

    #[test]
    fn test_prune_deep_chain() {
        let a = make_record("test.rs", Kind::Fail, "step 1");
        let b = make_superseding("test.rs", a.id());
        let c = make_superseding("test.rs", b.id());
        let d = make_superseding("test.rs", c.id());
        let e = make_superseding("test.rs", d.id());

        let e_id = e.id().to_string();

        let qf = make_qual_file(vec![a, b, c, d, e]);
        let (pruned, result) = prune(&qf);

        assert_eq!(result.after, 1);
        assert_eq!(pruned.records[0].id(), e_id);
    }

    #[test]
    fn test_snapshot_single_record() {
        let records = vec![make_record("test.rs", Kind::Praise, "good")];
        let qf = make_qual_file(records);
        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 1);
        assert_eq!(result.after, 1);
        assert_eq!(result.pruned, 0);
        assert!(snapped.records[0].as_epoch().is_some());
    }

    #[test]
    fn test_snapshot_multi_subject() {
        let records = vec![
            make_record("src/a.rs", Kind::Praise, "good"),
            make_record("src/a.rs", Kind::Concern, "meh"),
            make_record("src/b.rs", Kind::Pass, "ok"),
        ];

        let qf = QualFile {
            path: PathBuf::from("src/.qual"),
            subject: "src/".into(),
            records,
        };

        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 3);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 1);

        let epoch_a = snapped
            .records
            .iter()
            .find(|r| r.subject() == "src/a.rs")
            .unwrap()
            .as_epoch()
            .unwrap();
        let epoch_b = snapped
            .records
            .iter()
            .find(|r| r.subject() == "src/b.rs")
            .unwrap()
            .as_epoch()
            .unwrap();

        assert_eq!(epoch_a.body.refs.len(), 2);
        assert_eq!(epoch_b.body.refs.len(), 1);
    }

    fn make_unknown(subject: &str, id: &str) -> Record {
        let value = serde_json::json!({
            "metabox": "1",
            "type": "https://example.com/custom/v1",
            "subject": subject,
            "issuer": "https://ci.example.com",
            "created_at": "2026-04-01T00:00:00Z",
            "id": id,
            "body": {"foo": "bar"}
        });
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn test_prune_preserves_unknown_records() {
        // Spec §3.3.1: compaction MUST preserve records of unrecognized types.
        let unknown_id = "u".repeat(64);
        let original = make_record("test.rs", Kind::Concern, "issue");
        let replacement = make_superseding("test.rs", original.id());
        let unknown = make_unknown("test.rs", &unknown_id);

        let qf = make_qual_file(vec![original, replacement, unknown]);
        let (pruned, _) = prune(&qf);

        assert!(
            pruned
                .records
                .iter()
                .any(|r| matches!(r, Record::Unknown(_))),
            "prune must preserve unknown records"
        );
        assert!(
            pruned.records.iter().any(|r| r.id() == unknown_id),
            "unknown record id must round-trip"
        );
    }

    #[test]
    fn test_snapshot_preserves_unknown_records() {
        // Spec §3.3.1: snapshot compaction must pass through records of
        // unrecognized types unchanged while collapsing annotations into
        // an epoch.
        let unknown_id = "u".repeat(64);
        let unknown = make_unknown("test.rs", &unknown_id);
        let annotation = make_record("test.rs", Kind::Praise, "good");

        let qf = make_qual_file(vec![annotation, unknown]);
        let (snapped, _) = snapshot(&qf);

        assert_eq!(snapped.records.len(), 2);
        assert!(
            snapped.records.iter().any(|r| r.as_epoch().is_some()),
            "snapshot should produce an epoch for the annotation"
        );
        let preserved = snapped
            .records
            .iter()
            .find(|r| matches!(r, Record::Unknown(_)))
            .expect("unknown record should be preserved");
        assert_eq!(preserved.id(), unknown_id);
        assert_eq!(preserved.record_type(), "https://example.com/custom/v1");
    }

    #[test]
    fn test_prune_multi_subject() {
        let a1 = make_record("src/a.rs", Kind::Concern, "issue");
        let a2 = make_superseding("src/a.rs", a1.id());
        let b1 = make_record("src/b.rs", Kind::Pass, "ok");

        let a2_id = a2.id().to_string();
        let b1_id = b1.id().to_string();

        let qf = QualFile {
            path: PathBuf::from("src/.qual"),
            subject: "src/".into(),
            records: vec![a1, a2, b1],
        };

        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 3);
        assert_eq!(result.after, 2);
        assert!(pruned.records.iter().any(|r| r.id() == a2_id));
        assert!(pruned.records.iter().any(|r| r.id() == b1_id));
    }
}
