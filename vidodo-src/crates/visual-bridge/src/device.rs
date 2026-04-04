//! GPU device and queue abstraction.
//!
//! Wraps wgpu device/queue initialization. In the current phase, this module
//! defines the configuration and status types. Actual wgpu initialization
//! will be wired when the wgpu dependency is added.

use serde::{Deserialize, Serialize};

/// GPU backend preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuBackend {
    /// Apple Metal (macOS/iOS).
    Metal,
    /// Vulkan (Linux/Windows/Android).
    Vulkan,
    /// DirectX 12 (Windows).
    Dx12,
    /// OpenGL fallback.
    Gl,
    /// Automatic selection based on platform.
    #[default]
    Auto,
}

/// Configuration for GPU device initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Preferred GPU backend.
    pub backend: GpuBackend,
    /// Power preference: true = high-performance, false = low-power.
    pub high_performance: bool,
    /// Maximum texture dimension (0 = default).
    pub max_texture_dimension: u32,
    /// Label for diagnostic output.
    pub label: String,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            backend: GpuBackend::Auto,
            high_performance: true,
            max_texture_dimension: 0,
            label: String::from("vidodo-visual"),
        }
    }
}

/// Current state of the GPU device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceState {
    /// Not yet initialized.
    Uninitialized,
    /// Successfully initialized and ready to render.
    Ready,
    /// Device was lost and needs re-creation.
    Lost,
    /// Explicitly shut down.
    Shutdown,
}

/// Describes the GPU device capabilities after initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device name reported by the driver.
    pub name: String,
    /// GPU backend actually in use.
    pub backend: GpuBackend,
    /// Maximum supported texture dimension.
    pub max_texture_dimension_2d: u32,
    /// Maximum supported buffer size.
    pub max_buffer_size: u64,
    /// Whether the device supports compute shaders.
    pub compute_shaders: bool,
}

/// Manages GPU device lifecycle.
///
/// In the current phase, this holds configuration and state. Actual `wgpu::Device`
/// and `wgpu::Queue` handles will be stored here when wgpu is integrated.
pub struct DeviceManager {
    config: DeviceConfig,
    state: DeviceState,
    info: Option<DeviceInfo>,
}

impl DeviceManager {
    pub fn new(config: DeviceConfig) -> Self {
        Self { config, state: DeviceState::Uninitialized, info: None }
    }

    /// Initialize the GPU device.
    ///
    /// Currently simulates initialization. When wgpu is added, this will
    /// call `wgpu::Instance::request_adapter` and `adapter.request_device`.
    pub fn initialize(&mut self) -> Result<(), String> {
        if self.state == DeviceState::Ready {
            return Ok(());
        }

        let backend = match self.config.backend {
            GpuBackend::Auto => {
                if cfg!(target_os = "macos") {
                    GpuBackend::Metal
                } else if cfg!(target_os = "windows") {
                    GpuBackend::Dx12
                } else {
                    GpuBackend::Vulkan
                }
            }
            explicit => explicit,
        };

        self.info = Some(DeviceInfo {
            name: format!("vidodo-{backend:?}-device"),
            backend,
            max_texture_dimension_2d: if self.config.max_texture_dimension > 0 {
                self.config.max_texture_dimension
            } else {
                8192
            },
            max_buffer_size: 256 * 1024 * 1024, // 256 MB default
            compute_shaders: true,
        });
        self.state = DeviceState::Ready;

        Ok(())
    }

    /// Shut down the device and release resources.
    pub fn shutdown(&mut self) {
        self.state = DeviceState::Shutdown;
    }

    /// Report device loss.
    pub fn mark_lost(&mut self) {
        self.state = DeviceState::Lost;
    }

    /// Attempt to re-create the device after loss.
    pub fn recover(&mut self) -> Result<(), String> {
        if self.state != DeviceState::Lost {
            return Err("device not in Lost state".into());
        }
        self.state = DeviceState::Uninitialized;
        self.initialize()
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    pub fn info(&self) -> Option<&DeviceInfo> {
        self.info.as_ref()
    }

    pub fn config(&self) -> &DeviceConfig {
        &self.config
    }
}

#[cfg(test)]
mod device_tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = DeviceConfig::default();
        assert_eq!(cfg.backend, GpuBackend::Auto);
        assert!(cfg.high_performance);
    }

    #[test]
    fn initialize_and_query() {
        let mut mgr = DeviceManager::new(DeviceConfig::default());
        assert_eq!(*mgr.state(), DeviceState::Uninitialized);

        mgr.initialize().unwrap();
        assert_eq!(*mgr.state(), DeviceState::Ready);
        assert!(mgr.info().is_some());

        let info = mgr.info().unwrap();
        assert!(info.max_texture_dimension_2d >= 8192);
    }

    #[test]
    fn double_initialize_is_idempotent() {
        let mut mgr = DeviceManager::new(DeviceConfig::default());
        mgr.initialize().unwrap();
        mgr.initialize().unwrap(); // no error
        assert_eq!(*mgr.state(), DeviceState::Ready);
    }

    #[test]
    fn shutdown_and_recover() {
        let mut mgr = DeviceManager::new(DeviceConfig::default());
        mgr.initialize().unwrap();
        mgr.shutdown();
        assert_eq!(*mgr.state(), DeviceState::Shutdown);
    }

    #[test]
    fn lost_and_recover() {
        let mut mgr = DeviceManager::new(DeviceConfig::default());
        mgr.initialize().unwrap();
        mgr.mark_lost();
        assert_eq!(*mgr.state(), DeviceState::Lost);

        mgr.recover().unwrap();
        assert_eq!(*mgr.state(), DeviceState::Ready);
    }

    #[test]
    fn recover_not_lost_returns_error() {
        let mut mgr = DeviceManager::new(DeviceConfig::default());
        mgr.initialize().unwrap();
        assert!(mgr.recover().is_err());
    }
}
