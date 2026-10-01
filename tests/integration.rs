use qualifier::annotation::{self, Annotation, AnnotationBody, Kind, Record};
use qualifier::compact::{self, filter_superseded};
use qualifier::qual_file::{self, QualFile};
use qualifier::threads::{ThreadState, build_threads};

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
            .with_timezone(&Utc)
            .into(),
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

// --- Golden ID tests (regression guards for content-addressed hashing) ---

/// The MCF of `record`: its serialization with `id` set to "".
fn canonical_form(record: &Record) -> String {
    let json = serde_json::to_string(record).unwrap();
    json.replacen(&format!("\"id\":\"{}\"", record.id()), "\"id\":\"\"", 1)
}

#[test]
fn test_golden_annotation_id() {
    use qualifier::annotation::{IssuerType, Position, Span};

    // Every field populated, so reordering or renaming any of them (or
    // changing how a field serializes) changes the pinned ID.
    let att = annotation::finalize(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "src/parser.rs".into(),
        issuer: "mailto:alice@example.com".into(),
        issuer_type: Some(IssuerType::Human),
        created_at: "2026-02-24T10:00:00.250Z".parse().unwrap(),
        id: String::new(),
        body: AnnotationBody {
            detail: Some("Index past the end of the token buffer.".into()),
            kind: Kind::Concern,
            r#ref: Some("git:3aba500".into()),
            references: Some("a".repeat(64)),
            span: Some(Span {
                start: Position {
                    line: 42,
                    col: Some(5),
                },
                end: Some(Position {
                    line: 58,
                    col: Some(80),
                }),
                content_hash: Some("c".repeat(64)),
            }),
            suggested_fix: Some("Check the length first.".into()),
            summary: "Panics on malformed input".into(),
            supersedes: Some("b".repeat(64)),
            tags: vec!["security".into(), "parser".into()],
            extra: Default::default(),
        },
    });
    let record = Record::Annotation(Box::new(att));
    assert_eq!(
        canonical_form(&record),
        format!(
            concat!(
                r#"{{"metabox":"1","type":"annotation","subject":"src/parser.rs","#,
                r#""issuer":"mailto:alice@example.com","issuer_type":"human","#,
                r#""created_at":"2026-02-24T10:00:00.250Z","id":"","body":{{"#,
                r#""detail":"Index past the end of the token buffer.","kind":"concern","#,
                r#""ref":"git:3aba500","references":"{a}","#,
                r#""span":{{"start":{{"line":42,"col":5}},"end":{{"line":58,"col":80}},"#,
                r#""content_hash":"{c}"}},"suggested_fix":"Check the length first.","#,
                r#""summary":"Panics on malformed input","supersedes":"{b}","#,
                r#""tags":["security","parser"]}}}}"#
            ),
            a = "a".repeat(64),
            b = "b".repeat(64),
            c = "c".repeat(64),
        )
    );
    assert_eq!(
        record.id(),
        blake3::hash(canonical_form(&record).as_bytes())
            .to_hex()
            .to_string()
    );
    assert_eq!(
        record.id(),
        "0a8c5336e9d37411907ad014f952d35d446ce74ca49e7e196d8e3372121a359e",
        "Golden annotation ID changed! Canonical form or hashing is broken."
    );
}

