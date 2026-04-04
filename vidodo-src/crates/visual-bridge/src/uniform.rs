//! GPU uniform data model.
//!
//! Defines `repr(C)` structs for uniform buffers that will be uploaded
//! to the GPU via wgpu. All structs are 256-byte aligned for
//! `minUniformBufferOffsetAlignment`.

/// Scene-level uniforms passed to shaders every frame.
///
/// Layout: 256-byte aligned. Fields match the GLSL uniform block:
/// ```glsl
/// layout(set = 0, binding = 0) uniform SceneUniforms {
///     mat4 view_projection;
///     vec4 time_params;    // x=elapsed_sec, y=beat, z=bar, w=tempo
///     vec4 color_tint;     // rgba
///     vec4 resolution;     // x=width, y=height, z=1/w, w=1/h
/// };
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SceneUniformsGPU {
    /// 4x4 view-projection matrix (column-major).
    pub view_projection: [f32; 16],
    /// [elapsed_sec, beat, bar, tempo]
    pub time_params: [f32; 4],
    /// [r, g, b, a] color tint.
    pub color_tint: [f32; 4],
    /// [width, height, 1/width, 1/height]
    pub resolution: [f32; 4],
    /// Padding to 256 bytes (total: 16+4+4+4+36 = 64 f32 = 256 bytes).
    pub _pad: [f32; 36],
}

// Compile-time check: must be exactly 256 bytes
const _: () = assert!(std::mem::size_of::<SceneUniformsGPU>() == 256);

impl Default for SceneUniformsGPU {
    fn default() -> Self {
        let mut u = Self {
            view_projection: [0.0; 16],
            time_params: [0.0; 4],
            color_tint: [1.0, 1.0, 1.0, 1.0],
            resolution: [1920.0, 1080.0, 1.0 / 1920.0, 1.0 / 1080.0],
            _pad: [0.0; 36],
        };
        // Identity matrix
        u.view_projection[0] = 1.0;
        u.view_projection[5] = 1.0;
        u.view_projection[10] = 1.0;
        u.view_projection[15] = 1.0;
        u
    }
}

impl SceneUniformsGPU {
    /// Update time parameters from the scheduler tick.
    pub fn set_time(&mut self, elapsed_sec: f32, beat: f32, bar: f32, tempo: f32) {
        self.time_params = [elapsed_sec, beat, bar, tempo];
    }

    /// Update resolution.
    pub fn set_resolution(&mut self, width: f32, height: f32) {
        self.resolution = [
            width,
            height,
            if width > 0.0 { 1.0 / width } else { 0.0 },
            if height > 0.0 { 1.0 / height } else { 0.0 },
        ];
    }

    /// Update color tint.
    pub fn set_color_tint(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.color_tint = [r, g, b, a];
    }

    /// Get the raw bytes for GPU upload.
    ///
    /// # Safety
    /// `SceneUniformsGPU` is `repr(C)` with only `f32` fields, so
    /// any bit pattern is valid and there are no padding bytes.
    pub fn as_bytes(&self) -> &[u8] {
        let ptr = (self as *const Self).cast::<u8>();
        unsafe { core::slice::from_raw_parts(ptr, core::mem::size_of::<Self>()) }
    }
}

#[cfg(test)]
mod uniform_tests {
    use super::*;

    #[test]
    fn size_is_256_bytes() {
        assert_eq!(std::mem::size_of::<SceneUniformsGPU>(), 256);
    }

    #[test]
    fn default_has_identity_matrix() {
        let u = SceneUniformsGPU::default();
        assert_eq!(u.view_projection[0], 1.0);
        assert_eq!(u.view_projection[5], 1.0);
        assert_eq!(u.view_projection[10], 1.0);
        assert_eq!(u.view_projection[15], 1.0);
        // Off-diagonal should be 0
        assert_eq!(u.view_projection[1], 0.0);
    }

    #[test]
    fn set_time_updates_params() {
        let mut u = SceneUniformsGPU::default();
        u.set_time(1.5, 6.0, 2.0, 128.0);
        assert_eq!(u.time_params, [1.5, 6.0, 2.0, 128.0]);
    }

    #[test]
    fn set_resolution_computes_reciprocals() {
        let mut u = SceneUniformsGPU::default();
        u.set_resolution(800.0, 600.0);
        assert_eq!(u.resolution[0], 800.0);
        assert_eq!(u.resolution[1], 600.0);
        assert!((u.resolution[2] - 1.0 / 800.0).abs() < 1e-6);
        assert!((u.resolution[3] - 1.0 / 600.0).abs() < 1e-6);
    }

    #[test]
    fn as_bytes_returns_256_bytes() {
        let u = SceneUniformsGPU::default();
        assert_eq!(u.as_bytes().len(), 256);
    }
}
