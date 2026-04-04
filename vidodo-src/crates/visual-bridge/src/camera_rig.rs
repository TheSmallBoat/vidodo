//! Camera rig: CameraPreset → view/projection matrix computation via glam.
//!
//! Provides `CameraRig` which computes view and projection matrices from
//! a `CameraPreset` and exposes a combined view-projection matrix for
//! shader uniform upload.

use glam::{Mat4, Vec3};

use crate::types::CameraPreset;

/// A computed camera rig containing view and projection matrices.
#[derive(Debug, Clone)]
pub struct CameraRig {
    pub camera_id: String,
    pub view: Mat4,
    pub projection: Mat4,
}

impl CameraRig {
    /// Build a camera rig from a `CameraPreset`.
    ///
    /// View matrix: right-handed look-at.
    /// Projection matrix: perspective with vertical fov, aspect, near/far.
    pub fn from_preset(preset: &CameraPreset) -> Self {
        let eye = Vec3::from(preset.position);
        let center = Vec3::from(preset.target);
        let up = Vec3::from(preset.up);

        let view = Mat4::look_at_rh(eye, center, up);
        let projection = Mat4::perspective_rh(
            preset.fov_deg.to_radians(),
            preset.aspect,
            preset.near,
            preset.far,
        );

        Self { camera_id: preset.camera_id.clone(), view, projection }
    }

    /// Combined view-projection matrix (projection × view).
    pub fn view_projection(&self) -> Mat4 {
        self.projection * self.view
    }

    /// Extract the camera's forward direction from the view matrix.
    pub fn forward(&self) -> [f32; 3] {
        let inv = self.view.inverse();
        let fwd = -inv.z_axis.truncate();
        fwd.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset_front() -> CameraPreset {
        CameraPreset {
            camera_id: String::from("front"),
            position: [0.0, 0.0, 5.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 60.0,
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        }
    }

    fn preset_top() -> CameraPreset {
        CameraPreset {
            camera_id: String::from("top"),
            position: [0.0, 10.0, 0.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 0.0, -1.0],
            fov_deg: 45.0,
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 200.0,
        }
    }

    fn preset_side() -> CameraPreset {
        CameraPreset {
            camera_id: String::from("side"),
            position: [8.0, 0.0, 0.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 90.0,
            aspect: 1.0,
            near: 0.5,
            far: 50.0,
        }
    }

    #[test]
    fn three_presets_produce_different_matrices() {
        let front = CameraRig::from_preset(&preset_front());
        let top = CameraRig::from_preset(&preset_top());
        let side = CameraRig::from_preset(&preset_side());

        // All view matrices must differ
        assert_ne!(front.view, top.view);
        assert_ne!(front.view, side.view);
        assert_ne!(top.view, side.view);

        // All projection matrices must differ (different fov/aspect/near/far)
        assert_ne!(front.projection, top.projection);
        assert_ne!(front.projection, side.projection);
    }

    #[test]
    fn view_projection_is_product() {
        let rig = CameraRig::from_preset(&preset_front());
        let vp = rig.view_projection();
        let expected = rig.projection * rig.view;
        assert_eq!(vp, expected);
    }

    #[test]
    fn front_camera_looks_along_negative_z() {
        let rig = CameraRig::from_preset(&preset_front());
        let fwd = rig.forward();
        // Front camera at z=5 looking at origin → forward is -Z
        assert!(fwd[2] < -0.9, "forward z component should be strongly negative, got {}", fwd[2]);
        assert!(fwd[0].abs() < 0.01);
        assert!(fwd[1].abs() < 0.01);
    }

    #[test]
    fn top_camera_looks_down() {
        let rig = CameraRig::from_preset(&preset_top());
        let fwd = rig.forward();
        // Top camera at y=10 looking at origin → forward is -Y
        assert!(fwd[1] < -0.9, "forward y component should be strongly negative, got {}", fwd[1]);
    }
}
