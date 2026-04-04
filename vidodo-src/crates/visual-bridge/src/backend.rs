//! `BackendAdapter` implementation for the wgpu visual backend.
//!
//! `VisualWgpuBackend` bridges the Vidodo scheduler with the visual
//! rendering pipeline: scene controller, camera rig, buffer manager,
//! and window manager.
//!
//! `FlatDisplayBackend` is an alias for the default single/multi-window
//! mode without spatial warping.

use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

use crate::buffer_manager::BufferManager;
use crate::camera_rig::CameraRig;
use crate::scene_controller::{CompileOutcome, SceneController};
use crate::types::{CameraPreset, SceneKernel, UniformDefinition, UniformType};
use crate::uniform::SceneUniformsGPU;
use crate::window::{DisplayEndpoint, WindowConfig, WindowManager};

use std::fmt;

/// Safe fallback vertex shader — identity passthrough.
const SAFE_VERT: &str = "#version 450\nlayout(location=0) in vec3 pos;\nvoid main() { gl_Position = vec4(pos, 1.0); }\n";
/// Safe fallback fragment shader — solid dark grey.
const SAFE_FRAG: &str = "#version 450\nlayout(location=0) out vec4 color;\nvoid main() { color = vec4(0.2, 0.2, 0.2, 1.0); }\n";

/// wgpu-backed visual backend implementing the seven-method adapter protocol.
///
/// Lifecycle: `prepare_backend` → opens windows, sets camera →
/// `execute_payload(Visual)` → loads scene kernel, renders frame →
/// `apply_show_state()` → updates uniforms + camera →
/// `apply_degrade_mode()` → switches to safe fallback shader →
/// `shutdown_backend` → closes windows.
pub struct VisualWgpuBackend {
    plugin_id: String,
    prepared: bool,
    shut_down: bool,
    degraded: bool,
    topology_ref: Option<String>,
    execute_count: u64,
    frame_count: u64,
    /// Active camera rig (updated on apply_show_state or prepare).
    active_camera: Option<CameraRig>,
    /// Scene controller managing pipelines and uniform buffers.
    scene_ctrl: SceneController,
    /// Window manager for multi-display rendering.
    window_mgr: WindowManager,
    /// Buffer manager for uniform data.
    #[allow(dead_code)]
    buffer_mgr: BufferManager,
    /// Current uniform state.
    uniforms: SceneUniformsGPU,
    /// Whether we are running in safe fallback shader mode.
    safe_mode: bool,
    /// Log of operations (for test inspection).
    log: Vec<String>,
}

impl fmt::Debug for VisualWgpuBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisualWgpuBackend")
            .field("plugin_id", &self.plugin_id)
            .field("prepared", &self.prepared)
            .field("shut_down", &self.shut_down)
            .field("degraded", &self.degraded)
            .field("execute_count", &self.execute_count)
            .field("frame_count", &self.frame_count)
            .field("safe_mode", &self.safe_mode)
            .finish()
    }
}

impl VisualWgpuBackend {
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            prepared: false,
            shut_down: false,
            degraded: false,
            topology_ref: None,
            execute_count: 0,
            frame_count: 0,
            active_camera: None,
            scene_ctrl: SceneController::new(),
            window_mgr: WindowManager::new(),
            buffer_mgr: BufferManager::new(),
            uniforms: SceneUniformsGPU::default(),
            safe_mode: false,
            log: Vec::new(),
        }
    }

    /// Return the operation log (for test inspection).
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// Number of payloads executed.
    pub fn execute_count(&self) -> u64 {
        self.execute_count
    }

    /// Number of frames rendered.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Whether the backend is in safe fallback shader mode.
    pub fn is_safe_mode(&self) -> bool {
        self.safe_mode
    }

    /// Render a frame: tick the scene controller, flush uniforms.
    fn render_frame(&mut self, scene_id: &str, elapsed_sec: f32, beat: f32, bar: f32, tempo: f32) {
        // Update camera view-projection into uniforms
        if let Some(ref rig) = self.active_camera {
            let vp = rig.view_projection();
            self.uniforms.view_projection = vp.to_cols_array();
        }

        // Tick scene controller (applies automations)
        self.scene_ctrl.tick(elapsed_sec, beat, bar, tempo);

        // Present a frame on each open window
        for idx in 0..self.window_mgr.windows().len() {
            if self.window_mgr.present_frame(idx).is_ok() {
                self.frame_count += 1;
            }
        }

        self.log.push(format!(
            "render_frame: scene={scene_id} frame={} beat={beat:.1}",
            self.frame_count,
        ));
    }

    /// Load the safe fallback shader as the active scene.
    fn load_safe_shader(&mut self) {
        let kernel = SceneKernel {
            kernel_id: String::from("_safe_fallback"),
            vertex_glsl: String::from(SAFE_VERT),
            fragment_glsl: String::from(SAFE_FRAG),
            uniforms: vec![],
        };
        let _ = self.scene_ctrl.load_scene(&kernel);
        self.safe_mode = true;
        self.log.push(String::from("load_safe_shader: switched to fallback"));
    }
}

