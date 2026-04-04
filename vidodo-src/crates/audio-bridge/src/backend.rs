//! `BackendAdapter` implementation for the scsynth audio backend.
//!
//! `AudioScynthBackend` bridges the Vidodo scheduler with a SuperCollider
//! scsynth instance using the OSC command translator, ack collector, and
//! health monitor subsystems built in WSY-01..03.

use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

use crate::ack_collector::AckCollector;
use crate::command_translator::CommandTranslator;
use crate::health_monitor::{HealthMonitor, ScynthHealth};
use crate::osc::OscMessage;
use crate::process_manager::ScynthProcessManager;

/// scsynth-backed audio backend implementing the seven-method adapter protocol.
///
/// Lifecycle: `prepare_backend` → boots scsynth → `execute_payload` translates
/// IR audio commands into OSC sequences → `collect_backend_status` checks health
/// → `shutdown_backend` terminates the scsynth process.
pub struct AudioScynthBackend {
    plugin_id: String,
    prepared: bool,
    shut_down: bool,
    degraded: bool,
    #[allow(dead_code)]
    version: String,
    topology_ref: Option<String>,
    execute_count: u64,
    /// Outgoing OSC messages for inspection / send.
    outbox: Vec<OscMessage>,
    /// Pending action tracker.
    ack_collector: AckCollector,
    /// Health poller.
    health_monitor: HealthMonitor,
    /// IR → OSC translator.
    translator: CommandTranslator,
    /// scsynth process (optional, not started unless prepare is called).
    process: Option<ScynthProcessManager>,
}

impl std::fmt::Debug for AudioScynthBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioScynthBackend")
            .field("plugin_id", &self.plugin_id)
            .field("prepared", &self.prepared)
            .field("shut_down", &self.shut_down)
            .field("degraded", &self.degraded)
            .field("execute_count", &self.execute_count)
            .finish()
    }
}

impl AudioScynthBackend {
    /// Create a new scsynth backend with the given plugin id and version.
    pub fn new(plugin_id: &str, version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            prepared: false,
            shut_down: false,
            degraded: false,
            version: version.to_string(),
            topology_ref: None,
            execute_count: 0,
            outbox: Vec::new(),
            ack_collector: AckCollector::new(2000.0),
            health_monitor: HealthMonitor::new(3000.0),
            translator: CommandTranslator::new(),
            process: None,
        }
    }

    /// Drain outgoing OSC messages (for send layer / test inspection).
    pub fn drain_outbox(&mut self) -> Vec<OscMessage> {
        std::mem::take(&mut self.outbox)
    }

    /// Feed an incoming OSC reply (e.g. from scsynth) for ack correlation.
    pub fn process_reply(&mut self, msg: &OscMessage, now_ms: f64) {
        self.ack_collector.process_reply(msg);
        self.health_monitor.process_reply(msg, now_ms);
    }

    /// Number of payloads executed.
    pub fn execute_count(&self) -> u64 {
        self.execute_count
    }
}

