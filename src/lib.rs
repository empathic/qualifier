pub mod attestation;
pub mod compact;
pub mod graph;
pub mod qual_file;
pub mod scoring;

#[cfg(feature = "cli")]
pub mod cli;

/// Library-wide error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("cycle detected in {context}: {detail}")]
    Cycle { context: String, detail: String },

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    CheckFailed(String),
}

/// Library-wide result type.
pub type Result<T> = std::result::Result<T, Error>;
