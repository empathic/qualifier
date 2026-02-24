use crate::qual_file::QualFile;

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
pub fn prune(_qual_file: &QualFile) -> (QualFile, CompactResult) {
    todo!()
}

/// Collapse all attestations into a single epoch attestation.
pub fn snapshot(_qual_file: &QualFile) -> (QualFile, CompactResult) {
    todo!()
}
