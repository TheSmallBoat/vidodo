//! Scene controller: loads and compiles scene kernels, managing the
//! active render pipeline and uniform buffer for each scene.

use crate::buffer_manager::BufferManager;
use crate::render_pipeline::{
    BlendMode, RenderPipelineDescriptor, RenderPipelineManager, VertexBufferLayout,
};
use crate::shader::{ShaderSource, ShaderStage};
use crate::shader_compiler::compile_glsl_to_spirv;
use crate::types::SceneKernel;
use crate::uniform::SceneUniformsGPU;
use crate::uniform_automation::UniformAutomation;

/// Outcome of a scene compile attempt.
#[derive(Debug)]
pub enum CompileOutcome {
    /// Both shaders compiled, pipeline created.
    Ok { pipeline_label: String },
    /// One or both shaders failed compilation.
    ShaderError { diagnostics: Vec<String> },
    /// Pipeline validation failed.
    PipelineError { reason: String },
}

/// Controls loading, compiling, and managing the active scene.
pub struct SceneController {
    pipelines: RenderPipelineManager,
    buffers: BufferManager,
    automations: Vec<UniformAutomation>,
    active_scene: Option<String>,
}

impl SceneController {
    pub fn new() -> Self {
        Self {
            pipelines: RenderPipelineManager::new(),
            buffers: BufferManager::new(),
            automations: Vec::new(),
            active_scene: None,
        }
    }

    /// Load a scene kernel: compile its vertex + fragment GLSL, create
    /// a render pipeline, and allocate a uniform buffer slot.
    pub fn load_scene(&mut self, kernel: &SceneKernel) -> CompileOutcome {
        // 1. Compile vertex shader
        let vert_src = ShaderSource {
            stage: ShaderStage::Vertex,
            glsl_source: kernel.vertex_glsl.clone(),
            entry_point: String::from("main"),
            label: format!("{}_vert", kernel.kernel_id),
        };
        let (vert_result, _) = compile_glsl_to_spirv(&vert_src);
        if let crate::shader::CompileResult::Error { diagnostics } = &vert_result {
            return CompileOutcome::ShaderError {
                diagnostics: diagnostics.iter().map(|d| d.message.clone()).collect(),
            };
        }

        // 2. Compile fragment shader
        let frag_src = ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: kernel.fragment_glsl.clone(),
            entry_point: String::from("main"),
            label: format!("{}_frag", kernel.kernel_id),
        };
        let (frag_result, _) = compile_glsl_to_spirv(&frag_src);
        if let crate::shader::CompileResult::Error { diagnostics } = &frag_result {
            return CompileOutcome::ShaderError {
                diagnostics: diagnostics.iter().map(|d| d.message.clone()).collect(),
            };
        }

        // 3. Build the pipeline descriptor
        let descriptor = RenderPipelineDescriptor {
            label: kernel.kernel_id.clone(),
            vertex_shader: vert_src,
            fragment_shader: frag_src,
            vertex_layout: VertexBufferLayout::position_uv(),
            blend_mode: BlendMode::AlphaBlend,
            bind_group_count: 1,
        };

        let (_, status) = self.pipelines.create_pipeline(descriptor);
        if let crate::render_pipeline::PipelineStatus::Error(reason) = status {
            return CompileOutcome::PipelineError { reason };
        }

        // 4. Allocate uniform buffer
        self.buffers.allocate(&kernel.kernel_id);
        self.active_scene = Some(kernel.kernel_id.clone());

