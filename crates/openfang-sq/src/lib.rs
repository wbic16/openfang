//! SQ Scrollspace Integration for OpenFang Agent OS
//!
//! Provides a client interface to the SQ daemon for 9D coordinate-based
//! memory storage. Replaces or supplements SQLite for agent memory.
//!
//! # Architecture
//!
//! SQ uses shared memory IPC for high-performance local communication:
//! - `.sq/link` (1GB) — data exchange segment
//! - `.sq/work` (1KB) — work coordination segment
//!
//! Messages are phext-delimited: `command\x17coordinate\x17message\x17`

mod client;
mod coordinate;
mod error;
mod store;

pub use client::SqClient;
pub use coordinate::ScrollCoordinate;
pub use error::{SqError, SqResult};
pub use store::SqStore;

/// Phext scroll break delimiter (used in IPC protocol)
pub const SCROLL_BREAK: char = '\x17';

/// Default shared memory segment names
pub const SHARED_LINK: &str = ".sq/link";
pub const SHARED_WORK: &str = ".sq/work";

/// Segment sizes
pub const SHARED_SEGMENT_SIZE: usize = 1024 * 1024 * 1024; // 1 GB
pub const WORK_SEGMENT_SIZE: usize = 1024; // 1 KB
pub const MAX_MESSAGE_SIZE: usize = SHARED_SEGMENT_SIZE / 2;
