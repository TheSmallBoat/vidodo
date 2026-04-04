use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

/// Reference lighting backend adapter implementing the [`BackendAdapter`] trait.
///
/// Processes `ExecutablePayload::Lighting` variants, logs cue fires and
/// intensity changes. Does not drive real DMX fixtures — serves as a
/// reference implementation for testing and integration verification.
#[derive(Debug)]
pub struct LightingReferenceBackend {
    plugin_id: String,
    prepared: bool,
    shut_down: bool,
    degrade_mode: Option<String>,
    execute_count: u64,
    topology_ref: Option<String>,
    log: Vec<String>,
}

impl LightingReferenceBackend {
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

impl BackendAdapter for LightingReferenceBackend {
    fn describe_backend(&self) -> BackendDescription {
        BackendDescription {
            plugin_id: self.plugin_id.clone(),
            backend_kind: String::from("lighting"),
            capabilities: vec![
                String::from("cue_fire"),
                String::from("intensity_set"),
                String::from("color_set"),
                String::from("blackout"),
            ],
            topology_types: vec![String::from("dmx_universe"), String::from("artnet_subnet")],
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
            return Err(String::from("LIT-001: backend already shut down"));
        }
        match topology {
            BackendTopology::Lighting { topology_ref, .. } => {
                self.topology_ref = Some(topology_ref.clone());
                self.prepared = true;
                self.log.push(format!("prepare: topology={topology_ref}"));
                Ok(())
            }
            _ => Err(String::from("LIT-002: expected Lighting topology")),
        }
    }

    fn apply_show_state(&mut self, show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("LIT-003: backend already shut down"));
        }
        self.log.push(format!(
            "apply_show_state: show={} rev={} mode={}",
            show_state.show_id, show_state.revision, show_state.mode
        ));
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("LIT-004: backend already shut down"));
        }
        match payload {
            ExecutablePayload::Lighting { cue_set_id, source_ref, intensity, .. } => {
                self.execute_count += 1;
                let int_str = intensity.map_or(String::from("none"), |v| format!("{v}"));
                self.log.push(format!(
                    "execute: cue_set={cue_set_id} src={source_ref} intensity={int_str}"
                ));
                Ok(BackendAck {
                    backend: self.plugin_id.clone(),
                    target: cue_set_id.clone(),
                    status: String::from("ok"),
                    detail: format!("fire cue_set {cue_set_id} from {source_ref}"),
                })
            }
            _ => Err(String::from("LIT-005: expected Lighting payload")),
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
            latency_ms: Some(2.0),
            error_count: Some(0),
            last_ack_lag_ms: Some(1.0),
            detail: self.degrade_mode.clone(),
        }
    }

    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("LIT-006: backend already shut down"));
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

    fn lighting_topology() -> BackendTopology {
        BackendTopology::Lighting {
            topology_ref: String::from("dmx-universe-1"),
            calibration_profile: None,
            fixture_endpoints: vec![String::from("fixture-1"), String::from("fixture-2")],
        }
    }

    fn lighting_payload(cue_set: &str, intensity: f64) -> ExecutablePayload {
        ExecutablePayload::Lighting {
            cue_set_id: cue_set.to_string(),
            source_ref: String::from("timeline-main"),
            fixture_group: Vec::new(),
            intensity: Some(intensity),
            color: Some([1.0, 0.0, 0.0]),
            fade_beats: Some(4.0),
        }
    }

    #[test]
    fn describe_returns_lighting_kind() {
        let backend = LightingReferenceBackend::new("ref-light-1");
        let desc = backend.describe_backend();
        assert_eq!(desc.backend_kind, "lighting");
        assert_eq!(desc.status, "idle");
        assert!(desc.capabilities.contains(&String::from("cue_fire")));
    }

    #[test]
    fn full_lifecycle() {
        let mut backend = LightingReferenceBackend::new("ref-light-1");
        backend.prepare_backend(&lighting_topology()).unwrap();
        assert_eq!(backend.describe_backend().status, "ready");

        let ack = backend.execute_payload(&lighting_payload("cue-set-chorus", 0.85)).unwrap();
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "cue-set-chorus");
        assert!(ack.detail.contains("cue-set-chorus"));
        assert_eq!(backend.execute_count(), 1);

        assert_eq!(backend.collect_backend_status().status, "healthy");

        backend.shutdown_backend().unwrap();
        assert_eq!(backend.describe_backend().status, "shutdown");
        assert!(backend.execute_payload(&lighting_payload("x", 1.0)).is_err());
    }

    #[test]
    fn rejects_non_lighting_topology() {
        let mut backend = LightingReferenceBackend::new("ref-light-1");
        let audio_topo = BackendTopology::Audio {
            topology_ref: String::from("stereo"),
            calibration_profile: None,
            speaker_endpoints: Vec::new(),
        };
        assert!(backend.prepare_backend(&audio_topo).is_err());
    }

    #[test]
    fn rejects_non_lighting_payload() {
        let mut backend = LightingReferenceBackend::new("ref-light-1");
        let visual_payload = ExecutablePayload::Visual {
            scene_id: String::from("s1"),
            shader_program: String::from("sp"),
            uniforms: Default::default(),
            duration_beats: None,
            blend: None,
            view_group: None,
        };
        assert!(backend.execute_payload(&visual_payload).is_err());
    }

    #[test]
    fn degrade_mode_changes_status() {
        let mut backend = LightingReferenceBackend::new("ref-light-1");
        let mode = DegradeMode {
            mode: String::from("reduce_fixtures"),
            reason: String::from("DMX bus overload"),
            affected_backends: vec![String::from("ref-light-1")],
            fallback_action: None,
        };
        backend.apply_degrade_mode(&mode).unwrap();
        assert_eq!(backend.collect_backend_status().status, "degraded");
    }
}
