use qualifier::attestation::{self, Attestation, Kind, Record};
use qualifier::compact;
use qualifier::graph;
use qualifier::qual_file::{self, QualFile};
use qualifier::scoring;

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

// --- Golden ID tests (regression guards for content-addressed hashing) ---

#[test]
fn test_golden_attestation_id() {
    let att = attestation::finalize(Attestation {
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
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        r#ref: None,
        supersedes: None,
        id: String::new(),
    });
    // If this assertion fails, the canonical form or hashing has changed —
    // all existing record IDs in the wild are now broken.
    assert_eq!(
        att.id, "126ea3aa6728437ee5ce567f4682ebac5d2fb87c5dfcc53824812c0f89b6fe74",
        "Golden attestation ID changed! Canonical form or hashing is broken."
    );
}

#[test]
fn test_golden_epoch_id() {
    use qualifier::attestation::{self, AuthorType, Epoch};

    let epoch = attestation::finalize_epoch(Epoch {
        v: 3,
        record_type: "epoch".into(),
        artifact: "src/parser.rs".into(),
        span: None,
        score: 10,
        summary: "Compacted from 3 attestations".into(),
        refs: vec!["aaa".into(), "bbb".into(), "ccc".into()],
        author: "qualifier/compact".into(),
        author_type: Some(AuthorType::Tool),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-25T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        id: String::new(),
    });
    assert_eq!(
        epoch.id, "759bc0c7a3ffa09aad7a37d40f07916abdbdc535d7cc2850cf62efc830793342",
        "Golden epoch ID changed! Canonical form or hashing is broken."
    );
}

#[test]
fn test_golden_dependency_id() {
    use qualifier::attestation::{self, DependencyRecord};

    let dep = attestation::finalize_record(Record::Dependency(DependencyRecord {
        v: 3,
        record_type: "dependency".into(),
        artifact: "bin/server".into(),
        depends_on: vec!["lib/auth".into(), "lib/http".into()],
        author: "build-system".into(),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-25T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        id: String::new(),
    }));
    assert_eq!(
        dep.id(),
        "a7275c47c9f910556fd225da973a05b4c6edaf465bfcddf2d6bd3758761e8adb",
        "Golden dependency ID changed! Canonical form or hashing is broken."
    );
}

// --- Full attestation lifecycle ---

#[test]
fn test_attestation_lifecycle_write_parse_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let qual_path = dir.path().join("src/parser.rs.qual");
    std::fs::create_dir_all(qual_path.parent().unwrap()).unwrap();

    let r1 = make_record("src/parser.rs", Kind::Concern, -30, "Panics on bad input");
    let r2 = make_record("src/parser.rs", Kind::Praise, 40, "Good test coverage");

    qual_file::append(&qual_path, &r1).unwrap();
    qual_file::append(&qual_path, &r2).unwrap();

    let qf = qual_file::parse(&qual_path).unwrap();
    assert_eq!(qf.records.len(), 2);
    assert_eq!(qf.records[0].id(), r1.id());
    assert_eq!(qf.records[1].id(), r2.id());

    // IDs are deterministic and valid
    let att1 = r1.as_attestation().unwrap();
    let att2 = r2.as_attestation().unwrap();
    assert_eq!(attestation::generate_id(att1), att1.id);
    assert_eq!(attestation::generate_id(att2), att2.id);
}

#[test]
fn test_attestation_id_is_content_addressed() {
    let att1 = make_att("foo.rs", Kind::Pass, 10, "ok");
    let att2 = make_att("foo.rs", Kind::Pass, 10, "ok");
    // Same content, same ID
    assert_eq!(att1.id, att2.id);

    // Different content, different ID
    let att3 = make_att("foo.rs", Kind::Pass, 11, "ok");
    assert_ne!(att1.id, att3.id);
}

// --- Scoring with dependency graph ---

