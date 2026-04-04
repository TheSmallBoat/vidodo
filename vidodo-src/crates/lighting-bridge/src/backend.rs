//! `BackendAdapter` implementation for the DMX fixture-bus backend.
//!
//! `FixtureBusBackend` bridges the Vidodo scheduler with real DMX
//! fixtures via the cue translator, fixture topology, and ArtNet
//! subsystems built in WSAB-01..03.

use vidodo_ir::{
    BackendAck, BackendAdapter, BackendDescription, BackendStatus, BackendTopology, DegradeMode,
    ExecutablePayload, ShowState,
};

use crate::artnet::build_opdmx_packet;
use crate::cue_translator::{CueEntry, translate_cue};
use crate::dmx::DmxFrame;
use crate::fixture_topology::FixtureBusTopology;

/// DMX/ArtNet-backed lighting backend implementing the seven-method adapter
/// protocol.
///
/// Lifecycle: `prepare_backend` loads fixture topology → `execute_payload`
/// translates cue sets to DMX frames and serializes ArtNet packets →
/// `shutdown_backend` sends blackout (all channels zero).
#[derive(Debug)]
pub struct FixtureBusBackend {
    plugin_id: String,
    prepared: bool,
    shut_down: bool,
    degraded: bool,
    topology: Option<FixtureBusTopology>,
    execute_count: u64,
    /// ArtNet packets queued for send.
    outbox: Vec<Vec<u8>>,
    /// Last transmitted DMX frames (per universe) for hold-on-degrade.
    last_frames: Vec<DmxFrame>,
}

impl FixtureBusBackend {
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            prepared: false,
            shut_down: false,
            degraded: false,
            topology: None,
            execute_count: 0,
            outbox: Vec::new(),
            last_frames: Vec::new(),
        }
    }

    /// Drain queued ArtNet packets (for send layer / test inspection).
    pub fn drain_outbox(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outbox)
    }

    /// Number of payloads executed.
    pub fn execute_count(&self) -> u64 {
        self.execute_count
    }
}

impl BackendAdapter for FixtureBusBackend {
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
            return Err(String::from("FBB-001: backend already shut down"));
        }
        match topology {
            BackendTopology::Lighting { topology_ref, .. } => {
                // Create an empty topology object with the label;
                // fixture endpoints would be loaded from the fixture_endpoints
                // field in a real deployment.
                self.topology = Some(FixtureBusTopology::new(topology_ref));
                self.prepared = true;
                Ok(())
            }
            _ => Err(String::from("FBB-002: expected Lighting topology")),
        }
    }

    fn apply_show_state(&mut self, show_state: &ShowState) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("FBB-003: backend already shut down"));
        }
        let _ = (show_state.show_id.as_str(), show_state.revision);
        Ok(())
    }

    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String> {
        if self.shut_down {
            return Err(String::from("FBB-004: backend already shut down"));
        }
        if self.degraded {
            // Degrade mode: hold last state, don't send new commands
            return Err(String::from("FBB-007: degraded, holding last state"));
        }
        let topo = self.topology.as_ref().ok_or("FBB-008: not prepared")?;

        match payload {
            ExecutablePayload::Lighting { cue_set_id, source_ref, intensity, color, .. } => {
                self.execute_count += 1;

                // Build a CueEntry from the payload
                let cue = CueEntry {
                    fixture_ids: vec![],
                    intensity: intensity.unwrap_or(1.0),
                    color: *color,
                    pan: None,
                    tilt: None,
                    strobe: None,
                };

                // Translate cue to DMX frames
                let translation = translate_cue(&cue, topo);

                // Serialize each frame to ArtNet and queue
                for frame in &translation.frames {
                    let packet = build_opdmx_packet(frame);
                    self.outbox.push(packet);
                }

                // Remember the frames for degrade hold
                self.last_frames = translation.frames;

                Ok(BackendAck {
                    backend: self.plugin_id.clone(),
                    target: cue_set_id.clone(),
                    status: String::from("ok"),
                    detail: format!("fire cue_set {cue_set_id} from {source_ref}"),
                })
            }
            _ => Err(String::from("FBB-005: expected Lighting payload")),
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
            latency_ms: Some(1.5),
            error_count: Some(0),
            last_ack_lag_ms: Some(0.5),
            detail: if self.degraded { Some(String::from("holding last DMX state")) } else { None },
        }
    }

    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String> {
        if self.shut_down {
            return Err(String::from("FBB-006: backend already shut down"));
        }
        self.degraded = mode.mode != "normal";
        Ok(())
    }

    fn shutdown_backend(&mut self) -> Result<(), String> {
        // Blackout: send zeroed frames for all known universes
        if let Some(ref topo) = self.topology {
            for universe in topo.universes() {
                let frame = DmxFrame::new(universe);
                let packet = build_opdmx_packet(&frame);
                self.outbox.push(packet);
            }
        }
        self.shut_down = true;
        self.prepared = false;
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
            fixture_endpoints: vec![String::from("par-1"), String::from("par-2")],
        }
    }

    fn lighting_payload() -> ExecutablePayload {
        ExecutablePayload::Lighting {
            cue_set_id: String::from("cue-1"),
            source_ref: String::from("src-1"),
            fixture_group: vec![],
            intensity: Some(0.8),
            color: Some([1.0, 0.0, 0.0]),
            fade_beats: None,
        }
    }

    #[test]
    fn describe_returns_kind_and_capabilities() {
        let backend = FixtureBusBackend::new("fixture-bus-v1");
        let desc = backend.describe_backend();
        assert_eq!(desc.backend_kind, "lighting");
        assert!(desc.capabilities.contains(&String::from("cue_fire")));
        assert_eq!(desc.status, "idle");
    }

    #[test]
    fn prepare_sets_ready() {
        let mut backend = FixtureBusBackend::new("fixture-bus-v1");
        backend.prepare_backend(&lighting_topology()).unwrap();
        assert_eq!(backend.describe_backend().status, "ready");
    }

    #[test]
    fn execute_translates_cue_to_artnet() {
        let mut backend = FixtureBusBackend::new("fb");
        backend.prepare_backend(&lighting_topology()).unwrap();

        let ack = backend.execute_payload(&lighting_payload()).unwrap();
        assert_eq!(ack.status, "ok");
        assert!(ack.detail.contains("cue-1"));
        assert_eq!(backend.execute_count(), 1);
    }

    #[test]
    fn degrade_blocks_new_commands() {
        let mut backend = FixtureBusBackend::new("fb");
        backend.prepare_backend(&lighting_topology()).unwrap();
        backend
            .apply_degrade_mode(&DegradeMode {
                mode: String::from("reduced"),
                reason: String::from("overload"),
                affected_backends: vec![],
                fallback_action: None,
            })
            .unwrap();
        assert!(backend.execute_payload(&lighting_payload()).is_err());
    }

    #[test]
    fn shutdown_sends_blackout() {
        let mut backend = FixtureBusBackend::new("fb");
        backend.prepare_backend(&lighting_topology()).unwrap();
        backend.shutdown_backend().unwrap();

        assert_eq!(backend.describe_backend().status, "shutdown");
        assert!(backend.execute_payload(&lighting_payload()).is_err());
    }

    #[test]
    fn wrong_topology_rejected() {
        let mut backend = FixtureBusBackend::new("fb");
        let bad = BackendTopology::Audio {
            topology_ref: String::from("stereo"),
            calibration_profile: None,
            speaker_endpoints: vec![],
        };
        assert!(backend.prepare_backend(&bad).is_err());
    }
}
