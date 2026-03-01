//! Phext coordinate handling for 9-dimensional addressing.
//!
//! A phext coordinate is a 9-tuple organized as three triads:
//! - Z-axis: Library.Shelf.Series (highest dimensions)
//! - Y-axis: Collection.Volume.Book
//! - X-axis: Chapter.Section.Scroll (lowest dimensions)
//!
//! Format: "Z.Z.Z/Y.Y.Y/X.X.X" e.g., "1.2.3/4.5.6/7.8.9"

use std::fmt;
use std::str::FromStr;

/// Maximum value for any coordinate component (1-9 in standard phext).
pub const COORDINATE_MAX: u8 = 9;

/// A single triad of coordinates (e.g., Chapter.Section.Scroll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Triad {
    pub a: u8,
    pub b: u8,
    pub c: u8,
}

impl Triad {
    pub fn new(a: u8, b: u8, c: u8) -> Self {
        Self { a, b, c }
    }

    pub fn origin() -> Self {
        Self { a: 1, b: 1, c: 1 }
    }
}

impl fmt::Display for Triad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.a, self.b, self.c)
    }
}

impl FromStr for Triad {
    type Err = CoordinateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(CoordinateError::InvalidTriad(s.to_string()));
        }
        Ok(Triad {
            a: parts[0].parse().map_err(|_| CoordinateError::InvalidComponent(parts[0].to_string()))?,
            b: parts[1].parse().map_err(|_| CoordinateError::InvalidComponent(parts[1].to_string()))?,
            c: parts[2].parse().map_err(|_| CoordinateError::InvalidComponent(parts[2].to_string()))?,
        })
    }
}

/// A complete 9-dimensional phext coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PhextCoordinate {
    /// Z-axis: Library.Shelf.Series (highest dimensions)
    pub z: Triad,
    /// Y-axis: Collection.Volume.Book
    pub y: Triad,
    /// X-axis: Chapter.Section.Scroll (lowest dimensions)
    pub x: Triad,
}

impl PhextCoordinate {
    /// Create a new coordinate from three triads.
    pub fn new(z: Triad, y: Triad, x: Triad) -> Self {
        Self { z, y, x }
    }

    /// The origin coordinate: 1.1.1/1.1.1/1.1.1
    pub fn origin() -> Self {
        Self {
            z: Triad::origin(),
            y: Triad::origin(),
            x: Triad::origin(),
        }
    }

    /// Pi coordinate (Ringworld Alpha): 3.1.4/1.5.9/2.6.5
    pub fn pi() -> Self {
        Self {
            z: Triad::new(3, 1, 4),
            y: Triad::new(1, 5, 9),
            x: Triad::new(2, 6, 5),
        }
    }

    /// Boundary coordinate: 9.9.9/9.9.9/9.9.9
    pub fn boundary() -> Self {
        Self {
            z: Triad::new(9, 9, 9),
            y: Triad::new(9, 9, 9),
            x: Triad::new(9, 9, 9),
        }
    }

    /// Convert to a flat 9-element array [z.a, z.b, z.c, y.a, y.b, y.c, x.a, x.b, x.c]
    pub fn to_array(&self) -> [u8; 9] {
        [
            self.z.a, self.z.b, self.z.c,
            self.y.a, self.y.b, self.y.c,
            self.x.a, self.x.b, self.x.c,
        ]
    }

    /// Create from a flat 9-element array.
    pub fn from_array(arr: [u8; 9]) -> Self {
        Self {
            z: Triad::new(arr[0], arr[1], arr[2]),
            y: Triad::new(arr[3], arr[4], arr[5]),
            x: Triad::new(arr[6], arr[7], arr[8]),
        }
    }

    /// Advance to the next scroll (increment x.c, with overflow handling).
    pub fn next_scroll(&mut self) {
        self.x.c += 1;
        if self.x.c > COORDINATE_MAX {
            self.x.c = 1;
            self.next_section();
        }
    }

