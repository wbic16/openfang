//! Memory substrate for the OpenFang Agent Operating System.
//!
//! Provides a unified memory API over multiple storage backends:
//! - **Structured store** (SQLite): Key-value pairs, sessions, agent state
//! - **Semantic store**: Text-based search (Phase 1: LIKE matching, Phase 2: Qdrant vectors)
//! - **Knowledge graph** (SQLite): Entities and relations
//! - **SQ store** (Phext): 9-dimensional coordinate-addressed hierarchical storage
//!
//! Agents interact with a single `Memory` trait that abstracts over all stores.
//!
//! ## SQ Integration
//!
//! The `sq` module provides a phext-native storage backend using SQ's shared memory
//! daemon protocol. This enables coordinate-addressed storage where data is organized
//! in a 9-dimensional lattice: Library.Shelf.Series / Collection.Volume.Book / Chapter.Section.Scroll
//!
//! To use SQ storage, start the SQ daemon with `sq share <phext>` and connect via `SqClient`.

pub mod consolidation;
pub mod knowledge;
pub mod migration;
pub mod semantic;
pub mod session;
pub mod sq;
pub mod structured;
pub mod usage;

mod substrate;
pub use substrate::MemorySubstrate;

// Re-export SQ types for convenience
pub use sq::{CoordinateAllocator, PhextCoordinate, SqClient, SqCommand, SqRequest, SqResponse, SqStructuredStore};
