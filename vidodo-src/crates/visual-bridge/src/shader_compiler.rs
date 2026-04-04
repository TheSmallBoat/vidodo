//! GLSL → naga Module → SPIR-V compilation pipeline.
//!
//! Uses the `naga` crate to parse GLSL 450 source into an intermediate module,
//! validate it, and emit SPIR-V binary output.

use crate::shader::{
    CompileResult, DiagnosticSeverity, ShaderDiagnostic, ShaderSource, ShaderStage,
};
use naga::back::spv;
use naga::front::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

/// Compiled SPIR-V output from the naga pipeline.
#[derive(Debug, Clone)]
pub struct SpirVOutput {
    /// SPIR-V words (u32 each, so byte count = words.len() * 4).
    pub words: Vec<u32>,
    /// The shader label.
    pub label: String,
}

impl SpirVOutput {
    /// SPIR-V byte size.
    pub fn byte_size(&self) -> usize {
        self.words.len() * 4
    }
}

/// Map `ShaderStage` to `naga::ShaderStage`.
fn to_naga_stage(stage: ShaderStage) -> naga::ShaderStage {
    match stage {
        ShaderStage::Vertex => naga::ShaderStage::Vertex,
        ShaderStage::Fragment => naga::ShaderStage::Fragment,
    }
}

/// Compile a GLSL shader source to SPIR-V via naga.
///
/// Returns `CompileResult::Ok` with SPIR-V size on success, or
/// `CompileResult::Error` with diagnostics on failure. Never panics
/// on bad GLSL input.
pub fn compile_glsl_to_spirv(source: &ShaderSource) -> (CompileResult, Option<SpirVOutput>) {
    // 1. Parse GLSL
    let naga_stage = to_naga_stage(source.stage);
    let options = glsl::Options { stage: naga_stage, defines: Default::default() };

    let mut frontend = glsl::Frontend::default();
    let module = match frontend.parse(&options, &source.glsl_source) {
        Ok(m) => m,
        Err(errors) => {
            let diagnostics = vec![ShaderDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: errors.to_string(),
                line: None,
                column: None,
            }];
            return (CompileResult::Error { diagnostics }, None);
        }
    };

    // 2. Validate module
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = match validator.validate(&module) {
        Ok(info) => info,
        Err(e) => {
            let diagnostics = vec![ShaderDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: format!("validation: {e}"),
                line: None,
                column: None,
            }];
            return (CompileResult::Error { diagnostics }, None);
        }
    };

    // 3. Emit SPIR-V
    let spv_options = spv::Options { lang_version: (1, 0), ..Default::default() };

    let pipeline_options =
        spv::PipelineOptions { shader_stage: naga_stage, entry_point: source.entry_point.clone() };

    match spv::write_vec(&module, &info, &spv_options, Some(&pipeline_options)) {
        Ok(words) => {
            let spirv_size = words.len() * 4;
            let output = SpirVOutput { words, label: source.label.clone() };
            (CompileResult::Ok { spirv_size }, Some(output))
        }
        Err(e) => {
            let diagnostics = vec![ShaderDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: format!("SPIR-V emit: {e}"),
                line: None,
                column: None,
            }];
            (CompileResult::Error { diagnostics }, None)
        }
    }
}

#[cfg(test)]
mod naga_tests {
    use super::*;

    fn passthrough_vertex() -> ShaderSource {
        ShaderSource {
            stage: ShaderStage::Vertex,
            glsl_source: String::from(
                r"#version 450 core
layout(location = 0) in vec3 position;
void main() {
    gl_Position = vec4(position, 1.0);
}
",
            ),
            entry_point: String::from("main"),
            label: String::from("passthrough.vert"),
        }
    }

    fn color_fragment() -> ShaderSource {
        ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: String::from(
                r"#version 450 core
layout(location = 0) out vec4 fragColor;
void main() {
    fragColor = vec4(1.0, 0.0, 0.5, 1.0);
}
",
            ),
            entry_point: String::from("main"),
            label: String::from("color.frag"),
        }
    }

    #[test]
    fn passthrough_vert_compiles_to_spirv() {
        let src = passthrough_vertex();
        let (result, output) = compile_glsl_to_spirv(&src);

        match result {
            CompileResult::Ok { spirv_size } => {
                assert!(spirv_size > 0, "SPIR-V should have non-zero size");
                let out = output.unwrap();
                assert!(!out.words.is_empty());
                assert_eq!(out.byte_size(), spirv_size);
            }
            CompileResult::Error { diagnostics } => {
                panic!("expected success, got errors: {diagnostics:?}");
            }
        }
    }

    #[test]
    fn color_frag_compiles_to_spirv() {
        let src = color_fragment();
        let (result, output) = compile_glsl_to_spirv(&src);

        match result {
            CompileResult::Ok { spirv_size } => {
                assert!(spirv_size > 0);
                assert!(output.is_some());
            }
            CompileResult::Error { diagnostics } => {
                panic!("expected success, got errors: {diagnostics:?}");
            }
        }
    }

    #[test]
    fn syntax_error_returns_diagnostic_not_panic() {
        let bad = ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: String::from(
                r"#version 450 core
void main( {
    this is not valid glsl !!!
}
",
            ),
            entry_point: String::from("main"),
            label: String::from("bad.frag"),
        };

        let (result, output) = compile_glsl_to_spirv(&bad);

        assert!(output.is_none(), "bad shader should produce no SPIR-V");
        match result {
            CompileResult::Error { diagnostics } => {
                assert!(!diagnostics.is_empty(), "should have at least one diagnostic");
                assert!(diagnostics.iter().any(|d| d.severity == DiagnosticSeverity::Error));
            }
            CompileResult::Ok { .. } => {
                panic!("bad GLSL should not compile successfully");
            }
        }
    }
}
