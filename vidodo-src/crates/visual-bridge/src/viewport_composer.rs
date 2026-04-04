//! Viewport composer: DisplayTopology → ViewSet.
//!
//! Maps a display topology (multiple display endpoints) to a set of views,
//! each associating a camera with a target display. Supports multi-window
//! rendering where the same scene is shown from different camera angles.

use crate::types::CameraPreset;
use crate::window::DisplayEndpoint;

/// A single view: camera + display target.
#[derive(Debug, Clone)]
pub struct ViewEntry {
    pub view_id: String,
    pub camera: CameraPreset,
    pub display_id: String,
    pub display_role: String,
    pub viewport: ViewportRect,
}

/// Viewport rectangle in pixels.
#[derive(Debug, Clone, Copy)]
pub struct ViewportRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// A composed set of views derived from a display topology.
#[derive(Debug, Clone)]
pub struct ViewSet {
    pub views: Vec<ViewEntry>,
}

/// Compose a ViewSet from display endpoints and camera presets.
///
/// If `cameras` has fewer entries than `displays`, the default CameraPreset
/// is used for unmatched displays. If `cameras` has more, the extra cameras
/// are dropped.
pub fn compose_views(displays: &[DisplayEndpoint], cameras: &[CameraPreset]) -> ViewSet {
    let views = displays
        .iter()
        .enumerate()
        .map(|(i, display)| {
            let camera = cameras.get(i).cloned().unwrap_or_else(CameraPreset::default);
            let viewport = ViewportRect {
                x: 0,
                y: 0,
                width: display.window.width,
                height: display.window.height,
            };
            ViewEntry {
                view_id: format!("view-{}", i),
                camera,
                display_id: display.display_id.clone(),
                display_role: display.role.clone(),
                viewport,
            }
        })
        .collect();
    ViewSet { views }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::WindowConfig;

    fn make_display(id: &str, role: &str, width: u32, height: u32) -> DisplayEndpoint {
        DisplayEndpoint {
            display_id: id.to_string(),
            os_handle: None,
            window: WindowConfig {
                width,
                height,
                title: id.to_string(),
                ..WindowConfig::default()
            },
            role: role.to_string(),
        }
    }

    fn make_camera(id: &str, pos: [f32; 3], fov: f32) -> CameraPreset {
        CameraPreset {
            camera_id: id.to_string(),
            position: pos,
            fov_deg: fov,
            ..CameraPreset::default()
        }
    }

    #[test]
    fn two_displays_two_cameras_produces_two_views() {
        let displays = vec![
            make_display("disp-main", "main", 1920, 1080),
            make_display("disp-side", "spatial_view_left", 1280, 720),
        ];
        let cameras = vec![
            make_camera("cam-front", [0.0, 0.0, 5.0], 60.0),
            make_camera("cam-side", [8.0, 0.0, 0.0], 90.0),
        ];

        let view_set = compose_views(&displays, &cameras);
        assert_eq!(view_set.views.len(), 2);
        assert_eq!(view_set.views[0].display_id, "disp-main");
        assert_eq!(view_set.views[0].camera.camera_id, "cam-front");
        assert_eq!(view_set.views[0].viewport.width, 1920);

        assert_eq!(view_set.views[1].display_id, "disp-side");
        assert_eq!(view_set.views[1].camera.camera_id, "cam-side");
        assert_eq!(view_set.views[1].viewport.width, 1280);
    }

    #[test]
    fn more_displays_than_cameras_uses_default() {
        let displays =
            vec![make_display("d1", "main", 1920, 1080), make_display("d2", "aux", 1280, 720)];
        let cameras = vec![make_camera("cam-only", [0.0, 0.0, 5.0], 60.0)];

        let view_set = compose_views(&displays, &cameras);
        assert_eq!(view_set.views.len(), 2);
        assert_eq!(view_set.views[0].camera.camera_id, "cam-only");
        assert_eq!(view_set.views[1].camera.camera_id, "default"); // fallback
    }

    #[test]
    fn same_scene_different_viewpoints() {
        let displays = vec![
            make_display("d-front", "main", 1920, 1080),
            make_display("d-top", "monitor", 1280, 720),
        ];
        let cameras = vec![
            make_camera("cam-front", [0.0, 0.0, 5.0], 60.0),
            make_camera("cam-top", [0.0, 10.0, 0.0], 45.0),
        ];

        let view_set = compose_views(&displays, &cameras);
        // Both displays render same scene but from different cameras
        assert_ne!(view_set.views[0].camera.position, view_set.views[1].camera.position);
        assert_ne!(view_set.views[0].camera.fov_deg, view_set.views[1].camera.fov_deg);
    }

    #[test]
    fn viewport_matches_window_dimensions() {
        let displays = vec![make_display("d1", "main", 3840, 2160)];
        let cameras = vec![CameraPreset::default()];

        let view_set = compose_views(&displays, &cameras);
        assert_eq!(view_set.views[0].viewport.width, 3840);
        assert_eq!(view_set.views[0].viewport.height, 2160);
    }
}
