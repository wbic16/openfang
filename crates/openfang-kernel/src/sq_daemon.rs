//! SQ daemon lifecycle management.
//!
//! Manages the SQ daemon process, health checks, and graceful shutdown.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use openfang_types::config::SqConfig;

/// SQ daemon lifecycle manager.
#[derive(Clone)]
pub struct SqDaemon {
    config: Arc<SqConfig>,
    data_dir: Arc<PathBuf>,
    state: Arc<RwLock<DaemonState>>,
}

struct DaemonState {
    /// Child process handle (None if not started by us).
    process: Option<Child>,
    /// Last successful health check.
    last_health_check: Option<Instant>,
    /// Whether the daemon is healthy.
    healthy: bool,
}

impl SqDaemon {
    /// Create a new SQ daemon manager.
    pub fn new(config: SqConfig, data_dir: PathBuf) -> Self {
        Self {
            config: Arc::new(config),
            data_dir: Arc::new(data_dir),
            state: Arc::new(RwLock::new(DaemonState {
                process: None,
                last_health_check: None,
                healthy: false,
            })),
        }
    }

    /// Start the SQ daemon if auto_start is enabled and it's not already running.
    pub async fn start(&self) -> Result<(), String> {
        // Check if daemon is already running
        if self.check_health().await {
            info!("SQ daemon already running");
            return Ok(());
        }

        if !self.config.auto_start {
            return Err("SQ daemon not running and auto_start is disabled".to_string());
        }

        info!("Starting SQ daemon...");

        // Determine binary path
        let binary = self
            .config
            .binary_path
            .clone()
            .unwrap_or_else(|| "sq".to_string());

        // Determine phext file path
        let phext_path = self.data_dir.join(&self.config.phext_file);

        // Create phext file if it doesn't exist
        if !phext_path.exists() {
            info!("Creating new phext file: {}", phext_path.display());
            std::fs::write(&phext_path, "").map_err(|e| {
                format!("Failed to create phext file: {}", e)
            })?;
        }

        // Start daemon in "share" mode (shared memory IPC)
        let mut cmd = Command::new(&binary);
        cmd.arg("share")
            .arg(&phext_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().map_err(|e| {
            format!("Failed to spawn SQ daemon (binary: {}): {}", binary, e)
        })?;

        // Store process handle
        {
            let mut state = self.state.write().await;
            state.process = Some(child);
        }

        // Wait for daemon to be ready
        for i in 0..10 {
            sleep(Duration::from_millis(100 * (i + 1))).await;
            if self.check_health().await {
                info!("SQ daemon started successfully");
                return Ok(());
            }
        }

        Err("SQ daemon started but failed health check".to_string())
    }

    /// Check if the SQ daemon is healthy.
    ///
    /// Returns true if we can connect to the shared memory segment.
    pub async fn check_health(&self) -> bool {
        // Try to connect to the daemon
        let result = tokio::task::spawn_blocking(|| {
            openfang_sq::SqClient::connect().is_ok()
        })
        .await;

        let healthy = result.unwrap_or(false);

        // Update state
        {
            let mut state = self.state.write().await;
            state.healthy = healthy;
            if healthy {
                state.last_health_check = Some(Instant::now());
            }
        }

        healthy
    }

    /// Get daemon status for monitoring.
    pub async fn status(&self) -> DaemonStatus {
        let state = self.state.read().await;
        DaemonStatus {
            running: state.process.is_some(),
            healthy: state.healthy,
            last_health_check: state.last_health_check,
            namespace: self.config.namespace,
            phext_file: self.config.phext_file.clone(),
        }
    }

    /// Gracefully shutdown the SQ daemon.
    pub async fn shutdown(&self) -> Result<(), String> {
        let mut state = self.state.write().await;

        if let Some(mut child) = state.process.take() {
            info!("Shutting down SQ daemon...");

            // Try graceful shutdown first (SQ responds to SIGTERM)
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                if let Ok(pid) = child.id().try_into() {
                    let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
                }
            }

            // Wait up to 5 seconds for graceful shutdown
            for _ in 0..50 {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        info!("SQ daemon shut down gracefully");
                        return Ok(());
                    }
                    Ok(None) => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        warn!("Error checking SQ daemon status: {}", e);
                        break;
                    }
                }
            }

            // Force kill if still running
            warn!("SQ daemon did not shut down gracefully, force killing");
            child.kill().map_err(|e| format!("Failed to kill SQ daemon: {}", e))?;
            child.wait().map_err(|e| format!("Failed to wait for SQ daemon: {}", e))?;
        }

        Ok(())
    }

    /// Start a health check background task.
    ///
    /// Checks health every 30 seconds and logs warnings if unhealthy.
    pub fn spawn_health_monitor(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(30)).await;

                if !self.check_health().await {
                    error!("SQ daemon health check failed");

                    // Try to restart if auto_start is enabled
                    if self.config.auto_start {
                        warn!("Attempting to restart SQ daemon...");
                        if let Err(e) = self.start().await {
                            error!("Failed to restart SQ daemon: {}", e);
                        }
                    }
                }
            }
        })
    }
}

/// SQ daemon status information.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DaemonStatus {
    /// Whether we spawned the daemon process.
    pub running: bool,
    /// Whether the daemon is healthy (shared memory accessible).
    pub healthy: bool,
    /// Last successful health check timestamp.
    pub last_health_check: Option<Instant>,
    /// Namespace this instance is using.
    pub namespace: usize,
    /// Phext file being served.
    pub phext_file: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_lifecycle() {
        let config = SqConfig {
            enabled: true,
            binary_path: None,
            phext_file: "test.phext".to_string(),
            namespace: 99,
            auto_start: true,
            primary: false,
        };

        let daemon = SqDaemon::new(config, std::env::temp_dir());

        // This will fail if sq binary not in PATH, which is expected
        let _ = daemon.start().await;
    }
}
