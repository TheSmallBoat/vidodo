use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

/// A no-op backend adapter that satisfies the [`BackendAdapter`] trait
/// without performing any real work. Used as the default test/placeholder
/// backend that can replace [`super::FakeBackendClient`].
#[derive(Debug, Default)]
pub struct NullBackendAdapter {
    plugin_id: String,
    backend_kind: String,
    prepared: bool,
    shut_down: bool,
    degrade_mode: Option<String>,
    execute_count: u64,
}

impl NullBackendAdapter {
    pub fn new(plugin_id: &str, backend_kind: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            backend_kind: backend_kind.to_string(),
            prepared: false,
            shut_down: false,
            degrade_mode: None,
            execute_count: 0,
        }
    }

    /// Return the number of payloads executed so far.
    pub fn execute_count(&self) -> u64 {
        self.execute_count
    }

    /// Return whether the backend was prepared.
    pub fn is_prepared(&self) -> bool {
        self.prepared
    }

    /// Return whether the backend has been shut down.
    pub fn is_shut_down(&self) -> bool {
        self.shut_down
    }
}

impl BackendAdapter for NullBackendAdapter {
    fn describe_backend(&self) -> BackendDescription {
        BackendDescription {
            plugin_id: self.plugin_id.clone(),
            backend_kind: self.backend_kind.clone(),
            capabilities: vec![String::from("null")],
            topology_types: Vec::new(),
            status: if self.shut_down {
                String::from("shutdown")
            } else if self.prepared {
                String::from("ready")
            } else {
                String::from("idle")
            },
        }
    }

