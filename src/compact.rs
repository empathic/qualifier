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
    /// Number of records pruned.
    pub pruned: usize,
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

/// Prune superseded records, keeping only chain tips.
///
/// Non-annotation records (epochs, dependencies, unknowns) are always kept.
pub fn prune(qual_file: &QualFile) -> (QualFile, CompactResult) {
    let before = qual_file.records.len();
    let active = filter_superseded(&qual_file.records);
    let after = active.len();

    let pruned_file = QualFile {
        path: qual_file.path.clone(),
        subject: qual_file.subject.clone(),
        records: active.into_iter().cloned().collect(),
    };

    let result = CompactResult {
        before,
        after,
        pruned: before - after,
    };

    (pruned_file, result)
}

/// Collapse annotation/epoch records into a single epoch per subject.
///
/// Non-annotation, non-epoch records (dependencies, unknowns) are passed
/// through unchanged.
pub fn snapshot(qual_file: &QualFile) -> (QualFile, CompactResult) {
    let before = qual_file.records.len();

    if before == 0 {
        return (
            qual_file.clone(),
            CompactResult {
                before: 0,
                after: 0,
                pruned: 0,
            },
        );
    }

    let mut by_subject: HashMap<&str, Vec<&Record>> = HashMap::new();
    let mut passthrough: Vec<Record> = Vec::new();

    for record in &qual_file.records {
        if matches!(record, Record::Annotation(_) | Record::Epoch(_)) {
            by_subject.entry(record.subject()).or_default().push(record);
        } else {
            passthrough.push(record.clone());
        }
    }

    let mut epoch_records = Vec::new();
    for (subject, records) in &by_subject {
        let refs: Vec<String> = records.iter().map(|r| r.id().to_string()).collect();
        let count = records.len();

        let epoch = annotation::finalize_epoch(Epoch {
            metabox: "1".into(),
            record_type: "epoch".into(),
            subject: subject.to_string(),
            issuer: "urn:qualifier:compact".into(),
            issuer_type: Some(IssuerType::Tool),
            created_at: Utc::now(),
            id: String::new(),
            body: EpochBody {
                refs,
                span: None,
                summary: format!("Compacted from {} records", count),
            },
        });
        epoch_records.push(Record::Epoch(epoch));
    }

    epoch_records.sort_by(|a, b| a.subject().cmp(b.subject()));
    epoch_records.extend(passthrough);

    let after = epoch_records.len();
    let snapshot_file = QualFile {
        path: qual_file.path.clone(),
        subject: qual_file.subject.clone(),
        records: epoch_records,
    };

    let result = CompactResult {
        before,
        after,
        pruned: before - after,
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

        let qf = make_qual_file(vec![a, b, c]);
        let (snapped, _) = snapshot(&qf);
        assert_eq!(snapped.records.len(), 1);
        assert_eq!(snapped.records[0].as_epoch().unwrap().body.refs.len(), 3);
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
