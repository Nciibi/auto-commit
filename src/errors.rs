//! Error types for AutoCommit.
//!
//! Uses `thiserror` to derive `std::error::Error` for all custom error types,
//! and `anyhow` for top-level error propagation.

use thiserror::Error;

/// Top-level error type for AutoCommit.
#[derive(Error, Debug)]
pub enum AutoCommitError {
    /// The current directory is not inside a Git repository.
    #[error("Not inside a Git repository")]
    NotInRepository,

    /// Failed to open the Git repository.
    #[error("Failed to open Git repository: {0}")]
    RepoOpen(#[source] anyhow::Error),

    /// Detached HEAD state — cannot determine branch.
    #[error("Repository is in detached HEAD state")]
    DetachedHead,

    /// Commit failed.
    #[error("Commit failed: {0}")]
    CommitFailed(#[source] anyhow::Error),

    /// Push failed.
    #[error("Push failed: {0}")]
    PushFailed(#[source] anyhow::Error),

    /// Network error during push.
    #[error("Network error: {0}")]
    NetworkError(#[source] anyhow::Error),

    /// Merge conflict detected.
    #[error("Merge conflict detected — resolve conflicts before continuing")]
    MergeConflict,

    /// Failed to initialise the file watcher.
    #[error("Failed to start file watcher: {0}")]
    WatcherError(#[source] anyhow::Error),

    /// Permission denied accessing a file or directory.
    #[error("Permission denied: {0}")]
    PermissionDenied(#[source] anyhow::Error),

    /// An unexpected I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A catch-all for errors not covered above.
    #[error("{0}")]
    Other(#[source] anyhow::Error),
}

/// Convenience alias for `Result<T, AutoCommitError>`.
pub type Result<T> = std::result::Result<T, AutoCommitError>;

impl From<git2::Error> for AutoCommitError {
    fn from(e: git2::Error) -> Self {
        // Detect common git2 error patterns.
        let msg = e.message();
        if msg.contains("detached HEAD") {
            AutoCommitError::DetachedHead
        } else if msg.contains("merge conflict") || msg.contains("conflict") {
            AutoCommitError::MergeConflict
        } else if msg.contains("Permission denied") {
            AutoCommitError::PermissionDenied(e.into())
        } else if msg.contains("Authentication") || msg.contains("timeout") || msg.contains("Couldn't connect") {
            AutoCommitError::NetworkError(e.into())
        } else {
            AutoCommitError::Other(e.into())
        }
    }
}
