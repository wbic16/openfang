//! SQ (Scrollspace Query) client for daemon mode integration.
//!
//! This module provides a phext-native storage backend using SQ's shared memory
//! daemon protocol. SQ operates as a coordinate-addressed hierarchical database
//! using 9-dimensional phext coordinates.
//!
//! ## Architecture
//!
//! SQ daemon mode uses shared memory IPC:
//! - `.sq/link` — 1GB shared segment for data
//! - `.sq/work` — 1KB work segment for signaling
//!
//! Messages are phext-encoded with 3 scrolls:
//! - `1.1.1/1.1.1/1.1.1` → command
//! - `1.1.1/1.1.1/1.1.2` → coordinate
//! - `1.1.1/1.1.1/1.1.3` → content
//!
//! ## Store Adapters
//!
//! - `SqStructuredStore` — Key-value store backed by SQ coordinates
//! - (planned) `SqSessionStore` — Session/message store

mod client;
mod coordinate;
mod protocol;
mod store;

pub use client::{SqClient, CoordinateAllocator};
pub use coordinate::PhextCoordinate;
pub use protocol::{SqRequest, SqResponse, SqCommand};
pub use store::SqStructuredStore;

#[cfg(test)]
mod tests;