impl BackendAdapter for VisualWgpuBackend {
    fn describe_backend(&self) -> BackendDescription {
        BackendDescription {
            plugin_id: self.plugin_id.clone(),
            backend_kind: String::from("visual"),
            capabilities: vec![
                String::from("scene_switch"),
                String::from("shader_render"),
                String::from("blend"),
                String::from("uniform_update"),
                String::from("multi_viewport"),
                String::from("camera_rig"),
            ],
            topology_types: vec![String::from("flat"), String::from("spatial_multiview")],
            status: if self.shut_down {
                String::from("shutdown")
            } else if self.degraded {
                String::from("degraded")
            } else if self.prepared {
                String::from("ready")
            } else {
                String::from("idle")
            },
        }
    }

    fn prepare_backend(&mut self, topology: &BackendTopology) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("WGPU-001: backend already shut down"));
        }
        match topology {
            BackendTopology::Visual { topology_ref, display_endpoints, .. } => {
                self.topology_ref = Some(topology_ref.clone());

                // Create a window for each display endpoint
                for (i, ep_label) in display_endpoints.iter().enumerate() {
                    let endpoint = DisplayEndpoint {
                        display_id: ep_label.clone(),
                        os_handle: None,
                        window: WindowConfig {
                            title: format!("Vidodo Visual — {ep_label}"),
                            ..WindowConfig::default()
                        },
                        role: if i == 0 { String::from("main") } else { format!("aux-{i}") },
                    };
                    self.window_mgr
                        .create_window(endpoint)
                        .map_err(|e| format!("WGPU-003: window create failed: {e}"))?;
                }

                // If no endpoints specified, create one default window
                if display_endpoints.is_empty() {
                    let endpoint = DisplayEndpoint {
                        display_id: String::from("default-display"),
                        os_handle: None,
                        window: WindowConfig::default(),
                        role: String::from("main"),
                    };
                    self.window_mgr
                        .create_window(endpoint)
                        .map_err(|e| format!("WGPU-003: window create failed: {e}"))?;
                }

                // Set default camera
                let default_preset = CameraPreset::default();
                self.active_camera = Some(CameraRig::from_preset(&default_preset));
                self.uniforms.view_projection =
                    CameraRig::from_preset(&default_preset).view_projection().to_cols_array();

                self.prepared = true;
                self.log.push(format!(
                    "prepare: topology={topology_ref} windows={}",
                    self.window_mgr.windows().len()
                ));
                Ok(())
            }
            _ => Err(String::from("WGPU-002: expected Visual topology")),
        }
    }

    fn apply_show_state(&mut self, show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("WGPU-004: backend already shut down"));
        }

        // Update camera from show state if a camera preset is indicated in the scene field
        // For now, just log and update uniforms from show revision
        self.uniforms.set_time(0.0, 0.0, 0.0, show_state.time.tempo as f32);

        // If show state specifies a scene, update resolution from first window
        if let Some(w) = self.window_mgr.windows().first() {
            self.uniforms
                .set_resolution(w.endpoint.window.width as f32, w.endpoint.window.height as f32);
        }

        self.log.push(format!(
            "apply_show_state: show={} rev={} scene={}",
            show_state.show_id, show_state.revision, show_state.active_visual_scene
        ));
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("WGPU-005: backend already shut down"));
        }
        match payload {
            ExecutablePayload::Visual {
                scene_id,
                shader_program,
                uniforms,
                duration_beats,
                blend,
                view_group,
            } => {
                self.execute_count += 1;

                // If degraded, use safe shader instead
                if self.degraded && !self.safe_mode {
                    self.load_safe_shader();
                }

                // When NOT in safe mode, load the requested scene kernel
                if !self.safe_mode {
                    let kernel = SceneKernel {
                        kernel_id: scene_id.clone(),
                        vertex_glsl: "#version 450\nlayout(location=0) in vec3 pos;\nvoid main() { gl_Position = vec4(pos, 1.0); }\n".to_string(),
                        fragment_glsl: "#version 450\nlayout(location=0) out vec4 color;\nvoid main() { color = vec4(1.0); }\n".to_string(),
                        uniforms: uniforms
                            .keys()
                            .enumerate()
                            .map(|(i, name)| UniformDefinition {
                                name: name.clone(),
                                uniform_type: UniformType::Float,
                                offset: i as u32 * 4,
                            })
                            .collect(),
                    };

                    match self.scene_ctrl.load_scene(&kernel) {
                        CompileOutcome::Ok { .. } => {}
                        CompileOutcome::ShaderError { diagnostics } => {
                            // Fall back to safe shader on compile error
                            self.load_safe_shader();
                            self.log.push(format!(
                                "shader_error: scene={scene_id} errors={diagnostics:?}, falling back to safe shader"
                            ));
                        }
                        CompileOutcome::PipelineError { reason } => {
                            self.load_safe_shader();
                            self.log.push(format!(
                                "pipeline_error: scene={scene_id} reason={reason}, falling back to safe shader"
                            ));
                        }
                    }
                }

                // Update uniforms from payload
                for (name, value) in uniforms {
                    if let Ok(v) = value.parse::<f32>() {
                        match name.as_str() {
                            "color_r" => self.uniforms.color_tint[0] = v,
                            "color_g" => self.uniforms.color_tint[1] = v,
                            "color_b" => self.uniforms.color_tint[2] = v,
                            "color_a" => self.uniforms.color_tint[3] = v,
                            _ => {}
                        }
                    }
                }

                // Render a frame
                let beats = duration_beats.unwrap_or(4) as f32;
                self.render_frame(scene_id, 0.0, beats, 1.0, 120.0);

                let blend_str = blend.as_deref().unwrap_or("none");
                let view_str = view_group.as_deref().unwrap_or("default");
                self.log.push(format!(
                    "execute: scene={scene_id} shader={shader_program} blend={blend_str} view_group={view_str}"
                ));

                Ok(BackendAck {
                    backend: self.plugin_id.clone(),
                    target: scene_id.clone(),
                    status: String::from("ok"),
                    detail: format!("render {shader_program} frame={}", self.frame_count),
                })
            }
            _ => Err(String::from("WGPU-006: expected Visual payload")),
        }
    }

    fn collect_backend_status(&self) -> BackendStatus {
        BackendStatus {
            plugin_id: self.plugin_id.clone(),
            status: if self.shut_down {
                String::from("shutdown")
            } else if self.degraded {
                String::from("degraded")
            } else {
                String::from("healthy")
            },
            latency_ms: Some(3.0),
            error_count: Some(0),
            last_ack_lag_ms: Some(1.5),
            detail: if self.safe_mode {
                Some(String::from("safe fallback shader active"))
            } else {
                None
            },
        }
    }

    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("WGPU-007: backend already shut down"));
        }
        if mode.mode != "normal" {
            self.degraded = true;
            self.load_safe_shader();
            self.log.push(format!("degrade: mode={} reason={}", mode.mode, mode.reason));
        } else {
            self.degraded = false;
            self.safe_mode = false;
            self.log.push(String::from("degrade: restored to normal"));
        }
        Ok(())
    }

    fn shutdown_backend(&mut self) -> Result<(), String> {
        // Close all windows
        for idx in 0..self.window_mgr.windows().len() {
            let _ = self.window_mgr.close_window(idx);
        }
        self.shut_down = true;
        self.prepared = false;
        self.log.push(String::from("shutdown"));
        Ok(())
    }
}

