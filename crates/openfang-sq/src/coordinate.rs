//! Phext coordinate handling for SQ integration.

use crate::error::{SqError, SqResult};
use libphext::phext::Coordinate;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// A 9D scroll coordinate in phext space.
///
/// Format: `library.shelf.series/collection.volume.book/chapter.section.scroll`
///
/// Each component ranges from 1-127 (7-bit values in phext).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScrollCoordinate {
    inner: Coordinate,
}

// Custom serde implementation that serializes as string
impl Serialize for ScrollCoordinate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_string())
    }
}

impl<'de> Deserialize<'de> for ScrollCoordinate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl ScrollCoordinate {
    /// Create a new coordinate from components.
    pub fn new(
        library: usize,
        shelf: usize,
        series: usize,
        collection: usize,
        volume: usize,
        book: usize,
        chapter: usize,
        section: usize,
        scroll: usize,
    ) -> Self {
        let mut coord = Coordinate::new();
        coord.z.library = library;
        coord.z.shelf = shelf;
        coord.z.series = series;
        coord.y.collection = collection;
        coord.y.volume = volume;
        coord.y.book = book;
        coord.x.chapter = chapter;
        coord.x.section = section;
        coord.x.scroll = scroll;
        Self { inner: coord }
    }

    /// Create the origin coordinate (1.1.1/1.1.1/1.1.1).
    pub fn origin() -> Self {
        Self::new(1, 1, 1, 1, 1, 1, 1, 1, 1)
    }

    /// Get the inner libphext coordinate.
    pub fn inner(&self) -> &Coordinate {
        &self.inner
    }

    /// Convert to the string representation.
    pub fn as_string(&self) -> String {
        self.inner.to_string()
    }

    /// Increment to the next scroll position.
    pub fn next_scroll(&mut self) {
        self.inner.scroll_break();
    }

    /// Increment to the next section.
    pub fn next_section(&mut self) {
        self.inner.section_break();
    }

    /// Increment to the next chapter.
    pub fn next_chapter(&mut self) {
        self.inner.chapter_break();
    }

    /// Create a coordinate for agent memory: `agent_id.1.1/session.1.1/1.1.message_index`
    pub fn for_agent_message(agent_id: usize, session_id: usize, message_index: usize) -> Self {
        Self::new(agent_id, 1, 1, session_id, 1, 1, 1, 1, message_index)
    }

    /// Create a coordinate for session storage: `1.1.1/session.1.1/1.1.1`
    pub fn for_session(session_id: usize) -> Self {
        Self::new(1, 1, 1, session_id, 1, 1, 1, 1, 1)
    }

    /// Create a coordinate for structured key-value: `2.key_hash.1/1.1.1/1.1.1`
    pub fn for_key(key_hash: usize) -> Self {
        Self::new(2, key_hash, 1, 1, 1, 1, 1, 1, 1)
    }
}

impl Default for ScrollCoordinate {
    fn default() -> Self {
        Self::origin()
    }
}

impl fmt::Display for ScrollCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner.to_string())
    }
}

impl FromStr for ScrollCoordinate {
    type Err = SqError;

    fn from_str(s: &str) -> SqResult<Self> {
        let coord = libphext::phext::to_coordinate(s);
        // Validate that parsing didn't just return default
        if s.contains('/') && coord.z.library == 0 {
            return Err(SqError::InvalidCoordinate(s.to_string()));
        }
        Ok(Self { inner: coord })
    }
}

impl From<Coordinate> for ScrollCoordinate {
    fn from(coord: Coordinate) -> Self {
        Self { inner: coord }
    }
}

impl From<ScrollCoordinate> for Coordinate {
    fn from(coord: ScrollCoordinate) -> Self {
        coord.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_roundtrip() {
        let coord = ScrollCoordinate::new(1, 2, 3, 4, 5, 6, 7, 8, 9);
        let s = coord.as_string();
        let parsed: ScrollCoordinate = s.parse().unwrap();
        assert_eq!(coord, parsed);
    }

    #[test]
    fn test_origin() {
        let origin = ScrollCoordinate::origin();
        assert_eq!(origin.as_string(), "1.1.1/1.1.1/1.1.1");
    }
}
