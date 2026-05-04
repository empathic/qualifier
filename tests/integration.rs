use qualifier::annotation::{self, Annotation, AnnotationBody, Kind, Record};
use qualifier::compact::{self, filter_superseded};
use qualifier::graph;
use qualifier::qual_file::{self, QualFile};

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

// --- Golden ID tests (regression guards for content-addressed hashing) ---

#[test]
fn test_golden_annotation_id() {
    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "src/parser.rs".into(),
        issuer: "mailto:alice@example.com".into(),
        issuer_type: None,
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        id: String::new(),
        body: AnnotationBody {
            detail: None,
            kind: Kind::Concern,
            r#ref: None,
            references: None,
            span: None,
            suggested_fix: None,
            summary: "Panics on malformed input".into(),
            supersedes: None,
            tags: vec![],
        },
    });
    // ID is content-addressed: deterministic and matches generate_id.
    assert_eq!(annotation::generate_id(&att), att.id);
    assert_eq!(att.id.len(), 64);
}

#[test]
fn test_golden_epoch_id() {
    use qualifier::annotation::{self, Epoch, EpochBody, IssuerType};

    let epoch = annotation::finalize_epoch(Epoch {
        metabox: "1".into(),
        record_type: "epoch".into(),
        subject: "src/parser.rs".into(),
        issuer: "urn:qualifier:compact".into(),
        issuer_type: Some(IssuerType::Tool),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-25T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        id: String::new(),
        body: EpochBody {
            refs: vec!["aaa".into(), "bbb".into(), "ccc".into()],
            span: None,
            summary: "Compacted from 3 annotations".into(),
        },
    });
    assert_eq!(epoch.id.len(), 64);
}

#[test]
fn test_golden_dependency_id() {
    use qualifier::annotation::{self, DependencyBody, DependencyRecord};

    let dep = annotation::finalize_record(Record::Dependency(DependencyRecord {
        metabox: "1".into(),
        record_type: "dependency".into(),
        subject: "bin/server".into(),
        issuer: "https://build.example.com".into(),
        issuer_type: None,
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-25T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        id: String::new(),
        body: DependencyBody {
            depends_on: vec!["lib/auth".into(), "lib/http".into()],
        },
    }));
    assert_eq!(
        dep.id(),
        "dc97e9f3fa9b8d1f0c70e1a8aae30ddf305dba6f6c1d9720ef7e9d5db57eacfe",
        "Golden dependency ID changed! Canonical form or hashing is broken."
    );
}

// --- Full annotation lifecycle ---

#[test]
fn test_annotation_lifecycle_write_parse_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let qual_path = dir.path().join("src/parser.rs.qual");
    std::fs::create_dir_all(qual_path.parent().unwrap()).unwrap();

    let r1 = make_record("src/parser.rs", Kind::Concern, "Panics on bad input");
    let r2 = make_record("src/parser.rs", Kind::Praise, "Good test coverage");

    qual_file::append(&qual_path, &r1).unwrap();
    qual_file::append(&qual_path, &r2).unwrap();

    let qf = qual_file::parse(&qual_path).unwrap();
    assert_eq!(qf.records.len(), 2);
    assert_eq!(qf.records[0].id(), r1.id());
    assert_eq!(qf.records[1].id(), r2.id());

    // IDs are deterministic and valid
    let att1 = r1.as_annotation().unwrap();
    let att2 = r2.as_annotation().unwrap();
    assert_eq!(annotation::generate_id(att1), att1.id);
    assert_eq!(annotation::generate_id(att2), att2.id);
}

#[test]
fn test_annotation_id_is_content_addressed() {
    let att1 = make_att("foo.rs", Kind::Pass, "ok");
    let att2 = make_att("foo.rs", Kind::Pass, "ok");
    // Same content, same ID
    assert_eq!(att1.id, att2.id);

    // Different content, different ID
    let att3 = make_att("foo.rs", Kind::Pass, "ok with extra commentary");
    assert_ne!(att1.id, att3.id);
}

// --- Compaction ---

#[test]
fn test_compaction_prune_removes_superseded() {
    let original = make_record("mod.rs", Kind::Concern, "bad");
    let fix = Record::Annotation(Box::new(annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "mod.rs".into(),
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
            summary: "fixed".into(),
            supersedes: Some(original.id().to_string()),
            tags: vec![],
        },
    })));
    let extra = make_record("mod.rs", Kind::Praise, "nice");

    let qf = QualFile {
        path: PathBuf::from("mod.rs.qual"),
        subject: "mod.rs".into(),
        records: vec![original, fix.clone(), extra.clone()],
    };

    // Prune: only the chain tip and the unrelated record survive.
    let (pruned, _) = compact::prune(&qf);
    assert_eq!(pruned.records.len(), 2);
    assert!(pruned.records.iter().any(|r| r.id() == fix.id()));
    assert!(pruned.records.iter().any(|r| r.id() == extra.id()));

    // Snapshot collapses everything to one epoch.
    let (snapped, _) = compact::snapshot(&qf);
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

    let found = qual_file::discover(dir.path(), true).unwrap();
    assert_eq!(found.len(), 3);

    let subjects: Vec<&str> = found.iter().map(|qf| qf.subject.as_str()).collect();
    assert!(subjects.iter().any(|a| a.ends_with("src/lib.rs")));
    assert!(subjects.iter().any(|a| a.ends_with("src/parser.rs")));
    assert!(subjects.iter().any(|a| a.ends_with("src/util/helpers.rs")));
}

