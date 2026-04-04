//! WSZ-07: particles-basic shader integration tests.
//!
//! Verifies:
//! - naga compilation of vertex + fragment GLSL passes
//! - SceneController can load the particles-basic kernel
//! - uniform `particle_color` is automation-drivable
//! - 16-beat smooth ramp produces expected interpolated values

#[cfg(test)]
mod particles_basic_tests {
    use crate::scene_controller::{CompileOutcome, SceneController};
    use crate::shader::{CompileResult, ShaderSource, ShaderStage};
    use crate::shader_compiler::compile_glsl_to_spirv;
    use crate::types::{SceneKernel, UniformDefinition, UniformType};
    use crate::uniform_automation::{InterpolationMode, UniformAutomation};

    /// The GLSL source embedded for the particles-basic vertex shader.
    const PARTICLES_VERT: &str =
        include_str!("../../../assets/shaders/particles-basic/particles.vert");
    /// The GLSL source embedded for the particles-basic fragment shader.
    const PARTICLES_FRAG: &str =
        include_str!("../../../assets/shaders/particles-basic/particles.frag");

    fn particles_basic_kernel() -> SceneKernel {
        SceneKernel {
            kernel_id: String::from("particles-basic"),
            vertex_glsl: String::from(PARTICLES_VERT),
            fragment_glsl: String::from(PARTICLES_FRAG),
            uniforms: vec![
                UniformDefinition {
                    name: String::from("view_projection"),
                    uniform_type: UniformType::Mat4,
                    offset: 0,
                },
                UniformDefinition {
                    name: String::from("time_params"),
                    uniform_type: UniformType::Vec4,
                    offset: 64,
                },
                UniformDefinition {
                    name: String::from("particle_color"),
                    uniform_type: UniformType::Vec4,
                    offset: 80,
                },
                UniformDefinition {
                    name: String::from("resolution"),
                    uniform_type: UniformType::Vec4,
                    offset: 96,
                },
            ],
        }
    }

    #[test]
    fn vertex_shader_naga_compiles() {
        let source = ShaderSource {
            stage: ShaderStage::Vertex,
            glsl_source: String::from(PARTICLES_VERT),
            entry_point: String::from("main"),
            label: String::from("particles-basic.vert"),
        };
        let (result, spirv) = compile_glsl_to_spirv(&source);
        assert!(
            matches!(result, CompileResult::Ok { .. }),
            "vertex shader should compile: {result:?}"
        );
        assert!(spirv.is_some(), "SPIR-V output should be present");
        let spirv = spirv.unwrap();
        assert!(spirv.byte_size() > 0, "SPIR-V output should be non-empty");
    }

    #[test]
    fn fragment_shader_naga_compiles() {
        let source = ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: String::from(PARTICLES_FRAG),
            entry_point: String::from("main"),
            label: String::from("particles-basic.frag"),
        };
        let (result, spirv) = compile_glsl_to_spirv(&source);
        assert!(
            matches!(result, CompileResult::Ok { .. }),
            "fragment shader should compile: {result:?}"
        );
        assert!(spirv.is_some());
    }

    #[test]
    fn scene_controller_loads_particles_basic() {
        let mut ctrl = SceneController::new();
        let outcome = ctrl.load_scene(&particles_basic_kernel());
        assert!(
            matches!(outcome, CompileOutcome::Ok { .. }),
            "particles-basic kernel should load: {outcome:?}"
        );
        assert_eq!(ctrl.active_scene(), Some("particles-basic"));
        assert_eq!(ctrl.pipeline_count(), 1);
    }

    #[test]
    fn particle_color_automation_driven() {
        let mut ctrl = SceneController::new();
        ctrl.load_scene(&particles_basic_kernel());

        // Add a 16-beat linear ramp for particle_color (red channel)
        let mut auto = UniformAutomation::new("particle_color", InterpolationMode::Linear);
        auto.add_keyframe(0.0, 0.0);
        auto.add_keyframe(16.0, 1.0);
        ctrl.add_automation(auto);

        // At beat 0: expect ~0.0
        ctrl.tick(0.0, 0.0, 1.0, 120.0);
        let dirty = ctrl.buffers().flush_dirty();
        assert_eq!(dirty.len(), 1);
        let (label, uniforms) = &dirty[0];
        assert_eq!(label, "particles-basic");
        assert!(
            uniforms.color_tint[0].abs() < 0.01,
            "at beat 0 color_r should be ~0.0, got {}",
            uniforms.color_tint[0]
        );

        // At beat 8: expect ~0.5
        ctrl.tick(2.0, 8.0, 2.0, 120.0);
        let dirty = ctrl.buffers().flush_dirty();
        let (_, uniforms) = &dirty[0];
        assert!(
            (uniforms.color_tint[0] - 0.5).abs() < 0.01,
            "at beat 8 color_r should be ~0.5, got {}",
            uniforms.color_tint[0]
        );

        // At beat 16: expect ~1.0
        ctrl.tick(4.0, 16.0, 4.0, 120.0);
        let dirty = ctrl.buffers().flush_dirty();
        let (_, uniforms) = &dirty[0];
        assert!(
            (uniforms.color_tint[0] - 1.0).abs() < 0.01,
            "at beat 16 color_r should be ~1.0, got {}",
            uniforms.color_tint[0]
        );
    }

    #[test]
    fn particle_color_step_automation() {
        let mut ctrl = SceneController::new();
        ctrl.load_scene(&particles_basic_kernel());

        let mut auto = UniformAutomation::new("particle_color", InterpolationMode::Step);
        auto.add_keyframe(0.0, 0.0);
        auto.add_keyframe(8.0, 1.0);
        ctrl.add_automation(auto);

        // Before beat 8: expect 0.0
        ctrl.tick(1.0, 4.0, 1.0, 120.0);
        let dirty = ctrl.buffers().flush_dirty();
        let (_, uniforms) = &dirty[0];
        assert!(
            uniforms.color_tint[0].abs() < 0.01,
            "before beat 8 color_r should be 0.0, got {}",
            uniforms.color_tint[0]
        );

        // At beat 8: step to 1.0
        ctrl.tick(2.0, 8.0, 2.0, 120.0);
        let dirty = ctrl.buffers().flush_dirty();
        let (_, uniforms) = &dirty[0];
        assert!(
            (uniforms.color_tint[0] - 1.0).abs() < 0.01,
            "at beat 8 color_r should step to 1.0, got {}",
            uniforms.color_tint[0]
        );
    }
}
