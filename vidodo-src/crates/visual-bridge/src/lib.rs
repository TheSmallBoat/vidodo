//! wgpu-based visual rendering bridge for Vidodo visual runtime.
//!
//! This crate provides GPU device management, window abstraction, shader compilation,
//! and render pipeline types. Currently defines the abstraction layer; actual wgpu/winit
//! dependencies are gated behind feature flags for future phases.

pub mod backend;
pub mod buffer_manager;
pub mod camera_rig;
pub mod device;
pub mod render_pipeline;
pub mod scene_controller;
pub mod shader;
pub mod shader_compiler;
pub mod types;
pub mod uniform;
pub mod uniform_automation;
pub mod viewport_composer;
pub mod window;

#[cfg(test)]
mod tests;
