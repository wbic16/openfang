//! SQ daemon mode protocol implementation.
//!
//! SQ daemon mode uses shared memory IPC with a specific message format:
//! - 20-byte length prefix (zero-padded decimal string)
//! - Payload is phext-encoded with 3 scrolls for requests
//!
//! Request format (phext with scroll breaks \x17):
//! ```text
//! command\x17coordinate\x17content
//! ```
//!
//! Where scroll positions are:
//! - 1.1.1/1.1.1/1.1.1 → command
//! - 1.1.1/1.1.1/1.1.2 → coordinate
//! - 1.1.1/1.1.1/1.1.3 → content

use super::coordinate::PhextCoordinate;
use std::fmt;

/// Scroll break delimiter (lowest phext dimension).
pub const SCROLL_BREAK: char = '\x17';

/// Commands supported by SQ daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqCommand {
    /// Read a scroll at the given coordinate.
    Read,
    /// Write content to the given coordinate.
    Write,
    /// Append content to the given coordinate.
    Append,
    /// Delete the scroll at the given coordinate.
    Delete,
    /// Get the table of contents.
    Toc,
    /// Get server status.
    Status,
    /// Shutdown the daemon.
    Shutdown,
    /// Display help.
    Help,
    /// Initialize a new phext file.
    Init,
    /// Get a range of scrolls.
    Range,
    /// Count scrolls matching a pattern.
    Count,
    /// Search for content.
    Search,
    /// Export to JSON.
    Export,
    /// Import from JSON.
    Import,
    /// Unknown/custom command.
    Custom(String),
}

impl SqCommand {
    /// Parse a command string.
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "read" | "get" | "fetch" => SqCommand::Read,
            "write" | "set" | "put" => SqCommand::Write,
            "append" | "add" => SqCommand::Append,
            "delete" | "remove" | "rm" => SqCommand::Delete,
            "toc" | "index" | "list" => SqCommand::Toc,
            "status" | "info" => SqCommand::Status,
            "shutdown" | "stop" | "quit" => SqCommand::Shutdown,
            "help" | "?" => SqCommand::Help,
            "init" | "create" => SqCommand::Init,
            "range" | "slice" => SqCommand::Range,
            "count" | "len" => SqCommand::Count,
            "search" | "find" | "grep" => SqCommand::Search,
            "export" | "dump" => SqCommand::Export,
            "import" | "load" => SqCommand::Import,
            other => SqCommand::Custom(other.to_string()),
        }
    }

    /// Convert to command string for protocol.
    pub fn as_str(&self) -> &str {
        match self {
            SqCommand::Read => "read",
            SqCommand::Write => "write",
            SqCommand::Append => "append",
            SqCommand::Delete => "delete",
            SqCommand::Toc => "toc",
            SqCommand::Status => "status",
            SqCommand::Shutdown => "shutdown",
            SqCommand::Help => "help",
            SqCommand::Init => "init",
            SqCommand::Range => "range",
            SqCommand::Count => "count",
            SqCommand::Search => "search",
            SqCommand::Export => "export",
            SqCommand::Import => "import",
            SqCommand::Custom(s) => s.as_str(),
        }
    }

    /// Does this command mutate the phext?
    pub fn is_mutation(&self) -> bool {
        matches!(self, 
            SqCommand::Write | 
            SqCommand::Append | 
            SqCommand::Delete | 
            SqCommand::Init |
            SqCommand::Import
        )
    }
}

impl fmt::Display for SqCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A request to the SQ daemon.
#[derive(Debug, Clone)]
pub struct SqRequest {
    /// The command to execute.
    pub command: SqCommand,
    /// The target coordinate (optional for some commands).
    pub coordinate: PhextCoordinate,
    /// Content for write operations.
    pub content: String,
}

impl SqRequest {
    /// Create a new request.
    pub fn new(command: SqCommand, coordinate: PhextCoordinate, content: impl Into<String>) -> Self {
        Self {
            command,
            coordinate,
            content: content.into(),
        }
    }

    /// Create a read request.
    pub fn read(coordinate: PhextCoordinate) -> Self {
        Self::new(SqCommand::Read, coordinate, "")
    }

    /// Create a write request.
    pub fn write(coordinate: PhextCoordinate, content: impl Into<String>) -> Self {
        Self::new(SqCommand::Write, coordinate, content)
    }

    /// Create an append request.
    pub fn append(coordinate: PhextCoordinate, content: impl Into<String>) -> Self {
        Self::new(SqCommand::Append, coordinate, content)
    }

    /// Create a delete request.
    pub fn delete(coordinate: PhextCoordinate) -> Self {
        Self::new(SqCommand::Delete, coordinate, "")
    }

    /// Create a TOC request.
    pub fn toc() -> Self {
        Self::new(SqCommand::Toc, PhextCoordinate::origin(), "")
    }

    /// Create a status request.
    pub fn status() -> Self {
        Self::new(SqCommand::Status, PhextCoordinate::origin(), "")
    }