#[test]
fn test_golden_epoch_id() {
    use qualifier::annotation::{self, Epoch, EpochBody, IssuerType, Position, Span};

    let epoch = annotation::finalize_epoch(Epoch {
        metabox: "1".into(),
        record_type: "epoch".into(),
        subject: "src/parser.rs".into(),
        issuer: "urn:qualifier:compact".into(),
        issuer_type: Some(IssuerType::Tool),
        created_at: "2026-02-25T12:00:00Z".parse().unwrap(),
        id: String::new(),
        body: EpochBody {
            refs: vec!["aaa".into(), "bbb".into(), "ccc".into()],
            span: Some(Span {
                start: Position { line: 1, col: None },
                end: Some(Position { line: 9, col: None }),
                content_hash: Some("d".repeat(64)),
            }),
            summary: "Compacted from 3 records".into(),
            extra: Default::default(),
        },
    });
    let record = Record::Epoch(epoch);
    assert_eq!(
        canonical_form(&record),
        format!(
            concat!(
                r#"{{"metabox":"1","type":"epoch","subject":"src/parser.rs","#,
                r#""issuer":"urn:qualifier:compact","issuer_type":"tool","#,
                r#""created_at":"2026-02-25T12:00:00Z","id":"","body":{{"#,
                r#""refs":["aaa","bbb","ccc"],"#,
                r#""span":{{"start":{{"line":1}},"end":{{"line":9}},"content_hash":"{d}"}},"#,
                r#""summary":"Compacted from 3 records"}}}}"#
            ),
            d = "d".repeat(64),
        )
    );
    assert_eq!(
        record.id(),
        blake3::hash(canonical_form(&record).as_bytes())
            .to_hex()
            .to_string()
    );
    assert_eq!(
        record.id(),
        "6f0ad2ce85702b16851427de08637a6851f9b0f9acaae0f9c351e45a49048caa",
        "Golden epoch ID changed! Canonical form or hashing is broken."
    );
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
            .with_timezone(&Utc)
            .into(),
        id: String::new(),
        body: DependencyBody {
            depends_on: vec!["lib/auth".into(), "lib/http".into()],
            extra: Default::default(),
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
            .with_timezone(&Utc)
            .into(),
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
            extra: Default::default(),
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
        created_at: now.into(),
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
            extra: Default::default(),
        },
    }));
    let b = Record::Annotation(Box::new(Annotation {
        metabox: "1".into(),
        record_type: "annotation".into(),
        subject: "x".into(),
        issuer: "mailto:test@test.com".into(),
        issuer_type: None,
        created_at: now.into(),
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
            extra: Default::default(),
        },
    }));

    let result = annotation::check_supersession_cycles(&[a, b]);
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
            .with_timezone(&Utc)
            .into(),
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
            extra: Default::default(),
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
        created_at: Utc::now().into(),
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
            extra: Default::default(),
        },
    });

    let errors = annotation::validate(&att);
    assert!(
        errors.iter().any(|e| e.contains("did you mean 'pass'")),
        "expected typo warning, got: {:?}",
        errors
    );
}

#[test]
fn test_kind_typo_of_resolve_detected_in_validation() {
    let mut att = make_att("x.rs", Kind::Custom("resovle".into()), "typo");
    att = annotation::finalize(att);
    let errors = annotation::validate(&att);
    assert!(
        errors.iter().any(|e| e.contains("did you mean 'resolve'?")),
        "expected typo warning, got: {errors:?}"
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
            .with_timezone(&Utc)
            .into(),
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
            extra: Default::default(),
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
            .with_timezone(&Utc)
            .into(),
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
            extra: Default::default(),
        },
    })));

    let all = vec![original.clone(), replacement.clone()];

    let active = filter_superseded(&all);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id(), replacement.id());
}

// --- threads ---

fn at(secs: i64) -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(1_780_000_000 + secs, 0).unwrap()
}

/// An annotation at `secs` with optional references/supersedes.
fn ann(
    subject: &str,
    kind: Kind,
    summary: &str,
    secs: i64,
    references: Option<&str>,
    supersedes: Option<&str>,
) -> Record {
    let mut a = make_att(subject, kind, summary);
    a.created_at = at(secs).into();
    a.body.references = references.map(String::from);
    a.body.supersedes = supersedes.map(String::from);
    Record::Annotation(Box::new(annotation::finalize(a)))
}

fn summary_of(r: &Record) -> &str {
    &r.as_annotation().unwrap().body.summary
}

#[test]
fn test_threads_single_open_root() {
    let root = ann("a.rs", Kind::Concern, "root", 0, None, None);
    let records = vec![root.clone()];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert!(threads[0].open);
    assert_eq!(threads[0].origin, root.id());
    assert!(threads[0].replies.is_empty());
    assert!(threads[0].closed_by.is_none());
}

