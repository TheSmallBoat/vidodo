//! Window and surface management abstraction.
//!
//! Defines display endpoint configuration, window state, and surface lifecycle
//! for multi-window rendering. Actual winit/wgpu surface objects will be stored
//! when those dependencies are integrated.

use serde::{Deserialize, Serialize};

/// Display endpoint configuration matching the architecture doc's DisplayEndpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayEndpoint {
    /// Unique display identifier.
    pub display_id: String,
    /// OS-level display handle (macOS: CGDirectDisplayID UUID).
    pub os_handle: Option<String>,
    /// Window geometry.
    pub window: WindowConfig,
    /// Display role (e.g., "main", "spatial_view_left").
    pub role: String,
}

/// Window positioning and size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub title: String,
    pub resizable: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
            fullscreen: false,
            title: String::from("Vidodo Visual"),
            resizable: true,
        }
    }
}

/// State of a managed window/surface pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowState {
    /// Created but not yet presenting.
    Created,
    /// Actively presenting frames.
    Presenting,
    /// Resizing (surface needs re-creation).
    Resizing,
    /// Minimized or hidden.
    Hidden,
    /// Closed or destroyed.
    Closed,
}

/// A managed window entry tracking its configuration and current state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedWindow {
    pub endpoint: DisplayEndpoint,
    pub state: WindowState,
    pub frame_count: u64,
    pub last_resize: Option<[u32; 2]>,
}

/// Manages multiple windows for multi-display rendering.
pub struct WindowManager {
    windows: Vec<ManagedWindow>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self { windows: Vec::new() }
    }

    /// Create a window from an endpoint configuration.
    pub fn create_window(&mut self, endpoint: DisplayEndpoint) -> Result<usize, String> {
        if endpoint.window.width == 0 || endpoint.window.height == 0 {
            return Err("window dimensions must be non-zero".into());
        }

        let id = self.windows.len();
        self.windows.push(ManagedWindow {
            endpoint,
            state: WindowState::Created,
            frame_count: 0,
            last_resize: None,
        });
        Ok(id)
    }

    /// Simulate presenting a frame on a window.
    pub fn present_frame(&mut self, window_idx: usize) -> Result<(), String> {
        let w = self
            .windows
            .get_mut(window_idx)
            .ok_or_else(|| format!("window index {window_idx} out of range"))?;

        if w.state == WindowState::Closed {
            return Err("cannot present on closed window".into());
        }

        w.state = WindowState::Presenting;
        w.frame_count += 1;
        Ok(())
    }

    /// Handle a resize event.
    pub fn handle_resize(
        &mut self,
        window_idx: usize,
        new_width: u32,
        new_height: u32,
    ) -> Result<(), String> {
        let w = self
            .windows
            .get_mut(window_idx)
            .ok_or_else(|| format!("window index {window_idx} out of range"))?;

        if new_width == 0 || new_height == 0 {
            return Err("resize dimensions must be non-zero".into());
        }

        w.endpoint.window.width = new_width;
        w.endpoint.window.height = new_height;
        w.state = WindowState::Resizing;
        w.last_resize = Some([new_width, new_height]);
        Ok(())
    }

    /// Close a window.
    pub fn close_window(&mut self, window_idx: usize) -> Result<(), String> {
        let w = self
            .windows
            .get_mut(window_idx)
            .ok_or_else(|| format!("window index {window_idx} out of range"))?;
        w.state = WindowState::Closed;
        Ok(())
    }

    /// Get all managed windows.
    pub fn windows(&self) -> &[ManagedWindow] {
        &self.windows
    }

    /// Number of open (non-closed) windows.
    pub fn open_count(&self) -> usize {
        self.windows.iter().filter(|w| w.state != WindowState::Closed).count()
    }

    /// Find a window by display_id.
    pub fn find_by_display_id(&self, display_id: &str) -> Option<usize> {
        self.windows.iter().position(|w| w.endpoint.display_id == display_id)
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;

    fn test_endpoint(id: &str) -> DisplayEndpoint {
        DisplayEndpoint {
            display_id: id.to_string(),
            os_handle: None,
            window: WindowConfig::default(),
            role: String::from("main"),
        }
    }

    #[test]
    fn create_and_present() {
        let mut mgr = WindowManager::new();
        let idx = mgr.create_window(test_endpoint("disp-1")).unwrap();
        assert_eq!(mgr.windows()[idx].state, WindowState::Created);

        mgr.present_frame(idx).unwrap();
        assert_eq!(mgr.windows()[idx].state, WindowState::Presenting);
        assert_eq!(mgr.windows()[idx].frame_count, 1);
    }

    #[test]
    fn resize_updates_dimensions() {
        let mut mgr = WindowManager::new();
        let idx = mgr.create_window(test_endpoint("disp-1")).unwrap();

        mgr.handle_resize(idx, 1920, 1080).unwrap();
        assert_eq!(mgr.windows()[idx].state, WindowState::Resizing);
        assert_eq!(mgr.windows()[idx].endpoint.window.width, 1920);
        assert_eq!(mgr.windows()[idx].last_resize, Some([1920, 1080]));
    }

    #[test]
    fn zero_resize_rejected() {
        let mut mgr = WindowManager::new();
        let idx = mgr.create_window(test_endpoint("disp-1")).unwrap();
        assert!(mgr.handle_resize(idx, 0, 1080).is_err());
    }

    #[test]
    fn close_prevents_present() {
        let mut mgr = WindowManager::new();
        let idx = mgr.create_window(test_endpoint("disp-1")).unwrap();
        mgr.close_window(idx).unwrap();
        assert!(mgr.present_frame(idx).is_err());
    }

    #[test]
    fn multi_window_management() {
        let mut mgr = WindowManager::new();
        mgr.create_window(test_endpoint("left")).unwrap();
        mgr.create_window(test_endpoint("center")).unwrap();
        mgr.create_window(test_endpoint("right")).unwrap();

        assert_eq!(mgr.open_count(), 3);
        assert_eq!(mgr.find_by_display_id("center"), Some(1));
        assert_eq!(mgr.find_by_display_id("missing"), None);

        mgr.close_window(1).unwrap();
        assert_eq!(mgr.open_count(), 2);
    }

    #[test]
    fn zero_dimension_window_rejected() {
        let mut mgr = WindowManager::new();
        let mut ep = test_endpoint("bad");
        ep.window.width = 0;
        assert!(mgr.create_window(ep).is_err());
    }
}