        CompileOutcome::Ok { pipeline_label: kernel.kernel_id.clone() }
    }

    /// Compile a scene (alias for load_scene for API ergonomics).
    pub fn compile_scene(&mut self, kernel: &SceneKernel) -> CompileOutcome {
        self.load_scene(kernel)
    }

    /// Add a uniform automation track to the active scene.
    pub fn add_automation(&mut self, automation: UniformAutomation) {
        self.automations.push(automation);
    }

    /// Tick the scene at the given beat and elapsed time.
    ///
    /// Updates automated uniform parameters and marks the buffer dirty.
    pub fn tick(&mut self, elapsed_sec: f32, beat: f32, bar: f32, tempo: f32) {
        if let Some(ref scene_id) = self.active_scene {
            // Build updated uniforms
            let mut uniforms = SceneUniformsGPU::default();
            uniforms.set_time(elapsed_sec, beat, bar, tempo);

            // Apply automations to color tint channels as a demo pathway
            for auto in &self.automations {
                let val = auto.evaluate(beat as f64);
                match auto.uniform_name.as_str() {
                    "particle_color" | "color_r" => uniforms.color_tint[0] = val,
                    "color_g" => uniforms.color_tint[1] = val,
                    "color_b" => uniforms.color_tint[2] = val,
                    "color_a" => uniforms.color_tint[3] = val,
                    _ => {}
                }
            }

            self.buffers.update(scene_id, uniforms);
        }
    }

    /// Get the active scene id.
    pub fn active_scene(&self) -> Option<&str> {
        self.active_scene.as_deref()
    }

    /// Number of loaded pipelines.
    pub fn pipeline_count(&self) -> usize {
        self.pipelines.count()
    }

    /// Get the underlying buffer manager for flush.
    pub fn buffers(&mut self) -> &mut BufferManager {
        &mut self.buffers
    }
}

impl Default for SceneController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UniformDefinition;
    use crate::uniform_automation::InterpolationMode;

    fn particles_basic_kernel() -> SceneKernel {
        SceneKernel {
            kernel_id: String::from("particles-basic"),
            vertex_glsl: String::from(
                "#version 450\nlayout(location=0) in vec3 pos;\nvoid main() { gl_Position = vec4(pos, 1.0); }\n",
            ),
            fragment_glsl: String::from(
                "#version 450\nlayout(location=0) out vec4 color;\nvoid main() { color = vec4(1.0); }\n",
            ),
            uniforms: vec![UniformDefinition {
                name: String::from("particle_color"),
                uniform_type: crate::types::UniformType::Vec4,
                offset: 0,
            }],
        }
    }

    #[test]
    fn load_and_compile_particles_basic() {
        let mut ctrl = SceneController::new();
        let outcome = ctrl.load_scene(&particles_basic_kernel());
        assert!(matches!(outcome, CompileOutcome::Ok { .. }));
        assert_eq!(ctrl.active_scene(), Some("particles-basic"));
        assert_eq!(ctrl.pipeline_count(), 1);
    }

    #[test]
    fn compile_scene_alias() {
        let mut ctrl = SceneController::new();
        let outcome = ctrl.compile_scene(&particles_basic_kernel());
        assert!(matches!(outcome, CompileOutcome::Ok { .. }));
    }

    #[test]
    fn tick_updates_uniform_automation() {
        let mut ctrl = SceneController::new();
        ctrl.load_scene(&particles_basic_kernel());

        let mut auto = UniformAutomation::new("particle_color", InterpolationMode::Linear);
        auto.add_keyframe(0.0, 0.0);
        auto.add_keyframe(16.0, 1.0);
        ctrl.add_automation(auto);

        // At beat 8: expect ~0.5
        ctrl.tick(2.0, 8.0, 2.0, 120.0);
        let dirty = ctrl.buffers().flush_dirty();
        assert_eq!(dirty.len(), 1);
        let (label, uniforms) = &dirty[0];
        assert_eq!(label, "particles-basic");
        assert!((uniforms.color_tint[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn bad_glsl_returns_shader_error() {
        let kernel = SceneKernel {
            kernel_id: String::from("bad-shader"),
            vertex_glsl: String::from("this is not valid GLSL"),
            fragment_glsl: String::from("#version 450\nvoid main() {}"),
            uniforms: vec![],
        };
        let mut ctrl = SceneController::new();
        let outcome = ctrl.load_scene(&kernel);
        assert!(matches!(outcome, CompileOutcome::ShaderError { .. }));
        assert!(ctrl.active_scene().is_none());
    }
}
