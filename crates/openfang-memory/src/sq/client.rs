//! SQ daemon mode client using shared memory IPC.
//!
//! This client communicates with a running SQ daemon via shared memory segments.
//! The daemon must be started separately with `sq share <phext>` or `sq basic`.

use super::coordinate::PhextCoordinate;
use super::protocol::{ProtocolError, SqCommand, SqRequest, SqResponse};

use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared memory segment names (must match SQ daemon).
const SHARED_NAME: &str = ".sq/link";
const WORK_NAME: &str = ".sq/work";

/// Event byte offset in shared memory.
const EVENT_BYTE_OFFSET: usize = 4;

/// Length prefix size.
const LENGTH_PREFIX_SIZE: usize = 20;

/// Client for communicating with an SQ daemon via shared memory.
/// 
/// # Example
/// 
/// ```ignore
/// let client = SqClient::connect()?;
/// 
/// // Read a scroll
/// let content = client.read("1.2.3/4.5.6/7.8.9".parse()?).await?;
/// 
/// // Write a scroll
/// client.write("1.2.3/4.5.6/7.8.9".parse()?, "Hello, phext!").await?;
/// ```
pub struct SqClient {
    /// Internal state (placeholder for shared memory handles).
    /// In production, this would hold the shared memory mappings and event handles.
    state: Arc<Mutex<ClientState>>,
}

/// Internal client state.
struct ClientState {
    /// Whether we're connected to a daemon.
    connected: bool,
    /// The path to the shared memory segment.
    shared_path: String,
    /// The path to the work segment.
    work_path: String,
}

impl SqClient {
    /// Create a new SQ client (not yet connected).
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ClientState {
                connected: false,
                shared_path: SHARED_NAME.to_string(),
                work_path: WORK_NAME.to_string(),
            })),
        }
    }

    /// Connect to an SQ daemon using default paths.
    pub async fn connect() -> Result<Self, ProtocolError> {
        Self::connect_at(SHARED_NAME, WORK_NAME).await
    }

    /// Connect to an SQ daemon at custom paths.
    pub async fn connect_at(shared_path: &str, work_path: &str) -> Result<Self, ProtocolError> {
        // Check if the shared memory files exist
        if !Path::new(shared_path).exists() {
            return Err(ProtocolError::SharedMemoryError(
                format!("Shared memory segment not found: {}. Is the SQ daemon running?", shared_path)
            ));
        }
        if !Path::new(work_path).exists() {
            return Err(ProtocolError::SharedMemoryError(
                format!("Work segment not found: {}. Is the SQ daemon running?", work_path)
            ));
        }

        Ok(Self {
            state: Arc::new(Mutex::new(ClientState {
                connected: true,
                shared_path: shared_path.to_string(),
                work_path: work_path.to_string(),
            })),
        })
    }

    /// Check if the daemon is available.
    pub async fn is_connected(&self) -> bool {
        let state = self.state.lock().await;
        state.connected && Path::new(&state.shared_path).exists()
    }

    /// Execute a request against the SQ daemon.
    /// 
    /// This is a blocking operation wrapped in spawn_blocking for async compatibility.
    pub async fn execute(&self, request: SqRequest) -> Result<SqResponse, ProtocolError> {
        let state = self.state.lock().await;
        if !state.connected {
            return Err(ProtocolError::SharedMemoryError("Not connected to daemon".to_string()));
        }

        let shared_path = state.shared_path.clone();
        let work_path = state.work_path.clone();
        drop(state); // Release lock before blocking

        // Execute in blocking context
        let result = tokio::task::spawn_blocking(move || {
            execute_blocking(&shared_path, &work_path, request)
        }).await.map_err(|e| ProtocolError::IoError(e.to_string()))?;

        result
    }

    /// Read a scroll at the given coordinate.
    pub async fn read(&self, coordinate: PhextCoordinate) -> Result<String, ProtocolError> {
        let response = self.execute(SqRequest::read(coordinate)).await?;
        Ok(response.content)
    }

    /// Write content to the given coordinate.
    pub async fn write(&self, coordinate: PhextCoordinate, content: impl Into<String>) -> Result<(), ProtocolError> {
        let _ = self.execute(SqRequest::write(coordinate, content)).await?;
        Ok(())
    }

    /// Append content to the given coordinate.
    pub async fn append(&self, coordinate: PhextCoordinate, content: impl Into<String>) -> Result<(), ProtocolError> {
        let _ = self.execute(SqRequest::append(coordinate, content)).await?;
        Ok(())
    }

    /// Delete the scroll at the given coordinate.
    pub async fn delete(&self, coordinate: PhextCoordinate) -> Result<(), ProtocolError> {
        let _ = self.execute(SqRequest::delete(coordinate)).await?;
        Ok(())
    }

    /// Get the table of contents.
    pub async fn toc(&self) -> Result<String, ProtocolError> {
        let response = self.execute(SqRequest::toc()).await?;
        Ok(response.content)
    }

    /// Get daemon status.
    pub async fn status(&self) -> Result<String, ProtocolError> {
        let response = self.execute(SqRequest::status()).await?;
        Ok(response.content)
    }

    /// Execute a custom command.
    pub async fn custom(&self, command: &str, coordinate: PhextCoordinate, content: impl Into<String>) -> Result<String, ProtocolError> {
        let request = SqRequest::new(SqCommand::Custom(command.to_string()), coordinate, content);
        let response = self.execute(request).await?;
        Ok(response.content)
    }
}