    fn prepare_backend(&mut self, _topology: &BackendTopology) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("NULL-001: backend already shut down"));
        }
        self.prepared = true;
        Ok(())
    }

    fn apply_show_state(&mut self, _show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("NULL-002: backend already shut down"));
        }
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("NULL-003: backend already shut down"));
        }
        self.execute_count += 1;
        let target = match payload {
            ExecutablePayload::Audio { layer_id, .. } => layer_id.clone(),
            ExecutablePayload::Visual { scene_id, .. } => scene_id.clone(),
            ExecutablePayload::Lighting { cue_set_id, .. } => cue_set_id.clone(),
        };
        Ok(BackendAck {
            backend: self.plugin_id.clone(),
            target,
            status: String::from("ok"),
            detail: String::from("null-adapter-noop"),
        })
    }

    fn collect_backend_status(&self) -> BackendStatus {
        BackendStatus {
            plugin_id: self.plugin_id.clone(),
            status: if self.shut_down { String::from("shutdown") } else { String::from("healthy") },
            latency_ms: Some(0.0),
            error_count: Some(0),
            last_ack_lag_ms: Some(0.0),
            detail: self.degrade_mode.clone(),
        }
    }

    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("NULL-004: backend already shut down"));
        }
        self.degrade_mode = Some(mode.mode.clone());
        Ok(())
    }

    fn shutdown_backend(&mut self) -> Result<(), String> {
        self.shut_down = true;
        self.prepared = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn audio_topology() -> BackendTopology {
        BackendTopology::Audio {
            topology_ref: String::from("stereo-main"),
            calibration_profile: None,
            speaker_endpoints: vec![String::from("out-L"), String::from("out-R")],
        }
    }

    fn audio_payload() -> ExecutablePayload {
        ExecutablePayload::Audio {
            layer_id: String::from("layer-01"),
            op: String::from("play"),
            target_asset_id: Some(String::from("kick.wav")),
            gain_db: Some(-3.0),
            duration_beats: Some(4),
            route_set_ref: None,
            speaker_group: Vec::new(),
        }
    }

    #[test]
    fn full_lifecycle() {
        let mut adapter = NullBackendAdapter::new("null-audio", "audio");
        // describe before prepare
        let desc = adapter.describe_backend();
        assert_eq!(desc.status, "idle");
        assert_eq!(desc.backend_kind, "audio");

        // prepare
        adapter.prepare_backend(&audio_topology()).unwrap();
        assert!(adapter.is_prepared());
        assert_eq!(adapter.describe_backend().status, "ready");

        // execute
        let ack = adapter.execute_payload(&audio_payload()).unwrap();
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "layer-01");
        assert_eq!(adapter.execute_count(), 1);

        // status
        let status = adapter.collect_backend_status();
        assert_eq!(status.status, "healthy");

        // shutdown
        adapter.shutdown_backend().unwrap();
        assert!(adapter.is_shut_down());
        assert_eq!(adapter.describe_backend().status, "shutdown");
    }

    #[test]
    fn execute_visual_payload() {
        let mut adapter = NullBackendAdapter::new("null-visual", "visual");
        let payload = ExecutablePayload::Visual {
            scene_id: String::from("scene-drop"),
            shader_program: String::from("shader-a"),
            uniforms: BTreeMap::new(),
            duration_beats: Some(8),
            blend: None,
            view_group: None,
        };
        let ack = adapter.execute_payload(&payload).unwrap();
        assert_eq!(ack.target, "scene-drop");
        assert_eq!(ack.status, "ok");
    }

    #[test]
    fn execute_lighting_payload() {
        let mut adapter = NullBackendAdapter::new("null-lighting", "lighting");
        let payload = ExecutablePayload::Lighting {
            cue_set_id: String::from("cue-set-01"),
            source_ref: String::from("cue-0"),
            fixture_group: Vec::new(),
            intensity: Some(0.8),
            color: None,
            fade_beats: None,
        };
        let ack = adapter.execute_payload(&payload).unwrap();
        assert_eq!(ack.target, "cue-set-01");
    }

    fn minimal_show_state() -> ShowState {
        use vidodo_ir::{MusicalTime, OutputBinding, ShowPatchState, ShowSemantic, ShowTransition};
        ShowState {
            show_id: String::from("test-show"),
            revision: 1,
            mode: String::from("offline"),
            time: MusicalTime {
                bar: 1,
                beat: 1.0,
                beat_in_bar: 1.0,
                phrase: 1,
                section: String::from("intro"),
                tempo: 128.0,
                time_signature: [4, 4],
            },
            semantic: ShowSemantic {
                energy: 0.5,
                density: 0.5,
                tension: 0.3,
                brightness: 0.7,
                motion: 0.4,
                intent: String::from("ambient"),
            },
            transition: ShowTransition {
                state: String::from("stable"),
                from_scene: String::from("intro"),
                to_scene: String::from("intro"),
                window_open: false,
            },
            visual_output: OutputBinding {
                backend_id: String::from("null-visual"),
                topology_ref: String::from("flat"),
                calibration_profile: String::from("default"),
                active_group: String::from("main"),
            },
            audio_output: OutputBinding {
                backend_id: String::from("null-audio"),
                topology_ref: String::from("stereo"),
                calibration_profile: String::from("default"),
                active_group: String::from("main"),
            },
            patch: ShowPatchState {
                allowed: true,
                scope: String::from("full"),
                locked_sections: Vec::new(),
            },
            adapter_plugins: BTreeMap::new(),
            resource_hubs: BTreeMap::new(),
            active_audio_layers: Vec::new(),
            active_visual_scene: String::from("intro"),
        }
    }

    #[test]
    fn shutdown_prevents_further_operations() {
        let mut adapter = NullBackendAdapter::new("null-test", "audio");
        adapter.shutdown_backend().unwrap();
        assert!(adapter.prepare_backend(&audio_topology()).is_err());
        assert!(adapter.execute_payload(&audio_payload()).is_err());
        assert!(adapter.apply_show_state(&minimal_show_state()).is_err());
    }

    #[test]
    fn apply_degrade_mode_recorded() {
        let mut adapter = NullBackendAdapter::new("null-test", "audio");
        let mode = DegradeMode {
            mode: String::from("mute"),
            reason: String::from("high latency"),
            affected_backends: vec![String::from("null-test")],
            fallback_action: None,
        };
        adapter.apply_degrade_mode(&mode).unwrap();
        let status = adapter.collect_backend_status();
        assert_eq!(status.detail, Some(String::from("mute")));
    }
}
