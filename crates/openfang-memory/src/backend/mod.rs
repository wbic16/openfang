//! Storage backend abstraction for OpenFang memory.
//!
//! This module provides a trait-based abstraction over different storage backends:
//! - `SqliteBackend`: Original SQLite-based storage (default)
//! - `SqDaemonBackend`: Phext-native storage via SQ daemon (experimental)
//!
//! All memory stores (SessionStore, StructuredStore, etc.) are generic over
//! `StorageBackend`, allowing runtime selection of storage engine.

use openfang_types::error::OpenFangResult;
use std::fmt;

pub mod sqlite;

#[cfg(feature = "sq-backend")]
pub mod sq_daemon;

#[cfg(feature = "sq-backend")]
pub mod coord_mapper;

/// Abstract storage backend for OpenFang memory substrate.
///
/// Provides a minimal key-value interface that can be implemented
/// by different storage engines (SQLite, SQ daemon, etc.).
pub trait StorageBackend: Send + Sync + fmt::Debug {
    /// Read data from a key.
    ///
    /// Returns `Ok(None)` if key doesn't exist, `Err` on I/O failure.
    fn read(&self, key: &str) -> OpenFangResult<Option<Vec<u8>>>;

    /// Write data to a key.
    ///
    /// Creates key if doesn't exist, overwrites if exists.
    fn write(&self, key: &str, value: &[u8]) -> OpenFangResult<()>;

    /// Delete data at a key.
    ///
    /// Returns `Ok(())` even if key didn't exist (idempotent).
    fn delete(&self, key: &str) -> OpenFangResult<()>;

    /// List all keys matching a prefix.
    ///
    /// Returns sorted list of keys (for iteration).
    fn list(&self, prefix: &str) -> OpenFangResult<Vec<String>>;

    /// Range query: read all keys between start and end (inclusive).
    ///
    /// Returns list of (key, value) pairs sorted by key.
    /// Used for retrieving all messages in a session.
    fn read_range(&self, start: &str, end: &str) -> OpenFangResult<Vec<(String, Vec<u8>)>>;

    /// Search for keys/values matching a query pattern.
    ///
    /// Fallback: linear scan all keys with `prefix`, filter client-side.
    /// Optimized backends can use native search (SQLite FTS, SQ search, etc.).
    fn search(&self, prefix: &str, query: &str) -> OpenFangResult<Vec<(String, Vec<u8>)>>;

    /// Begin a transaction (if supported).
    ///
    /// Returns a transaction handle that can be committed or rolled back.
    /// If backend doesn't support transactions, returns a no-op handle.
    fn begin_transaction(&self) -> OpenFangResult<Box<dyn Transaction>>;
}

/// Transaction handle for atomic multi-operation updates.
pub trait Transaction: Send {
    /// Write within transaction (buffered until commit).
    fn write(&mut self, key: &str, value: &[u8]) -> OpenFangResult<()>;

    /// Delete within transaction (buffered until commit).
    fn delete(&mut self, key: &str) -> OpenFangResult<()>;

    /// Commit all buffered operations atomically.
    fn commit(self: Box<Self>) -> OpenFangResult<()>;

    /// Rollback and discard all buffered operations.
    fn rollback(self: Box<Self>) -> OpenFangResult<()>;
}

/// No-op transaction for backends without native transaction support.
#[derive(Debug)]
pub struct NoOpTransaction;

impl Transaction for NoOpTransaction {
    fn write(&mut self, _key: &str, _value: &[u8]) -> OpenFangResult<()> {
        Ok(())
    }

    fn delete(&mut self, _key: &str) -> OpenFangResult<()> {
        Ok(())
    }

    fn commit(self: Box<Self>) -> OpenFangResult<()> {
        Ok(())
    }

    fn rollback(self: Box<Self>) -> OpenFangResult<()> {
        Ok(())
    }
}
