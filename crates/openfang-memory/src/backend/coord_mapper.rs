//! Coordinate mapper: OpenFang keys ↔ Phext coordinates.
//!
//! Maps OpenFang's hierarchical key namespace to 11-dimensional phext coordinates:
//! - Sessions → Library.Shelf.Series
//! - Messages → Collection.Volume.Book  
//! - Timestamps → Chapter.Section.Scroll

use openfang_types::error::{OpenFangError, OpenFangResult};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Phext coordinate (11 dimensions).
///
/// Simplified representation for SQ daemon integration.
/// Full implementation would use `PhextCoord` from `libphext-rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhextCoord {
    pub library: u8,
    pub shelf: u8,
    pub series: u8,
    pub collection: u8,
    pub volume: u8,
    pub book: u8,
    pub chapter: u8,
    pub section: u8,
    pub scroll: u8,
}

impl PhextCoord {
    /// Create new coordinate (all dimensions 1-9).
    pub fn new(
        library: u8,
        shelf: u8,
        series: u8,
        collection: u8,
        volume: u8,
        book: u8,
        chapter: u8,
        section: u8,
        scroll: u8,
    ) -> Self {
        Self {
            library,
            shelf,
            series,
            collection,
            volume,
            book,
            chapter,
            section,
            scroll,
        }
    }

    /// Format as phext coordinate string.
    pub fn to_string(&self) -> String {
        format!(
            "{}.{}.{}/{}.{}.{}/{}.{}.{}",
            self.library,
            self.shelf,
            self.series,
            self.collection,
            self.volume,
            self.book,
            self.chapter,
            self.section,
            self.scroll
        )
    }
}

/// Maps OpenFang key namespace to phext coordinates.
///
/// Key format conventions:
/// - `session/{agent_id}/{session_id}` → Session metadata
/// - `message/{agent_id}/{session_id}/{index}` → Message content
/// - `agent/{agent_id}` → Agent config
/// - `kv/{namespace}/{key}` → Generic key-value
pub struct CoordMapper;

impl CoordMapper {
    pub fn new() -> Self {
        Self
    }

    /// Map generic key to phext coordinate.
    ///
    /// Strategy: Hash key components to 1-9 for each dimension.
    pub fn key_to_coord(&self, key: &str) -> OpenFangResult<PhextCoord> {
        let parts: Vec<&str> = key.split('/').collect();

        if parts.is_empty() {
            return Err(OpenFangError::Memory("Empty key".into()));
        }

        match parts[0] {
            "session" => self.session_key_to_coord(parts),
            "message" => self.message_key_to_coord(parts),
            "agent" => self.agent_key_to_coord(parts),
            "kv" => self.kv_key_to_coord(parts),
            _ => self.generic_key_to_coord(key),
        }
    }

    /// Map session key: `session/{agent_id}/{session_id}`
    fn session_key_to_coord(&self, parts: Vec<&str>) -> OpenFangResult<PhextCoord> {
        if parts.len() < 3 {
            return Err(OpenFangError::Memory("Invalid session key".into()));
        }

        let agent_id = parts[1];
        let session_id = parts[2];

        // Hash agent_id → Library.Shelf.Series
        let agent_hash = hash_to_1_9(agent_id);
        let library = (agent_hash % 9) + 1;
        let shelf = ((agent_hash / 9) % 9) + 1;
        let series = ((agent_hash / 81) % 9) + 1;

        // Hash session_id → Collection.Volume.Book
        let session_hash = hash_to_1_9(session_id);
        let collection = (session_hash % 9) + 1;
        let volume = ((session_hash / 9) % 9) + 1;
        let book = ((session_hash / 81) % 9) + 1;

        Ok(PhextCoord::new(
            library, shelf, series, collection, volume, book, 1, 1, 1,
        ))
    }

    /// Map message key: `message/{agent_id}/{session_id}/{index}`
    fn message_key_to_coord(&self, parts: Vec<&str>) -> OpenFangResult<PhextCoord> {
        if parts.len() < 4 {
            return Err(OpenFangError::Memory("Invalid message key".into()));
        }

        // Start with session coordinate
        let session_parts = vec![parts[0], parts[1], parts[2]];
        let mut coord = self.session_key_to_coord(session_parts)?;

        // Parse message index → Chapter.Section.Scroll
        let index: usize = parts[3]
            .parse()
            .map_err(|_| OpenFangError::Memory("Invalid message index".into()))?;

        coord.chapter = ((index / (9 * 9)) % 9) as u8 + 1;
        coord.section = ((index / 9) % 9) as u8 + 1;
        coord.scroll = (index % 9) as u8 + 1;

        Ok(coord)
    }

