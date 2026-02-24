use chrono::Utc;

use crate::attestation::{self, Attestation, Kind};
use crate::qual_file::QualFile;
use crate::scoring;

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactResult {
    /// Number of attestations before compaction.
    pub before: usize,
    /// Number of attestations after compaction.
    pub after: usize,
    /// Number of superseded attestations pruned.
    pub pruned: usize,
}

/// Prune superseded attestations, keeping only chain tips.
///
/// The raw score of the artifact is preserved as an invariant.
pub fn prune(qual_file: &QualFile) -> (QualFile, CompactResult) {
    let before = qual_file.attestations.len();
    let active = scoring::filter_superseded(&qual_file.attestations);
    let after = active.len();

    let pruned_file = QualFile {
        path: qual_file.path.clone(),
        artifact: qual_file.artifact.clone(),
        attestations: active.into_iter().cloned().collect(),
    };

    let result = CompactResult {
        before,
        after,
        pruned: before - after,
    };

    (pruned_file, result)
}

/// Collapse all attestations into a single epoch attestation.
///
/// The epoch attestation's score equals the raw score of all non-superseded
/// attestations, preserving the scoring invariant.
pub fn snapshot(qual_file: &QualFile) -> (QualFile, CompactResult) {
    let before = qual_file.attestations.len();

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

    let raw = scoring::raw_score(&qual_file.attestations);

    // Collect IDs of all attestations being compacted
    let epoch_refs: Vec<String> = qual_file
        .attestations
        .iter()
        .map(|a| a.id.clone())
        .collect();

    let epoch = attestation::finalize(Attestation {
        artifact: qual_file.artifact.clone(),
        kind: Kind::Epoch,
        score: raw,
        summary: format!("Compacted from {} attestations", before),
        detail: None,
        suggested_fix: None,
        tags: vec!["epoch".into()],
        author: "qualifier/compact".into(),
        created_at: Utc::now(),
        supersedes: None,
        epoch_refs: Some(epoch_refs),
        id: String::new(),
    });

    let snapshot_file = QualFile {
        path: qual_file.path.clone(),
        artifact: qual_file.artifact.clone(),
        attestations: vec![epoch],
    };

    let result = CompactResult {
        before,
        after: 1,
        pruned: before - 1,
    };

    (snapshot_file, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::{self, Kind};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_att(artifact: &str, kind: Kind, score: i32, summary: &str) -> Attestation {
        attestation::finalize(Attestation {
            artifact: artifact.into(),
            kind,
            score,
            summary: summary.into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            supersedes: None,
            epoch_refs: None,
            id: String::new(),
        })
    }

    fn make_superseding(artifact: &str, score: i32, supersedes_id: &str) -> Attestation {
        attestation::finalize(Attestation {
            artifact: artifact.into(),
            kind: Kind::Pass,
            score,
            summary: "updated".into(),
            detail: None,
            suggested_fix: None,
            tags: vec![],
            author: "test@test.com".into(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            supersedes: Some(supersedes_id.into()),
            epoch_refs: None,
            id: String::new(),
        })
    }

    fn make_qual_file(attestations: Vec<Attestation>) -> QualFile {
        QualFile {
            path: PathBuf::from("test.rs.qual"),
            artifact: "test.rs".into(),
            attestations,
        }
    }

    #[test]
    fn test_prune_no_supersession() {
        let atts = vec![
            make_att("test.rs", Kind::Praise, 40, "good"),
            make_att("test.rs", Kind::Concern, -10, "meh"),
        ];
        let qf = make_qual_file(atts);
        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 2);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 0);
        assert_eq!(pruned.attestations.len(), 2);
    }

    #[test]
    fn test_prune_removes_superseded() {
        let original = make_att("test.rs", Kind::Concern, -30, "bad");
        let replacement = make_superseding("test.rs", 10, &original.id);
        let unrelated = make_att("test.rs", Kind::Praise, 20, "nice");

        let qf = make_qual_file(vec![original, replacement.clone(), unrelated.clone()]);
        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 3);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 1);
        assert!(pruned.attestations.iter().any(|a| a.id == replacement.id));
        assert!(pruned.attestations.iter().any(|a| a.id == unrelated.id));
    }

    #[test]
    fn test_prune_preserves_score() {
        let original = make_att("test.rs", Kind::Concern, -30, "bad");
        let replacement = make_superseding("test.rs", 10, &original.id);
        let extra = make_att("test.rs", Kind::Praise, 20, "nice");

        let qf = make_qual_file(vec![original, replacement, extra]);
        let score_before = scoring::raw_score(&qf.attestations);
        let (pruned, _) = prune(&qf);
        let score_after = scoring::raw_score(&pruned.attestations);

        assert_eq!(score_before, score_after, "prune must preserve raw score");
    }

    #[test]
    fn test_snapshot_empty() {
        let qf = make_qual_file(vec![]);
        let (snapped, result) = snapshot(&qf);
        assert_eq!(result.before, 0);
        assert_eq!(result.after, 0);
        assert!(snapped.attestations.is_empty());
    }

    #[test]
    fn test_snapshot_collapses_to_epoch() {
        let atts = vec![
            make_att("test.rs", Kind::Praise, 40, "good"),
            make_att("test.rs", Kind::Concern, -10, "meh"),
        ];
        let qf = make_qual_file(atts);
        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 2);
        assert_eq!(result.after, 1);
        assert_eq!(result.pruned, 1);

        let epoch = &snapped.attestations[0];
        assert_eq!(epoch.kind, Kind::Epoch);
        assert_eq!(epoch.score, 30); // 40 + -10
        assert_eq!(epoch.author, "qualifier/compact");
        assert!(epoch.tags.contains(&"epoch".to_string()));
        assert_eq!(epoch.epoch_refs.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_snapshot_preserves_score() {
        let original = make_att("test.rs", Kind::Concern, -30, "bad");
        let replacement = make_superseding("test.rs", 10, &original.id);
        let extra = make_att("test.rs", Kind::Praise, 20, "nice");

        let qf = make_qual_file(vec![original, replacement, extra]);
        let score_before = scoring::raw_score(&qf.attestations);
        let (snapped, _) = snapshot(&qf);
        let score_after = scoring::raw_score(&snapped.attestations);

        assert_eq!(
            score_before, score_after,
            "snapshot must preserve raw score"
        );
    }

    #[test]
    fn test_snapshot_with_supersession_chain() {
        let a = make_att("test.rs", Kind::Fail, -50, "terrible");
        let b = make_superseding("test.rs", -20, &a.id);
        let c = make_superseding("test.rs", 10, &b.id);
        // Raw score: only c counts -> 10

        let qf = make_qual_file(vec![a, b, c]);
        let score_before = scoring::raw_score(&qf.attestations);
        assert_eq!(score_before, 10);

        let (snapped, _) = snapshot(&qf);
        assert_eq!(snapped.attestations.len(), 1);
        assert_eq!(snapped.attestations[0].score, 10);
        assert_eq!(
            snapped.attestations[0].epoch_refs.as_ref().unwrap().len(),
            3
        );
    }

    #[test]
    fn test_prune_with_dangling_supersedes() {
        // Attestation supersedes a non-existent ID — should be kept as active
        let a = make_att("test.rs", Kind::Praise, 20, "good");
        let mut b = make_att("test.rs", Kind::Pass, 10, "fixed");
        b.supersedes = Some("nonexistent_id_12345".into());
        b = attestation::finalize(b);

        let qf = make_qual_file(vec![a.clone(), b.clone()]);
        let (pruned, result) = prune(&qf);
        // Both are active since the supersession target doesn't exist in this set
        assert_eq!(result.pruned, 0);
        assert_eq!(pruned.attestations.len(), 2);
    }

    #[test]
    fn test_prune_multiple_disjoint_chains() {
        // Two independent supersession chains
        let a1 = make_att("test.rs", Kind::Concern, -10, "issue 1");
        let a2 = make_superseding("test.rs", 5, &a1.id);
        let b1 = make_att("test.rs", Kind::Concern, -20, "issue 2");
        let b2 = make_superseding("test.rs", 10, &b1.id);

        let qf = make_qual_file(vec![a1, a2.clone(), b1, b2.clone()]);
        let (pruned, result) = prune(&qf);

        assert_eq!(result.before, 4);
        assert_eq!(result.after, 2);
        assert_eq!(result.pruned, 2);
        assert!(pruned.attestations.iter().any(|a| a.id == a2.id));
        assert!(pruned.attestations.iter().any(|a| a.id == b2.id));
    }

    #[test]
    fn test_prune_deep_chain() {
        // 5-level chain: a -> b -> c -> d -> e
        let a = make_att("test.rs", Kind::Fail, -50, "step 1");
        let b = make_superseding("test.rs", -40, &a.id);
        let c = make_superseding("test.rs", -20, &b.id);
        let d = make_superseding("test.rs", 0, &c.id);
        let e = make_superseding("test.rs", 30, &d.id);

        let qf = make_qual_file(vec![a, b, c, d, e.clone()]);
        let score_before = scoring::raw_score(&qf.attestations);
        let (pruned, result) = prune(&qf);
        let score_after = scoring::raw_score(&pruned.attestations);

        assert_eq!(result.after, 1); // only tip survives
        assert_eq!(pruned.attestations[0].id, e.id);
        assert_eq!(score_before, score_after);
    }

    #[test]
    fn test_snapshot_single_attestation() {
        let a = make_att("test.rs", Kind::Praise, 40, "good");
        let qf = make_qual_file(vec![a]);
        let (snapped, result) = snapshot(&qf);

        assert_eq!(result.before, 1);
        assert_eq!(result.after, 1);
        assert_eq!(result.pruned, 0);
        assert_eq!(snapped.attestations[0].kind, Kind::Epoch);
        assert_eq!(snapped.attestations[0].score, 40);
    }
}
