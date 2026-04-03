use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

/// Reference audio backend adapter implementing the [`BackendAdapter`] trait.
///
/// Processes `ExecutablePayload::Audio` variants, logs operations, and
/// maintains basic state. Does not perform real audio I/O — serves as a
/// reference implementation for testing and integration verification.
#[derive(Debug)]
pub struct AudioReferenceBackend {
    plugin_id: String,
    prepared: bool,
    shut_down: bool,
    degrade_mode: Option<String>,
    execute_count: u64,
    topology_ref: Option<String>,
    log: Vec<String>,
}

impl AudioReferenceBackend {
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

impl BackendAdapter for AudioReferenceBackend {
    fn describe_backend(&self) -> BackendDescription {
        BackendDescription {
            plugin_id: self.plugin_id.clone(),
            backend_kind: String::from("audio"),
            capabilities: vec![
                String::from("play"),
                String::from("stop"),
                String::from("crossfade"),
                String::from("gain"),
            ],
            topology_types: vec![String::from("stereo"), String::from("spatial_speaker_matrix")],
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
            return Err(String::from("AUD-001: backend already shut down"));
        }
        match topology {
            BackendTopology::Audio { topology_ref, .. } => {
                self.topology_ref = Some(topology_ref.clone());
                self.prepared = true;
                self.log.push(format!("prepare: topology={topology_ref}"));
                Ok(())
            }
            _ => Err(String::from("AUD-002: expected Audio topology")),
        }
    }

    fn apply_show_state(&mut self, show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("AUD-003: backend already shut down"));
        }
        self.log.push(format!(
            "apply_show_state: show={} rev={} bar={}",
            show_state.show_id, show_state.revision, show_state.time.bar
        ));
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("AUD-004: backend already shut down"));
        }
        match payload {
            ExecutablePayload::Audio {
                layer_id,
                op,
                target_asset_id,
                gain_db,
                ..
            } => {
                self.execute_count += 1;
                let asset = target_asset_id.as_deref().unwrap_or("none");
                let gain = gain_db.unwrap_or(0.0);
                self.log.push(format!("execute: layer={layer_id} op={op} asset={asset} gain={gain}"));
                Ok(BackendAck {
                    backend: self.plugin_id.clone(),
                    target: layer_id.clone(),
                    status: String::from("ok"),
                    detail: format!("{op} {asset}"),
                })
            }
            _ => Err(String::from("AUD-005: expected Audio payload")),
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
            latency_ms: Some(2.5),
            error_count: Some(0),
            last_ack_lag_ms: Some(1.0),
            detail: self.degrade_mode.clone(),
        }
    }

    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("AUD-006: backend already shut down"));
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

    fn audio_topology() -> BackendTopology {
        BackendTopology::Audio {
            topology_ref: String::from("stereo-main"),
            calibration_profile: None,
            speaker_endpoints: vec![String::from("out-L"), String::from("out-R")],
        }
    }

    fn audio_payload(layer: &str, op: &str, asset: Option<&str>) -> ExecutablePayload {
        ExecutablePayload::Audio {
            layer_id: layer.to_string(),
            op: op.to_string(),
            target_asset_id: asset.map(String::from),
            gain_db: Some(-3.0),
            duration_beats: Some(4),
            route_set_ref: None,
            speaker_group: Vec::new(),
        }
    }

    #[test]
    fn describe_returns_audio_kind() {
        let backend = AudioReferenceBackend::new("ref-audio-1");
        let desc = backend.describe_backend();
        assert_eq!(desc.backend_kind, "audio");
        assert_eq!(desc.status, "idle");
        assert!(desc.capabilities.contains(&String::from("play")));
    }

    #[test]
    fn full_lifecycle() {
        let mut backend = AudioReferenceBackend::new("ref-audio-1");
        backend.prepare_backend(&audio_topology()).unwrap();
        assert_eq!(backend.describe_backend().status, "ready");

        let ack = backend
            .execute_payload(&audio_payload("layer-bass", "play", Some("bass.wav")))
            .unwrap();
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "layer-bass");
        assert_eq!(ack.detail, "play bass.wav");
        assert_eq!(backend.execute_count(), 1);

        let status = backend.collect_backend_status();
        assert_eq!(status.status, "healthy");

        backend.shutdown_backend().unwrap();
        assert_eq!(backend.describe_backend().status, "shutdown");
        assert!(backend.execute_payload(&audio_payload("x", "play", None)).is_err());
    }

    #[test]
    fn rejects_non_audio_topology() {
        let mut backend = AudioReferenceBackend::new("ref-audio-1");
        let visual_topo = BackendTopology::Visual {
            topology_ref: String::from("flat"),
            calibration_profile: None,
            display_endpoints: Vec::new(),
        };
        assert!(backend.prepare_backend(&visual_topo).is_err());
    }

    #[test]
    fn rejects_non_audio_payload() {
        let mut backend = AudioReferenceBackend::new("ref-audio-1");
        let visual_payload = ExecutablePayload::Visual {
            scene_id: String::from("s1"),
            shader_program: String::from("p1"),
            uniforms: Default::default(),
            duration_beats: None,
            blend: None,
            view_group: None,
        };
        assert!(backend.execute_payload(&visual_payload).is_err());
    }

    #[test]
    fn degrade_mode_changes_status() {
        let mut backend = AudioReferenceBackend::new("ref-audio-1");
        let mode = DegradeMode {
            mode: String::from("mute"),
            reason: String::from("high latency"),
            affected_backends: vec![String::from("ref-audio-1")],
            fallback_action: None,
        };
        backend.apply_degrade_mode(&mode).unwrap();
        assert_eq!(backend.collect_backend_status().status, "degraded");
        assert!(backend.log().last().unwrap().contains("mute"));
    }
}
