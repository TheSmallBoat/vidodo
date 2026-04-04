//! Common visual rendering types.

use serde::{Deserialize, Serialize};

/// Calibration profile for multi-display setups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub profile_id: String,
    pub display_endpoints: Vec<crate::window::DisplayEndpoint>,
    pub camera_presets: Vec<CameraPreset>,
    pub version: String,
}

/// A camera preset defining view parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraPreset {
    pub camera_id: String,
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_deg: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraPreset {
    fn default() -> Self {
        Self {
            camera_id: String::from("default"),
            position: [0.0, 0.0, 5.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_deg: 60.0,
            aspect: 16.0 / 9.0,
            near: 0.1,
            far: 100.0,
        }
    }
}

/// Scene kernel: a pair of shaders with uniform definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneKernel {
    pub kernel_id: String,
    pub vertex_glsl: String,
    pub fragment_glsl: String,
    pub uniforms: Vec<UniformDefinition>,
}

/// Definition of a shader uniform variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniformDefinition {
    pub name: String,
    pub uniform_type: UniformType,
    pub offset: u32,
}

/// Supported uniform data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UniformType {
    Mat4,
    Vec3,
    Vec4,
    Float,
    UInt,
}

#[cfg(test)]
mod type_tests {
    use super::*;

    #[test]
    fn camera_preset_serde_roundtrip() {
        let preset = CameraPreset::default();
        let json = serde_json::to_string(&preset).unwrap();
        let decoded: CameraPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.camera_id, "default");
        assert!((decoded.fov_deg - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scene_kernel_serde_roundtrip() {
        let kernel = SceneKernel {
            kernel_id: "particles-basic".into(),
            vertex_glsl: "#version 450\nvoid main() {}".into(),
            fragment_glsl: "#version 450\nvoid main() {}".into(),
            uniforms: vec![UniformDefinition {
                name: "time".into(),
                uniform_type: UniformType::Float,
                offset: 0,
            }],
        };
        let json = serde_json::to_string(&kernel).unwrap();
        let decoded: SceneKernel = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.kernel_id, "particles-basic");
        assert_eq!(decoded.uniforms.len(), 1);
    }

    #[test]
    fn calibration_profile_serde_roundtrip() {
        let profile = CalibrationProfile {
            profile_id: "studio-3screen".into(),
            display_endpoints: vec![],
            camera_presets: vec![CameraPreset::default()],
            version: "1.0".into(),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let decoded: CalibrationProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.profile_id, "studio-3screen");
        assert_eq!(decoded.camera_presets.len(), 1);
    }
}