impl Default for SqClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a request using blocking shared memory IPC.
/// 
/// This function performs the actual shared memory communication:
/// 1. Open the shared memory segment
/// 2. Write the request
/// 3. Signal the daemon
/// 4. Wait for response
/// 5. Read and decode the response
fn execute_blocking(
    shared_path: &str,
    work_path: &str,
    request: SqRequest,
) -> Result<SqResponse, ProtocolError> {
    // NOTE: This is a stub implementation. The full implementation requires
    // the `shared_memory` and `raw_sync` crates that SQ uses.
    //
    // For now, we return a placeholder error indicating the daemon integration
    // is not yet complete.
    //
    // TODO: Implement using:
    // - shared_memory::ShmemConf for memory mapping
    // - raw_sync::events::Event for signaling
    
    #[cfg(feature = "sq-daemon")]
    {
        use shared_memory::ShmemConf;
        use raw_sync::events::{Event, EventState};
        
        // Open existing shared memory segments
        let shmem = ShmemConf::new()
            .flink(shared_path)
            .open()
            .map_err(|e| ProtocolError::SharedMemoryError(e.to_string()))?;
        
        let wkmem = ShmemConf::new()
            .flink(work_path)
            .open()
            .map_err(|e| ProtocolError::SharedMemoryError(e.to_string()))?;
        
        // Get event handles
        let (evt, evt_used_bytes) = unsafe {
            Event::from_existing(shmem.as_ptr())
                .map_err(|e| ProtocolError::SharedMemoryError(format!("{:?}", e)))?
        };
        let (work, _) = unsafe {
            Event::from_existing(wkmem.as_ptr())
                .map_err(|e| ProtocolError::SharedMemoryError(format!("{:?}", e)))?
        };
        
        let length_offset = EVENT_BYTE_OFFSET + evt_used_bytes;
        
        // Encode and write request
        let encoded = request.encode();
        unsafe {
            let ptr = shmem.as_ptr().add(length_offset);
            std::ptr::write_bytes(ptr, 0, encoded.len() + 1);
            std::ptr::copy_nonoverlapping(encoded.as_ptr(), ptr, encoded.len());
        }
        
        // Signal the daemon
        evt.set(EventState::Signaled)
            .map_err(|e| ProtocolError::SharedMemoryError(format!("{:?}", e)))?;
        
        // Wait for response
        work.wait(raw_sync::Timeout::Infinite)
            .map_err(|e| ProtocolError::SharedMemoryError(format!("{:?}", e)))?;
        
        // Read response
        let response_data = unsafe {
            let ptr = shmem.as_ptr().add(length_offset);
            let length_bytes = std::slice::from_raw_parts(ptr, LENGTH_PREFIX_SIZE);
            let length_str = std::str::from_utf8_unchecked(length_bytes);
            let length: usize = length_str.trim_start_matches('0')
                .parse()
                .unwrap_or(0);
            
            if length == 0 {
                return Ok(SqResponse::success(""));
            }
            
            std::slice::from_raw_parts(ptr, LENGTH_PREFIX_SIZE + length).to_vec()
        };
        
        SqResponse::decode(&response_data)
    }
    
    #[cfg(not(feature = "sq-daemon"))]
    {
        // Stub for when sq-daemon feature is not enabled
        let _ = (shared_path, work_path);
        Err(ProtocolError::SharedMemoryError(
            "SQ daemon integration requires the 'sq-daemon' feature. \
             Add `features = [\"sq-daemon\"]` to your Cargo.toml.".to_string()
        ))
    }
}

