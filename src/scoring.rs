use std::collections::HashMap;

use crate::attestation::Attestation;
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
pub fn raw_score(_attestations: &[Attestation]) -> i32 {
    todo!()
}

/// Compute effective scores for all artifacts in the graph.
pub fn effective_scores(
    _graph: &DependencyGraph,
    _qual_files: &[QualFile],
) -> HashMap<String, ScoreReport> {
    todo!()
}