#[test]
fn test_scoring_with_dependency_graph() {
    let graph_str = r#"{"artifact":"bin/server","depends_on":["lib/auth","lib/http"]}
{"artifact":"lib/auth","depends_on":["lib/crypto"]}
{"artifact":"lib/http","depends_on":[]}
{"artifact":"lib/crypto","depends_on":[]}
"#;
    let g = graph::parse_graph(graph_str).unwrap();

    let qfs = vec![
        QualFile {
            path: PathBuf::from("bin/server.qual"),
            artifact: "bin/server".into(),
            records: vec![make_record("bin/server", Kind::Praise, 80, "solid")],
        },
        QualFile {
            path: PathBuf::from("lib/auth.qual"),
            artifact: "lib/auth".into(),
            records: vec![make_record("lib/auth", Kind::Praise, 60, "decent")],
        },
        QualFile {
            path: PathBuf::from("lib/http.qual"),
            artifact: "lib/http".into(),
            records: vec![make_record("lib/http", Kind::Praise, 70, "good")],
        },
        QualFile {
            path: PathBuf::from("lib/crypto.qual"),
            artifact: "lib/crypto".into(),
            records: vec![make_record("lib/crypto", Kind::Blocker, -40, "vulnerable")],
        },
    ];

    let scores = scoring::effective_scores(&g, &qfs);

    // lib/crypto is the poison
    assert_eq!(scores["lib/crypto"].raw, -40);
    assert_eq!(scores["lib/crypto"].effective, -40);

    // lib/auth depends on crypto, should be limited
    assert_eq!(scores["lib/auth"].raw, 60);
    assert_eq!(scores["lib/auth"].effective, -40);
    assert!(scores["lib/auth"].limiting_path.is_some());

    // lib/http has no bad deps
    assert_eq!(scores["lib/http"].raw, 70);
    assert_eq!(scores["lib/http"].effective, 70);

    // bin/server depends on both, limited by crypto through auth
    assert_eq!(scores["bin/server"].raw, 80);
    assert_eq!(scores["bin/server"].effective, -40);
}

#[test]
fn test_artifacts_in_qual_but_not_in_graph() {
    let graph_str = r#"{"artifact":"app","depends_on":["lib"]}
{"artifact":"lib","depends_on":[]}
"#;
    let g = graph::parse_graph(graph_str).unwrap();

    // "standalone" has a qual file but isn't in the graph
    let qfs = vec![QualFile {
        path: PathBuf::from("standalone.qual"),
        artifact: "standalone".into(),
        records: vec![make_record("standalone", Kind::Praise, 50, "fine")],
    }];

    let scores = scoring::effective_scores(&g, &qfs);

    // standalone should appear with effective = raw
    assert_eq!(scores["standalone"].raw, 50);
    assert_eq!(scores["standalone"].effective, 50);
    assert!(scores["standalone"].limiting_path.is_none());

    // Graph artifacts with no qual files should appear with score 0
    assert_eq!(scores["app"].raw, 0);
    assert_eq!(scores["lib"].raw, 0);
}

// --- Compaction preserves scores ---

#[test]
fn test_compaction_roundtrip_preserves_scores() {
    let original = make_record("mod.rs", Kind::Concern, -30, "bad");
    let fix = Record::Attestation(attestation::finalize(Attestation {
        v: 3,
        record_type: "attestation".into(),
        artifact: "mod.rs".into(),
        span: None,
        kind: Kind::Pass,
        score: 20,
        summary: "fixed".into(),
        detail: None,
        suggested_fix: None,
        tags: vec![],
        author: "test@test.com".into(),
        author_type: None,
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        r#ref: None,
        supersedes: Some(original.id().to_string()),
        id: String::new(),
    }));
    let extra = make_record("mod.rs", Kind::Praise, 40, "nice");

    let qf = QualFile {
        path: PathBuf::from("mod.rs.qual"),
        artifact: "mod.rs".into(),
        records: vec![original, fix, extra],
    };

    let score_before = scoring::raw_score(&qf.records);

    // Prune
    let (pruned, _) = compact::prune(&qf);
    assert_eq!(scoring::raw_score(&pruned.records), score_before);

    // Snapshot
    let (snapped, _) = compact::snapshot(&qf);
    assert_eq!(scoring::raw_score(&snapped.records), score_before);
    assert_eq!(snapped.records.len(), 1);
    assert!(snapped.records[0].as_epoch().is_some());
}

// --- Discovery ---

#[test]
fn test_discovery_walks_tree() {
    let dir = tempfile::tempdir().unwrap();

    // Create nested .qual files
    let paths = [
        "src/lib.rs.qual",
        "src/parser.rs.qual",
        "src/util/helpers.rs.qual",
    ];
    for p in &paths {
        let full = dir.path().join(p);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, "").unwrap();
    }

    // Create a hidden dir that should be skipped
    let hidden = dir.path().join(".git/objects/foo.qual");
    std::fs::create_dir_all(hidden.parent().unwrap()).unwrap();
    std::fs::write(&hidden, "").unwrap();

    let found = qual_file::discover(dir.path()).unwrap();
    assert_eq!(found.len(), 3);

    let artifacts: Vec<&str> = found.iter().map(|qf| qf.artifact.as_str()).collect();
    // artifact_name uses full paths, so check suffixes
    assert!(artifacts.iter().any(|a| a.ends_with("src/lib.rs")));
    assert!(artifacts.iter().any(|a| a.ends_with("src/parser.rs")));
    assert!(artifacts.iter().any(|a| a.ends_with("src/util/helpers.rs")));
}

// --- Supersession cycle detection ---

#[test]
fn test_supersession_cycle_detected() {
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

    let result = attestation::check_supersession_cycles(&[a, b]);
    assert!(result.is_err());
}

// --- Graph cycle detection ---

