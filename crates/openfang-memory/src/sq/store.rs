//! SQ-backed structured store for key-value operations.
//!
//! This module provides an SQ (phext) alternative to the SQLite structured store.
//! Data is stored at coordinates derived from agent IDs and keys.

use super::client::{CoordinateAllocator, SqClient};
use super::coordinate::PhextCoordinate;
use super::protocol::ProtocolError;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Structured store backed by SQ daemon for coordinate-addressed storage.
#[derive(Clone)]
pub struct SqStructuredStore {
    /// SQ client for daemon communication.
    client: Arc<SqClient>,
    /// Local cache for frequently accessed values.
    cache: Arc<RwLock<HashMap<String, CachedValue>>>,
    /// Namespace for this store (e.g., "agents", "sessions").
    namespace: String,
}

/// A cached value with metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedValue {
    value: serde_json::Value,
    version: u64,
    updated_at: String,
}

impl SqStructuredStore {
    /// Create a new SQ structured store.
    pub fn new(client: Arc<SqClient>, namespace: impl Into<String>) -> Self {
        Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
            namespace: namespace.into(),
        }
    }

    /// Create a store for agent data (namespace = "agents", library = 1).
    pub fn agents(client: Arc<SqClient>) -> Self {
        Self::new(client, "agents")
    }

    /// Create a store for session data (namespace = "sessions", library = 2).
    pub fn sessions(client: Arc<SqClient>) -> Self {
        Self::new(client, "sessions")
    }

    /// Generate a coordinate for an agent+key combination.
    fn coordinate_for(&self, agent_id: &str, key: &str) -> PhextCoordinate {
        // Hash the agent_id and key to get a coordinate
        let combined = format!("{}:{}", agent_id, key);
        let hash = Self::simple_hash(&combined);
        
        // Map to coordinate space based on namespace
        let library = match self.namespace.as_str() {
            "agents" => 1,
            "sessions" => 2,
            "knowledge" => 3,
            "semantic" => 4,
            "usage" => 5,
            _ => 6,
        };

        // Distribute across 9D space using hash
        let mut coord = PhextCoordinate::origin();
        coord.z.a = library;
        coord.z.b = ((hash >> 0) % 9 + 1) as u8;
        coord.z.c = ((hash >> 4) % 9 + 1) as u8;
        coord.y.a = ((hash >> 8) % 9 + 1) as u8;
        coord.y.b = ((hash >> 12) % 9 + 1) as u8;
        coord.y.c = ((hash >> 16) % 9 + 1) as u8;
        coord.x.a = ((hash >> 20) % 9 + 1) as u8;
        coord.x.b = ((hash >> 24) % 9 + 1) as u8;
        coord.x.c = ((hash >> 28) % 9 + 1) as u8;
        
        coord
    }

    /// Simple hash function for coordinate generation.
    fn simple_hash(s: &str) -> u64 {
        let mut hash: u64 = 5381;
        for byte in s.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
        }
        hash
    }

    /// Cache key for a given agent+key.
    fn cache_key(agent_id: &str, key: &str) -> String {
        format!("{}:{}", agent_id, key)
    }

    /// Get a value from the store.
    pub async fn get(&self, agent_id: &str, key: &str) -> Result<Option<serde_json::Value>, ProtocolError> {
        let cache_key = Self::cache_key(agent_id, key);
        
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(Some(cached.value.clone()));
            }
        }

        // Read from SQ
        let coord = self.coordinate_for(agent_id, key);
        let content = self.client.read(coord).await?;
        
        if content.is_empty() {
            return Ok(None);
        }

        // Parse and cache
        let cached: CachedValue = serde_json::from_str(&content)
            .map_err(|e| ProtocolError::IoError(format!("JSON parse error: {}", e)))?;
        
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, cached.clone());
        }

        Ok(Some(cached.value))
    }

    /// Set a value in the store.
    pub async fn set(&self, agent_id: &str, key: &str, value: serde_json::Value) -> Result<(), ProtocolError> {
        let cache_key = Self::cache_key(agent_id, key);
        let coord = self.coordinate_for(agent_id, key);
        
        // Get current version
        let version = {
            let cache = self.cache.read().await;
            cache.get(&cache_key).map(|c| c.version + 1).unwrap_or(1)
        };

        let cached = CachedValue {
            value: value.clone(),
            version,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        let content = serde_json::to_string(&cached)
            .map_err(|e| ProtocolError::IoError(format!("JSON serialize error: {}", e)))?;

        // Write to SQ
        self.client.write(coord, content).await?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, cached);
        }

        Ok(())
    }

    /// Delete a value from the store.
    pub async fn delete(&self, agent_id: &str, key: &str) -> Result<(), ProtocolError> {
        let cache_key = Self::cache_key(agent_id, key);
        let coord = self.coordinate_for(agent_id, key);

        // Delete from SQ
        self.client.delete(coord).await?;

        // Remove from cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(&cache_key);
        }

        Ok(())
    }

    /// List all keys for an agent.
    /// 
    /// Note: This requires scanning the coordinate space, which is expensive.
    /// Consider using an index table for production.
    pub async fn list_keys(&self, _agent_id: &str) -> Result<Vec<String>, ProtocolError> {
        // TODO: Implement using TOC or index
        // For now, this requires a separate index structure
        Err(ProtocolError::IoError(
            "list_keys requires index support (not yet implemented)".to_string()
        ))
    }

    /// Clear the local cache.
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_generation() {
        let client = Arc::new(SqClient::new());
        let store = SqStructuredStore::agents(client);
        
        let c1 = store.coordinate_for("agent-1", "key-1");
        let c2 = store.coordinate_for("agent-1", "key-2");
        let c3 = store.coordinate_for("agent-2", "key-1");
        
        // Same agent+key should produce same coordinate
        let c1_again = store.coordinate_for("agent-1", "key-1");
        assert_eq!(c1, c1_again);
        
        // Different keys should produce different coordinates
        assert_ne!(c1, c2);
        
        // Different agents should produce different coordinates
        assert_ne!(c1, c3);
        
        // All should be in library 1 (agents)
        assert_eq!(c1.z.a, 1);
        assert_eq!(c2.z.a, 1);
        assert_eq!(c3.z.a, 1);
    }

    #[test]
    fn test_namespace_library_mapping() {
        let client = Arc::new(SqClient::new());
        
        let agents = SqStructuredStore::agents(Arc::clone(&client));
        let sessions = SqStructuredStore::sessions(Arc::clone(&client));
        
        let c_agent = agents.coordinate_for("test", "key");
        let c_session = sessions.coordinate_for("test", "key");
        
        // Different namespaces should use different libraries
        assert_eq!(c_agent.z.a, 1);
        assert_eq!(c_session.z.a, 2);
    }
}
