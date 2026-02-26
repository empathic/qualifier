use std::collections::HashMap;

use chrono::Utc;

use crate::attestation::{self, AuthorType, Epoch, Record};
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
        artifact: qual_file.artifact.clone(),
        records: active.into_iter().cloned().collect(),
    };

    let result = CompactResult {
        before,
        after,
        pruned: before - after,
    };

    (pruned_file, result)
}

/// Collapse all scored records into epoch records — one per distinct artifact.
///
/// Each epoch record's score equals the raw score of its artifact's
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
    let mut by_artifact: HashMap<&str, Vec<&Record>> = HashMap::new();
    let mut passthrough: Vec<Record> = Vec::new();

    for record in &qual_file.records {
        if record.is_scored() {
            by_artifact
                .entry(record.artifact())
                .or_default()
                .push(record);
        } else {
            passthrough.push(record.clone());
        }
    }

    let mut epoch_records = Vec::new();
    for (artifact, records) in &by_artifact {
        let raw = scoring::raw_score_from_refs(records);
        let refs: Vec<String> = records.iter().map(|r| r.id().to_string()).collect();
        let count = records.len();

        let epoch = attestation::finalize_epoch(Epoch {
            v: 3,
            record_type: "epoch".into(),
            artifact: artifact.to_string(),
            span: None,
            score: raw,
            summary: format!("Compacted from {} records", count),
            refs,
            author: "qualifier/compact".into(),
            author_type: Some(AuthorType::Tool),
            created_at: Utc::now(),
            id: String::new(),
        });
        epoch_records.push(Record::Epoch(epoch));
    }

    // Sort by artifact name for deterministic output
    epoch_records.sort_by(|a, b| a.artifact().cmp(b.artifact()));

    // Append passthrough records
    epoch_records.extend(passthrough);

    let after = epoch_records.len();
    let snapshot_file = QualFile {
        path: qual_file.path.clone(),
        artifact: qual_file.artifact.clone(),
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
    use crate::attestation::{self, Attestation, Kind};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_att(artifact: &str, kind: Kind, score: i32, summary: &str) -> Attestation {
        attestation::finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: artifact.into(),
            span: None,
            kind,
            score,
            summary: summary.into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            r#ref: None,
            supersedes: None,
            id: String::new(),
        })
    }

    fn make_record(artifact: &str, kind: Kind, score: i32, summary: &str) -> Record {
        Record::Attestation(make_att(artifact, kind, score, summary))
    }

    fn make_superseding(artifact: &str, score: i32, supersedes_id: &str) -> Record {
        Record::Attestation(attestation::finalize(Attestation {
            v: 3,
            record_type: "attestation".into(),
            artifact: artifact.into(),
            span: None,
            kind: Kind::Pass,
            score,
            summary: "updated".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            author_type: None,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            r#ref: None,
            supersedes: Some(supersedes_id.into()),
            id: String::new(),
        }))
    }

    fn make_qual_file(records: Vec<Record>) -> QualFile {
        QualFile {
            path: PathBuf::from("test.rs.qual"),
            artifact: "test.rs".into(),
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
        assert_eq!(epoch.score, 30); // 40 + -10
        assert_eq!(epoch.author, "qualifier/compact");
        assert_eq!(epoch.refs.len(), 2);
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
        assert_eq!(snapped.records[0].as_epoch().unwrap().score, 10);
        assert_eq!(snapped.records[0].as_epoch().unwrap().refs.len(), 3);
    }

    #[test]
    fn test_prune_with_dangling_supersedes() {
        let a = make_record("test.rs", Kind::Praise, 20, "good");
        let mut b_att = make_att("test.rs", Kind::Pass, 10, "fixed");
        b_att.supersedes = Some("nonexistent_id_12345".into());
        b_att = attestation::finalize(b_att);
        let b = Record::Attestation(b_att);

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
        assert_eq!(snapped.records[0].as_epoch().unwrap().score, 40);
    }

    #[test]
    fn test_snapshot_multi_artifact() {
        let records = vec![
            make_record("src/a.rs", Kind::Praise, 40, "good"),
            make_record("src/a.rs", Kind::Concern, -10, "meh"),
            make_record("src/b.rs", Kind::Pass, 20, "ok"),
        ];

        let qf = QualFile {
            path: PathBuf::from("src/.qual"),
            artifact: "src/".into(),
            records,
        };

        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 3);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 1);

        let epoch_a = snapped
            .records
            .iter()
            .find(|r| r.artifact() == "src/a.rs")
            .unwrap()
            .as_epoch()
            .unwrap();
        let epoch_b = snapped
            .records
            .iter()
            .find(|r| r.artifact() == "src/b.rs")
            .unwrap()
            .as_epoch()
            .unwrap();

        assert_eq!(epoch_a.score, 30); // 40 + -10
        assert_eq!(epoch_a.refs.len(), 2);

        assert_eq!(epoch_b.score, 20);
        assert_eq!(epoch_b.refs.len(), 1);
    }

    #[test]
    fn test_prune_multi_artifact() {
        let a1 = make_record("src/a.rs", Kind::Concern, -10, "issue");
        let a2 = make_superseding("src/a.rs", 5, a1.id());
        let b1 = make_record("src/b.rs", Kind::Pass, 20, "ok");

        let a2_id = a2.id().to_string();
        let b1_id = b1.id().to_string();

        let qf = QualFile {
            path: PathBuf::from("src/.qual"),
            artifact: "src/".into(),
            records: vec![a1, a2, b1],
        };

        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 3);
        assert_eq!(result.after, 2);
        assert!(pruned.records.iter().any(|r| r.id() == a2_id));
        assert!(pruned.records.iter().any(|r| r.id() == b1_id));
    }
}