#[test]
fn test_threads_nested_replies_join_root() {
    let root = ann("a.rs", Kind::Concern, "root", 0, None, None);
    let r1 = ann("a.rs", Kind::Comment, "reply", 10, Some(root.id()), None);
    let r2 = ann(
        "a.rs",
        Kind::Comment,
        "reply to reply",
        20,
        Some(r1.id()),
        None,
    );
    let records = vec![r2, root, r1];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    let summaries: Vec<&str> = threads[0]
        .replies
        .iter()
        .map(|e| summary_of(e.record))
        .collect();
    assert_eq!(summaries, vec!["reply", "reply to reply"], "oldest first");
    assert_eq!(threads[0].latest_at, at(20));
}

#[test]
fn test_threads_resolved_root_is_closed() {
    let root = ann("a.rs", Kind::Concern, "root", 0, None, None);
    let fix = ann("a.rs", Kind::Resolve, "fixed", 10, None, Some(root.id()));
    let records = vec![root.clone(), fix.clone()];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert!(!threads[0].open);
    assert_eq!(threads[0].root.id(), root.id());
    assert_eq!(threads[0].closed_by.map(|r| r.id()), Some(fix.id()));
}

/// `ann` with tags, re-finalized so the ID covers them.
fn tagged(mut r: Record, tags: &[&str]) -> Record {
    let Record::Annotation(a) = &mut r else {
        unreachable!()
    };
    a.body.tags = tags.iter().map(|t| t.to_string()).collect();
    Record::Annotation(Box::new(annotation::finalize((**a).clone())))
}

#[test]
fn test_thread_state_follows_latest_status_and_closer() {
    let root = ann("a.rs", Kind::Concern, "root", 0, None, None);
    let ask = tagged(
        ann("a.rs", Kind::Comment, "A or B?", 10, Some(root.id()), None),
        &["status:needs-decision:mailto:o@x"],
    );
    let records = vec![root.clone(), ask.clone()];
    let threads = build_threads(&records);
    assert!(matches!(
        threads[0].state(),
        ThreadState::NeedsDecision {
            addressee: Some("mailto:o@x")
        }
    ));

    // Closed while the question was still open: pending.
    let fix = tagged(
        ann("a.rs", Kind::Resolve, "moved on", 20, None, Some(root.id())),
        &["reason:wontfix"],
    );
    let records = vec![root.clone(), ask.clone(), fix.clone()];
    let threads = build_threads(&records);
    match threads[0].state() {
        ThreadState::Closed {
            reason,
            closer,
            pending_question,
        } => {
            assert_eq!(reason, Some("wontfix"));
            assert_eq!(closer.id(), fix.id());
            assert!(pending_question);
        }
        other => panic!("expected closed, got {other:?}"),
    }

    // A decided reply before the close answers the question.
    let decided = tagged(
        ann("a.rs", Kind::Comment, "B", 15, Some(root.id()), None),
        &["status:decided"],
    );
    let records = vec![root, ask, decided, fix];
    let threads = build_threads(&records);
    assert!(matches!(
        threads[0].state(),
        ThreadState::Closed {
            pending_question: false,
            ..
        }
    ));
}

#[test]
fn test_threads_rerecorded_root_keeps_replies() {
    let a = ann("a.rs", Kind::Concern, "v1", 0, None, None);
    let reply = ann("a.rs", Kind::Comment, "on v1", 5, Some(a.id()), None);
    let b = ann("a.rs", Kind::Concern, "v2", 10, None, Some(a.id()));
    let records = vec![a.clone(), reply, b.clone()];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1, "a re-recorded root stays one thread");
    assert!(threads[0].open);
    assert_eq!(threads[0].root.id(), b.id(), "root is the live head");
    assert_eq!(threads[0].origin, a.id());
    assert_eq!(threads[0].replies.len(), 1);
    let history_ids: Vec<&str> = threads[0].history.iter().map(|r| r.id()).collect();
    assert_eq!(history_ids, vec![a.id()]);
}