/// Coordinate space allocator for mapping OpenFang memory to phext coordinates.
/// 
/// This struct helps manage coordinate allocation for different memory types:
/// - Agents: 1.x.x/...
/// - Sessions: 2.x.x/...
/// - Knowledge: 3.x.x/...
/// - Semantic: 4.x.x/...
/// - Usage: 5.x.x/...
#[derive(Debug, Clone)]
pub struct CoordinateAllocator {
    /// Base coordinate for this allocator's namespace.
    base: PhextCoordinate,
    /// Current allocation pointer.
    current: PhextCoordinate,
}

impl CoordinateAllocator {
    /// Create a new allocator with the given base coordinate.
    pub fn new(base: PhextCoordinate) -> Self {
        Self {
            base,
            current: base,
        }
    }

    /// Create an allocator for agent storage (library 1).
    pub fn agents() -> Self {
        Self::new("1.1.1/1.1.1/1.1.1".parse().unwrap())
    }

    /// Create an allocator for session storage (library 2).
    pub fn sessions() -> Self {
        Self::new("2.1.1/1.1.1/1.1.1".parse().unwrap())
    }

    /// Create an allocator for knowledge graph storage (library 3).
    pub fn knowledge() -> Self {
        Self::new("3.1.1/1.1.1/1.1.1".parse().unwrap())
    }

    /// Create an allocator for semantic/embedding storage (library 4).
    pub fn semantic() -> Self {
        Self::new("4.1.1/1.1.1/1.1.1".parse().unwrap())
    }

    /// Create an allocator for usage tracking storage (library 5).
    pub fn usage() -> Self {
        Self::new("5.1.1/1.1.1/1.1.1".parse().unwrap())
    }

    /// Allocate the next coordinate in sequence.
    pub fn allocate(&mut self) -> PhextCoordinate {
        let result = self.current;
        self.current.next_scroll();
        result
    }

    /// Get the base coordinate.
    pub fn base(&self) -> PhextCoordinate {
        self.base
    }

    /// Get the current allocation pointer.
    pub fn current(&self) -> PhextCoordinate {
        self.current
    }

    /// Generate a coordinate for an agent by ID.
    pub fn agent_coordinate(agent_id: u64) -> PhextCoordinate {
        // Map agent ID to coordinate space
        // agent_id 0 → 1.1.1/1.1.1/1.1.1
        // agent_id 1 → 1.1.1/1.1.1/1.1.2
        // etc.
        let mut coord = PhextCoordinate::origin();
        coord.z.a = 1; // Agents are in library 1
        
        // Distribute across coordinate space
        // x.c = scroll, x.b = section, x.a = chapter
        // y.c = book, y.b = volume, y.a = collection
        let scroll = (agent_id % 9) as u8 + 1;
        let section = ((agent_id / 9) % 9) as u8 + 1;
        let chapter = ((agent_id / 81) % 9) as u8 + 1;
        let book = ((agent_id / 729) % 9) as u8 + 1;
        let volume = ((agent_id / 6561) % 9) as u8 + 1;
        let collection = ((agent_id / 59049) % 9) as u8 + 1;
        
        coord.x.c = scroll;     // scroll
        coord.x.b = section;    // section
        coord.x.a = chapter;    // chapter
        coord.y.c = book;       // book
        coord.y.b = volume;     // volume
        coord.y.a = collection; // collection
        
        coord
    }

    /// Generate a coordinate for a session by ID.
    pub fn session_coordinate(session_id: u64) -> PhextCoordinate {
        let mut coord = Self::agent_coordinate(session_id);
        coord.z.a = 2; // Sessions are in library 2
        coord
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_allocator() {
        let mut alloc = CoordinateAllocator::agents();
        
        let c1 = alloc.allocate();
        assert_eq!(c1.to_string(), "1.1.1/1.1.1/1.1.1");
        
        let c2 = alloc.allocate();
        assert_eq!(c2.to_string(), "1.1.1/1.1.1/1.1.2");
    }

    #[test]
    fn test_agent_coordinate() {
        let c0 = CoordinateAllocator::agent_coordinate(0);
        assert_eq!(c0.z.a, 1); // Library 1
        assert_eq!(c0.x.c, 1); // scroll
        
        let c1 = CoordinateAllocator::agent_coordinate(1);
        assert_eq!(c1.x.c, 2); // scroll
        
        let c9 = CoordinateAllocator::agent_coordinate(9);
        assert_eq!(c9.x.c, 1); // scroll
        assert_eq!(c9.x.b, 2); // section
    }

    #[test]
    fn test_session_coordinate() {
        let c0 = CoordinateAllocator::session_coordinate(0);
        assert_eq!(c0.z.a, 2); // Library 2
    }
}
