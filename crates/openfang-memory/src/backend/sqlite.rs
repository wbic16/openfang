//! SQLite backend implementation (original/default).
//!
//! Wraps the existing `Arc<Mutex<Connection>>` pattern behind the
//! `StorageBackend` trait for compatibility with new abstraction layer.

use super::{NoOpTransaction, StorageBackend, Transaction};
use openfang_types::error::{OpenFangError, OpenFangResult};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// SQLite storage backend (default implementation).
///
/// Maps key-value operations to a simple SQLite schema:
/// ```sql
/// CREATE TABLE kv_store (
///     key TEXT PRIMARY KEY,
///     value BLOB NOT NULL
/// );
/// ```
#[derive(Debug, Clone)]
pub struct SqliteBackend {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteBackend {
    /// Create a new SQLite backend from an existing connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> OpenFangResult<Self> {
        // Ensure KV table exists
        {
            let lock = conn
                .lock()
                .map_err(|e| OpenFangError::Memory(format!("Lock poisoned: {}", e)))?;

            lock.execute(
                "CREATE TABLE IF NOT EXISTS kv_store (
                    key TEXT PRIMARY KEY,
                    value BLOB NOT NULL
                )",
                [],
            )
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

            lock.execute(
                "CREATE INDEX IF NOT EXISTS idx_kv_prefix ON kv_store(key)",
                [],
            )
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;
        }

        Ok(Self { conn })
    }

    /// Get reference to underlying connection (for legacy code).
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }
}

impl StorageBackend for SqliteBackend {
    fn read(&self, key: &str) -> OpenFangResult<Option<Vec<u8>>> {
        let lock = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Memory(format!("Lock poisoned: {}", e)))?;

        let mut stmt = lock
            .prepare_cached("SELECT value FROM kv_store WHERE key = ?")
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        let result = stmt
            .query_row([key], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        Ok(result)
    }

    fn write(&self, key: &str, value: &[u8]) -> OpenFangResult<()> {
        let lock = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Memory(format!("Lock poisoned: {}", e)))?;

        lock.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?, ?)",
            rusqlite::params![key, value],
        )
        .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        Ok(())
    }

    fn delete(&self, key: &str) -> OpenFangResult<()> {
        let lock = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Memory(format!("Lock poisoned: {}", e)))?;

        lock.execute("DELETE FROM kv_store WHERE key = ?", [key])
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        Ok(())
    }

    fn list(&self, prefix: &str) -> OpenFangResult<Vec<String>> {
        let lock = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Memory(format!("Lock poisoned: {}", e)))?;

        let mut stmt = lock
            .prepare_cached("SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key")
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        let pattern = format!("{}%", prefix);
        let keys: Result<Vec<String>, _> = stmt
            .query_map([pattern], |row| row.get(0))
            .map_err(|e| OpenFangError::Memory(e.to_string()))?
            .collect();

        keys.map_err(|e| OpenFangError::Memory(e.to_string()))
    }

    fn read_range(&self, start: &str, end: &str) -> OpenFangResult<Vec<(String, Vec<u8>)>> {
        let lock = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Memory(format!("Lock poisoned: {}", e)))?;

        let mut stmt = lock
            .prepare_cached(
                "SELECT key, value FROM kv_store 
                 WHERE key >= ? AND key <= ? 
                 ORDER BY key",
            )
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        let results: Result<Vec<(String, Vec<u8>)>, _> = stmt
            .query_map([start, end], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| OpenFangError::Memory(e.to_string()))?
            .collect();

        results.map_err(|e| OpenFangError::Memory(e.to_string()))
    }

    fn search(&self, prefix: &str, query: &str) -> OpenFangResult<Vec<(String, Vec<u8>)>> {
        // Simple implementation: list all keys with prefix, filter by query
        // (Could use SQLite FTS for better performance)
        let lock = self
            .conn
            .lock()
            .map_err(|e| OpenFangError::Memory(format!("Lock poisoned: {}", e)))?;

        let mut stmt = lock
            .prepare_cached(
                "SELECT key, value FROM kv_store 
                 WHERE key LIKE ? AND value LIKE ?
                 ORDER BY key",
            )
            .map_err(|e| OpenFangError::Memory(e.to_string()))?;

        let key_pattern = format!("{}%", prefix);
        let value_pattern = format!("%{}%", query);

        let results: Result<Vec<(String, Vec<u8>)>, _> = stmt
            .query_map([key_pattern, value_pattern], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| OpenFangError::Memory(e.to_string()))?
            .collect();

        results.map_err(|e| OpenFangError::Memory(e.to_string()))
    }

    fn begin_transaction(&self) -> OpenFangResult<Box<dyn Transaction>> {
        // SQLite transactions are not exposed through this simple KV interface
        // (could be added later if needed)
        Ok(Box::new(NoOpTransaction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sqlite_backend_basic_ops() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        let backend = SqliteBackend::new(Arc::new(Mutex::new(conn))).unwrap();

        // Write
        backend.write("test/key1", b"value1").unwrap();
        backend.write("test/key2", b"value2").unwrap();

        // Read
        assert_eq!(
            backend.read("test/key1").unwrap().unwrap(),
            b"value1".to_vec()
        );
        assert_eq!(
            backend.read("test/key2").unwrap().unwrap(),
            b"value2".to_vec()
        );
        assert_eq!(backend.read("test/nonexistent").unwrap(), None);

        // List
        let keys = backend.list("test/").unwrap();
        assert_eq!(keys, vec!["test/key1", "test/key2"]);

        // Delete
        backend.delete("test/key1").unwrap();
        assert_eq!(backend.read("test/key1").unwrap(), None);

        // Range
        backend.write("test/a", b"a").unwrap();
        backend.write("test/b", b"b").unwrap();
        backend.write("test/c", b"c").unwrap();

        let range = backend.read_range("test/a", "test/b").unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].0, "test/a");
        assert_eq!(range[1].0, "test/b");
    }

    #[test]
    fn test_sqlite_backend_search() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        let backend = SqliteBackend::new(Arc::new(Mutex::new(conn))).unwrap();

        backend.write("test/doc1", b"hello world").unwrap();
        backend.write("test/doc2", b"foo bar").unwrap();
        backend.write("test/doc3", b"hello universe").unwrap();

        let results = backend.search("test/", "hello").unwrap();
        assert_eq!(results.len(), 2);
    }
}
