//! Fixture-bus backend client for the scheduler.
//!
//! Wraps the `FixtureBusBackend` from the lighting-bridge crate and falls
//! back to the reference lighting backend when no DMX hardware is available.
//! Audio and visual events are forwarded to the reference backends.

use std::cell::RefCell;

use vidodo_ir::{
    AudioEvent, BackendAck, BackendAdapter, BackendHealthSnapshot, BackendTopology,
    ExecutablePayload, LightingEvent, VisualEvent,
};

use crate::BackendClient;
use crate::audio_backend::AudioReferenceBackend;
use crate::visual_backend::VisualReferenceBackend;

/// Indicates whether the fixture-bus backend was successfully prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureBusAvailability {
    Available,
    Fallback,
}

/// Backend client that attempts to use the real fixture-bus lighting backend.
pub struct FixtureBusBackendClient {
    audio: RefCell<AudioReferenceBackend>,
    visual: RefCell<VisualReferenceBackend>,
    lighting: RefCell<LightingBackendState>,
    availability: FixtureBusAvailability,
    diagnostics: Vec<String>,
}

enum LightingBackendState {
    FixtureBus(vidodo_lighting_bridge::backend::FixtureBusBackend),
    Fallback(crate::lighting_backend::LightingReferenceBackend),
}

impl FixtureBusBackendClient {
    pub fn new() -> Self {
        let mut fixture_bus =
            vidodo_lighting_bridge::backend::FixtureBusBackend::new("fixture-bus-v1");
        let topology = BackendTopology::Lighting {
            topology_ref: String::from("dmx-universe-1"),
            calibration_profile: None,
            fixture_endpoints: vec![String::from("par-1"), String::from("par-2")],
        };

        let (lighting, availability, diagnostics) = match fixture_bus.prepare_backend(&topology) {
            Ok(()) => (
                LightingBackendState::FixtureBus(fixture_bus),
                FixtureBusAvailability::Available,
                vec![],
            ),
            Err(reason) => (
                LightingBackendState::Fallback(
                    crate::lighting_backend::LightingReferenceBackend::new("fallback-lighting"),
                ),
                FixtureBusAvailability::Fallback,
                vec![format!(
                    "fixture-bus unavailable ({reason}), fell back to reference lighting backend"
                )],
            ),
        };

        Self {
            audio: RefCell::new(AudioReferenceBackend::new("ref-audio")),
            visual: RefCell::new(VisualReferenceBackend::new("ref-visual")),
            lighting: RefCell::new(lighting),
            availability,
            diagnostics,
        }
    }

    pub fn availability(&self) -> FixtureBusAvailability {
        self.availability
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

impl Default for FixtureBusBackendClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendClient for FixtureBusBackendClient {
    fn dispatch_audio(&self, event: &AudioEvent) -> BackendAck {
        let payload = ExecutablePayload::Audio {
            layer_id: event.layer_id.clone(),
            op: event.op.clone(),
            target_asset_id: event.target_asset_id.clone(),
            gain_db: event.gain_db,
            duration_beats: event.duration_beats,
            route_set_ref: event.route_set_ref.clone(),
            speaker_group: event.speaker_group.clone(),
        };
        self.audio.borrow_mut().execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
            backend: String::from("ref-audio"),
            target: event.layer_id.clone(),
            status: String::from("error"),
            detail,
        })
    }

    fn dispatch_visual(&self, event: &VisualEvent) -> BackendAck {
        let payload = ExecutablePayload::Visual {
            scene_id: event.scene_id.clone(),
            shader_program: event.shader_program.clone(),
            uniforms: event.uniforms.clone(),
            duration_beats: event.duration_beats,
            blend: event.blend.clone(),
            view_group: event.view_group.clone(),
        };
        self.visual.borrow_mut().execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
            backend: String::from("ref-visual"),
            target: event.scene_id.clone(),
            status: String::from("error"),
            detail,
        })
    }

    fn dispatch_lighting(&self, event: &LightingEvent) -> BackendAck {
        let payload = ExecutablePayload::Lighting {
            cue_set_id: event.cue_set_id.clone(),
            source_ref: event.source_ref.clone(),
            fixture_group: event.fixture_group.clone(),
            intensity: event.intensity,
            color: event.color,
            fade_beats: event.fade_beats,
        };
        match &mut *self.lighting.borrow_mut() {
            LightingBackendState::FixtureBus(backend) => {
                backend.execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
                    backend: String::from("fixture-bus-v1"),
                    target: event.cue_set_id.clone(),
                    status: String::from("error"),
                    detail,
                })
            }
            LightingBackendState::Fallback(backend) => {
                backend.execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
                    backend: String::from("fallback-lighting"),
                    target: event.cue_set_id.clone(),
                    status: String::from("error"),
                    detail,
                })
            }
        }
    }

    fn health_snapshots(&self) -> Vec<BackendHealthSnapshot> {
        let status = match &*self.lighting.borrow() {
            LightingBackendState::FixtureBus(b) => b.collect_backend_status(),
            LightingBackendState::Fallback(b) => b.collect_backend_status(),
        };
        vec![BackendHealthSnapshot {
            backend_ref: String::from("fixture-bus-lighting"),
            plugin_ref: status.plugin_id,
            status: status.status,
            timestamp: String::from("0"),
            latency_ms: status.latency_ms,
            error_count: status.error_count,
            last_ack_lag_ms: status.last_ack_lag_ms,
            degrade_reason: status.detail,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_bus_client_creates_successfully() {
        let client = FixtureBusBackendClient::new();
        // Should succeed (FixtureBusBackend prepare always works in lib mode)
        assert_eq!(client.availability(), FixtureBusAvailability::Available);
    }

    #[test]
    fn dispatch_lighting_returns_ack() {
        let client = FixtureBusBackendClient::new();
        let event = LightingEvent {
            cue_set_id: String::from("cue-1"),
            source_ref: String::from("src-1"),
            fixture_group: vec![],
            intensity: Some(0.8),
            color: Some([1.0, 0.0, 0.0]),
            fade_beats: None,
        };
        let ack = client.dispatch_lighting(&event);
        assert_eq!(ack.target, "cue-1");
    }

    #[test]
    fn audio_visual_forwarded_to_reference() {
        let client = FixtureBusBackendClient::new();
        let audio_event = AudioEvent {
            layer_id: String::from("layer-1"),
            op: String::from("play"),
            output_backend: String::from("ref"),
            route_mode: None,
            route_set_ref: None,
            speaker_group: vec![],
            gain_db: Some(-3.0),
            duration_beats: Some(4),
            filter: None,
            automation: std::collections::BTreeMap::new(),
            target_asset_id: Some(String::from("kick.wav")),
        };
        let ack = client.dispatch_audio(&audio_event);
        assert_eq!(ack.status, "ok");
    }
}
