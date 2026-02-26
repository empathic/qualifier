use std::collections::HashMap;

use chrono::Utc;

use crate::attestation::{self, AuthorType, Epoch, EpochBody, Record};
use crate::qual_file::QualFile;
use crate::scoring;

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

/// Prune superseded records, keeping only chain tips.
///
/// The raw score of the artifact is preserved as an invariant.
/// Non-attestation records (epochs, dependencies, unknowns) are always kept.
pub fn prune(qual_file: &QualFile) -> (QualFile, CompactResult) {
    let before = qual_file.records.len();
    let active = scoring::filter_superseded(&qual_file.records);
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

/// Collapse all scored records into epoch records — one per distinct subject.
///
/// Each epoch record's score equals the raw score of its subject's
/// non-superseded scored records, preserving the scoring invariant.
///
/// Non-scored records (dependencies, unknowns) are passed through unchanged.
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

    // Separate scored records from passthrough (non-scored)
    let mut by_subject: HashMap<&str, Vec<&Record>> = HashMap::new();
    let mut passthrough: Vec<Record> = Vec::new();

    for record in &qual_file.records {
        if record.is_scored() {
            by_subject.entry(record.subject()).or_default().push(record);
        } else {
            passthrough.push(record.clone());
        }
    }

    let mut epoch_records = Vec::new();
    for (subject, records) in &by_subject {
        let raw = scoring::raw_score_from_refs(records);
        let refs: Vec<String> = records.iter().map(|r| r.id().to_string()).collect();
        let count = records.len();

        let epoch = attestation::finalize_epoch(Epoch {
            metabox: "1".into(),
            record_type: "epoch".into(),
            subject: subject.to_string(),
            author: "qualifier/compact".into(),
            created_at: Utc::now(),
            id: String::new(),
            body: EpochBody {
                author_type: Some(AuthorType::Tool),
                refs,
                score: raw,
                span: None,
                summary: format!("Compacted from {} records", count),
            },
        });
        epoch_records.push(Record::Epoch(epoch));
    }

    // Sort by subject name for deterministic output
    epoch_records.sort_by(|a, b| a.subject().cmp(b.subject()));

    // Append passthrough records
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
    use crate::attestation::{self, Attestation, AttestationBody, Kind};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_att(subject: &str, kind: Kind, score: i32, summary: &str) -> Attestation {
        attestation::finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: subject.into(),
            author: "test@test.com".into(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind,
                r#ref: None,
                score,
                span: None,
                suggested_fix: None,
                summary: summary.into(),
                supersedes: None,
                tags: vec![],
            },
        })
    }

    fn make_record(subject: &str, kind: Kind, score: i32, summary: &str) -> Record {
        Record::Attestation(Box::new(make_att(subject, kind, score, summary)))
    }

    fn make_superseding(subject: &str, score: i32, supersedes_id: &str) -> Record {
        Record::Attestation(Box::new(attestation::finalize(Attestation {
            metabox: "1".into(),
            record_type: "attestation".into(),
            subject: subject.into(),
            author: "test@test.com".into(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            id: String::new(),
            body: AttestationBody {
                author_type: None,
                detail: None,
                kind: Kind::Pass,
                r#ref: None,
                score,
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
            make_record("test.rs", Kind::Praise, 40, "good"),
            make_record("test.rs", Kind::Concern, -10, "meh"),
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
        let original = make_record("test.rs", Kind::Concern, -30, "bad");
        let replacement = make_superseding("test.rs", 10, original.id());
        let unrelated = make_record("test.rs", Kind::Praise, 20, "nice");

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
    fn test_prune_preserves_score() {
        let original = make_record("test.rs", Kind::Concern, -30, "bad");
        let replacement = make_superseding("test.rs", 10, original.id());
        let extra = make_record("test.rs", Kind::Praise, 20, "nice");

        let qf = make_qual_file(vec![original, replacement, extra]);
        let score_before = scoring::raw_score(&qf.records);
        let (pruned, _) = prune(&qf);
        let score_after = scoring::raw_score(&pruned.records);

        assert_eq!(score_before, score_after, "prune must preserve raw score");
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
            make_record("test.rs", Kind::Praise, 40, "good"),
            make_record("test.rs", Kind::Concern, -10, "meh"),
        ];
        let qf = make_qual_file(records);
        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 2);
        assert_eq!(result.after, 1);
        assert_eq!(result.pruned, 1);

        let epoch = snapped.records[0].as_epoch().unwrap();
        assert_eq!(epoch.body.score, 30); // 40 + -10
        assert_eq!(epoch.author, "qualifier/compact");
        assert_eq!(epoch.body.refs.len(), 2);
    }

    #[test]
    fn test_snapshot_preserves_score() {
        let original = make_record("test.rs", Kind::Concern, -30, "bad");
        let replacement = make_superseding("test.rs", 10, original.id());
        let extra = make_record("test.rs", Kind::Praise, 20, "nice");

        let qf = make_qual_file(vec![original, replacement, extra]);
        let score_before = scoring::raw_score(&qf.records);
        let (snapped, _) = snapshot(&qf);
        let score_after = scoring::raw_score(&snapped.records);

        assert_eq!(
            score_before, score_after,
            "snapshot must preserve raw score"
        );
    }

    #[test]
    fn test_snapshot_with_supersession_chain() {
        let a = make_record("test.rs", Kind::Fail, -50, "terrible");
        let b = make_superseding("test.rs", -20, a.id());
        let c = make_superseding("test.rs", 10, b.id());

        let qf = make_qual_file(vec![a, b, c]);
        let score_before = scoring::raw_score(&qf.records);
        assert_eq!(score_before, 10);

        let (snapped, _) = snapshot(&qf);
        assert_eq!(snapped.records.len(), 1);
        assert_eq!(snapped.records[0].as_epoch().unwrap().body.score, 10);
        assert_eq!(snapped.records[0].as_epoch().unwrap().body.refs.len(), 3);
    }

    #[test]
    fn test_prune_with_dangling_supersedes() {
        let a = make_record("test.rs", Kind::Praise, 20, "good");
        let mut b_att = make_att("test.rs", Kind::Pass, 10, "fixed");
        b_att.body.supersedes = Some("nonexistent_id_12345".into());
        b_att = attestation::finalize(b_att);
        let b = Record::Attestation(Box::new(b_att));

        let qf = make_qual_file(vec![a, b]);
        let (pruned, result) = prune(&qf);
        assert_eq!(result.pruned, 0);
        assert_eq!(pruned.records.len(), 2);
    }

    #[test]
    fn test_prune_multiple_disjoint_chains() {
        let a1 = make_record("test.rs", Kind::Concern, -10, "issue 1");
        let a2 = make_superseding("test.rs", 5, a1.id());
        let b1 = make_record("test.rs", Kind::Concern, -20, "issue 2");
        let b2 = make_superseding("test.rs", 10, b1.id());

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
        let a = make_record("test.rs", Kind::Fail, -50, "step 1");
        let b = make_superseding("test.rs", -40, a.id());
        let c = make_superseding("test.rs", -20, b.id());
        let d = make_superseding("test.rs", 0, c.id());
        let e = make_superseding("test.rs", 30, d.id());

        let e_id = e.id().to_string();

        let qf = make_qual_file(vec![a, b, c, d, e]);
        let score_before = scoring::raw_score(&qf.records);
        let (pruned, result) = prune(&qf);
        let score_after = scoring::raw_score(&pruned.records);

        assert_eq!(result.after, 1);
        assert_eq!(pruned.records[0].id(), e_id);
        assert_eq!(score_before, score_after);
    }

    #[test]
    fn test_snapshot_single_record() {
        let records = vec![make_record("test.rs", Kind::Praise, 40, "good")];
        let qf = make_qual_file(records);
        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 1);
        assert_eq!(result.after, 1);
        assert_eq!(result.pruned, 0);
        assert!(snapped.records[0].as_epoch().is_some());
        assert_eq!(snapped.records[0].as_epoch().unwrap().body.score, 40);
    }

    #[test]
    fn test_snapshot_multi_subject() {
        let records = vec![
            make_record("src/a.rs", Kind::Praise, 40, "good"),
            make_record("src/a.rs", Kind::Concern, -10, "meh"),
            make_record("src/b.rs", Kind::Pass, 20, "ok"),
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

        assert_eq!(epoch_a.body.score, 30); // 40 + -10
        assert_eq!(epoch_a.body.refs.len(), 2);

        assert_eq!(epoch_b.body.score, 20);
        assert_eq!(epoch_b.body.refs.len(), 1);
    }

    #[test]
    fn test_prune_multi_subject() {
        let a1 = make_record("src/a.rs", Kind::Concern, -10, "issue");
        let a2 = make_superseding("src/a.rs", 5, a1.id());
        let b1 = make_record("src/b.rs", Kind::Pass, 20, "ok");

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