#[test]
fn test_threads_edited_reply_marks_old_inactive() {
    let root = ann("a.rs", Kind::Concern, "root", 0, None, None);
    let r1 = ann("a.rs", Kind::Comment, "draft", 10, Some(root.id()), None);
    let r2 = ann(
        "a.rs",
        Kind::Comment,
        "final",
        20,
        Some(root.id()),
        Some(r1.id()),
    );
    let records = vec![root, r1, r2];
    let threads = build_threads(&records);
    let entries: Vec<(&str, bool)> = threads[0]
        .replies
        .iter()
        .map(|e| (summary_of(e.record), e.active))
        .collect();
    assert_eq!(entries, vec![("draft", false), ("final", true)]);
}

#[test]
fn test_threads_resolving_a_reply_keeps_thread_open() {
    let root = ann("a.rs", Kind::Concern, "root", 0, None, None);
    let r1 = ann(
        "a.rs",
        Kind::Comment,
        "wrong claim",
        10,
        Some(root.id()),
        None,
    );
    let close_reply = ann("a.rs", Kind::Resolve, "retracted", 20, None, Some(r1.id()));
    let records = vec![root, r1, close_reply];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert!(
        threads[0].open,
        "resolving a reply must not close the thread"
    );
    assert_eq!(threads[0].replies.len(), 2);
}

#[test]
fn test_threads_ignore_epochs() {
    let qf = QualFile {
        path: PathBuf::from("a.rs.qual"),
        subject: "a.rs".into(),
        records: vec![
            ann("a.rs", Kind::Concern, "one", 0, None, None),
            ann("a.rs", Kind::Suggestion, "two", 10, None, None),
        ],
    };
    let (snap, _) = compact::snapshot(&qf);
    let mut records = snap.records.clone();
    records.push(ann("a.rs", Kind::Blocker, "after snapshot", 20, None, None));
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert_eq!(summary_of(threads[0].root), "after snapshot");
    assert!(threads[0].replies.is_empty());
    assert!(threads[0].history.is_empty());
}

#[test]
fn test_threads_ordered_by_subject_then_line() {
    let mut b = make_att("b.rs", Kind::Concern, "b");
    b.created_at = at(0).into();
    let mut a2 = make_att("a.rs", Kind::Concern, "a line 20");
    a2.created_at = at(1).into();
    a2.body.span = Some(annotation::parse_span("20").unwrap());
    let mut a1 = make_att("a.rs", Kind::Concern, "a line 5");
    a1.created_at = at(2).into();
    a1.body.span = Some(annotation::parse_span("5").unwrap());
    let records: Vec<Record> = [b, a2, a1]
        .into_iter()
        .map(|a| Record::Annotation(Box::new(annotation::finalize(a))))
        .collect();
    let threads = build_threads(&records);
    let order: Vec<&str> = threads.iter().map(|t| summary_of(t.root)).collect();
    assert_eq!(order, vec!["a line 5", "a line 20", "b"]);
}

#[test]
fn test_threads_reply_supersedes_root_stays_a_reply() {
    // A reply that also supersedes its parent is still a reply (it
    // `references` an existing record), so it never joins the root chain.
    let a = ann("a.rs", Kind::Concern, "root", 0, None, None);
    let r = ann(
        "a.rs",
        Kind::Comment,
        "reply that supersedes",
        10,
        Some(a.id()),
        Some(a.id()),
    );
    let records = vec![a.clone(), r.clone()];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert!(threads[0].open);
    assert_eq!(threads[0].root.id(), a.id());
    assert_eq!(threads[0].replies.len(), 1);
    assert!(threads[0].history.is_empty());
}

#[test]
fn test_threads_fork_open_when_any_non_resolve_tip() {
    let a = ann("a.rs", Kind::Concern, "a", 0, None, None);
    let b = ann("a.rs", Kind::Concern, "b", 10, None, Some(a.id()));
    let c = ann("a.rs", Kind::Resolve, "c", 20, None, Some(a.id()));
    let records = vec![a.clone(), b.clone(), c.clone()];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert!(
        threads[0].open,
        "a live non-resolve tip keeps the thread open"
    );
    assert_eq!(threads[0].root.id(), b.id());
    assert!(threads[0].closed_by.is_none());
    let history_ids: Vec<&str> = threads[0].history.iter().map(|r| r.id()).collect();
    assert_eq!(history_ids, vec![a.id(), c.id()]);
}

