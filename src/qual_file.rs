use std::path::{Path, PathBuf};

use crate::attestation::Attestation;

/// A parsed `.qual` file.
#[derive(Debug, Clone)]
pub struct QualFile {
    pub path: PathBuf,
    pub attestations: Vec<Attestation>,
}

/// Parse a `.qual` file from disk.
pub fn parse(_path: &Path) -> crate::Result<QualFile> {
    todo!()
}

/// Append an attestation to a `.qual` file.
pub fn append(_path: &Path, _attestation: &Attestation) -> crate::Result<()> {
    todo!()
}

/// Discover all `.qual` files under a root directory.
pub fn discover(_root: &Path) -> crate::Result<Vec<QualFile>> {
    todo!()
}
