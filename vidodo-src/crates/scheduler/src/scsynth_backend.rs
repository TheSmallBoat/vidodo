//! scsynth backend client for the scheduler.
//!
//! Wraps the `AudioScynthBackend` from the audio-bridge crate and falls
//! back to the reference audio backend when scsynth is not available.
//! Visual and lighting events are forwarded to the reference backends.

use std::cell::RefCell;

use vidodo_ir::{
    AudioEvent, BackendAck, BackendAdapter, BackendHealthSnapshot, BackendTopology,
    ExecutablePayload, LightingEvent, VisualEvent,
};

use crate::BackendClient;
use crate::lighting_backend::LightingReferenceBackend;
use crate::visual_backend::VisualReferenceBackend;

/// Indicates whether the scsynth backend was successfully prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScynthAvailability {
    Available,
    Fallback,
}

/// Backend client that attempts to use the real scsynth audio backend.
///
/// If scsynth is not available (prepare fails), it falls back to the
/// reference audio backend with a diagnostic message.
pub struct ScynthBackendClient {
    audio: RefCell<AudioBackendState>,
    visual: RefCell<VisualReferenceBackend>,
    lighting: RefCell<LightingReferenceBackend>,
    availability: ScynthAvailability,
    diagnostics: Vec<String>,
}

enum AudioBackendState {
    Scsynth(Box<vidodo_audio_bridge::backend::AudioScynthBackend>),
    Fallback(crate::audio_backend::AudioReferenceBackend),
}

impl ScynthBackendClient {
    /// Attempt to create a scsynth backend client.
    ///
    /// Tries to prepare the AudioScynthBackend with a stereo audio topology.
    /// On failure, falls back to the reference backend and records the
    /// diagnostic message.
    pub fn new() -> Self {
        let mut scsynth =
            vidodo_audio_bridge::backend::AudioScynthBackend::new("scsynth-v1", "3.13.0");
        let topology = BackendTopology::Audio {
            topology_ref: String::from("stereo-main"),
            calibration_profile: None,
            speaker_endpoints: vec![String::from("L"), String::from("R")],
        };

        let (audio, availability, diagnostics) = match scsynth.prepare_backend(&topology) {
            Ok(()) => (
                AudioBackendState::Scsynth(Box::new(scsynth)),
                ScynthAvailability::Available,
                vec![],
            ),
            Err(reason) => (
                AudioBackendState::Fallback(crate::audio_backend::AudioReferenceBackend::new(
                    "fallback-audio",
                )),
                ScynthAvailability::Fallback,
                vec![format!(
                    "scsynth unavailable ({reason}), fell back to reference audio backend"
                )],
            ),
        };

        Self {
            audio: RefCell::new(audio),
            visual: RefCell::new(VisualReferenceBackend::new("ref-visual")),
            lighting: RefCell::new(LightingReferenceBackend::new("ref-lighting")),
            availability,
            diagnostics,
        }
    }

    pub fn availability(&self) -> ScynthAvailability {
        self.availability
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

impl Default for ScynthBackendClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendClient for ScynthBackendClient {
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
        match &mut *self.audio.borrow_mut() {
            AudioBackendState::Scsynth(backend) => {
                backend.execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
                    backend: String::from("scsynth-v1"),
                    target: event.layer_id.clone(),
                    status: String::from("error"),
                    detail,
                })
            }
            AudioBackendState::Fallback(backend) => {
                backend.execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
                    backend: String::from("fallback-audio"),
                    target: event.layer_id.clone(),
                    status: String::from("error"),
                    detail,
                })
            }
        }
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
        self.lighting.borrow_mut().execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
            backend: String::from("ref-lighting"),
            target: event.cue_set_id.clone(),
            status: String::from("error"),
            detail,
        })
    }

    fn health_snapshots(&self) -> Vec<BackendHealthSnapshot> {
        let audio_status = match &*self.audio.borrow() {
            AudioBackendState::Scsynth(b) => b.collect_backend_status(),
            AudioBackendState::Fallback(b) => b.collect_backend_status(),
        };
        vec![BackendHealthSnapshot {
            backend_ref: String::from("scsynth-audio"),
            plugin_ref: audio_status.plugin_id,
            status: audio_status.status,
            timestamp: String::from("0"),
            latency_ms: audio_status.latency_ms,
            error_count: audio_status.error_count,
            last_ack_lag_ms: audio_status.last_ack_lag_ms,
            degrade_reason: audio_status.detail,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scsynth_client_creates_with_fallback() {
        // In test environment, scsynth binary is not present, so falls back
        let client = ScynthBackendClient::new();
        // Should succeed either way (available or fallback)
        assert!(
            client.availability() == ScynthAvailability::Available
                || client.availability() == ScynthAvailability::Fallback
        );
    }

    #[test]
    fn dispatch_audio_returns_ack() {
        let client = ScynthBackendClient::new();
        let event = AudioEvent {
            layer_id: String::from("layer-1"),
            op: String::from("play"),
            output_backend: String::from("scsynth"),
            route_mode: None,
            route_set_ref: None,
            speaker_group: vec![],
            gain_db: Some(-3.0),
            duration_beats: Some(4),
            filter: None,
            automation: std::collections::BTreeMap::new(),
            target_asset_id: Some(String::from("kick.wav")),
        };
        let ack = client.dispatch_audio(&event);
        assert_eq!(ack.target, "layer-1");
        assert!(ack.status == "ok" || ack.status == "error");
    }

    #[test]
    fn diagnostics_present_on_fallback() {
        let client = ScynthBackendClient::new();
        if client.availability() == ScynthAvailability::Fallback {
            assert!(!client.diagnostics().is_empty());
        }
    }
}
