use std::collections::{HashMap, HashSet};

use crate::attestation::{Record, clamp_score};
use crate::graph::DependencyGraph;
use crate::qual_file::QualFile;

/// Score report for a single artifact.
#[derive(Debug, Clone)]
pub struct ScoreReport {
    /// Sum of non-superseded scored record scores, clamped to [-100, 100].
    pub raw: i32,
    /// Raw score floored by worst dependency effective score.
    pub effective: i32,
    /// The dependency path that limits the effective score, if any.
    pub limiting_path: Option<Vec<String>>,
}

/// Compute the raw score for a set of records (single artifact).
///
/// Filters out superseded records, sums scores of scored types
/// (attestations and epochs), and clamps to [-100, 100].
pub fn raw_score(records: &[Record]) -> i32 {
    let active = filter_superseded(records);
    let sum = active
        .iter()
        .filter_map(|r| r.score())
        .fold(0i32, |acc, s| acc.saturating_add(s));
    clamp_score(sum)
}

/// Filter out superseded records, returning only the active ones.
///
/// A record is superseded if any other record's `supersedes` field
/// points to its ID. Only attestations can supersede or be superseded.
/// Non-attestation records always pass through.
pub fn filter_superseded(records: &[Record]) -> Vec<&Record> {
    // Collect all IDs that are superseded by something
    let superseded_ids: HashSet<&str> = records.iter().filter_map(|r| r.supersedes()).collect();

    records
        .iter()
        .filter(|r| !superseded_ids.contains(r.id()))
        .collect()
}

/// Compute effective scores for all artifacts in the graph.
///
/// Uses topological ordering to propagate scores from leaves to roots.
/// An artifact's effective score is the minimum of its raw score and the
/// effective scores of all its dependencies.
///
/// Artifacts that appear in qual files but not in the graph are included
/// with effective score = raw score (no dependencies).
pub fn effective_scores(
    graph: &DependencyGraph,
    qual_files: &[QualFile],
) -> HashMap<String, ScoreReport> {
    // Build a map of subject -> records
    let mut subject_records: HashMap<&str, Vec<&Record>> = HashMap::new();
    for qf in qual_files {
        for record in &qf.records {
            subject_records
                .entry(record.subject())
                .or_default()
                .push(record);
        }
    }

    // Compute raw scores for all known subjects
    let mut raw_scores: HashMap<String, i32> = HashMap::new();
    for (subject, records) in &subject_records {
        raw_scores.insert(subject.to_string(), raw_score_from_refs(records));
    }

    // Include graph artifacts with no records (raw score = 0)
    for artifact in graph.artifacts() {
        raw_scores.entry(artifact.to_string()).or_insert(0);
    }

    let mut reports: HashMap<String, ScoreReport> = HashMap::new();

    // If graph is empty, all artifacts get effective = raw
    if graph.is_empty() {
        for (subject, &raw) in &raw_scores {
            reports.insert(
                subject.clone(),
                ScoreReport {
                    raw,
                    effective: raw,
                    limiting_path: None,
                },
            );
        }
        return reports;
    }

    // Topological sort — dependencies processed before dependents
    let topo_order = match graph.toposort() {
        Ok(order) => order,
        Err(_) => {
            // If there's a cycle, fall back to raw = effective for everything
            for (subject, &raw) in &raw_scores {
                reports.insert(
                    subject.clone(),
                    ScoreReport {
                        raw,
                        effective: raw,
                        limiting_path: None,
                    },
                );
            }
            return reports;
        }
    };

    // Effective scores computed in topo order
    let mut effective: HashMap<String, i32> = HashMap::new();
    let mut limiting: HashMap<String, Vec<String>> = HashMap::new();

    for &artifact in &topo_order {
        let raw = *raw_scores.get(artifact).unwrap_or(&0);
        let deps = graph.dependencies(artifact);

        if deps.is_empty() {
            effective.insert(artifact.to_string(), raw);
        } else {
            // Find the worst dependency
            let mut min_eff = raw;
            let mut min_path: Option<Vec<String>> = None;

            for dep in &deps {
                let dep_eff = effective.get(*dep).copied().unwrap_or(0);
                if dep_eff < min_eff {
                    min_eff = dep_eff;
                    // Build the limiting path
                    let mut path = vec![dep.to_string()];
                    if let Some(dep_path) = limiting.get(*dep) {
                        path.extend(dep_path.iter().cloned());
                    }
                    min_path = Some(path);
                }
            }

            effective.insert(artifact.to_string(), min_eff);
            if let Some(path) = min_path {
                limiting.insert(artifact.to_string(), path);
            }
        }
    }

    // Build reports for all artifacts in the graph
    for &artifact in &topo_order {
        let raw = *raw_scores.get(artifact).unwrap_or(&0);
        let eff = effective.get(artifact).copied().unwrap_or(raw);
        let lim = if eff < raw {
            limiting.get(artifact).cloned()
        } else {
            None
        };

        reports.insert(
            artifact.to_string(),
            ScoreReport {
                raw,
                effective: eff,
                limiting_path: lim,
            },
        );
    }

    // Also include subjects with qual files but not in the graph
    for (subject, &raw) in &raw_scores {
        reports.entry(subject.clone()).or_insert(ScoreReport {
            raw,
            effective: raw,
            limiting_path: None,
        });
    }

    reports
}