#[test]
fn test_threads_closed_when_all_tips_resolve() {
    let a = ann("a.rs", Kind::Concern, "a", 0, None, None);
    let b = ann("a.rs", Kind::Concern, "b", 10, None, Some(a.id()));
    let r1 = ann("a.rs", Kind::Resolve, "closed", 20, None, Some(b.id()));
    let records = vec![a.clone(), b.clone(), r1.clone()];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert!(!threads[0].open);
    assert_eq!(threads[0].closed_by.map(|r| r.id()), Some(r1.id()));
    assert_eq!(threads[0].root.id(), b.id());
    let history_ids: Vec<&str> = threads[0].history.iter().map(|r| r.id()).collect();
    assert_eq!(history_ids, vec![a.id()]);
}

#[test]
fn test_threads_dangling_targets_form_own_thread() {
    let fake_id = "nonexistent_id_12345";
    let r = ann(
        "a.rs",
        Kind::Comment,
        "orphan",
        0,
        Some(fake_id),
        Some(fake_id),
    );
    let records = vec![r.clone()];
    let threads = build_threads(&records);
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].origin, r.id());
    assert_eq!(threads[0].root.id(), r.id());
    assert!(threads[0].replies.is_empty());
    assert!(threads[0].history.is_empty());
}

#[test]
fn test_threads_tie_break_by_origin_id_is_deterministic() {
    // Same subject, same (absent) span line, same created_at: only the
    // origin ID orders them, and it must not depend on input order.
    let a = ann("a.rs", Kind::Concern, "root one", 0, None, None);
    let b = ann("a.rs", Kind::Concern, "root two", 0, None, None);
    let forward = vec![a.clone(), b.clone()];
    let backward = vec![b.clone(), a.clone()];

    let order_forward: Vec<&str> = build_threads(&forward).iter().map(|t| t.origin).collect();
    let threads_backward = build_threads(&backward);
    let order_backward: Vec<&str> = threads_backward.iter().map(|t| t.origin).collect();

    let mut expected = vec![a.id(), b.id()];
    expected.sort();
    assert_eq!(order_forward, expected);
    assert_eq!(order_backward, expected);
}

// --- Canonical form stability ---

/// Every record checked into this repository was written by qualifier. Their
/// stored IDs must keep verifying across changes to the canonical form.
#[test]
fn test_repository_record_ids_still_verify() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let qual_files = qual_file::discover(root, true).unwrap();
    let mut checked = 0;
    for qf in &qual_files {
        for record in &qf.records {
            if matches!(record, Record::Unknown(_)) {
                continue;
            }
            assert_eq!(
                annotation::generate_record_id(record),
                record.id(),
                "stored ID no longer verifies in {}",
                qf.path.display()
            );
            checked += 1;
        }
    }
    assert!(
        checked > 100,
        "expected the repository's own records, found {checked}"
    );
}

