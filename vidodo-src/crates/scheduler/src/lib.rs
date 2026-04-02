use serde::{Deserialize, Serialize};
use vidodo_ir::{
    AudioEvent, BackendAck, CompiledRevision, EventRecord, MusicalTime, PatchDecision, PatchEvent,
    ResourceSample, RunSummary, RuntimePayload, ShowState, TimingEvent, VisualEvent,
};

pub mod clock;
pub mod lookahead;
pub mod show_state;

/// Trait for dispatching events to audio and visual backends.
///
/// Implementors produce a [`BackendAck`] for each dispatched event.
/// The scheduler calls into this trait during a run so that fake, stub,
/// and future real backends share the same interface.
pub trait BackendClient {
    fn dispatch_audio(&self, event: &AudioEvent) -> BackendAck;
    fn dispatch_visual(&self, event: &VisualEvent) -> BackendAck;
}

/// Deterministic fake backend that always returns `ok` acks.
pub struct FakeBackendClient;

impl BackendClient for FakeBackendClient {
    fn dispatch_audio(&self, event: &AudioEvent) -> BackendAck {
        BackendAck {
            backend: String::from("fake_audio_backend"),
            target: event.layer_id.clone(),
            status: String::from("ok"),
            detail: format!(
                "{} {}",
                event.op,
                event.target_asset_id.clone().unwrap_or_else(|| String::from("none"))
            ),
        }
    }