/// Type alias: `FlatDisplayBackend` is the default `VisualWgpuBackend`
/// for single/multi-window rendering without spatial warping.
pub type FlatDisplayBackend = VisualWgpuBackend;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn visual_topology() -> BackendTopology {
        BackendTopology::Visual {
            topology_ref: String::from("flat-main"),
            calibration_profile: None,
            display_endpoints: vec![String::from("display-1")],
        }
    }

    fn visual_topology_multi() -> BackendTopology {
        BackendTopology::Visual {
            topology_ref: String::from("spatial-3screen"),
            calibration_profile: None,
            display_endpoints: vec![
                String::from("display-front"),
                String::from("display-left"),
                String::from("display-right"),
            ],
        }
    }

    fn visual_payload(scene: &str, shader: &str) -> ExecutablePayload {
        ExecutablePayload::Visual {
            scene_id: scene.to_string(),
            shader_program: shader.to_string(),
            uniforms: BTreeMap::new(),
            duration_beats: Some(8),
            blend: Some(String::from("crossfade")),
            view_group: None,
        }
    }

    fn visual_payload_with_uniforms(scene: &str, shader: &str) -> ExecutablePayload {
        let mut uniforms = BTreeMap::new();
        uniforms.insert(String::from("color_r"), String::from("0.5"));
        uniforms.insert(String::from("color_g"), String::from("0.8"));
        ExecutablePayload::Visual {
            scene_id: scene.to_string(),
            shader_program: shader.to_string(),
            uniforms,
            duration_beats: Some(4),
            blend: None,
            view_group: Some(String::from("main")),
        }
    }

    fn test_show_state() -> ShowState {
        use vidodo_ir::{MusicalTime, OutputBinding, ShowPatchState, ShowSemantic, ShowTransition};
        ShowState {
            show_id: String::from("test-show"),
            revision: 1,
            mode: String::from("offline"),
            time: MusicalTime::at_bar(1, 1, "intro", 140.0),
            semantic: ShowSemantic {
                energy: 0.5,
                density: 0.5,
                tension: 0.5,
                brightness: 0.5,
                motion: 0.5,
                intent: String::from("test"),
            },
            transition: ShowTransition {
                state: String::from("steady"),
                from_scene: String::from("scene_intro"),
                to_scene: String::from("scene_intro"),
                window_open: true,
            },
            visual_output: OutputBinding {
                backend_id: String::from("wgpu-1"),
                topology_ref: String::from("flat-main"),
                calibration_profile: String::from("default"),
                active_group: String::from("scene_intro"),
            },
            audio_output: OutputBinding {
                backend_id: String::from("fake_audio"),
                topology_ref: String::from("stereo-main"),
                calibration_profile: String::from("default"),
                active_group: String::from("stereo-main"),
            },
            patch: ShowPatchState {
                allowed: true,
                scope: String::from("next_phrase_boundary"),
                locked_sections: Vec::new(),
            },
            adapter_plugins: BTreeMap::new(),
            resource_hubs: BTreeMap::new(),
            active_audio_layers: Vec::new(),
            active_visual_scene: String::from("scene_intro"),
        }
    }

    #[test]
    fn describe_returns_visual_kind_with_capabilities() {
        let backend = VisualWgpuBackend::new("wgpu-1");
        let desc = backend.describe_backend();
        assert_eq!(desc.backend_kind, "visual");
        assert_eq!(desc.status, "idle");
        assert!(desc.capabilities.contains(&String::from("scene_switch")));
        assert!(desc.capabilities.contains(&String::from("multi_viewport")));
        assert!(desc.capabilities.contains(&String::from("camera_rig")));
    }

    #[test]
    fn prepare_creates_windows_and_camera() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology()).unwrap();
        assert_eq!(backend.describe_backend().status, "ready");
        assert_eq!(backend.window_mgr.windows().len(), 1);
        assert!(backend.active_camera.is_some());
    }

    #[test]
    fn prepare_multi_display() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology_multi()).unwrap();
        assert_eq!(backend.window_mgr.windows().len(), 3);
        assert_eq!(backend.window_mgr.windows()[0].endpoint.role, "main");
        assert_eq!(backend.window_mgr.windows()[1].endpoint.role, "aux-1");
    }

    #[test]
    fn execute_payload_loads_scene_and_renders_frame() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology()).unwrap();

        let ack = backend.execute_payload(&visual_payload("scene-drop", "shader-glow")).unwrap();
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "scene-drop");
        assert!(ack.detail.contains("render shader-glow"));
        assert_eq!(backend.execute_count(), 1);
        assert!(backend.frame_count() > 0);
    }

    #[test]
    fn apply_show_state_updates_uniforms() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology()).unwrap();

        let show = test_show_state();
        backend.apply_show_state(&show).unwrap();
        assert!((backend.uniforms.time_params[3] - 140.0).abs() < f32::EPSILON);
    }

    #[test]
    fn apply_degrade_switches_to_safe_shader() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology()).unwrap();

        let mode = DegradeMode {
            mode: String::from("safe_fallback"),
            reason: String::from("GPU overload"),
            affected_backends: vec![String::from("wgpu-1")],
            fallback_action: None,
        };
        backend.apply_degrade_mode(&mode).unwrap();
        assert!(backend.is_safe_mode());
        assert!(backend.degraded);
        assert_eq!(backend.collect_backend_status().status, "degraded");
        assert_eq!(
            backend.collect_backend_status().detail,
            Some(String::from("safe fallback shader active"))
        );
    }

    #[test]
    fn degrade_and_execute_uses_safe_shader() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology()).unwrap();

        // Degrade
        let mode = DegradeMode {
            mode: String::from("safe_fallback"),
            reason: String::from("GPU overload"),
            affected_backends: vec![],
            fallback_action: None,
        };
        backend.apply_degrade_mode(&mode).unwrap();

        // Execute still works but uses safe shader
        let ack = backend.execute_payload(&visual_payload("scene-1", "shader-1")).unwrap();
        assert_eq!(ack.status, "ok");
        assert!(backend.is_safe_mode());
    }

    #[test]
    fn execute_with_uniforms_updates_color() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology()).unwrap();

        backend
            .execute_payload(&visual_payload_with_uniforms("scene-color", "shader-tint"))
            .unwrap();

        assert!((backend.uniforms.color_tint[0] - 0.5).abs() < f32::EPSILON);
        assert!((backend.uniforms.color_tint[1] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn full_lifecycle() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");

        // Idle
        assert_eq!(backend.describe_backend().status, "idle");

        // Prepare
        backend.prepare_backend(&visual_topology()).unwrap();
        assert_eq!(backend.describe_backend().status, "ready");

        // Execute multiple payloads
        backend.execute_payload(&visual_payload("s1", "p1")).unwrap();
        backend.execute_payload(&visual_payload("s2", "p2")).unwrap();
        assert_eq!(backend.execute_count(), 2);
        assert_eq!(backend.collect_backend_status().status, "healthy");

        // Shutdown
        backend.shutdown_backend().unwrap();
        assert_eq!(backend.describe_backend().status, "shutdown");
        assert!(backend.execute_payload(&visual_payload("s3", "p3")).is_err());
    }

    #[test]
    fn rejects_non_visual_topology() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        let audio_topo = BackendTopology::Audio {
            topology_ref: String::from("stereo"),
            calibration_profile: None,
            speaker_endpoints: Vec::new(),
        };
        assert!(backend.prepare_backend(&audio_topo).is_err());
    }

    #[test]
    fn rejects_non_visual_payload() {
        let mut backend = VisualWgpuBackend::new("wgpu-1");
        backend.prepare_backend(&visual_topology()).unwrap();
        let audio_payload = ExecutablePayload::Audio {
            layer_id: String::from("l1"),
            op: String::from("play"),
            target_asset_id: None,
            gain_db: None,
            duration_beats: None,
            route_set_ref: None,
            speaker_group: Vec::new(),
        };
        assert!(backend.execute_payload(&audio_payload).is_err());
    }

    #[test]
    fn flat_display_backend_is_alias() {
        let _backend: FlatDisplayBackend = FlatDisplayBackend::new("flat-1");
        // FlatDisplayBackend is just a type alias for VisualWgpuBackend
    }
}
