//! Render pipeline configuration and management.
//!
//! Defines the CPU-side render pipeline descriptor that will drive
//! `wgpu::RenderPipeline` creation when GPU support is wired.

use crate::shader::{ShaderSource, ShaderStage};

/// Vertex attribute format (subset of wgpu formats used by Vidodo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexFormat {
    Float32x2,
    Float32x3,
    Float32x4,
}

impl VertexFormat {
    /// Byte size of this format.
    pub fn byte_size(self) -> u32 {
        match self {
            VertexFormat::Float32x2 => 8,
            VertexFormat::Float32x3 => 12,
            VertexFormat::Float32x4 => 16,
        }
    }
}

/// A single vertex attribute in a vertex buffer layout.
#[derive(Debug, Clone)]
pub struct VertexAttribute {
    pub location: u32,
    pub format: VertexFormat,
    pub offset: u32,
}

/// Vertex buffer layout descriptor.
#[derive(Debug, Clone)]
pub struct VertexBufferLayout {
    pub stride: u32,
    pub attributes: Vec<VertexAttribute>,
}

impl VertexBufferLayout {
    /// Standard position-only layout (vec3 at location 0).
    pub fn position_only() -> Self {
        Self {
            stride: 12,
            attributes: vec![VertexAttribute {
                location: 0,
                format: VertexFormat::Float32x3,
                offset: 0,
            }],
        }
    }

    /// Position + UV layout (vec3 pos at 0, vec2 uv at 1).
    pub fn position_uv() -> Self {
        Self {
            stride: 20,
            attributes: vec![
                VertexAttribute { location: 0, format: VertexFormat::Float32x3, offset: 0 },
                VertexAttribute { location: 1, format: VertexFormat::Float32x2, offset: 12 },
            ],
        }
    }

    /// Compute expected stride from attributes.
    pub fn computed_stride(&self) -> u32 {
        self.attributes.iter().map(|a| a.offset + a.format.byte_size()).max().unwrap_or(0)
    }
}

/// Blend mode for the fragment output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Opaque,
    AlphaBlend,
    Additive,
}

/// CPU-side render pipeline descriptor.
#[derive(Debug, Clone)]
pub struct RenderPipelineDescriptor {
    pub label: String,
    pub vertex_shader: ShaderSource,
    pub fragment_shader: ShaderSource,
    pub vertex_layout: VertexBufferLayout,
    pub blend_mode: BlendMode,
    /// Number of bind groups (uniform blocks).
    pub bind_group_count: u32,
}

/// Status of a compiled pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineStatus {
    /// Successfully created.
    Ready,
    /// Shader compilation or validation failed.
    Error(String),
}

/// Manages a set of render pipelines.
pub struct RenderPipelineManager {
    pipelines: Vec<(RenderPipelineDescriptor, PipelineStatus)>,
}

impl RenderPipelineManager {
    pub fn new() -> Self {
        Self { pipelines: Vec::new() }
    }

    /// Register a pipeline descriptor.
    ///
    /// Validates the descriptor and returns the pipeline index.
    pub fn create_pipeline(&mut self, desc: RenderPipelineDescriptor) -> (usize, PipelineStatus) {
        let status = validate_descriptor(&desc);
        let index = self.pipelines.len();
        self.pipelines.push((desc, status.clone()));
        (index, status)
    }

    /// Get the status of a pipeline by index.
    pub fn status(&self, index: usize) -> Option<&PipelineStatus> {
        self.pipelines.get(index).map(|(_, s)| s)
    }

    /// Get the descriptor of a pipeline by index.
    pub fn descriptor(&self, index: usize) -> Option<&RenderPipelineDescriptor> {
        self.pipelines.get(index).map(|(d, _)| d)
    }

    /// Number of registered pipelines.
    pub fn count(&self) -> usize {
        self.pipelines.len()
    }
}

impl Default for RenderPipelineManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a pipeline descriptor (CPU-side checks).
fn validate_descriptor(desc: &RenderPipelineDescriptor) -> PipelineStatus {
    if desc.vertex_shader.stage != ShaderStage::Vertex {
        return PipelineStatus::Error("vertex_shader must be Vertex stage".into());
    }
    if desc.fragment_shader.stage != ShaderStage::Fragment {
        return PipelineStatus::Error("fragment_shader must be Fragment stage".into());
    }
    if desc.vertex_layout.attributes.is_empty() {
        return PipelineStatus::Error("vertex_layout must have at least one attribute".into());
    }
    if desc.vertex_layout.computed_stride() > desc.vertex_layout.stride {
        return PipelineStatus::Error(format!(
            "vertex stride {} is less than computed minimum {}",
            desc.vertex_layout.stride,
            desc.vertex_layout.computed_stride()
        ));
    }
    PipelineStatus::Ready
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    fn vert_source() -> ShaderSource {
        ShaderSource {
            stage: ShaderStage::Vertex,
            glsl_source: String::from("#version 450\nvoid main() {}"),
            entry_point: String::from("main"),
            label: String::from("test.vert"),
        }
    }

    fn frag_source() -> ShaderSource {
        ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: String::from("#version 450\nvoid main() {}"),
            entry_point: String::from("main"),
            label: String::from("test.frag"),
        }
    }

    #[test]
    fn create_valid_pipeline() {
        let mut mgr = RenderPipelineManager::new();
        let (idx, status) = mgr.create_pipeline(RenderPipelineDescriptor {
            label: String::from("quad"),
            vertex_shader: vert_source(),
            fragment_shader: frag_source(),
            vertex_layout: VertexBufferLayout::position_only(),
            blend_mode: BlendMode::Opaque,
            bind_group_count: 1,
        });

        assert_eq!(idx, 0);
        assert_eq!(status, PipelineStatus::Ready);
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn wrong_shader_stage_rejected() {
        let mut mgr = RenderPipelineManager::new();
        let (_, status) = mgr.create_pipeline(RenderPipelineDescriptor {
            label: String::from("bad"),
            vertex_shader: frag_source(), // wrong stage!
            fragment_shader: frag_source(),
            vertex_layout: VertexBufferLayout::position_only(),
            blend_mode: BlendMode::AlphaBlend,
            bind_group_count: 1,
        });

        assert!(matches!(status, PipelineStatus::Error(_)));
    }

    #[test]
    fn position_uv_layout() {
        let layout = VertexBufferLayout::position_uv();
        assert_eq!(layout.stride, 20);
        assert_eq!(layout.attributes.len(), 2);
        assert_eq!(layout.computed_stride(), 20); // 12 + 8
    }

    #[test]
    fn stride_too_small_rejected() {
        let mut mgr = RenderPipelineManager::new();
        let (_, status) = mgr.create_pipeline(RenderPipelineDescriptor {
            label: String::from("bad-stride"),
            vertex_shader: vert_source(),
            fragment_shader: frag_source(),
            vertex_layout: VertexBufferLayout {
                stride: 4, // too small for vec3
                attributes: vec![VertexAttribute {
                    location: 0,
                    format: VertexFormat::Float32x3,
                    offset: 0,
                }],
            },
            blend_mode: BlendMode::Opaque,
            bind_group_count: 1,
        });

        assert!(matches!(status, PipelineStatus::Error(_)));
    }
}