    fn dispatch_visual(&self, event: &VisualEvent) -> BackendAck {
        BackendAck {
            backend: String::from("fake_visual_backend"),
            target: event.scene_id.clone(),
            status: String::from("ok"),
            detail: format!("render {}", event.shader_program),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledRun {
    pub events: Vec<EventRecord>,
    pub summary: RunSummary,
    pub final_show_state: ShowState,
    pub patch_decisions: Vec<PatchDecision>,
    pub resource_samples: Vec<ResourceSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatusRecord {
    pub show_id: String,
    pub run_id: String,
    pub revision: u64,
    pub status: String,
    pub trace_manifest: String,
    pub summary: RunSummary,
    pub final_show_state: ShowState,
}

pub fn prepare_run_summary(compiled: &CompiledRevision) -> RunSummary {
    RunSummary {
        show_id: compiled.show_id.clone(),
        revision: compiled.revision,
        starting_bar: 1,
        final_bar: compiled.final_bar(),
        event_count: compiled.timeline.len() + compiled.structure_ir.sections.len(),
        final_section: compiled
            .structure_ir
            .sections
            .last()
            .map(|section| section.section_id.clone())
            .unwrap_or_else(|| String::from("intro")),
    }
}

pub fn simulate_run(compiled: &CompiledRevision, run_id: &str) -> ScheduledRun {
    simulate_run_with_backend(compiled, run_id, &FakeBackendClient)
}

pub fn simulate_run_with_backend(
    compiled: &CompiledRevision,
    run_id: &str,
    backend: &dyn BackendClient,
) -> ScheduledRun {
    let mut events = Vec::new();
    let mut patch_decisions = Vec::new();
    let mut resource_samples = Vec::new();
    let mut next_event = 1_u64;
    let mut scheduler_time_ms = 0_u64;
    let mut active_audio_layers = Vec::new();
    let mut active_visual_scene = compiled
        .visual_ir
        .visual_actions
        .first()
        .map(|action| action.scene_id.clone())
        .unwrap_or_else(|| String::from("scene_intro"));
    let mut final_show_state = show_state::build_show_state(
        compiled,
        &active_visual_scene,
        &active_audio_layers,
        1,
        "intro",
    );

    for section in &compiled.structure_ir.sections {
        let phrase = section.order as u32 + 1;
        let musical_time =
            MusicalTime::at_bar(section.span.start_bar, phrase, section.section_id.clone(), 128.0);
        events.push(EventRecord {
            event_id: format!("evt-{next_event:04}"),
            show_id: compiled.show_id.clone(),
            revision: compiled.revision,
            kind: String::from("timing.section.enter"),
            phase: String::from("executed"),
            source: String::from("scheduler"),
            musical_time: musical_time.clone(),
            scheduler_time_ms,
            wallclock_time_ms: scheduler_time_ms,
            causation_id: section.section_id.clone(),
            payload: RuntimePayload::Timing(TimingEvent {
                phrase,
                section: section.section_id.clone(),
                tempo: 128.0,
                downbeat: true,
                bar: Some(section.span.start_bar),
                beat: Some(musical_time.beat),
                time_signature: Some([4, 4]),
                transition_window_open: Some(true),
            }),
            ack: None,
        });
        next_event += 1;
        scheduler_time_ms += 1_000;

        for entry in compiled
            .timeline
            .iter()
            .filter(|entry| entry.effective_window.from_bar == section.span.start_bar)
        {
            match entry.channel.as_str() {
                "audio" => {
                    if let Some(action) = compiled
                        .performance_ir
                        .performance_actions
                        .iter()
                        .find(|action| action.action_id == entry.target_ref)
                    {
                        if !active_audio_layers.contains(&action.layer_id) {
                            active_audio_layers.push(action.layer_id.clone());
                        }
                        let payload = AudioEvent {
                            layer_id: action.layer_id.clone(),
                            op: action.op.clone(),
                            output_backend: action
                                .output_backend_hint
                                .clone()
                                .unwrap_or_else(|| String::from("fake_audio_backend")),
                            route_mode: Some(String::from("bus")),
                            route_set_ref: action.route_set_ref.clone(),
                            speaker_group: vec![String::from("stereo-main")],
                            gain_db: Some(-3.0),
                            duration_beats: Some(action.duration_beats),
                            filter: None,
                            automation: std::collections::BTreeMap::new(),
                            target_asset_id: action.target_asset_id.clone(),
                        };
                        events.push(EventRecord {
                            event_id: format!("evt-{next_event:04}"),
                            show_id: compiled.show_id.clone(),
                            revision: compiled.revision,
                            kind: format!("audio.{}", action.op),
                            phase: String::from("executed"),
                            source: String::from("scheduler"),
                            musical_time: action.musical_time.clone(),
                            scheduler_time_ms,
                            wallclock_time_ms: scheduler_time_ms,
                            causation_id: entry.id.clone(),
                            payload: RuntimePayload::Audio(payload.clone()),
                            ack: Some(backend.dispatch_audio(&payload)),
                        });
                        next_event += 1;
                        scheduler_time_ms += 1_000;
                    }
                }
                "visual" => {
                    if let Some(action) = compiled
                        .visual_ir
                        .visual_actions
                        .iter()
                        .find(|action| action.visual_action_id == entry.target_ref)
                    {
                        active_visual_scene = action.scene_id.clone();
                        let payload = VisualEvent {
                            scene_id: action.scene_id.clone(),
                            shader_program: action.program_ref.clone(),
                            output_backend: action
                                .output_backend
                                .clone()
                                .unwrap_or_else(|| String::from("fake_visual_backend")),
                            view_group: action.view_group_ref.clone(),
                            display_topology: action.display_topology_ref.clone(),
                            calibration_profile: Some(String::from("default-calibration")),
                            uniforms: action.uniform_set.clone(),
                            views: Vec::new(),
                            duration_beats: Some(action.duration_beats),
                            blend: action.blend_mode.clone(),
                        };
                        events.push(EventRecord {
                            event_id: format!("evt-{next_event:04}"),
                            show_id: compiled.show_id.clone(),
                            revision: compiled.revision,
                            kind: String::from("visual.scene.enter"),
                            phase: String::from("executed"),
                            source: String::from("scheduler"),
                            musical_time: MusicalTime::at_bar(
                                section.span.start_bar,
                                phrase,
                                section.section_id.clone(),
                                128.0,
                            ),
                            scheduler_time_ms,
                            wallclock_time_ms: scheduler_time_ms,
                            causation_id: entry.id.clone(),
                            payload: RuntimePayload::Visual(payload.clone()),
                            ack: Some(backend.dispatch_visual(&payload)),
                        });
                        next_event += 1;
                        scheduler_time_ms += 1_000;
                    }
                }
                "patch" => {
                    if let Some(decision) = compiled
                        .patch_history
                        .iter()
                        .find(|decision| decision.patch_id == entry.target_ref)
                    {
                        let payload = PatchEvent {
                            patch_id: decision.patch_id.clone(),
                            scope: decision.scope.clone(),
                            effective_revision: decision.candidate_revision,
                            fallback_revision: decision.fallback_revision,
                            decision: Some(decision.decision.clone()),
                            reason: Some(String::from("scheduled patch activation")),
                        };
                        events.push(EventRecord {
                            event_id: format!("evt-{next_event:04}"),
                            show_id: compiled.show_id.clone(),
                            revision: compiled.revision,
                            kind: String::from("patch.applied"),
                            phase: String::from("executed"),
                            source: String::from("scheduler"),
                            musical_time: MusicalTime::at_bar(
                                section.span.start_bar,
                                phrase,
                                section.section_id.clone(),
                                128.0,
                            ),
                            scheduler_time_ms,
                            wallclock_time_ms: scheduler_time_ms,
                            causation_id: entry.id.clone(),
                            payload: RuntimePayload::Patch(payload),
                            ack: Some(BackendAck {
                                backend: String::from("patch_manager"),
                                target: decision.patch_id.clone(),
                                status: String::from("ok"),
                                detail: String::from("patch activated"),
                            }),
                        });
                        next_event += 1;
                        scheduler_time_ms += 1_000;
                    }
                }
                _ => {}
            }
        }

        final_show_state = show_state::build_show_state(
            compiled,
            &active_visual_scene,
            &active_audio_layers,
            section.span.end_bar,
            &section.section_id,
        );

        // Emit a resource sample at the end of each section
        resource_samples.push(ResourceSample {
            sample_time_ms: scheduler_time_ms,
            show_id: compiled.show_id.clone(),
            revision: compiled.revision,
            bar: section.span.end_bar,
            section: section.section_id.clone(),
            cpu: 0.35 + (section.order as f64 * 0.05),
            gpu: if active_visual_scene.contains("drop") { 0.55 } else { 0.25 },
            memory_mb: 512 + (active_audio_layers.len() as u32 * 64),
            audio_xruns: 0,
            video_dropped_frames: 0,
            active_scene: active_visual_scene.clone(),
        });
    }

    // Collect patch decisions from the compiled revision's history
    for decision in &compiled.patch_history {
        patch_decisions.push(decision.clone());
    }

    let summary = RunSummary {
        show_id: compiled.show_id.clone(),
        revision: compiled.revision,
        starting_bar: 1,
        final_bar: compiled.final_bar(),
        event_count: events.len(),
        final_section: final_show_state.time.section.clone(),
    };

    let _ = run_id;

    ScheduledRun { events, summary, final_show_state, patch_decisions, resource_samples }
}

// ShowState construction logic lives in show_state module.

#[cfg(test)]
mod tests {
    use super::{BackendClient, FakeBackendClient, simulate_run, simulate_run_with_backend};
    use vidodo_compiler::compile_plan;
    use vidodo_ir::{AudioEvent, BackendAck, PlanBundle, VisualEvent};
    use vidodo_patch_manager::apply_patch;

    #[test]
    fn emits_patch_event_for_patched_revision() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let patch = serde_json::from_str::<vidodo_ir::LivePatchProposal>(
            r#"{
                "patch_id": "patch-phase0-pad-swap",
                "submitted_by": "tests",
                "patch_class": "local_content",
                "base_revision": 1,
                "scope": {"from_bar": 9, "to_bar": 16, "window": "next_phrase_boundary"},
                "intent": {},
                "changes": [{"op": "replace_asset", "target": "texture-bed", "from": "audio.loop.pad-a", "to": "audio.loop.pad-b"}],
                "fallback_revision": 1
            }"#,
        )
        .unwrap();

        let patched = apply_patch(&compiled, &patch).expect("patch should apply");
        let run = simulate_run(&patched, "run-show-phase0-rev-2");

        assert!(run.events.iter().any(|event| event.kind == "patch.applied"));
    }

    #[test]
    fn custom_backend_receives_dispatches() {
        struct CountingBackend {
            audio_count: std::cell::Cell<u32>,
            visual_count: std::cell::Cell<u32>,
        }
        impl BackendClient for CountingBackend {
            fn dispatch_audio(&self, event: &AudioEvent) -> BackendAck {
                self.audio_count.set(self.audio_count.get() + 1);
                FakeBackendClient.dispatch_audio(event)
            }
            fn dispatch_visual(&self, event: &VisualEvent) -> BackendAck {
                self.visual_count.set(self.visual_count.get() + 1);
                FakeBackendClient.dispatch_visual(event)
            }
        }
        let backend = CountingBackend {
            audio_count: std::cell::Cell::new(0),
            visual_count: std::cell::Cell::new(0),
        };
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let run = simulate_run_with_backend(&compiled, "run-test-backend", &backend);

        assert!(backend.audio_count.get() > 0, "audio backend should be called");
        assert!(backend.visual_count.get() > 0, "visual backend should be called");
        assert!(!run.events.is_empty());
    }

    #[test]
    fn produces_resource_samples_per_section() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let run = simulate_run(&compiled, "run-test-resource");

        assert_eq!(
            run.resource_samples.len(),
            compiled.structure_ir.sections.len(),
            "one resource sample per section"
        );
        for sample in &run.resource_samples {
            assert_eq!(sample.show_id, "show-phase0");
            assert_eq!(sample.audio_xruns, 0);
            assert!(sample.cpu > 0.0);
        }
    }

    #[test]
    fn collects_patch_decisions_from_patched_revision() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let patch = serde_json::from_str::<vidodo_ir::LivePatchProposal>(
            r#"{
                "patch_id": "patch-phase0-pad-swap",
                "submitted_by": "tests",
                "patch_class": "local_content",
                "base_revision": 1,
                "scope": {"from_bar": 9, "to_bar": 16, "window": "next_phrase_boundary"},
                "intent": {},
                "changes": [{"op": "replace_asset", "target": "texture-bed", "from": "audio.loop.pad-a", "to": "audio.loop.pad-b"}],
                "fallback_revision": 1
            }"#,
        )
        .unwrap();

        let patched = apply_patch(&compiled, &patch).expect("patch should apply");
        let run = simulate_run(&patched, "run-test-patch-capture");

        assert_eq!(run.patch_decisions.len(), 1);
        assert_eq!(run.patch_decisions[0].patch_id, "patch-phase0-pad-swap");
        assert_eq!(run.patch_decisions[0].decision, "applied");
    }
}
