//! Error types for SQ integration.

use thiserror::Error;

/// Result type for SQ operations.
pub type SqResult<T> = Result<T, SqError>;

/// Errors that can occur during SQ operations.
#[derive(Debug, Error)]
pub enum SqError {
    /// Failed to open or create shared memory segment.
    #[error("shared memory error: {0}")]
    SharedMemory(String),

    /// Failed to synchronize with SQ daemon.
    #[error("sync error: {0}")]
    Sync(String),

    /// SQ daemon is not running.
    #[error("SQ daemon not running (no shared memory segment found)")]
    DaemonNotRunning,

    /// Invalid coordinate format.
    #[error("invalid coordinate: {0}")]
    InvalidCoordinate(String),

    /// Message too large for shared memory segment.
    #[error("message too large: {size} bytes (max {max})")]
    MessageTooLarge { size: usize, max: usize },

    /// Timeout waiting for SQ daemon response.
    #[error("timeout waiting for SQ daemon")]
    Timeout,

    /// Protocol error in daemon communication.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<shared_memory::ShmemError> for SqError {
    fn from(e: shared_memory::ShmemError) -> Self {
        SqError::SharedMemory(e.to_string())
    }
}
