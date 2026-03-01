//! SQ daemon client using shared memory IPC.

use crate::error::{SqError, SqResult};
use crate::{ScrollCoordinate, SCROLL_BREAK, SHARED_LINK, SHARED_WORK, MAX_MESSAGE_SIZE};

use raw_sync::events::{Event, EventInit, EventState};
use raw_sync::Timeout;
use shared_memory::{Shmem, ShmemConf};
use std::sync::{Arc, Mutex};
use tracing::{debug, trace};

/// Client for communicating with the SQ daemon via shared memory.
pub struct SqClient {
    /// Shared memory segment for data exchange.
    shmem: Arc<Mutex<Shmem>>,
    /// Work segment for synchronization.
    wkmem: Arc<Mutex<Shmem>>,
    /// Offset where messages begin (after event header).
    message_offset: usize,
}

impl SqClient {
    /// Connect to an existing SQ daemon.
    ///
    /// The daemon must already be running (`sq basic` or `sq share`).
    pub fn connect() -> SqResult<Self> {
        // Open existing shared memory segments
        let shmem = ShmemConf::new()
            .flink(SHARED_LINK)
            .open()
            .map_err(|e| {
                if matches!(e, shared_memory::ShmemError::LinkDoesNotExist) {
                    SqError::DaemonNotRunning
                } else {
                    SqError::SharedMemory(e.to_string())
                }
            })?;

        let wkmem = ShmemConf::new()
            .flink(SHARED_WORK)
            .open()
            .map_err(|e| SqError::SharedMemory(e.to_string()))?;

        // Get event header size to determine message offset
        let (_, evt_used_bytes) = unsafe {
            Event::from_existing(shmem.as_ptr())
                .map_err(|e| SqError::Sync(format!("failed to get event: {:?}", e)))?
        };

        let message_offset = Self::event_byte_offset(evt_used_bytes);

        debug!("Connected to SQ daemon, message offset: {}", message_offset);

        Ok(Self {
            shmem: Arc::new(Mutex::new(shmem)),
            wkmem: Arc::new(Mutex::new(wkmem)),
            message_offset,
        })
    }

    /// Calculate message offset accounting for event header and alignment.
    fn event_byte_offset(event_bytes: usize) -> usize {
        // Align to 8-byte boundary
        (event_bytes + 7) & !7
    }

    /// Send a command to the SQ daemon and wait for response.
    pub fn execute(&self, command: &str, coordinate: &ScrollCoordinate, message: &str) -> SqResult<String> {
        // Build the phext-delimited message
        let mut encoded = String::new();
        encoded.push_str(command);
        encoded.push(SCROLL_BREAK);
        encoded.push_str(&coordinate.as_string());
        encoded.push(SCROLL_BREAK);
        encoded.push_str(message);
        encoded.push(SCROLL_BREAK);

        if encoded.len() > MAX_MESSAGE_SIZE {
            return Err(SqError::MessageTooLarge {
                size: encoded.len(),
                max: MAX_MESSAGE_SIZE,
            });
        }

        trace!("Sending to SQ: {} @ {}", command, coordinate);

        // Lock segments
        let shmem = self.shmem.lock().map_err(|e| SqError::Sync(e.to_string()))?;
        let wkmem = self.wkmem.lock().map_err(|e| SqError::Sync(e.to_string()))?;

        // Get events
        let (evt, _) = unsafe {
            Event::from_existing(shmem.as_ptr())
                .map_err(|e| SqError::Sync(format!("event error: {:?}", e)))?
        };
        let (work, _) = unsafe {
            Event::from_existing(wkmem.as_ptr())
                .map_err(|e| SqError::Sync(format!("work event error: {:?}", e)))?
        };

        // Send message
        self.send_message(shmem.as_ptr(), &encoded)?;

        // Signal the daemon
        evt.set(EventState::Signaled)
            .map_err(|e| SqError::Sync(format!("signal error: {:?}", e)))?;

        // Wait for response (30 second timeout)
        work.wait(Timeout::Val(std::time::Duration::from_secs(30)))
            .map_err(|e| SqError::Sync(format!("wait error: {}", e)))?;

        // Read response
        let response = self.fetch_message(shmem.as_ptr())?;

        trace!("SQ response: {} bytes", response.len());

        Ok(response)
    }

