use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

/// Example third-party visual executor adapter.
///
/// This stub implementation returns deterministic acks for visual payloads.
/// It serves as a reference for how external visual execution plugins will
/// integrate with the adapter registry.
#[derive(Debug)]
pub struct ExampleVisualExecutor {
    plugin_id: String,
    prepared: bool,
    shut_down: bool,
    execute_count: u64,
}

impl ExampleVisualExecutor {
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            prepared: false,
            shut_down: false,
            execute_count: 0,
        }
    }

    pub fn execute_count(&self) -> u64 {
        self.execute_count
    }
}

impl BackendAdapter for ExampleVisualExecutor {
    fn describe_backend(&self) -> BackendDescription {
        BackendDescription {
            plugin_id: self.plugin_id.clone(),
            backend_kind: String::from("visual"),
            capabilities: vec![String::from("scene_switch"), String::from("shader_render")],
            topology_types: vec![String::from("flat")],
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
            return Err(String::from("EXVIS-001: backend already shut down"));
        }
        match topology {
            BackendTopology::Visual { .. } => {
                self.prepared = true;
                Ok(())
            }
            _ => Err(String::from("EXVIS-002: expected Visual topology")),
        }
    }

    fn apply_show_state(&mut self, _show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("EXVIS-003: backend already shut down"));
        }
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("EXVIS-004: backend already shut down"));
        }
        match payload {
            ExecutablePayload::Visual { scene_id, .. } => {
                self.execute_count += 1;
                Ok(BackendAck {
                    backend: self.plugin_id.clone(),
                    target: scene_id.clone(),
                    status: String::from("ok"),
                    detail: format!("rendered scene {scene_id}"),
                })
            }
            _ => Err(String::from("EXVIS-005: expected Visual payload")),
        }
    }

    fn collect_backend_status(&self) -> BackendStatus {
        BackendStatus {
            plugin_id: self.plugin_id.clone(),
            status: if self.shut_down {
                String::from("shutdown")
            } else if self.prepared {
                String::from("ready")
            } else {
                String::from("idle")
            },
            latency_ms: Some(0.5),
            error_count: Some(0),
            last_ack_lag_ms: None,
            detail: Some(format!("executed {} payloads", self.execute_count)),
        }
    }

    fn apply_degrade_mode(&mut self, _mode: &DegradeMode) -> Result<(), String> {
        Ok(())
    }

    fn shutdown_backend(&mut self) -> Result<(), String> {
        self.shut_down = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::BackendTopology;

    #[test]
    fn prepare_and_execute_visual_payload() {
        let mut executor = ExampleVisualExecutor::new("example-vis-1");
        let desc = executor.describe_backend();
        assert_eq!(desc.backend_kind, "visual");
        assert_eq!(desc.status, "idle");

        let topology = BackendTopology::Visual {
            topology_ref: String::from("flat-display-a"),
            calibration_profile: None,
            display_endpoints: vec![],
        };
        executor.prepare_backend(&topology).unwrap();
        assert_eq!(executor.describe_backend().status, "ready");

        let payload = ExecutablePayload::Visual {
            scene_id: String::from("scene_intro"),
            shader_program: String::from("default"),
            uniforms: std::collections::BTreeMap::new(),
            duration_beats: Some(4),
            blend: Some(String::from("alpha")),
            view_group: Some(String::from("main")),
        };
        let ack = executor.execute_payload(&payload).unwrap();
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "scene_intro");
        assert_eq!(executor.execute_count(), 1);
    }

    #[test]
    fn reject_non_visual_payload() {
        let mut executor = ExampleVisualExecutor::new("example-vis-1");
        let topology = BackendTopology::Visual {
            topology_ref: String::from("flat-display-a"),
            calibration_profile: None,
            display_endpoints: vec![],
        };
        executor.prepare_backend(&topology).unwrap();

        let payload = ExecutablePayload::Audio {
            layer_id: String::from("layer-a"),
            op: String::from("start"),
            target_asset_id: None,
            gain_db: None,
            duration_beats: Some(4),
            route_set_ref: None,
            speaker_group: vec![],
        };
        assert!(executor.execute_payload(&payload).is_err());
    }

    #[test]
    fn shutdown_prevents_execution() {
        let mut executor = ExampleVisualExecutor::new("example-vis-1");
        executor.shutdown_backend().unwrap();
        let payload = ExecutablePayload::Visual {
            scene_id: String::from("scene_intro"),
            shader_program: String::from("default"),
            uniforms: std::collections::BTreeMap::new(),
            duration_beats: Some(4),
            blend: Some(String::from("alpha")),
            view_group: Some(String::from("main")),
        };
        assert!(executor.execute_payload(&payload).is_err());
    }

    #[test]
    fn registry_can_load_and_query_visual_executor() {
        let executor = ExampleVisualExecutor::new("example-vis-1");
        let desc = executor.describe_backend();
        assert_eq!(desc.plugin_id, "example-vis-1");
        assert!(desc.capabilities.contains(&String::from("scene_switch")));
        let status = executor.collect_backend_status();
        assert_eq!(status.status, "idle");
        assert_eq!(status.error_count, Some(0));
    }
}