/// Compute raw score from a slice of record references.
pub fn raw_score_from_refs(records: &[&Record]) -> i32 {
    let superseded_ids: HashSet<&str> = records.iter().filter_map(|r| r.supersedes()).collect();

    let sum = records
        .iter()
        .filter(|r| !superseded_ids.contains(r.id()))
        .filter_map(|r| r.score())
        .fold(0i32, |acc, s| acc.saturating_add(s));

    clamp_score(sum)
}

/// Describe the status of a score for display purposes.
///
/// Score severity takes priority; "limited" is appended when the effective
/// score is constrained by a dependency.
pub fn score_status(report: &ScoreReport) -> &'static str {
    let limited = report.limiting_path.is_some();
    if report.effective < 0 {
        "blocker"
    } else if report.effective == 0 {
        if limited {
            "unqualified (limited)"
        } else {
            "unqualified"
        }
    } else if report.effective >= 60 {
        if limited {
            "healthy (limited)"
        } else {
            "healthy"
        }
    } else if limited {
        "ok (limited)"
    } else {
        "ok"
    }
}

/// Generate a simple bar chart string for a score.
pub fn score_bar(score: i32, width: usize) -> String {
    // Map -100..100 to 0..width
    let normalized = ((score + 100) as f64 / 200.0 * width as f64).round() as usize;
    let filled = normalized.min(width);
    let empty = width - filled;
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::{self, Attestation, AttestationBody, Kind};
    use crate::graph;
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

    #[test]
    fn test_raw_score_simple() {
        let records = vec![
            make_record("x", Kind::Praise, 40, "good"),
            make_record("x", Kind::Concern, -30, "bad"),
        ];
        assert_eq!(raw_score(&records), 10);
    }

    #[test]
    fn test_raw_score_empty() {
        assert_eq!(raw_score(&[]), 0);
    }

    #[test]
    fn test_raw_score_clamped() {
        let records = vec![
            make_record("x", Kind::Praise, 80, "great"),
            make_record("x", Kind::Praise, 80, "also great"),
        ];
        assert_eq!(raw_score(&records), 100); // clamped
    }

    #[test]
    fn test_raw_score_clamped_negative() {
        let records = vec![
            make_record("x", Kind::Fail, -80, "bad"),
            make_record("x", Kind::Fail, -80, "worse"),
        ];
        assert_eq!(raw_score(&records), -100); // clamped
    }

    #[test]
    fn test_raw_score_with_supersession() {
        let original = make_record("x", Kind::Concern, -30, "bad");
        let replacement = make_superseding("x", 10, original.id());
        let records = vec![original, replacement];
        // Original (-30) is superseded, only replacement (10) counts
        assert_eq!(raw_score(&records), 10);
    }

    #[test]
    fn test_filter_superseded() {
        let a = make_record("x", Kind::Pass, 10, "a");
        let b = make_superseding("x", 20, a.id());
        let c = make_record("x", Kind::Praise, 30, "c");

        let a_id = a.id().to_string();
        let b_id = b.id().to_string();
        let c_id = c.id().to_string();

        let records = vec![a, b, c];
        let active = filter_superseded(&records);
        assert_eq!(active.len(), 2);
        assert!(active.iter().any(|r| r.id() == b_id));
        assert!(active.iter().any(|r| r.id() == c_id));
        assert!(!active.iter().any(|r| r.id() == a_id));
    }

    #[test]
    fn test_effective_scores_no_graph() {
        let graph = DependencyGraph::empty();
        let qf = QualFile {
            path: PathBuf::from("x.qual"),
            subject: "x".into(),
            records: vec![make_record("x", Kind::Praise, 50, "good")],
        };

        let scores = effective_scores(&graph, &[qf]);
        let report = scores.get("x").unwrap();
        assert_eq!(report.raw, 50);
        assert_eq!(report.effective, 50);
        assert!(report.limiting_path.is_none());
    }

    #[test]
    fn test_effective_scores_limited_by_dependency() {
        let graph_str = r#"{"subject":"app","depends_on":["lib"]}
{"subject":"lib","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let qf_app = QualFile {
            path: PathBuf::from("app.qual"),
            subject: "app".into(),
            records: vec![make_record("app", Kind::Praise, 80, "great app")],
        };
        let qf_lib = QualFile {
            path: PathBuf::from("lib.qual"),
            subject: "lib".into(),
            records: vec![make_record("lib", Kind::Concern, -20, "bad lib")],
        };

        let scores = effective_scores(&g, &[qf_app, qf_lib]);

        let app_report = scores.get("app").unwrap();
        assert_eq!(app_report.raw, 80);
        assert_eq!(app_report.effective, -20); // limited by lib
        assert!(app_report.limiting_path.is_some());
        assert_eq!(app_report.limiting_path.as_ref().unwrap()[0], "lib");

        let lib_report = scores.get("lib").unwrap();
        assert_eq!(lib_report.raw, -20);
        assert_eq!(lib_report.effective, -20);
    }

    #[test]
    fn test_effective_scores_chain_propagation() {
        let graph_str = r#"{"subject":"app","depends_on":["mid"]}
{"subject":"mid","depends_on":["leaf"]}
{"subject":"leaf","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let qfs = vec![
            QualFile {
                path: PathBuf::from("app.qual"),
                subject: "app".into(),
                records: vec![make_record("app", Kind::Praise, 90, "great")],
            },
            QualFile {
                path: PathBuf::from("mid.qual"),
                subject: "mid".into(),
                records: vec![make_record("mid", Kind::Praise, 70, "good")],
            },
            QualFile {
                path: PathBuf::from("leaf.qual"),
                subject: "leaf".into(),
                records: vec![make_record("leaf", Kind::Blocker, -50, "cursed")],
            },
        ];

        let scores = effective_scores(&g, &qfs);

        assert_eq!(scores["leaf"].effective, -50);
        assert_eq!(scores["mid"].effective, -50);
        assert_eq!(scores["app"].effective, -50);
    }

    #[test]
    fn test_effective_scores_unqualified_artifact() {
        let graph_str = r#"{"subject":"app","depends_on":["lib"]}
{"subject":"lib","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();
        let scores = effective_scores(&g, &[]);

        assert_eq!(scores["app"].raw, 0);
        assert_eq!(scores["app"].effective, 0);
        assert_eq!(scores["lib"].raw, 0);
        assert_eq!(scores["lib"].effective, 0);
    }

    #[test]
    fn test_score_bar() {
        assert_eq!(
            score_bar(100, 10),
            "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}"
        );
        assert_eq!(
            score_bar(-100, 10),
            "\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}"
        );
        assert_eq!(
            score_bar(0, 10),
            "\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}"
        );
    }

    #[test]
    fn test_score_status() {
        assert_eq!(
            score_status(&ScoreReport {
                raw: 80,
                effective: 80,
                limiting_path: None
            }),
            "healthy"
        );
        assert_eq!(
            score_status(&ScoreReport {
                raw: 80,
                effective: -20,
                limiting_path: Some(vec!["lib".into()])
            }),
            "blocker"
        );
        assert_eq!(
            score_status(&ScoreReport {
                raw: -20,
                effective: -20,
                limiting_path: None
            }),
            "blocker"
        );
        assert_eq!(
            score_status(&ScoreReport {
                raw: 0,
                effective: 0,
                limiting_path: None
            }),
            "unqualified"
        );
        assert_eq!(
            score_status(&ScoreReport {
                raw: 30,
                effective: 30,
                limiting_path: None
            }),
            "ok"
        );
        assert_eq!(
            score_status(&ScoreReport {
                raw: 80,
                effective: 30,
                limiting_path: Some(vec!["lib".into()])
            }),
            "ok (limited)"
        );
        assert_eq!(
            score_status(&ScoreReport {
                raw: 80,
                effective: 60,
                limiting_path: Some(vec!["lib".into()])
            }),
            "healthy (limited)"
        );
        assert_eq!(
            score_status(&ScoreReport {
                raw: 30,
                effective: 0,
                limiting_path: Some(vec!["lib".into()])
            }),
            "unqualified (limited)"
        );
    }

    #[test]
    fn test_raw_score_exact_boundaries() {
        let records = vec![make_record("x", Kind::Fail, -100, "terrible")];
        assert_eq!(raw_score(&records), -100);

        let records = vec![make_record("x", Kind::Praise, 100, "perfect")];
        assert_eq!(raw_score(&records), 100);

        let records = vec![
            make_record("x", Kind::Praise, 30, "good"),
            make_record("x", Kind::Concern, -30, "bad"),
        ];
        assert_eq!(raw_score(&records), 0);
    }

    #[test]
    fn test_effective_score_zero_propagation() {
        let graph_str = r#"{"subject":"app","depends_on":["lib"]}
{"subject":"lib","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let qfs = vec![QualFile {
            path: PathBuf::from("app.qual"),
            subject: "app".into(),
            records: vec![make_record("app", Kind::Praise, 50, "good")],
        }];
        let scores = effective_scores(&g, &qfs);
        assert_eq!(scores["app"].effective, 0);
        assert_eq!(scores["lib"].effective, 0);
    }

    #[test]
    fn test_effective_score_negative_deep_chain() {
        let graph_str = r#"{"subject":"app","depends_on":["mid"]}
{"subject":"mid","depends_on":["leaf1","leaf2"]}
{"subject":"leaf1","depends_on":[]}
{"subject":"leaf2","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let qfs = vec![
            QualFile {
                path: PathBuf::from("app.qual"),
                subject: "app".into(),
                records: vec![make_record("app", Kind::Praise, 90, "great")],
            },
            QualFile {
                path: PathBuf::from("mid.qual"),
                subject: "mid".into(),
                records: vec![make_record("mid", Kind::Praise, 70, "good")],
            },
            QualFile {
                path: PathBuf::from("leaf1.qual"),
                subject: "leaf1".into(),
                records: vec![make_record("leaf1", Kind::Blocker, -100, "cursed")],
            },
            QualFile {
                path: PathBuf::from("leaf2.qual"),
                subject: "leaf2".into(),
                records: vec![make_record("leaf2", Kind::Praise, 80, "fine")],
            },
        ];

        let scores = effective_scores(&g, &qfs);
        assert_eq!(scores["leaf1"].effective, -100);
        assert_eq!(scores["leaf2"].effective, 80);
        assert_eq!(scores["mid"].effective, -100);
        assert_eq!(scores["app"].effective, -100);
    }
}