    /// Send a length-prefixed message to shared memory.
    fn send_message(&self, shmem_ptr: *mut u8, encoded: &str) -> SqResult<()> {
        let _length_size = 20;
        let prepared = format!("{:020}{}", encoded.len(), encoded);

        unsafe {
            // Zero the region first
            let zero_length = prepared.len() + 1;
            std::ptr::write_bytes(shmem_ptr.add(self.message_offset), 0, zero_length);

            // Write the message
            std::ptr::copy_nonoverlapping(
                prepared.as_ptr(),
                shmem_ptr.add(self.message_offset),
                prepared.len(),
            );
        }

        Ok(())
    }

    /// Fetch a length-prefixed message from shared memory.
    fn fetch_message(&self, shmem_ptr: *mut u8) -> SqResult<String> {
        let length_size = 20;

        unsafe {
            // Read length prefix
            let raw = std::slice::from_raw_parts(shmem_ptr.add(self.message_offset), length_size);
            let length_string = String::from_utf8_lossy(raw);
            let length: usize = length_string
                .trim()
                .parse()
                .map_err(|e| SqError::Protocol(format!("invalid length: {}", e)))?;

            if length == 0 {
                return Ok(String::new());
            }

            if length > MAX_MESSAGE_SIZE {
                return Err(SqError::MessageTooLarge {
                    size: length,
                    max: MAX_MESSAGE_SIZE,
                });
            }

            // Read message content
            let content = std::slice::from_raw_parts(
                shmem_ptr.add(self.message_offset + length_size),
                length,
            );

            Ok(String::from_utf8_lossy(content).to_string())
        }
    }

    // --- High-level convenience methods ---

    /// Write a scroll to the given coordinate.
    pub fn write(&self, coordinate: &ScrollCoordinate, content: &str) -> SqResult<String> {
        self.execute("write", coordinate, content)
    }

    /// Read a scroll from the given coordinate.
    pub fn read(&self, coordinate: &ScrollCoordinate) -> SqResult<String> {
        self.execute("read", coordinate, "")
    }

    /// Delete a scroll at the given coordinate.
    pub fn delete(&self, coordinate: &ScrollCoordinate) -> SqResult<String> {
        self.execute("delete", coordinate, "")
    }

    /// List all coordinates in a range.
    pub fn range(&self, start: &ScrollCoordinate, end: &ScrollCoordinate) -> SqResult<String> {
        // Range is specified as start coordinate with end in message
        self.execute("range", start, &end.to_string())
    }

    /// Get the total number of scrolls.
    pub fn count(&self, coordinate: &ScrollCoordinate) -> SqResult<String> {
        self.execute("count", coordinate, "")
    }
}

// Thread-safe: the mutexes protect the shared memory access
unsafe impl Send for SqClient {}
unsafe impl Sync for SqClient {}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running SQ daemon
    // Run `sq basic` in another terminal first

    #[test]
    #[ignore] // Requires daemon
    fn test_connect() {
        let client = SqClient::connect();
        assert!(client.is_ok() || matches!(client.unwrap_err(), SqError::DaemonNotRunning));
    }

    #[test]
    #[ignore] // Requires daemon
    fn test_write_read() {
        let client = SqClient::connect().unwrap();
        let coord = ScrollCoordinate::new(9, 9, 9, 9, 9, 9, 9, 9, 9);

        // Write
        let write_result = client.write(&coord, "test content");
        assert!(write_result.is_ok());

        // Read
        let read_result = client.read(&coord);
        assert!(read_result.is_ok());
        assert!(read_result.unwrap().contains("test content"));
    }
}