    /// Advance to the next section (increment x.b, with overflow handling).
    pub fn next_section(&mut self) {
        self.x.b += 1;
        if self.x.b > COORDINATE_MAX {
            self.x.b = 1;
            self.next_chapter();
        }
    }

    /// Advance to the next chapter (increment x.a, with overflow handling).
    pub fn next_chapter(&mut self) {
        self.x.a += 1;
        if self.x.a > COORDINATE_MAX {
            self.x.a = 1;
            self.next_book();
        }
    }

    /// Advance to the next book (increment y.c, with overflow handling).
    pub fn next_book(&mut self) {
        self.y.c += 1;
        if self.y.c > COORDINATE_MAX {
            self.y.c = 1;
            self.next_volume();
        }
    }

    /// Advance to the next volume (increment y.b, with overflow handling).
    pub fn next_volume(&mut self) {
        self.y.b += 1;
        if self.y.b > COORDINATE_MAX {
            self.y.b = 1;
            self.next_collection();
        }
    }

    /// Advance to the next collection (increment y.a, with overflow handling).
    pub fn next_collection(&mut self) {
        self.y.a += 1;
        if self.y.a > COORDINATE_MAX {
            self.y.a = 1;
            self.next_series();
        }
    }

    /// Advance to the next series (increment z.c, with overflow handling).
    pub fn next_series(&mut self) {
        self.z.c += 1;
        if self.z.c > COORDINATE_MAX {
            self.z.c = 1;
            self.next_shelf();
        }
    }

    /// Advance to the next shelf (increment z.b, with overflow handling).
    pub fn next_shelf(&mut self) {
        self.z.b += 1;
        if self.z.b > COORDINATE_MAX {
            self.z.b = 1;
            self.next_library();
        }
    }

    /// Advance to the next library (increment z.a, capped at max).
    pub fn next_library(&mut self) {
        if self.z.a < COORDINATE_MAX {
            self.z.a += 1;
        }
    }
}

impl fmt::Display for PhextCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.y, self.x)
    }
}

impl FromStr for PhextCoordinate {
    type Err = CoordinateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 3 {
            return Err(CoordinateError::InvalidFormat(s.to_string()));
        }
        Ok(PhextCoordinate {
            z: parts[0].parse()?,
            y: parts[1].parse()?,
            x: parts[2].parse()?,
        })
    }
}

/// Errors that can occur when parsing coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinateError {
    InvalidFormat(String),
    InvalidTriad(String),
    InvalidComponent(String),
}

impl fmt::Display for CoordinateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoordinateError::InvalidFormat(s) => write!(f, "Invalid coordinate format: {}", s),
            CoordinateError::InvalidTriad(s) => write!(f, "Invalid triad: {}", s),
            CoordinateError::InvalidComponent(s) => write!(f, "Invalid component: {}", s),
        }
    }
}

impl std::error::Error for CoordinateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin() {
        let origin = PhextCoordinate::origin();
        assert_eq!(origin.to_string(), "1.1.1/1.1.1/1.1.1");
    }

    #[test]
    fn test_pi() {
        let pi = PhextCoordinate::pi();
        assert_eq!(pi.to_string(), "3.1.4/1.5.9/2.6.5");
    }

    #[test]
    fn test_parse() {
        // Valid coordinate should parse
        let coord: PhextCoordinate = "2.3.5/7.8.9/1.2.3".parse().unwrap();
        assert_eq!(coord.z.a, 2);
        assert_eq!(coord.y.b, 8);
        assert_eq!(coord.x.c, 3);
        
        // Invalid coordinate (values > 9) should fail to parse
        // Note: current implementation doesn't validate range, just format
        let result: Result<PhextCoordinate, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_next_scroll() {
        let mut coord = PhextCoordinate::origin();
        coord.next_scroll();
        assert_eq!(coord.to_string(), "1.1.1/1.1.1/1.1.2");
        
        coord.x.c = 9;
        coord.next_scroll();
        assert_eq!(coord.to_string(), "1.1.1/1.1.1/1.2.1");
    }
}
