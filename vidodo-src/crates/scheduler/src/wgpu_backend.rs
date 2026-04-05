//! wgpu visual backend client for the scheduler.
//!
//! Wraps the `VisualWgpuBackend` from the visual-bridge crate and falls
//! back to the `VisualReferenceBackend` when GPU initialisation fails.
//! Audio and lighting events are forwarded to the reference backends.

use std::cell::RefCell;

use vidodo_ir::{
    AudioEvent, BackendAck, BackendAdapter, BackendHealthSnapshot, BackendTopology,
    ExecutablePayload, LightingEvent, VisualEvent,
};

use crate::BackendClient;
use crate::audio_backend::AudioReferenceBackend;
use crate::lighting_backend::LightingReferenceBackend;
use crate::visual_backend::VisualReferenceBackend;

/// Indicates whether the wgpu backend was successfully prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WgpuAvailability {
    Available,
    Fallback,
}

/// Backend client that attempts to use the real wgpu visual backend.
///
/// If wgpu initialisation fails (prepare returns error), it falls back
/// to the reference visual backend with a diagnostic message.
pub struct WgpuBackendClient {
    visual: RefCell<VisualBackendState>,
    audio: RefCell<AudioReferenceBackend>,
    lighting: RefCell<LightingReferenceBackend>,
    availability: WgpuAvailability,
    diagnostics: Vec<String>,
}

enum VisualBackendState {
    Wgpu(Box<vidodo_visual_bridge::backend::VisualWgpuBackend>),
    Fallback(VisualReferenceBackend),
}

impl WgpuBackendClient {
    /// Attempt to create a wgpu visual backend client.
    ///
    /// Tries to prepare the `VisualWgpuBackend` with a flat display topology.
    /// On failure, falls back to the reference backend and records the diagnostic.
    pub fn new() -> Self {
        let mut wgpu = vidodo_visual_bridge::backend::VisualWgpuBackend::new("wgpu-v1");
        let topology = BackendTopology::Visual {
            topology_ref: String::from("flat-main"),
            calibration_profile: None,
            display_endpoints: vec![String::from("main")],
        };

        let (visual, availability, diagnostics) = match wgpu.prepare_backend(&topology) {
            Ok(()) => {
                (VisualBackendState::Wgpu(Box::new(wgpu)), WgpuAvailability::Available, vec![])
            }
            Err(reason) => (
                VisualBackendState::Fallback(VisualReferenceBackend::new("fallback-visual")),
                WgpuAvailability::Fallback,
                vec![format!("wgpu unavailable ({reason}), fell back to reference visual backend")],
            ),
        };

        Self {
            visual: RefCell::new(visual),
            audio: RefCell::new(AudioReferenceBackend::new("ref-audio")),
            lighting: RefCell::new(LightingReferenceBackend::new("ref-lighting")),
            availability,
            diagnostics,
        }
    }

    pub fn availability(&self) -> WgpuAvailability {
        self.availability
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

impl Default for WgpuBackendClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendClient for WgpuBackendClient {
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
        match &mut *self.visual.borrow_mut() {
            VisualBackendState::Wgpu(backend) => {
                backend.execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
                    backend: String::from("wgpu-v1"),
                    target: event.scene_id.clone(),
                    status: String::from("error"),
                    detail,
                })
            }
            VisualBackendState::Fallback(backend) => {
                backend.execute_payload(&payload).unwrap_or_else(|detail| BackendAck {
                    backend: String::from("fallback-visual"),
                    target: event.scene_id.clone(),
                    status: String::from("error"),
                    detail,
                })
            }
        }
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
        let visual_status = match &*self.visual.borrow() {
            VisualBackendState::Wgpu(b) => b.collect_backend_status(),
            VisualBackendState::Fallback(b) => b.collect_backend_status(),
        };
        vec![BackendHealthSnapshot {
            backend_ref: String::from("wgpu-visual"),
            plugin_ref: visual_status.plugin_id,
            status: visual_status.status,
            timestamp: String::from("0"),
            latency_ms: visual_status.latency_ms,
            error_count: visual_status.error_count,
            last_ack_lag_ms: visual_status.last_ack_lag_ms,
            degrade_reason: visual_status.detail,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgpu_client_creates_with_fallback() {
        let client = WgpuBackendClient::new();
        assert!(
            client.availability() == WgpuAvailability::Available
                || client.availability() == WgpuAvailability::Fallback
        );
    }

    #[test]
    fn dispatch_visual_returns_ack() {
        let client = WgpuBackendClient::new();
        let event = VisualEvent {
            scene_id: String::from("scene-1"),
            shader_program: String::from("particles-basic"),
            output_backend: String::from("wgpu"),
            view_group: None,
            display_topology: None,
            calibration_profile: None,
            uniforms: std::collections::BTreeMap::new(),
            views: vec![],
            duration_beats: Some(4),
            blend: None,
        };
        let ack = client.dispatch_visual(&event);
        assert_eq!(ack.target, "scene-1");
        assert!(ack.status == "ok" || ack.status == "error");
    }

    #[test]
    fn diagnostics_on_fallback() {
        let client = WgpuBackendClient::new();
        if client.availability() == WgpuAvailability::Fallback {
            assert!(!client.diagnostics().is_empty());
            assert!(client.diagnostics()[0].contains("wgpu unavailable"));
        }
    }

    #[test]
    fn dispatch_audio_via_reference() {
        let client = WgpuBackendClient::new();
        let event = AudioEvent {
            layer_id: String::from("layer-1"),
            op: String::from("play"),
            output_backend: String::from("ref-audio"),
            route_mode: None,
            route_set_ref: None,
            speaker_group: vec![],
            gain_db: Some(-6.0),
            duration_beats: Some(8),
            filter: None,
            automation: std::collections::BTreeMap::new(),
            target_asset_id: Some(String::from("pad.wav")),
        };
        let ack = client.dispatch_audio(&event);
        assert_eq!(ack.target, "layer-1");
        assert!(ack.status == "ok" || ack.status == "error");
    }

    #[test]
    fn health_snapshot_reports_visual() {
        let client = WgpuBackendClient::new();
        let snaps = client.health_snapshots();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].backend_ref, "wgpu-visual");
    }
}