#[test]
fn test_graph_cycle_rejected() {
    let graph_str = r#"{"artifact":"a","depends_on":["b"]}
{"artifact":"b","depends_on":["c"]}
{"artifact":"c","depends_on":["a"]}
"#;
    let result = graph::parse_graph(graph_str);
    assert!(result.is_err());
}

// --- Cross-artifact supersession ---

#[test]
fn test_cross_artifact_supersession_rejected() {
    let a = make_record("foo.rs", Kind::Concern, -10, "issue in foo");
    let b = Record::Attestation(attestation::finalize(Attestation {
        v: 3,
        record_type: "attestation".into(),
        artifact: "bar.rs".into(),
        span: None,
        kind: Kind::Pass,
        score: 20,
        summary: "fix in bar".into(),
        detail: None,
        suggested_fix: None,
        tags: vec![],
        author: "test@test.com".into(),
        author_type: None,
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        r#ref: None,
        supersedes: Some(a.id().to_string()),
        id: String::new(),
    }));

    let result = attestation::validate_supersession_targets(&[a, b]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cross-artifact"));
}

// --- Kind typo detection ---

#[test]
fn test_kind_typo_detected_in_validation() {
    let att = attestation::finalize(Attestation {
        v: 3,
        record_type: "attestation".into(),
        artifact: "x.rs".into(),
        span: None,
        kind: Kind::Custom("pss".into()),
        score: 10,
        summary: "oops".into(),
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

    let errors = attestation::validate(&att);
    assert!(
        errors.iter().any(|e| e.contains("did you mean 'pass'")),
        "expected typo warning, got: {:?}",
        errors
    );
}

// --- Qual file with only comments ---

#[test]
fn test_parse_qual_file_only_comments() {
    let content = "// This is a comment\n// Another comment\n\n";
    let records = qual_file::parse_str(content).unwrap();
    assert!(records.is_empty());
}

#[test]
fn test_v3_roundtrip() {
    use qualifier::attestation::AuthorType;

    let dir = tempfile::tempdir().unwrap();
    let qual_path = dir.path().join("test.rs.qual");

    let att = attestation::finalize(Attestation {
        v: 3,
        record_type: "attestation".into(),
        artifact: "test.rs".into(),
        span: None,
        kind: Kind::Praise,
        score: 30,
        summary: "Great code".into(),
        detail: None,
        suggested_fix: None,
        tags: vec!["quality".into()],
        author: "alice@example.com".into(),
        author_type: Some(AuthorType::Human),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        r#ref: Some("git:3aba500".into()),
        supersedes: None,
        id: String::new(),
    });
    assert_eq!(att.v, 3);

    qual_file::append(&qual_path, &Record::Attestation(att.clone())).unwrap();
    let qf = qual_file::parse(&qual_path).unwrap();
    assert_eq!(qf.records.len(), 1);

    let parsed = qf.records[0].as_attestation().unwrap();
    assert_eq!(parsed.v, 3);
    assert_eq!(parsed.author_type, Some(AuthorType::Human));
    assert_eq!(parsed.r#ref.as_deref(), Some("git:3aba500"));
    assert_eq!(parsed.id, att.id);
}

#[test]
fn test_compact_snapshot_produces_epoch() {
    use qualifier::attestation::AuthorType;

    let records = vec![
        make_record("src/a.rs", Kind::Praise, 40, "good"),
        make_record("src/a.rs", Kind::Concern, -10, "meh"),
    ];
    let qf = QualFile {
        path: PathBuf::from("src/.qual"),
        artifact: "src/".into(),
        records,
    };

    let (snapped, _) = compact::snapshot(&qf);
    assert_eq!(snapped.records.len(), 1);

    let epoch = snapped.records[0].as_epoch().unwrap();
    assert_eq!(epoch.v, 3);
    assert_eq!(epoch.author_type, Some(AuthorType::Tool));
    assert_eq!(epoch.score, 30); // 40 + -10
}

#[test]
fn test_supersession_with_new_fields() {
    let original = make_record("mod.rs", Kind::Concern, -20, "problem");
    let replacement = Record::Attestation(attestation::finalize(Attestation {
        v: 3,
        record_type: "attestation".into(),
        artifact: "mod.rs".into(),
        span: None,
        kind: Kind::Pass,
        score: 20,
        summary: "fixed it".into(),
        detail: None,
        suggested_fix: None,
        tags: vec![],
        author: "test@test.com".into(),
        author_type: Some(qualifier::attestation::AuthorType::Human),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        r#ref: Some("git:abc123".into()),
        supersedes: Some(original.id().to_string()),
        id: String::new(),
    }));

    let all = vec![original.clone(), replacement.clone()];

    // Supersession should work
    let active = scoring::filter_superseded(&all);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id(), replacement.id());

    // Raw score should be replacement's score only
    assert_eq!(scoring::raw_score(&all), 20);
}
