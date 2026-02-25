use std::collections::{HashMap, HashSet};

use crate::attestation::{Attestation, clamp_score};
use crate::graph::DependencyGraph;
use crate::qual_file::QualFile;

/// Score report for a single artifact.
#[derive(Debug, Clone)]
pub struct ScoreReport {
    /// Sum of non-superseded attestation scores, clamped to [-100, 100].
    pub raw: i32,
    /// Raw score floored by worst dependency effective score.
    pub effective: i32,
    /// The dependency path that limits the effective score, if any.
    pub limiting_path: Option<Vec<String>>,
}

/// Compute the raw score for a set of attestations (single artifact).
///
/// Filters out superseded attestations, sums scores, and clamps to [-100, 100].
pub fn raw_score(attestations: &[Attestation]) -> i32 {
    let active = filter_superseded(attestations);
    let sum = active
        .iter()
        .fold(0i32, |acc, a| acc.saturating_add(a.score));
    clamp_score(sum)
}

/// Filter out superseded attestations, returning only the active ones.
///
/// An attestation is superseded if any other attestation's `supersedes` field
/// points to its ID. We follow chains: if B supersedes A, and C supersedes B,
/// only C is active.
pub fn filter_superseded(attestations: &[Attestation]) -> Vec<&Attestation> {
    // Collect all IDs that are superseded by something
    let superseded_ids: HashSet<&str> = attestations
        .iter()
        .filter_map(|a| a.supersedes.as_deref())
        .collect();

    attestations
        .iter()
        .filter(|a| !superseded_ids.contains(a.id.as_str()))
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
    // Build a map of artifact -> attestations
    let mut artifact_attestations: HashMap<&str, Vec<&Attestation>> = HashMap::new();
    for qf in qual_files {
        for att in &qf.attestations {
            artifact_attestations
                .entry(att.artifact.as_str())
                .or_default()
                .push(att);
        }
    }

    // Compute raw scores for all known artifacts
    let mut raw_scores: HashMap<String, i32> = HashMap::new();
    for (artifact, atts) in &artifact_attestations {
        raw_scores.insert(artifact.to_string(), raw_score_from_refs(atts));
    }

    // Include graph artifacts with no attestations (raw score = 0)
    for artifact in graph.artifacts() {
        raw_scores.entry(artifact.to_string()).or_insert(0);
    }

    let mut reports: HashMap<String, ScoreReport> = HashMap::new();

    // If graph is empty, all artifacts get effective = raw
    if graph.is_empty() {
        for (artifact, &raw) in &raw_scores {
            reports.insert(
                artifact.clone(),
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
            for (artifact, &raw) in &raw_scores {
                reports.insert(
                    artifact.clone(),
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

    // Also include artifacts with qual files but not in the graph
    for (artifact, &raw) in &raw_scores {
        reports.entry(artifact.clone()).or_insert(ScoreReport {
            raw,
            effective: raw,
            limiting_path: None,
        });
    }

    reports
}

/// Compute raw score from a slice of attestation references.
pub fn raw_score_from_refs(attestations: &[&Attestation]) -> i32 {
    let superseded_ids: HashSet<&str> = attestations
        .iter()
        .filter_map(|a| a.supersedes.as_deref())
        .collect();

    let sum = attestations
        .iter()
        .filter(|a| !superseded_ids.contains(a.id.as_str()))
        .fold(0i32, |acc, a| acc.saturating_add(a.score));

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
    use crate::attestation::{self, Kind};
    use crate::graph;
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

    #[test]
    fn test_raw_score_simple() {
        let atts = vec![
            make_att("x", Kind::Praise, 40, "good"),
            make_att("x", Kind::Concern, -30, "bad"),
        ];
        assert_eq!(raw_score(&atts), 10);
    }

    #[test]
    fn test_raw_score_empty() {
        assert_eq!(raw_score(&[]), 0);
    }

    #[test]
    fn test_raw_score_clamped() {
        let atts = vec![
            make_att("x", Kind::Praise, 80, "great"),
            make_att("x", Kind::Praise, 80, "also great"),
        ];
        assert_eq!(raw_score(&atts), 100); // clamped
    }

    #[test]
    fn test_raw_score_clamped_negative() {
        let atts = vec![
            make_att("x", Kind::Fail, -80, "bad"),
            make_att("x", Kind::Fail, -80, "worse"),
        ];
        assert_eq!(raw_score(&atts), -100); // clamped
    }

    #[test]
    fn test_raw_score_with_supersession() {
        let original = make_att("x", Kind::Concern, -30, "bad");
        let replacement = make_superseding("x", 10, &original.id);
        let atts = vec![original, replacement];
        // Original (-30) is superseded, only replacement (10) counts
        assert_eq!(raw_score(&atts), 10);
    }

    #[test]
    fn test_filter_superseded() {
        let a = make_att("x", Kind::Pass, 10, "a");
        let b = make_superseding("x", 20, &a.id);
        let c = make_att("x", Kind::Praise, 30, "c");

        let atts = vec![a.clone(), b.clone(), c.clone()];
        let active = filter_superseded(&atts);
        assert_eq!(active.len(), 2);
        assert!(active.iter().any(|att| att.id == b.id));
        assert!(active.iter().any(|att| att.id == c.id));
        assert!(!active.iter().any(|att| att.id == a.id));
    }

    #[test]
    fn test_effective_scores_no_graph() {
        let graph = DependencyGraph::empty();
        let att = make_att("x", Kind::Praise, 50, "good");
        let qf = QualFile {
            path: PathBuf::from("x.qual"),
            artifact: "x".into(),
            attestations: vec![att],
        };

        let scores = effective_scores(&graph, &[qf]);
        let report = scores.get("x").unwrap();
        assert_eq!(report.raw, 50);
        assert_eq!(report.effective, 50);
        assert!(report.limiting_path.is_none());
    }

    #[test]
    fn test_effective_scores_limited_by_dependency() {
        let graph_str = r#"{"artifact":"app","depends_on":["lib"]}
{"artifact":"lib","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let app_att = make_att("app", Kind::Praise, 80, "great app");
        let lib_att = make_att("lib", Kind::Concern, -20, "bad lib");

        let qf_app = QualFile {
            path: PathBuf::from("app.qual"),
            artifact: "app".into(),
            attestations: vec![app_att],
        };
        let qf_lib = QualFile {
            path: PathBuf::from("lib.qual"),
            artifact: "lib".into(),
            attestations: vec![lib_att],
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
        // app -> mid -> leaf
        // leaf is terrible, should propagate up
        let graph_str = r#"{"artifact":"app","depends_on":["mid"]}
{"artifact":"mid","depends_on":["leaf"]}
{"artifact":"leaf","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let qfs = vec![
            QualFile {
                path: PathBuf::from("app.qual"),
                artifact: "app".into(),
                attestations: vec![make_att("app", Kind::Praise, 90, "great")],
            },
            QualFile {
                path: PathBuf::from("mid.qual"),
                artifact: "mid".into(),
                attestations: vec![make_att("mid", Kind::Praise, 70, "good")],
            },
            QualFile {
                path: PathBuf::from("leaf.qual"),
                artifact: "leaf".into(),
                attestations: vec![make_att("leaf", Kind::Blocker, -50, "cursed")],
            },
        ];

        let scores = effective_scores(&g, &qfs);

        assert_eq!(scores["leaf"].effective, -50);
        assert_eq!(scores["mid"].effective, -50); // limited by leaf
        assert_eq!(scores["app"].effective, -50); // limited by leaf via mid
    }

    #[test]
    fn test_effective_scores_unqualified_artifact() {
        // Artifact in graph with no qual file -> raw = 0
        let graph_str = r#"{"artifact":"app","depends_on":["lib"]}
{"artifact":"lib","depends_on":[]}
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
        // Limited variants
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
        // Exactly -100
        let atts = vec![make_att("x", Kind::Fail, -100, "terrible")];
        assert_eq!(raw_score(&atts), -100);

        // Exactly +100
        let atts = vec![make_att("x", Kind::Praise, 100, "perfect")];
        assert_eq!(raw_score(&atts), 100);

        // Sum to exactly 0
        let atts = vec![
            make_att("x", Kind::Praise, 30, "good"),
            make_att("x", Kind::Concern, -30, "bad"),
        ];
        assert_eq!(raw_score(&atts), 0);
    }

    #[test]
    fn test_effective_score_zero_propagation() {
        // Dependency has score 0, parent has positive score
        // Parent should NOT be limited because 0 < 50 is true
        let graph_str = r#"{"artifact":"app","depends_on":["lib"]}
{"artifact":"lib","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let qfs = vec![QualFile {
            path: PathBuf::from("app.qual"),
            artifact: "app".into(),
            attestations: vec![make_att("app", Kind::Praise, 50, "good")],
        }];
        // lib has no qual file, so raw = 0
        let scores = effective_scores(&g, &qfs);
        assert_eq!(scores["app"].effective, 0); // limited by lib's 0
        assert_eq!(scores["lib"].effective, 0);
    }

    #[test]
    fn test_effective_score_negative_deep_chain() {
        // app -> mid -> leaf1, leaf1 has -100
        // app -> mid -> leaf2, leaf2 has +80
        // mid depends on both, should be limited by leaf1
        let graph_str = r#"{"artifact":"app","depends_on":["mid"]}
{"artifact":"mid","depends_on":["leaf1","leaf2"]}
{"artifact":"leaf1","depends_on":[]}
{"artifact":"leaf2","depends_on":[]}
"#;
        let g = graph::parse_graph(graph_str).unwrap();

        let qfs = vec![
            QualFile {
                path: PathBuf::from("app.qual"),
                artifact: "app".into(),
                attestations: vec![make_att("app", Kind::Praise, 90, "great")],
            },
            QualFile {
                path: PathBuf::from("mid.qual"),
                artifact: "mid".into(),
                attestations: vec![make_att("mid", Kind::Praise, 70, "good")],
            },
            QualFile {
                path: PathBuf::from("leaf1.qual"),
                artifact: "leaf1".into(),
                attestations: vec![make_att("leaf1", Kind::Blocker, -100, "cursed")],
            },
            QualFile {
                path: PathBuf::from("leaf2.qual"),
                artifact: "leaf2".into(),
                attestations: vec![make_att("leaf2", Kind::Praise, 80, "fine")],
            },
        ];

        let scores = effective_scores(&g, &qfs);
        assert_eq!(scores["leaf1"].effective, -100);
        assert_eq!(scores["leaf2"].effective, 80);
        assert_eq!(scores["mid"].effective, -100); // limited by leaf1
        assert_eq!(scores["app"].effective, -100); // propagated up
    }
}
