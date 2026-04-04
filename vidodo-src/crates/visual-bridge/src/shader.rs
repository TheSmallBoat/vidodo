//! GLSL shader compilation types (naga integration placeholder).
//!
//! Defines shader source management, compilation status, and diagnostic types.
//! Actual naga GLSL→SPIR-V compilation will be integrated when naga is added.

use serde::{Deserialize, Serialize};

/// Shader stage in the render pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderStage {
    Vertex,
    Fragment,
}

/// A shader source with its stage and GLSL code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderSource {
    pub stage: ShaderStage,
    pub glsl_source: String,
    pub entry_point: String,
    pub label: String,
}

/// Result of attempting to compile a shader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompileResult {
    /// Compilation succeeded, SPIR-V bytes available.
    Ok { spirv_size: usize },
    /// Compilation failed with diagnostics.
    Error { diagnostics: Vec<ShaderDiagnostic> },
}

/// A diagnostic message from shader compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// Shader compilation manager.
///
/// Currently validates GLSL source structure. Full naga compilation
/// will be wired when naga is added as a dependency.
pub struct ShaderCompiler {
    compiled_count: u64,
}

impl ShaderCompiler {
    pub fn new() -> Self {
        Self { compiled_count: 0 }
    }

    /// Validate and "compile" a shader source.
    ///
    /// Currently performs structural validation (non-empty source, version directive).
    /// Returns Ok with estimated SPIR-V size or Error with diagnostics.
    pub fn compile(&mut self, source: &ShaderSource) -> CompileResult {
        let mut diagnostics = Vec::new();

        if source.glsl_source.is_empty() {
            diagnostics.push(ShaderDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: "empty shader source".into(),
                line: None,
                column: None,
            });
        }

        if !source.glsl_source.contains("#version") {
            diagnostics.push(ShaderDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: "missing #version directive".into(),
                line: Some(1),
                column: Some(1),
            });
        }

        let expected_main = "void main()";
        if !source.glsl_source.contains(expected_main) && !source.glsl_source.contains("void main(")
        {
            diagnostics.push(ShaderDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: "missing main() entry point".into(),
                line: None,
                column: None,
            });
        }

        if diagnostics.iter().any(|d| d.severity == DiagnosticSeverity::Error) {
            return CompileResult::Error { diagnostics };
        }

        self.compiled_count += 1;

        // Estimate SPIR-V size (~4x source size is a rough estimate)
        CompileResult::Ok { spirv_size: source.glsl_source.len() * 4 }
    }

    pub fn compiled_count(&self) -> u64 {
        self.compiled_count
    }
}

impl Default for ShaderCompiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod shader_tests {
    use super::*;

    fn valid_vertex_source() -> ShaderSource {
        ShaderSource {
            stage: ShaderStage::Vertex,
            glsl_source: String::from(
                r#"#version 450 core
layout(location = 0) in vec3 position;
void main() {
    gl_Position = vec4(position, 1.0);
}"#,
            ),
            entry_point: String::from("main"),
            label: String::from("test.vert"),
        }
    }

    #[test]
    fn compile_valid_shader() {
        let mut compiler = ShaderCompiler::new();
        let result = compiler.compile(&valid_vertex_source());
        assert!(matches!(result, CompileResult::Ok { .. }));
        assert_eq!(compiler.compiled_count(), 1);
    }

    #[test]
    fn empty_source_fails() {
        let mut compiler = ShaderCompiler::new();
        let source = ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: String::new(),
            entry_point: String::from("main"),
            label: String::from("empty.frag"),
        };
        let result = compiler.compile(&source);
        assert!(matches!(result, CompileResult::Error { .. }));
    }

    #[test]
    fn missing_version_fails() {
        let mut compiler = ShaderCompiler::new();
        let source = ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: String::from("void main() { }"),
            entry_point: String::from("main"),
            label: String::from("noversion.frag"),
        };
        let result = compiler.compile(&source);
        assert!(matches!(result, CompileResult::Error { .. }));
    }

    #[test]
    fn missing_main_fails() {
        let mut compiler = ShaderCompiler::new();
        let source = ShaderSource {
            stage: ShaderStage::Fragment,
            glsl_source: String::from("#version 450\nfloat x = 1.0;"),
            entry_point: String::from("main"),
            label: String::from("nomain.frag"),
        };
        let result = compiler.compile(&source);
        assert!(matches!(result, CompileResult::Error { .. }));
    }
}
