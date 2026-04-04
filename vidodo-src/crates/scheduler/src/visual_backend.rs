use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

/// Reference visual backend adapter implementing the [`BackendAdapter`] trait.
///
/// Processes `ExecutablePayload::Visual` variants, logs scene renders and
/// shader programs. Does not perform real GPU rendering — serves as a
/// reference implementation for testing and integration verification.
#[derive(Debug)]
pub struct VisualReferenceBackend {
    plugin_id: String,
    prepared: bool,
    shut_down: bool,
    degrade_mode: Option<String>,
    execute_count: u64,
    topology_ref: Option<String>,
    log: Vec<String>,
}

impl VisualReferenceBackend {
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            prepared: false,
            shut_down: false,
            degrade_mode: None,
            execute_count: 0,
            topology_ref: None,
            log: Vec::new(),
        }
    }

    /// Return the execution log.
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// Return the number of payloads executed.
    pub fn execute_count(&self) -> u64 {
        self.execute_count
    }
}

impl BackendAdapter for VisualReferenceBackend {
    fn describe_backend(&self) -> BackendDescription {
        BackendDescription {
            plugin_id: self.plugin_id.clone(),
            backend_kind: String::from("visual"),
            capabilities: vec![
                String::from("scene_switch"),
                String::from("shader_render"),
                String::from("blend"),
                String::from("uniform_update"),
            ],
            topology_types: vec![String::from("flat"), String::from("spatial_multiview")],
            status: if self.shut_down {
                String::from("shutdown")
            } else if self.prepared {
                String::from("ready")
            } else {
                String::from("idle")
            },
        }
    }

    fn prepare_backend(&mut self, topology: &BackendTopology) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("VIS-001: backend already shut down"));
        }
        match topology {
            BackendTopology::Visual { topology_ref, .. } => {
                self.topology_ref = Some(topology_ref.clone());
                self.prepared = true;
                self.log.push(format!("prepare: topology={topology_ref}"));
                Ok(())
            }
            _ => Err(String::from("VIS-002: expected Visual topology")),
        }
    }

    fn apply_show_state(&mut self, show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("VIS-003: backend already shut down"));
        }
        self.log.push(format!(
            "apply_show_state: show={} rev={} scene={}",
            show_state.show_id, show_state.revision, show_state.active_visual_scene
        ));
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("VIS-004: backend already shut down"));
        }
        match payload {
            ExecutablePayload::Visual { scene_id, shader_program, blend, .. } => {
                self.execute_count += 1;
                let blend_str = blend.as_deref().unwrap_or("none");
                self.log.push(format!(
                    "execute: scene={scene_id} shader={shader_program} blend={blend_str}"
                ));
                Ok(BackendAck {
                    backend: self.plugin_id.clone(),
                    target: scene_id.clone(),
                    status: String::from("ok"),
                    detail: format!("render {shader_program}"),
                })
            }
            _ => Err(String::from("VIS-005: expected Visual payload")),
        }
    }

    fn collect_backend_status(&self) -> BackendStatus {
        BackendStatus {
            plugin_id: self.plugin_id.clone(),
            status: if self.shut_down {
                String::from("shutdown")
            } else if self.degrade_mode.is_some() {
                String::from("degraded")
            } else {
                String::from("healthy")
            },
            latency_ms: Some(4.0),
            error_count: Some(0),
            last_ack_lag_ms: Some(2.0),
            detail: self.degrade_mode.clone(),
        }
    }

    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("VIS-006: backend already shut down"));
        }
        self.degrade_mode = Some(mode.mode.clone());
        self.log.push(format!("degrade: mode={}", mode.mode));
        Ok(())
    }

    fn shutdown_backend(&mut self) -> Result<(), String> {
        self.shut_down = true;
        self.prepared = false;
        self.log.push(String::from("shutdown"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visual_topology() -> BackendTopology {
        BackendTopology::Visual {
            topology_ref: String::from("flat-main"),
            calibration_profile: None,
            display_endpoints: vec![String::from("display-1")],
        }
    }

    fn visual_payload(scene: &str, shader: &str) -> ExecutablePayload {
        ExecutablePayload::Visual {
            scene_id: scene.to_string(),
            shader_program: shader.to_string(),
            uniforms: Default::default(),
            duration_beats: Some(8),
            blend: Some(String::from("crossfade")),
            view_group: None,
        }
    }

    #[test]
    fn describe_returns_visual_kind() {
        let backend = VisualReferenceBackend::new("ref-visual-1");
        let desc = backend.describe_backend();
        assert_eq!(desc.backend_kind, "visual");
        assert_eq!(desc.status, "idle");
        assert!(desc.capabilities.contains(&String::from("scene_switch")));
    }

    #[test]
    fn full_lifecycle() {
        let mut backend = VisualReferenceBackend::new("ref-visual-1");
        backend.prepare_backend(&visual_topology()).unwrap();
        assert_eq!(backend.describe_backend().status, "ready");

        let ack = backend.execute_payload(&visual_payload("scene-drop", "shader-glow")).unwrap();
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "scene-drop");
        assert_eq!(ack.detail, "render shader-glow");
        assert_eq!(backend.execute_count(), 1);

        assert_eq!(backend.collect_backend_status().status, "healthy");

        backend.shutdown_backend().unwrap();
        assert_eq!(backend.describe_backend().status, "shutdown");
        assert!(backend.execute_payload(&visual_payload("x", "y")).is_err());
    }

    #[test]
    fn rejects_non_visual_topology() {
        let mut backend = VisualReferenceBackend::new("ref-visual-1");
        let audio_topo = BackendTopology::Audio {
            topology_ref: String::from("stereo"),
            calibration_profile: None,
            speaker_endpoints: Vec::new(),
        };
        assert!(backend.prepare_backend(&audio_topo).is_err());
    }

    #[test]
    fn rejects_non_visual_payload() {
        let mut backend = VisualReferenceBackend::new("ref-visual-1");
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
    fn degrade_mode_changes_status() {
        let mut backend = VisualReferenceBackend::new("ref-visual-1");
        let mode = DegradeMode {
            mode: String::from("reduced_resolution"),
            reason: String::from("GPU overload"),
            affected_backends: vec![String::from("ref-visual-1")],
            fallback_action: None,
        };
        backend.apply_degrade_mode(&mode).unwrap();
        assert_eq!(backend.collect_backend_status().status, "degraded");
    }
}
