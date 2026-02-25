use qualifier::attestation::{self, Attestation, Kind};
use qualifier::compact;
use qualifier::graph;
use qualifier::qual_file::{self, QualFile};
use qualifier::scoring;

use chrono::Utc;
use std::path::PathBuf;

fn make_att(artifact: &str, kind: Kind, score: i32, summary: &str) -> Attestation {
    attestation::finalize(Attestation {
        v: 2,
        artifact: artifact.into(),
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
        epoch_refs: None,
        id: String::new(),
    })
}

// --- Full attestation lifecycle ---

#[test]
fn test_attestation_lifecycle_write_parse_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let qual_path = dir.path().join("src/parser.rs.qual");
    std::fs::create_dir_all(qual_path.parent().unwrap()).unwrap();

    let att1 = make_att("src/parser.rs", Kind::Concern, -30, "Panics on bad input");
    let att2 = make_att("src/parser.rs", Kind::Praise, 40, "Good test coverage");

    qual_file::append(&qual_path, &att1).unwrap();
    qual_file::append(&qual_path, &att2).unwrap();

    let qf = qual_file::parse(&qual_path).unwrap();
    assert_eq!(qf.attestations.len(), 2);
    assert_eq!(qf.attestations[0].id, att1.id);
    assert_eq!(qf.attestations[1].id, att2.id);

    // IDs are deterministic and valid
    assert_eq!(attestation::generate_id(&att1), att1.id);
    assert_eq!(attestation::generate_id(&att2), att2.id);
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
            attestations: vec![make_att("bin/server", Kind::Praise, 80, "solid")],
        },
        QualFile {
            path: PathBuf::from("lib/auth.qual"),
            artifact: "lib/auth".into(),
            attestations: vec![make_att("lib/auth", Kind::Praise, 60, "decent")],
        },
        QualFile {
            path: PathBuf::from("lib/http.qual"),
            artifact: "lib/http".into(),
            attestations: vec![make_att("lib/http", Kind::Praise, 70, "good")],
        },
        QualFile {
            path: PathBuf::from("lib/crypto.qual"),
            artifact: "lib/crypto".into(),
            attestations: vec![make_att("lib/crypto", Kind::Blocker, -40, "vulnerable")],
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
        attestations: vec![make_att("standalone", Kind::Praise, 50, "fine")],
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
    let original = make_att("mod.rs", Kind::Concern, -30, "bad");
    let fix = attestation::finalize(Attestation {
        v: 2,
        artifact: "mod.rs".into(),
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
        supersedes: Some(original.id.clone()),
        epoch_refs: None,
        id: String::new(),
    });
    let extra = make_att("mod.rs", Kind::Praise, 40, "nice");

    let qf = QualFile {
        path: PathBuf::from("mod.rs.qual"),
        artifact: "mod.rs".into(),
        attestations: vec![original, fix, extra],
    };

    let score_before = scoring::raw_score(&qf.attestations);

    // Prune
    let (pruned, _) = compact::prune(&qf);
    assert_eq!(scoring::raw_score(&pruned.attestations), score_before);

    // Snapshot
    let (snapped, _) = compact::snapshot(&qf);
    assert_eq!(scoring::raw_score(&snapped.attestations), score_before);
    assert_eq!(snapped.attestations.len(), 1);
    assert_eq!(snapped.attestations[0].kind, Kind::Epoch);
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
    let mut a = Attestation {
        v: 2,
        artifact: "x".into(),
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
        supersedes: None,
        epoch_refs: None,
        id: "aaa".into(),
    };
    let mut b = a.clone();
    b.id = "bbb".into();
    b.supersedes = Some("aaa".into());
    a.supersedes = Some("bbb".into());

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
    let a = make_att("foo.rs", Kind::Concern, -10, "issue in foo");
    let b = attestation::finalize(Attestation {
        v: 2,
        artifact: "bar.rs".into(),
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
        supersedes: Some(a.id.clone()),
        epoch_refs: None,
        id: String::new(),
    });

    let result = attestation::validate_supersession_targets(&[a, b]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cross-artifact"));
}

// --- Kind typo detection ---

#[test]
fn test_kind_typo_detected_in_validation() {
    let mut att = make_att("x.rs", Kind::Custom("pss".into()), 10, "oops");
    att.id = attestation::generate_id(&att);
    // Need to use Custom kind directly since make_att uses finalize
    let att = attestation::finalize(Attestation {
        v: 2,
        artifact: "x.rs".into(),
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
        epoch_refs: None,
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
    let atts = qual_file::parse_str(content).unwrap();
    assert!(atts.is_empty());
}

#[test]
fn test_v2_roundtrip() {
    use qualifier::attestation::AuthorType;

    let dir = tempfile::tempdir().unwrap();
    let qual_path = dir.path().join("test.rs.qual");

    let att = attestation::finalize(Attestation {
        v: 2,
        artifact: "test.rs".into(),
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
        epoch_refs: None,
        id: String::new(),
    });
    assert_eq!(att.v, 2);

    qual_file::append(&qual_path, &att).unwrap();
    let qf = qual_file::parse(&qual_path).unwrap();
    assert_eq!(qf.attestations.len(), 1);

    let parsed = &qf.attestations[0];
    assert_eq!(parsed.v, 2);
    assert_eq!(parsed.author_type, Some(AuthorType::Human));
    assert_eq!(parsed.r#ref.as_deref(), Some("git:3aba500"));
    assert_eq!(parsed.id, att.id);
}

#[test]
fn test_compact_snapshot_produces_v2() {
    // Create v1-style attestations (finalize makes them v2, but the point is
    // the snapshot output should always be v2 with author_type=tool)
    use qualifier::attestation::AuthorType;

    let atts = vec![
        make_att("src/a.rs", Kind::Praise, 40, "good"),
        make_att("src/a.rs", Kind::Concern, -10, "meh"),
    ];
    let qf = QualFile {
        path: PathBuf::from("src/.qual"),
        artifact: "src/".into(),
        attestations: atts,
    };

    let (snapped, _) = compact::snapshot(&qf);
    assert_eq!(snapped.attestations.len(), 1);

    let epoch = &snapped.attestations[0];
    assert_eq!(epoch.v, 2);
    assert_eq!(epoch.author_type, Some(AuthorType::Tool));
    assert_eq!(epoch.score, 30); // 40 + -10
}

#[test]
fn test_supersession_with_new_fields() {
    let original = make_att("mod.rs", Kind::Concern, -20, "problem");
    let replacement = attestation::finalize(Attestation {
        v: 2,
        artifact: "mod.rs".into(),
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
        supersedes: Some(original.id.clone()),
        epoch_refs: None,
        id: String::new(),
    });

    let all = vec![original.clone(), replacement.clone()];

    // Supersession should work
    let active = scoring::filter_superseded(&all);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, replacement.id);

    // Raw score should be replacement's score only
    assert_eq!(scoring::raw_score(&all), 20);
}
