//! SqStore: High-level memory store backed by SQ daemon.
//!
//! Provides a key-value-like interface over phext coordinates,
//! suitable for integration with OpenFang's memory substrate.

use crate::client::SqClient;
use crate::coordinate::ScrollCoordinate;
use crate::error::{SqError, SqResult};

use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use tracing::{debug, trace};

/// High-level store interface over SQ scrollspace.
///
/// Provides typed storage, session management, and coordinate allocation.
pub struct SqStore {
    client: Arc<SqClient>,
    /// Namespace prefix for this store (maps to library coordinate).
    namespace: usize,
}

impl SqStore {
    /// Create a new store with the given namespace.
    ///
    /// Namespace maps to the library coordinate, allowing multiple
    /// stores to coexist in the same SQ daemon.
    pub fn new(client: Arc<SqClient>, namespace: usize) -> Self {
        Self { client, namespace }
    }

    /// Connect to SQ daemon and create a store with default namespace.
    pub fn connect() -> SqResult<Self> {
        let client = SqClient::connect()?;
        Ok(Self::new(Arc::new(client), 1))
    }

    /// Get the underlying client.
    pub fn client(&self) -> &SqClient {
        &self.client
    }

    // --- Coordinate Schemes ---
    //
    // We use the 9D coordinate space to organize different data types:
    //
    // Library: namespace (allows multiple stores)
    // Shelf:   data type (1=kv, 2=sessions, 3=agents, 4=knowledge, 5=semantic)
    // Series:  shard/partition
    //
    // Collection: context ID (agent, session, etc.)
    // Volume:     sub-context
    // Book:       category
    //
    // Chapter:    group
    // Section:    sub-group
    // Scroll:     item index

    /// Coordinate for key-value storage.
    fn kv_coordinate(&self, key_hash: usize) -> ScrollCoordinate {
        ScrollCoordinate::new(self.namespace, 1, 1, key_hash, 1, 1, 1, 1, 1)
    }

    /// Coordinate for session storage.
    fn session_coordinate(&self, session_id: usize, message_index: usize) -> ScrollCoordinate {
        ScrollCoordinate::new(self.namespace, 2, 1, session_id, 1, 1, 1, 1, message_index)
    }

    /// Coordinate for agent storage.
    fn agent_coordinate(&self, agent_id: usize) -> ScrollCoordinate {
        ScrollCoordinate::new(self.namespace, 3, 1, agent_id, 1, 1, 1, 1, 1)
    }

    /// Hash a string key to a coordinate component.
    fn hash_key(key: &str) -> usize {
        let mut hash: usize = 0;
        for byte in key.bytes() {
            hash ^= byte as usize;
        }
        // Ensure non-zero (coordinates are 1-indexed) and within valid range
        let result = (hash % 126) + 1;
        result
    }

    // --- Key-Value Operations ---

    /// Set a JSON-serializable value.
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> SqResult<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| SqError::Serialization(e.to_string()))?;

        // Store key -> coordinate mapping and value
        // Format: key\x17value
        let content = format!("{}\x17{}", key, json);
        let coord = self.kv_coordinate(Self::hash_key(key));

        self.client.write(&coord, &content)?;
        trace!("Set {} @ {}", key, coord);
        Ok(())
    }

    /// Get a JSON-deserializable value.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> SqResult<Option<T>> {
        let coord = self.kv_coordinate(Self::hash_key(key));
        let content = self.client.read(&coord)?;

        if content.is_empty() {
            return Ok(None);
        }

        // Parse key\x17value format
        let parts: Vec<&str> = content.splitn(2, '\x17').collect();
        if parts.len() < 2 || parts[0] != key {
            // Hash collision or empty - key doesn't match
            return Ok(None);
        }

        let value: T = serde_json::from_str(parts[1])
            .map_err(|e| SqError::Serialization(e.to_string()))?;

        Ok(Some(value))
    }

    /// Delete a key.
    pub fn delete(&self, key: &str) -> SqResult<()> {
        let coord = self.kv_coordinate(Self::hash_key(key));
        self.client.delete(&coord)?;
        Ok(())
    }

    // --- Session Operations ---

    /// Store a session message.
    pub fn store_message(&self, session_id: usize, message_index: usize, content: &str) -> SqResult<()> {
        let coord = self.session_coordinate(session_id, message_index);
        self.client.write(&coord, content)?;
        trace!("Stored message {} for session {} @ {}", message_index, session_id, coord);
        Ok(())
    }

    /// Retrieve a session message.
    pub fn get_message(&self, session_id: usize, message_index: usize) -> SqResult<String> {
        let coord = self.session_coordinate(session_id, message_index);
        self.client.read(&coord)
    }

    /// Get all messages for a session (up to max_index).
    pub fn get_session_messages(&self, session_id: usize, max_index: usize) -> SqResult<Vec<String>> {
        let mut messages = Vec::new();
        for i in 1..=max_index {
            let content = self.get_message(session_id, i)?;
            if content.is_empty() {
                break;
            }
            messages.push(content);
        }
        Ok(messages)
    }

    // --- Agent Operations ---

    /// Store agent data.
    pub fn store_agent<T: Serialize>(&self, agent_id: usize, data: &T) -> SqResult<()> {
        let json = serde_json::to_string(data)
            .map_err(|e| SqError::Serialization(e.to_string()))?;
        let coord = self.agent_coordinate(agent_id);
        self.client.write(&coord, &json)?;
        debug!("Stored agent {} @ {}", agent_id, coord);
        Ok(())
    }

    /// Load agent data.
    pub fn load_agent<T: DeserializeOwned>(&self, agent_id: usize) -> SqResult<Option<T>> {
        let coord = self.agent_coordinate(agent_id);
        let content = self.client.read(&coord)?;

        if content.is_empty() {
            return Ok(None);
        }

        let data: T = serde_json::from_str(&content)
            .map_err(|e| SqError::Serialization(e.to_string()))?;

        Ok(Some(data))
    }

    // --- Raw Coordinate Access ---

    /// Write raw content to a coordinate.
    pub fn write_raw(&self, coord: &ScrollCoordinate, content: &str) -> SqResult<()> {
        self.client.write(coord, content)?;
        Ok(())
    }

    /// Read raw content from a coordinate.
    pub fn read_raw(&self, coord: &ScrollCoordinate) -> SqResult<String> {
        self.client.read(coord)
    }

    /// Delete content at a coordinate.
    pub fn delete_raw(&self, coord: &ScrollCoordinate) -> SqResult<()> {
        self.client.delete(coord)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_key() {
        let h1 = SqStore::hash_key("test");
        let h2 = SqStore::hash_key("test");
        assert_eq!(h1, h2);

        let h3 = SqStore::hash_key("different");
        // Might collide, but probably won't
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_coordinate_schemes() {
        let store = SqStore {
            client: Arc::new(SqClient::connect().unwrap_or_else(|_| panic!("Need daemon"))),
            namespace: 5,
        };

        let kv = store.kv_coordinate(42);
        assert_eq!(kv.inner().z.library, 5);
        assert_eq!(kv.inner().z.shelf, 1);

        let sess = store.session_coordinate(10, 3);
        assert_eq!(sess.inner().z.shelf, 2);
        assert_eq!(sess.inner().y.collection, 10);
        assert_eq!(sess.inner().x.scroll, 3);
    }
}
