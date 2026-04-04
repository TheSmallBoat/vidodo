use std::cell::RefCell;

use vidodo_ir::{
    AudioEvent, BackendAck, BackendAdapter, BackendHealthSnapshot, ExecutablePayload,
    LightingEvent, VisualEvent,
};

use crate::BackendClient;
use crate::audio_backend::AudioReferenceBackend;
use crate::lighting_backend::LightingReferenceBackend;
use crate::visual_backend::VisualReferenceBackend;

/// A [`BackendClient`] implementation that delegates to the three reference
/// [`BackendAdapter`] implementations (audio, visual, lighting).
///
/// Events are converted to [`ExecutablePayload`] and forwarded to the
/// matching adapter. Interior mutability via `RefCell` allows the
/// immutable `BackendClient` interface to drive mutable adapters.
pub struct ReferenceBackendClient {
    audio: RefCell<AudioReferenceBackend>,
    visual: RefCell<VisualReferenceBackend>,
    lighting: RefCell<LightingReferenceBackend>,
}

impl ReferenceBackendClient {
    pub fn new() -> Self {
        Self {
            audio: RefCell::new(AudioReferenceBackend::new("ref-audio")),
            visual: RefCell::new(VisualReferenceBackend::new("ref-visual")),
            lighting: RefCell::new(LightingReferenceBackend::new("ref-lighting")),
        }
    }
}

impl Default for ReferenceBackendClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendClient for ReferenceBackendClient {
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
        self.lighting.borrow_mut().execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
            backend: String::from("ref-lighting"),
            target: event.cue_set_id.clone(),
            status: String::from("error"),
            detail,
        })
    }

    fn health_snapshots(&self) -> Vec<BackendHealthSnapshot> {
        let audio_status = self.audio.borrow().collect_backend_status();
        let visual_status = self.visual.borrow().collect_backend_status();
        let lighting_status = self.lighting.borrow().collect_backend_status();
        vec![
            BackendHealthSnapshot {
                backend_ref: String::from("ref-audio"),
                plugin_ref: audio_status.plugin_id,
                status: audio_status.status,
                timestamp: String::from("0"),
                latency_ms: audio_status.latency_ms,
                error_count: audio_status.error_count,
                last_ack_lag_ms: audio_status.last_ack_lag_ms,
                degrade_reason: audio_status.detail,
            },
            BackendHealthSnapshot {
                backend_ref: String::from("ref-visual"),
                plugin_ref: visual_status.plugin_id,
                status: visual_status.status,
                timestamp: String::from("0"),
                latency_ms: visual_status.latency_ms,
                error_count: visual_status.error_count,
                last_ack_lag_ms: visual_status.last_ack_lag_ms,
                degrade_reason: visual_status.detail,
            },
            BackendHealthSnapshot {
                backend_ref: String::from("ref-lighting"),
                plugin_ref: lighting_status.plugin_id,
                status: lighting_status.status,
                timestamp: String::from("0"),
                latency_ms: lighting_status.latency_ms,
                error_count: lighting_status.error_count,
                last_ack_lag_ms: lighting_status.last_ack_lag_ms,
                degrade_reason: lighting_status.detail,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_audio_returns_ok() {
        let client = ReferenceBackendClient::new();
        let event = AudioEvent {
            layer_id: String::from("layer-1"),
            op: String::from("play"),
            output_backend: String::from("ref-audio"),
            route_mode: None,
            route_set_ref: None,
            speaker_group: vec![String::from("stereo-main")],
            gain_db: Some(-3.0),
            duration_beats: Some(16),
            filter: None,
            automation: Default::default(),
            target_asset_id: Some(String::from("asset-1")),
        };
        let ack = client.dispatch_audio(&event);
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "layer-1");
        assert_eq!(ack.backend, "ref-audio");
    }

    #[test]
    fn dispatch_visual_returns_ok() {
        let client = ReferenceBackendClient::new();
        let event = VisualEvent {
            scene_id: String::from("scene-drop"),
            shader_program: String::from("shader-glow"),
            output_backend: String::from("ref-visual"),
            view_group: None,
            display_topology: None,
            calibration_profile: None,
            uniforms: Default::default(),
            views: Vec::new(),
            duration_beats: Some(8),
            blend: Some(String::from("crossfade")),
        };
        let ack = client.dispatch_visual(&event);
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "scene-drop");
        assert_eq!(ack.backend, "ref-visual");
    }

    #[test]
    fn dispatch_lighting_returns_ok() {
        let client = ReferenceBackendClient::new();
        let event = LightingEvent {
            cue_set_id: String::from("cue-set-1"),
            source_ref: String::from("timeline-main"),
            fixture_group: Vec::new(),
            intensity: Some(0.85),
            color: Some([1.0, 0.0, 0.0]),
            fade_beats: Some(4.0),
        };
        let ack = client.dispatch_lighting(&event);
        assert_eq!(ack.status, "ok");
        assert_eq!(ack.target, "cue-set-1");
        assert_eq!(ack.backend, "ref-lighting");
    }

    #[test]
    fn health_snapshots_returns_three() {
        let client = ReferenceBackendClient::new();
        let snapshots = client.health_snapshots();
        assert_eq!(snapshots.len(), 3);
        assert_eq!(snapshots[0].backend_ref, "ref-audio");
        assert_eq!(snapshots[1].backend_ref, "ref-visual");
        assert_eq!(snapshots[2].backend_ref, "ref-lighting");
    }

    #[test]
    fn multiple_dispatches_accumulate() {
        let client = ReferenceBackendClient::new();
        let event = AudioEvent {
            layer_id: String::from("layer-1"),
            op: String::from("play"),
            output_backend: String::from("ref-audio"),
            route_mode: None,
            route_set_ref: None,
            speaker_group: Vec::new(),
            gain_db: None,
            duration_beats: None,
            filter: None,
            automation: Default::default(),
            target_asset_id: None,
        };
        client.dispatch_audio(&event);
        client.dispatch_audio(&event);
        assert_eq!(client.audio.borrow().execute_count(), 2);
    }
}
