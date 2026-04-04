//! wgpu-based visual rendering bridge for Vidodo visual runtime.
//!
//! This crate provides GPU device management, window abstraction, shader compilation,
//! and render pipeline types. Currently defines the abstraction layer; actual wgpu/winit
//! dependencies are gated behind feature flags for future phases.

pub mod device;
pub mod shader;
pub mod types;
pub mod window;

#[cfg(test)]
mod tests;
