//! SQ daemon backend implementation (experimental).
//!
//! Connects to local SQ daemon via Unix socket (Linux/Mac) or named pipe (Windows)
//! and maps key-value operations to phext coordinates.

use super::{NoOpTransaction, StorageBackend, Transaction};
use crate::backend::coord_mapper::CoordMapper;
use openfang_types::error::{OpenFangError, OpenFangResult};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// SQ daemon storage backend (phext-native).
///
/// Maps OpenFang's key-value operations to phext coordinates via SQ daemon.
/// Requires `sq` binary in PATH and daemon socket configured.
#[derive(Debug)]
pub struct SqDaemonBackend {
    socket_path: PathBuf,
    coord_mapper: CoordMapper,
}

impl SqDaemonBackend {
    /// Connect to SQ daemon at the given socket path.
    ///
    /// Socket path examples:
    /// - Linux/Mac: `/var/run/sq.sock` or `/tmp/sq.sock`
    /// - Windows: `\\.\pipe\sq`
    pub fn connect(socket_path: PathBuf) -> OpenFangResult<Self> {
        let backend = Self {
            socket_path,
            coord_mapper: CoordMapper::new(),
        };

        // Ensure daemon is running
        backend.ensure_daemon_running()?;

        Ok(backend)
    }

    /// Ensure SQ daemon is running, start if not.
    fn ensure_daemon_running(&self) -> OpenFangResult<()> {
        if self.socket_exists() {
            return Ok(());
        }

        // Start SQ daemon
        Command::new("sq")
            .arg("--daemon")
            .arg("--socket")
            .arg(&self.socket_path)
            .spawn()
            .map_err(|e| {
                OpenFangError::Memory(format!("Failed to start SQ daemon: {}", e))
            })?;

        // Wait for socket to be ready
        self.wait_for_socket(Duration::from_secs(5))?;

        Ok(())
    }

    /// Check if daemon socket exists.
    fn socket_exists(&self) -> bool {
        self.socket_path.exists()
    }

    /// Wait for daemon socket to become available.
    fn wait_for_socket(&self, timeout: Duration) -> OpenFangResult<()> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if self.socket_exists() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        Err(OpenFangError::Memory(
            "Timeout waiting for SQ daemon socket".into(),
        ))
    }

    /// Execute SQ command via daemon socket.
    ///
    /// TODO: Replace with proper SQ client library when available.
    /// For now, this is a placeholder showing the interface.
    fn execute_command(&self, _command: &str) -> OpenFangResult<Vec<u8>> {
        // TODO: Implement actual SQ daemon protocol
        // This requires either:
        // 1. Using sq-client crate (if available)
        // 2. Implementing wire protocol manually
        // 3. Shelling out to `sq` CLI (slow, not recommended)

        Err(OpenFangError::Memory(
            "SQ daemon client not yet implemented (Phase 2)".into(),
        ))
    }
}

impl StorageBackend for SqDaemonBackend {
    fn read(&self, key: &str) -> OpenFangResult<Option<Vec<u8>>> {
        // TODO: Map key → phext coordinate, then read from SQ
        let _coord = self.coord_mapper.key_to_coord(key)?;
        
        // Placeholder:
        Err(OpenFangError::Memory(
            "SQ daemon read not yet implemented".into(),
        ))
    }

    fn write(&self, key: &str, value: &[u8]) -> OpenFangResult<()> {
        // TODO: Map key → phext coordinate, then write to SQ
        let _coord = self.coord_mapper.key_to_coord(key)?;
        let _value = value;
        
        // Placeholder:
        Err(OpenFangError::Memory(
            "SQ daemon write not yet implemented".into(),
        ))
    }

    fn delete(&self, key: &str) -> OpenFangResult<()> {
        // TODO: Map key → phext coordinate, then delete from SQ
        let _coord = self.coord_mapper.key_to_coord(key)?;
        
        // Placeholder:
        Err(OpenFangError::Memory(
            "SQ daemon delete not yet implemented".into(),
        ))
    }

    fn list(&self, prefix: &str) -> OpenFangResult<Vec<String>> {
        // TODO: Map prefix → phext coordinate range, then list from SQ
        let _coord_range = self.coord_mapper.prefix_to_coord_range(prefix)?;
        
        // Placeholder:
        Err(OpenFangError::Memory(
            "SQ daemon list not yet implemented".into(),
        ))
    }

    fn read_range(&self, start: &str, end: &str) -> OpenFangResult<Vec<(String, Vec<u8>)>> {
        // TODO: Map start/end → phext coordinate range, then range query from SQ
        let _start_coord = self.coord_mapper.key_to_coord(start)?;
        let _end_coord = self.coord_mapper.key_to_coord(end)?;
        
        // Placeholder:
        Err(OpenFangError::Memory(
            "SQ daemon range query not yet implemented".into(),
        ))
    }

    fn search(&self, prefix: &str, query: &str) -> OpenFangResult<Vec<(String, Vec<u8>)>> {
        // TODO: Either use SQ native search or fallback to linear scan
        let _coord_range = self.coord_mapper.prefix_to_coord_range(prefix)?;
        let _query = query;
        
        // Placeholder:
        Err(OpenFangError::Memory(
            "SQ daemon search not yet implemented".into(),
        ))
    }

    fn begin_transaction(&self) -> OpenFangResult<Box<dyn Transaction>> {
        // SQ transactions TBD (may not be supported in daemon mode)
        Ok(Box::new(NoOpTransaction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Skip until SQ daemon client implemented
    fn test_sq_daemon_connection() {
        let socket = PathBuf::from("/tmp/test-openfang-sq.sock");
        let backend = SqDaemonBackend::connect(socket).unwrap();
        
        // Basic smoke test
        assert!(backend.socket_exists());
    }
}