// --- Supersession cycle detection ---

#[test]
fn test_supersession_cycle_detected() {
    let now = Utc::now();
    let a = Record::Annotation(Box::new(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "x".into(),
        issuer: "mailto:test@test.com".into(),
        issuer_type: None,
        created_at: now,
        id: "aaa".into(),
        body: AnnotationBody {
            detail: None,
            kind: Kind::Pass,
            r#ref: None,
            references: None,
            span: None,
            suggested_fix: None,
            summary: "a".into(),
            supersedes: Some("bbb".into()),
            tags: vec![],
        },
    }));
    let b = Record::Annotation(Box::new(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "x".into(),
        issuer: "mailto:test@test.com".into(),
        issuer_type: None,
        created_at: now,
        id: "bbb".into(),
        body: AnnotationBody {
            detail: None,
            kind: Kind::Pass,
            r#ref: None,
            references: None,
            span: None,
            suggested_fix: None,
            summary: "b".into(),
            supersedes: Some("aaa".into()),
            tags: vec![],
        },
    }));

    let result = annotation::check_supersession_cycles(&[a, b]);
    assert!(result.is_err());
}

// --- Graph cycle detection ---

#[test]
fn test_graph_cycle_rejected() {
    let graph_str = r#"{"subject":"a","depends_on":["b"]}
{"subject":"b","depends_on":["c"]}
{"subject":"c","depends_on":["a"]}
"#;
    let result = graph::parse_graph(graph_str);
    assert!(result.is_err());
}

// --- Cross-artifact supersession ---

#[test]
fn test_cross_artifact_supersession_rejected() {
    let a = make_record("foo.rs", Kind::Concern, "issue in foo");
    let b = Record::Annotation(Box::new(annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "bar.rs".into(),
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
            summary: "fix in bar".into(),
            supersedes: Some(a.id().to_string()),
            tags: vec![],
        },
    })));

    let result = annotation::validate_supersession_targets(&[a, b]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cross-subject"));
}

// --- Kind typo detection ---

#[test]
fn test_kind_typo_detected_in_validation() {
    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "x.rs".into(),
        issuer: "mailto:test@test.com".into(),
        issuer_type: None,
        created_at: Utc::now(),
        id: String::new(),
        body: AnnotationBody {
            detail: None,
            kind: Kind::Custom("pss".into()),
            r#ref: None,
            references: None,
            span: None,
            suggested_fix: None,
            summary: "oops".into(),
            supersedes: None,
            tags: vec![],
        },
    });

    let errors = annotation::validate(&att);
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
fn test_metabox_roundtrip() {
    use qualifier::annotation::IssuerType;

    let dir = tempfile::tempdir().unwrap();
    let qual_path = dir.path().join("test.rs.qual");

    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "test.rs".into(),
        issuer: "mailto:alice@example.com".into(),
        issuer_type: Some(IssuerType::Human),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        id: String::new(),
        body: AnnotationBody {
            detail: None,
            kind: Kind::Praise,
            r#ref: Some("git:3aba500".into()),
            references: None,
            span: None,
            suggested_fix: None,
            summary: "Great code".into(),
            supersedes: None,
            tags: vec!["quality".into()],
        },
    });
    assert_eq!(att.metabox, "1");

    qual_file::append(&qual_path, &Record::Annotation(Box::new(att.clone()))).unwrap();
    let qf = qual_file::parse(&qual_path).unwrap();
    assert_eq!(qf.records.len(), 1);

    let parsed = qf.records[0].as_annotation().unwrap();
    assert_eq!(parsed.metabox, "1");
    assert_eq!(parsed.issuer_type, Some(IssuerType::Human));
    assert_eq!(parsed.body.r#ref.as_deref(), Some("git:3aba500"));
    assert_eq!(parsed.id, att.id);
}

#[test]
fn test_compact_snapshot_produces_epoch() {
    use qualifier::annotation::IssuerType;

    let records = vec![
        make_record("src/a.rs", Kind::Praise, "good"),
        make_record("src/a.rs", Kind::Concern, "meh"),
    ];
    let qf = QualFile {
        path: PathBuf::from("src/.qual"),
        subject: "src/".into(),
        records,
    };

    let (snapped, _) = compact::snapshot(&qf);
    assert_eq!(snapped.records.len(), 1);

    let epoch = snapped.records[0].as_epoch().unwrap();
    assert_eq!(epoch.metabox, "1");
    assert_eq!(epoch.issuer_type, Some(IssuerType::Tool));
    assert_eq!(epoch.body.refs.len(), 2);
}

#[test]
fn test_supersession_filter() {
    let original = make_record("mod.rs", Kind::Concern, "problem");
    let replacement = Record::Annotation(Box::new(annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "mod.rs".into(),
        issuer: "mailto:test@test.com".into(),
        issuer_type: Some(qualifier::annotation::IssuerType::Human),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-02-24T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        id: String::new(),
        body: AnnotationBody {
            detail: None,
            kind: Kind::Pass,
            r#ref: Some("git:abc123".into()),
            references: None,
            span: None,
            suggested_fix: None,
            summary: "fixed it".into(),
            supersedes: Some(original.id().to_string()),
            tags: vec![],
        },
    })));

    let all = vec![original.clone(), replacement.clone()];

    let active = filter_superseded(&all);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id(), replacement.id());
}