    /// Encode to the wire format (phext with scroll breaks).
    pub fn encode(&self) -> Vec<u8> {
        let payload = format!(
            "{}{}{}{}{}",
            self.command.as_str(),
            SCROLL_BREAK,
            self.coordinate,
            SCROLL_BREAK,
            self.content
        );
        
        // Prepend 20-byte length prefix
        let length_prefix = format!("{:020}", payload.len());
        let mut result = Vec::with_capacity(20 + payload.len());
        result.extend_from_slice(length_prefix.as_bytes());
        result.extend_from_slice(payload.as_bytes());
        result
    }

    /// Decode from wire format.
    pub fn decode(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < 20 {
            return Err(ProtocolError::TooShort);
        }

        let length_str = std::str::from_utf8(&data[..20])
            .map_err(|_| ProtocolError::InvalidLengthPrefix)?;
        let length: usize = length_str.trim_start_matches('0')
            .parse()
            .map_err(|_| ProtocolError::InvalidLengthPrefix)?;

        if data.len() < 20 + length {
            return Err(ProtocolError::IncompletePayload);
        }

        let payload = std::str::from_utf8(&data[20..20 + length])
            .map_err(|_| ProtocolError::InvalidUtf8)?;

        let parts: Vec<&str> = payload.split(SCROLL_BREAK).collect();
        if parts.len() < 2 {
            return Err(ProtocolError::MissingFields);
        }

        let command = SqCommand::parse(parts[0]);
        let coordinate: PhextCoordinate = parts[1].parse()
            .map_err(|_| ProtocolError::InvalidCoordinate)?;
        let content = parts.get(2).map(|s| s.to_string()).unwrap_or_default();

        Ok(Self { command, coordinate, content })
    }
}

/// A response from the SQ daemon.
#[derive(Debug, Clone)]
pub struct SqResponse {
    /// The response content.
    pub content: String,
    /// Whether the operation succeeded.
    pub success: bool,
}

impl SqResponse {
    /// Create a successful response.
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            success: true,
        }
    }

    /// Create a failure response.
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            content: error.into(),
            success: false,
        }
    }

    /// Decode from wire format (20-byte length prefix + content).
    pub fn decode(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < 20 {
            return Err(ProtocolError::TooShort);
        }

        let length_str = std::str::from_utf8(&data[..20])
            .map_err(|_| ProtocolError::InvalidLengthPrefix)?;
        
        // Handle empty length (all zeros)
        let length: usize = if length_str.trim_start_matches('0').is_empty() {
            0
        } else {
            length_str.trim_start_matches('0')
                .parse()
                .map_err(|_| ProtocolError::InvalidLengthPrefix)?
        };

        if data.len() < 20 + length {
            return Err(ProtocolError::IncompletePayload);
        }

        let content = std::str::from_utf8(&data[20..20 + length])
            .map_err(|_| ProtocolError::InvalidUtf8)?
            .to_string();

        Ok(Self { content, success: true })
    }

    /// Encode to wire format.
    pub fn encode(&self) -> Vec<u8> {
        let length_prefix = format!("{:020}", self.content.len());
        let mut result = Vec::with_capacity(20 + self.content.len());
        result.extend_from_slice(length_prefix.as_bytes());
        result.extend_from_slice(self.content.as_bytes());
        result
    }
}

/// Protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    TooShort,
    InvalidLengthPrefix,
    IncompletePayload,
    InvalidUtf8,
    MissingFields,
    InvalidCoordinate,
    IoError(String),
    SharedMemoryError(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::TooShort => write!(f, "Message too short"),
            ProtocolError::InvalidLengthPrefix => write!(f, "Invalid length prefix"),
            ProtocolError::IncompletePayload => write!(f, "Incomplete payload"),
            ProtocolError::InvalidUtf8 => write!(f, "Invalid UTF-8"),
            ProtocolError::MissingFields => write!(f, "Missing required fields"),
            ProtocolError::InvalidCoordinate => write!(f, "Invalid coordinate"),
            ProtocolError::IoError(e) => write!(f, "I/O error: {}", e),
            ProtocolError::SharedMemoryError(e) => write!(f, "Shared memory error: {}", e),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_encode_decode() {
        let req = SqRequest::read("2.3.4/5.6.7/8.9.1".parse().unwrap());
        let encoded = req.encode();
        let decoded = SqRequest::decode(&encoded).unwrap();
        
        assert_eq!(decoded.command, SqCommand::Read);
        assert_eq!(decoded.coordinate.to_string(), "2.3.4/5.6.7/8.9.1");
    }

    #[test]
    fn test_write_request() {
        let coord = PhextCoordinate::origin();
        let req = SqRequest::write(coord, "Hello, phext!");
        let encoded = req.encode();
        let decoded = SqRequest::decode(&encoded).unwrap();
        
        assert_eq!(decoded.command, SqCommand::Write);
        assert_eq!(decoded.content, "Hello, phext!");
    }

    #[test]
    fn test_response_encode_decode() {
        let resp = SqResponse::success("Test content");
        let encoded = resp.encode();
        let decoded = SqResponse::decode(&encoded).unwrap();
        
        assert_eq!(decoded.content, "Test content");
        assert!(decoded.success);
    }
}
