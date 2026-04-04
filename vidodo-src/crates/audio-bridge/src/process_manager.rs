//! scsynth process lifecycle management.
//!
//! Manages spawning, monitoring, restarting, and shutting down the SuperCollider
//! scsynth audio synthesis server as a child process.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Configuration for the scsynth process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScynthConfig {
    /// Path to the scsynth binary. Defaults to "scsynth" (assumes PATH).
    pub binary_path: PathBuf,
    /// UDP port for OSC communication.
    pub port: u16,
    /// Maximum number of synth nodes.
    pub max_nodes: u32,
    /// Number of audio bus channels.
    pub num_audio_bus_channels: u32,
    /// Number of control bus channels.
    pub num_control_bus_channels: u32,
    /// Block size (samples per control period).
    pub block_size: u32,
    /// Sample rate.
    pub sample_rate: u32,
    /// Number of output channels.
    pub num_output_channels: u32,
    /// Number of input channels.
    pub num_input_channels: u32,
    /// Maximum restart attempts before giving up.
    pub max_restart_attempts: u32,
}

impl Default for ScynthConfig {
    fn default() -> Self {
        Self {
            binary_path: PathBuf::from("scsynth"),
            port: 57110,
            max_nodes: 1024,
            num_audio_bus_channels: 128,
            num_control_bus_channels: 4096,
            block_size: 64,
            sample_rate: 44100,
            num_output_channels: 2,
            num_input_channels: 0,
            max_restart_attempts: 3,
        }
    }
}

/// Status of the scsynth process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessStatus {
    /// Not started yet.
    NotStarted,
    /// Running and responsive.
    Running,
    /// Process exited unexpectedly.
    Crashed,
    /// Shut down gracefully.
    Stopped,
    /// Attempting restart.
    Restarting,
}

/// Manages the scsynth child process lifecycle.
pub struct ScynthProcessManager {
    config: ScynthConfig,
    child: Option<Child>,
    status: ProcessStatus,
    boot_time: Option<Instant>,
    restart_count: u32,
}

impl ScynthProcessManager {
    pub fn new(config: ScynthConfig) -> Self {
        Self {
            config,
            child: None,
            status: ProcessStatus::NotStarted,
            boot_time: None,
            restart_count: 0,
        }
    }

    /// Build the scsynth command line arguments.
    fn build_args(&self) -> Vec<String> {
        vec![
            "-u".to_string(),
            self.config.port.to_string(),
            "-a".to_string(),
            self.config.num_audio_bus_channels.to_string(),
            "-c".to_string(),
            self.config.num_control_bus_channels.to_string(),
            "-z".to_string(),
            self.config.block_size.to_string(),
            "-S".to_string(),
            self.config.sample_rate.to_string(),
            "-o".to_string(),
            self.config.num_output_channels.to_string(),
            "-i".to_string(),
            self.config.num_input_channels.to_string(),
            "-n".to_string(),
            self.config.max_nodes.to_string(),
        ]
    }

    /// Boot the scsynth server process.
    pub fn boot(&mut self) -> Result<(), String> {
        if self.status == ProcessStatus::Running {
            return Ok(());
        }

        let args = self.build_args();
        let child = Command::new(&self.config.binary_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn scsynth: {e}"))?;

        self.child = Some(child);
        self.status = ProcessStatus::Running;
        self.boot_time = Some(Instant::now());
        self.restart_count = 0;

        Ok(())
    }

    /// Check if the scsynth process is still running.
    pub fn check_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_exit_status)) => {
                    self.status = ProcessStatus::Crashed;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.status = ProcessStatus::Crashed;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Attempt to restart the scsynth process.
    pub fn restart(&mut self) -> Result<(), String> {
        if self.restart_count >= self.config.max_restart_attempts {
            return Err(format!(
                "exceeded max restart attempts ({})",
                self.config.max_restart_attempts
            ));
        }

        self.status = ProcessStatus::Restarting;
        self.restart_count += 1;

        // Kill existing process if any
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;

        self.boot()
    }

    /// Gracefully shut down the scsynth process.
    pub fn shutdown(&mut self) -> Result<(), String> {
        if let Some(ref mut child) = self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
        self.status = ProcessStatus::Stopped;
        Ok(())
    }

    /// Current process status.
    pub fn status(&self) -> ProcessStatus {
        self.status
    }

    /// How long the server has been running since last boot.
    pub fn uptime(&self) -> Option<Duration> {
        self.boot_time.map(|t| t.elapsed())
    }

    /// UDP port the server is listening on.
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Number of restarts since last successful boot.
    pub fn restart_count(&self) -> u32 {
        self.restart_count
    }

    /// The config used.
    pub fn config(&self) -> &ScynthConfig {
        &self.config
    }
}

impl Drop for ScynthProcessManager {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod process_tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let cfg = ScynthConfig::default();
        assert_eq!(cfg.port, 57110);
        assert_eq!(cfg.sample_rate, 44100);
        assert_eq!(cfg.block_size, 64);
        assert_eq!(cfg.max_restart_attempts, 3);
    }

    #[test]
    fn build_args_includes_all_params() {
        let mgr = ScynthProcessManager::new(ScynthConfig::default());
        let args = mgr.build_args();
        assert!(args.contains(&"-u".to_string()));
        assert!(args.contains(&"57110".to_string()));
        assert!(args.contains(&"-S".to_string()));
        assert!(args.contains(&"44100".to_string()));
    }

    #[test]
    fn initial_status_is_not_started() {
        let mgr = ScynthProcessManager::new(ScynthConfig::default());
        assert_eq!(mgr.status(), ProcessStatus::NotStarted);
        assert!(mgr.uptime().is_none());
        assert_eq!(mgr.restart_count(), 0);
    }

    #[test]
    fn boot_with_missing_binary_returns_error() {
        let cfg = ScynthConfig {
            binary_path: PathBuf::from("/nonexistent/scsynth_not_here"),
            ..ScynthConfig::default()
        };
        let mut mgr = ScynthProcessManager::new(cfg);
        assert!(mgr.boot().is_err());
    }

    #[test]
    fn shutdown_from_not_started_is_ok() {
        let mut mgr = ScynthProcessManager::new(ScynthConfig::default());
        assert!(mgr.shutdown().is_ok());
        assert_eq!(mgr.status(), ProcessStatus::Stopped);
    }

    #[test]
    fn restart_limit_enforced() {
        let cfg = ScynthConfig {
            binary_path: PathBuf::from("/nonexistent/scsynth"),
            max_restart_attempts: 2,
            ..ScynthConfig::default()
        };
        let mut mgr = ScynthProcessManager::new(cfg);
        // Simulate that we've already hit the limit
        mgr.restart_count = 2;
        assert!(mgr.restart().is_err());
    }
}
