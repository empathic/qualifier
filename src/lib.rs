pub mod annotation;
pub mod compact;
pub mod content_hash;
pub mod qual_file;
pub mod threads;

/// The `qualifier` binary's implementation. Not part of the library API
/// (SPEC.md §7).
#[cfg(feature = "cli")]
#[doc(hidden)]
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

    /// The failure was already reported on stderr. The command-line
    /// binary exits with this status and prints nothing more.
    #[error("failure already reported (exit status {0})")]
    AlreadyReported(i32),
}

/// Library-wide result type.
pub type Result<T> = std::result::Result<T, Error>;