impl BackendAdapter for AudioScynthBackend {
    fn describe_backend(&self) -> BackendDescription {
        BackendDescription {
            plugin_id: self.plugin_id.clone(),
            backend_kind: String::from("audio"),
            capabilities: vec![
                String::from("asset_playback"),
                String::from("synth_render"),
                String::from("stop"),
                String::from("crossfade"),
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
            return Err(String::from("SCSYNTH-001: backend already shut down"));
        }
        match topology {
            BackendTopology::Audio { topology_ref, .. } => {
                self.topology_ref = Some(topology_ref.clone());
                // Boot scsynth process manager (but don't actually exec in lib mode)
                self.process = Some(ScynthProcessManager::new(Default::default()));
                self.prepared = true;
                Ok(())
            }
            _ => Err(String::from("SCSYNTH-002: expected Audio topology")),
        }
    }

    fn apply_show_state(&mut self, show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("SCSYNTH-003: backend already shut down"));
        }
        // Inform scsynth of the current show state (no direct OSC command,
        // but we update internal state for future execute_payload calls).
        let _ = (show_state.show_id.as_str(), show_state.revision);
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("SCSYNTH-004: backend already shut down"));
        }
        if self.degraded {
            return Err(String::from("SCSYNTH-007: backend is degraded, new nodes blocked"));
        }
        match payload {
            ExecutablePayload::Audio {
                layer_id,
                op,
                target_asset_id,
                gain_db,
                duration_beats,
                ..
            } => {
                self.execute_count += 1;
                let action_id = format!("act-{}", self.execute_count);
                let asset = target_asset_id.as_deref().unwrap_or("none");

                // Translate IR op to OSC commands
                let cmds = self.translator.translate(
                    &action_id,
                    op,
                    target_asset_id.as_deref(),
                    *gain_db,
                    *duration_beats,
                );

                // Register in ack collector
                let node_id = (1000 + self.execute_count) as i32;
                self.ack_collector.register(&action_id, node_id, 0.0);

                // Queue all OSC messages for sending
                self.outbox.extend(cmds.messages);

                Ok(BackendAck {
                    backend: self.plugin_id.clone(),
                    target: layer_id.clone(),
                    status: String::from("ok"),
                    detail: format!("{op} {asset}"),
                })
            }
            _ => Err(String::from("SCSYNTH-005: expected Audio payload")),
        }
    }

    fn collect_backend_status(&self) -> BackendStatus {
        let health = self.health_monitor.health();
        let status_str = match health {
            ScynthHealth::Healthy if self.shut_down => String::from("shutdown"),
            ScynthHealth::Healthy if self.degraded => String::from("degraded"),
            ScynthHealth::Healthy => String::from("healthy"),
            ScynthHealth::Degraded => String::from("degraded"),
            ScynthHealth::Offline => String::from("offline"),
        };

        BackendStatus {
            plugin_id: self.plugin_id.clone(),
            status: status_str,
            latency_ms: Some(2.0),
            error_count: Some(0),
            last_ack_lag_ms: Some(1.0),
            detail: if self.degraded { Some(String::from("degraded: no new nodes")) } else { None },
        }
    }

    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("SCSYNTH-006: backend already shut down"));
        }
        // Stop creating new nodes but keep existing ones running
        self.degraded = mode.mode != "normal";
        Ok(())
    }

    fn shutdown_backend(&mut self) -> Result<(), String> {
        // Send /quit to scsynth, then mark shut down
        self.outbox.push(OscMessage::new("/quit", vec![]));
        self.shut_down = true;
        self.prepared = false;
        self.process = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::BackendTopology;

    fn audio_topology() -> BackendTopology {
        BackendTopology::Audio {
            topology_ref: String::from("stereo-main"),
            calibration_profile: None,
            speaker_endpoints: vec![String::from("L"), String::from("R")],
        }
    }

    fn audio_payload(op: &str) -> ExecutablePayload {
        ExecutablePayload::Audio {
            layer_id: String::from("layer-1"),
            op: op.to_string(),
            target_asset_id: Some(String::from("kick.wav")),
            gain_db: Some(-3.0),
            duration_beats: None,
            route_set_ref: None,
            speaker_group: vec![],
        }
    }

    #[test]
    fn describe_returns_name_and_version() {
        let backend = AudioScynthBackend::new("scsynth-v1", "3.13.0");
        let desc = backend.describe_backend();
        assert_eq!(desc.plugin_id, "scsynth-v1");
        assert_eq!(desc.backend_kind, "audio");
        assert_eq!(desc.status, "idle");
        assert!(desc.capabilities.contains(&String::from("asset_playback")));
    }

    #[test]
    fn prepare_and_describe_ready() {
        let mut backend = AudioScynthBackend::new("scsynth-v1", "3.13.0");
        backend.prepare_backend(&audio_topology()).unwrap();
        assert_eq!(backend.describe_backend().status, "ready");
    }

    #[test]
    fn execute_payload_translates_and_collects() {
        let mut backend = AudioScynthBackend::new("scsynth-v1", "3.13.0");
        backend.prepare_backend(&audio_topology()).unwrap();

        let ack = backend.execute_payload(&audio_payload("play")).unwrap();
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "layer-1");
        assert!(ack.detail.contains("play"));
        assert_eq!(backend.execute_count(), 1);

        // OSC messages were queued
        let msgs = backend.drain_outbox();
        assert!(!msgs.is_empty());
    }

    #[test]
    fn shutdown_sends_quit_and_blocks_further_calls() {
        let mut backend = AudioScynthBackend::new("scsynth-v1", "3.13.0");
        backend.prepare_backend(&audio_topology()).unwrap();
        backend.shutdown_backend().unwrap();

        // /quit should be in outbox
        let msgs = backend.drain_outbox();
        assert!(msgs.iter().any(|m| m.address == "/quit"));

        // Post-shutdown calls fail
        assert!(backend.execute_payload(&audio_payload("play")).is_err());
        assert_eq!(backend.describe_backend().status, "shutdown");
    }

    #[test]
    fn apply_degrade_blocks_new_nodes() {
        let mut backend = AudioScynthBackend::new("scsynth-v1", "3.13.0");
        backend.prepare_backend(&audio_topology()).unwrap();
        backend
            .apply_degrade_mode(&DegradeMode {
                mode: String::from("reduced"),
                reason: String::from("high latency"),
                affected_backends: vec![],
                fallback_action: None,
            })
            .unwrap();

        // New execute should fail when degraded
        assert!(backend.execute_payload(&audio_payload("play")).is_err());
    }

    #[test]
    fn collect_status_reports_health() {
        let backend = AudioScynthBackend::new("scsynth-v1", "3.13.0");
        let status = backend.collect_backend_status();
        assert_eq!(status.plugin_id, "scsynth-v1");
        assert_eq!(status.status, "healthy");
    }

    #[test]
    fn wrong_topology_rejected() {
        let mut backend = AudioScynthBackend::new("scsynth-v1", "3.13.0");
        let bad_topo = BackendTopology::Lighting {
            topology_ref: String::from("dmx"),
            calibration_profile: None,
            fixture_endpoints: vec![],
        };
        assert!(backend.prepare_backend(&bad_topo).is_err());
    }
}