    /// Map agent key: `agent/{agent_id}`
    fn agent_key_to_coord(&self, parts: Vec<&str>) -> OpenFangResult<PhextCoord> {
        if parts.len() < 2 {
            return Err(OpenFangError::Memory("Invalid agent key".into()));
        }

        let agent_id = parts[1];
        let hash = hash_to_1_9(agent_id);

        Ok(PhextCoord::new(
            (hash % 9) + 1,
            ((hash / 9) % 9) + 1,
            ((hash / 81) % 9) + 1,
            1,
            1,
            1,
            1,
            1,
            1,
        ))
    }

    /// Map generic KV key: `kv/{namespace}/{key}`
    fn kv_key_to_coord(&self, parts: Vec<&str>) -> OpenFangResult<PhextCoord> {
        if parts.len() < 3 {
            return Err(OpenFangError::Memory("Invalid KV key".into()));
        }

        let namespace = parts[1];
        let key = parts[2];

        let ns_hash = hash_to_1_9(namespace);
        let key_hash = hash_to_1_9(key);

        Ok(PhextCoord::new(
            (ns_hash % 9) + 1,
            ((ns_hash / 9) % 9) + 1,
            ((ns_hash / 81) % 9) + 1,
            (key_hash % 9) + 1,
            ((key_hash / 9) % 9) + 1,
            ((key_hash / 81) % 9) + 1,
            1,
            1,
            1,
        ))
    }

    /// Fallback: hash entire key to coordinate
    fn generic_key_to_coord(&self, key: &str) -> OpenFangResult<PhextCoord> {
        let hash = hash_to_1_9(key);

        Ok(PhextCoord::new(
            (hash % 9) + 1,
            ((hash / 9) % 9) + 1,
            ((hash / 81) % 9) + 1,
            ((hash / 729) % 9) + 1,
            ((hash / 6561) % 9) + 1,
            ((hash / 59049) % 9) + 1,
            1,
            1,
            1,
        ))
    }

    /// Map key prefix to phext coordinate range.
    ///
    /// Used for list() operations (e.g., list all messages in session).
    pub fn prefix_to_coord_range(&self, prefix: &str) -> OpenFangResult<(PhextCoord, PhextCoord)> {
        // Start: map prefix to coordinate
        let start = self.key_to_coord(prefix)?;

        // End: same but with all trailing dimensions set to 9
        let end = PhextCoord::new(
            start.library,
            start.shelf,
            start.series,
            start.collection,
            start.volume,
            start.book,
            9,
            9,
            9,
        );

        Ok((start, end))
    }
}

/// Hash string to value in range [0, 728] (covers 9^3 = 729 values).
fn hash_to_1_9(s: &str) -> u8 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    let hash = hasher.finish();
    ((hash % 729) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_key_mapping() {
        let mapper = CoordMapper::new();

        let coord1 = mapper
            .key_to_coord("session/agent-123/session-456")
            .unwrap();
        let coord2 = mapper
            .key_to_coord("session/agent-123/session-456")
            .unwrap();

        // Same key → same coordinate
        assert_eq!(coord1, coord2);

        let coord3 = mapper
            .key_to_coord("session/agent-123/session-789")
            .unwrap();

        // Different session → different coordinate
        assert_ne!(coord1, coord3);
    }

    #[test]
    fn test_message_key_mapping() {
        let mapper = CoordMapper::new();

        let coord1 = mapper
            .key_to_coord("message/agent-123/session-456/0")
            .unwrap();
        let coord2 = mapper
            .key_to_coord("message/agent-123/session-456/1")
            .unwrap();

        // Same session, different index → different Chapter.Section.Scroll
        assert_eq!(coord1.library, coord2.library);
        assert_eq!(coord1.shelf, coord2.shelf);
        assert_ne!(coord1.scroll, coord2.scroll);
    }

    #[test]
    fn test_prefix_to_range() {
        let mapper = CoordMapper::new();

        let (start, end) = mapper
            .prefix_to_coord_range("session/agent-123/")
            .unwrap();

        // Start and end should share higher dimensions
        assert_eq!(start.library, end.library);
        assert_eq!(start.shelf, end.shelf);

        // End should have trailing 9s
        assert_eq!(end.chapter, 9);
        assert_eq!(end.section, 9);
        assert_eq!(end.scroll, 9);
    }
}