#[test]
fn test_custom_body_fields_survive_roundtrip_and_hash_in_key_order() {
    let line = r#"{"metabox":"1","type":"annotation","subject":"x.rs","issuer":"mailto:t@t.com","created_at":"2026-02-24T10:00:00Z","id":"","body":{"kind":"concern","score":-20,"summary":"custom field","zeta":{"b":1,"a":2}}}"#;
    let record: Record = serde_json::from_str(line).unwrap();
    let record = annotation::finalize_record(record);
    let json = serde_json::to_string(&record).unwrap();
    // Custom fields sort together with the defined ones.
    assert!(
        json.contains(
            r#""body":{"kind":"concern","score":-20,"summary":"custom field","zeta":{"a":2,"b":1}}"#
        ),
        "unexpected body serialization: {json}"
    );
    // The ID is the BLAKE3 hash of that serialization with `id` emptied.
    let canonical = json.replace(&format!(r#""id":"{}""#, record.id()), r#""id":"""#);
    assert_eq!(
        record.id(),
        blake3::hash(canonical.as_bytes()).to_hex().to_string()
    );
    let reparsed: Record = serde_json::from_str(&json).unwrap();
    assert_eq!(annotation::generate_record_id(&reparsed), record.id());
    assert_eq!(reparsed, record);
}

#[test]
fn test_custom_body_fields_on_epoch_and_dependency_survive() {
    let epoch = r#"{"metabox":"1","type":"epoch","subject":"x.rs","issuer":"urn:qualifier:compact","created_at":"2026-02-24T10:00:00Z","id":"","body":{"note":"kept","refs":["a"],"summary":"s"}}"#;
    let dep = r#"{"metabox":"1","type":"dependency","subject":"x.rs","issuer":"urn:t:t","created_at":"2026-02-24T10:00:00Z","id":"","body":{"depends_on":["y"],"weight":3}}"#;
    for (line, expect) in [
        (
            epoch,
            r#""body":{"note":"kept","refs":["a"],"summary":"s"}"#,
        ),
        (dep, r#""body":{"depends_on":["y"],"weight":3}"#),
    ] {
        let record = annotation::finalize_record(serde_json::from_str(line).unwrap());
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains(expect), "unexpected serialization: {json}");
        let reparsed: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(annotation::generate_record_id(&reparsed), record.id());
    }
}

// --- created_at is hashed as written ---

#[test]
fn test_created_at_is_kept_and_hashed_as_written() {
    use qualifier::annotation::Timestamp;

    let line = r#"{"metabox":"1","type":"annotation","subject":"x.rs","issuer":"mailto:t@t.com","created_at":"2026-02-24T10:00:00.5+00:00","id":"","body":{"kind":"concern","summary":"s"}}"#;
    let record = annotation::finalize_record(serde_json::from_str(line).unwrap());
    let json = serde_json::to_string(&record).unwrap();
    assert!(
        json.contains(r#""created_at":"2026-02-24T10:00:00.5+00:00""#),
        "{json}"
    );
    assert_eq!(
        record.id(),
        "5a9a039a6c9a66f708ed504e6cf23df7f62af152c7030703f8a0527383cf3242",
        "golden ID for a non-canonical created_at"
    );
    // The input line is already in canonical form, with `id` empty.
    assert_eq!(
        record.id(),
        blake3::hash(line.as_bytes()).to_hex().to_string()
    );

    // The same instant in canonical form is a different record.
    let canonical_line = line.replace("10:00:00.5+00:00", "10:00:00.500Z");
    let other = annotation::finalize_record(serde_json::from_str(&canonical_line).unwrap());
    assert_ne!(other.id(), record.id());
    let a = record.as_annotation().unwrap();
    let b = other.as_annotation().unwrap();
    assert_eq!(a.created_at.instant(), b.created_at.instant());

    // Records qualifier creates use the canonical form.
    let t: Timestamp = "2026-02-24T12:00:00.5+02:00".parse().unwrap();
    assert_eq!(
        Timestamp::from(t.instant()).as_str(),
        "2026-02-24T10:00:00.500Z"
    );
    assert!(Timestamp::parse("2026-02-24 10:00").is_err());
}

#[test]
fn test_canonical_timestamp_matches_previous_serialization() {
    use qualifier::annotation::Timestamp;

    // Records written before created_at kept its text were serialized by
    // chrono's serde impl; the canonical form must match it byte for byte.
    for text in [
        "2026-02-24T10:00:00Z",
        "2026-02-24T10:00:00.5Z",
        "2026-02-24T10:00:00.123456Z",
        "2026-02-24T10:00:00.123456789Z",
        "2026-02-24T10:00:00.000001+05:30",
    ] {
        let instant = chrono::DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc);
        let chrono_json = serde_json::to_string(&instant).unwrap();
        assert_eq!(
            format!("\"{}\"", Timestamp::canonical(instant)),
            chrono_json
        );
    }
}

#[test]
fn test_invalid_created_at_is_rejected() {
    let line = r#"{"metabox":"1","type":"annotation","subject":"x.rs","issuer":"mailto:t@t.com","created_at":"yesterday","id":"","body":{"kind":"concern","summary":"s"}}"#;
    let err = qual_file::parse_str(line).unwrap_err().to_string();
    assert!(err.contains("RFC 3339"), "{err}");
}

// --- Records of custom types ---

#[test]
fn test_golden_custom_type_id_and_envelope_order() {
    // Keys deliberately out of order, plus an extra top-level field.
    let input = r#"{"body":{"z":1,"a":{"y":2,"x":3}},"created_at":"2026-04-01T00:00:00Z","extension":true,"id":"","issuer":"https://ci.example.com","issuer_type":"tool","metabox":"1","subject":"widget.rs","type":"https://example.com/custom/v1"}"#;
    let record = annotation::finalize_record(serde_json::from_str(input).unwrap());
    assert!(matches!(record, Record::Unknown(_)));

    let json = serde_json::to_string(&record).unwrap();
    let expected_canonical = concat!(
        r#"{"metabox":"1","type":"https://example.com/custom/v1","subject":"widget.rs","#,
        r#""issuer":"https://ci.example.com","issuer_type":"tool","#,
        r#""created_at":"2026-04-01T00:00:00Z","id":"","body":{"a":{"x":3,"y":2},"z":1},"#,
        r#""extension":true}"#
    );
    assert_eq!(
        json.replacen(&format!(r#""id":"{}""#, record.id()), r#""id":"""#, 1),
        expected_canonical
    );
    assert_eq!(
        record.id(),
        blake3::hash(expected_canonical.as_bytes())
            .to_hex()
            .to_string()
    );
    assert_eq!(
        record.id(),
        "47150273a6852379894eedf80d4677edfa64db064df4e82400ba2fb25d60f9a3",
        "Golden custom-type ID changed! Canonical form or hashing is broken."
    );
    assert_eq!(annotation::generate_record_id(&record), record.id());
}

// --- Lenient parsing ---

#[test]
fn test_parse_lenient_skips_bad_lines_and_discover_keeps_going() {
    let dir = tempfile::tempdir().unwrap();
    let good = serde_json::to_string(&make_record("a.rs", Kind::Praise, "fine")).unwrap();
    let content = format!("{good}\n{{not json\n\n{good}\n");
    std::fs::write(dir.path().join(".qual"), &content).unwrap();
    std::fs::create_dir_all(dir.path().join("b")).unwrap();
    std::fs::write(dir.path().join("b/.qual"), format!("{good}\n")).unwrap();

    let path = dir.path().join(".qual");
    let (qf, issues) = qual_file::parse_lenient(&path).unwrap();
    assert_eq!(qf.records.len(), 2);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].line, 2);
    assert!(issues[0].to_string().contains(".qual:2:"));

    let err = qual_file::parse(&path).unwrap_err().to_string();
    assert!(err.contains(".qual:2:"), "{err}");

    let found = qual_file::discover(dir.path(), true).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found.iter().map(|qf| qf.records.len()).sum::<usize>(), 3);
}

#[test]
fn test_discover_honors_gitignore_outside_git() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".hg")).unwrap();
    std::fs::write(dir.path().join(".gitignore"), "vendor/\n").unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    let line = serde_json::to_string(&make_record("vendor/v.rs", Kind::Praise, "x")).unwrap();
    std::fs::write(dir.path().join("vendor/.qual"), format!("{line}\n")).unwrap();
    std::fs::write(dir.path().join(".qual"), format!("{line}\n")).unwrap();

    let found = qual_file::discover(dir.path(), true).unwrap();
    let paths: Vec<_> = found.iter().map(|qf| qf.path.clone()).collect();
    assert_eq!(paths, vec![dir.path().join(".qual")]);
    assert_eq!(qual_file::discover(dir.path(), false).unwrap().len(), 2);
}
