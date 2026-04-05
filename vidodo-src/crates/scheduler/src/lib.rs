use serde::{Deserialize, Serialize};
use vidodo_ir::{
    AudioEvent, BackendAck, BackendHealthSnapshot, CompiledRevision, DegradeEvent, EventRecord,
    ExternalControlAdapter, ExternalControlEvent, LightingEvent, MusicalTime, PatchDecision,
    PatchEvent, ResourceSample, RunSummary, RuntimePayload, ShowState, TimingEvent, VisualEvent,
};

pub mod audio_backend;
pub mod clock;
pub mod fault_injection;
pub mod fixture_bus_backend;
pub mod health_monitor;
pub mod lighting_backend;
pub mod lookahead;
pub mod null_backend;
pub mod null_control_adapter;
pub mod patch_window;
pub mod realtime_clock;
pub mod realtime_dispatch;
pub mod reference_backend;
pub mod scene_manager;
pub mod scsynth_backend;
pub mod show_state;
pub mod transport;
pub mod visual_backend;
pub mod wgpu_backend;

/// Trait for dispatching events to audio and visual backends.
///
/// Implementors produce a [`BackendAck`] for each dispatched event.
/// The scheduler calls into this trait during a run so that fake, stub,
/// and future real backends share the same interface.
pub trait BackendClient {
    fn dispatch_audio(&self, event: &AudioEvent) -> BackendAck;
    fn dispatch_visual(&self, event: &VisualEvent) -> BackendAck;
    fn dispatch_lighting(&self, event: &LightingEvent) -> BackendAck;
    fn health_snapshots(&self) -> Vec<BackendHealthSnapshot> {
        Vec::new()
    }
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

    fn dispatch_lighting(&self, event: &LightingEvent) -> BackendAck {
        BackendAck {
            backend: String::from("fake_lighting_backend"),
            target: event.cue_set_id.clone(),
            status: String::from("ok"),
            detail: format!("cue {}", event.source_ref),
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
    pub degrade_events: Vec<EventRecord>,
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
                "lighting" => {
                    // target_ref is "cue_set_id:index"
                    if let Some((cue_set_id, index_str)) = entry.target_ref.split_once(':')
                        && let Ok(cue_index) = index_str.parse::<usize>()
                        && let Some(cue) = compiled
                            .cue_sets
                            .iter()
                            .find(|cs| cs.cue_set_id == cue_set_id)
                            .and_then(|cs| cs.entries.get(cue_index))
                    {
                        let payload = LightingEvent {
                            cue_set_id: cue_set_id.to_string(),
                            source_ref: cue.source_ref.clone(),
                            fixture_group: cue.fixture_group.clone(),
                            intensity: cue.intensity,
                            color: cue.color,
                            fade_beats: cue.fade_beats,
                        };
                        events.push(EventRecord {
                            event_id: format!("evt-{next_event:04}"),
                            show_id: compiled.show_id.clone(),
                            revision: compiled.revision,
                            kind: String::from("lighting.cue.enter"),
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
                            payload: RuntimePayload::Lighting(payload.clone()),
                            ack: Some(backend.dispatch_lighting(&payload)),
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

    // Evaluate backend health and emit degrade events if thresholds are exceeded.
    let snapshots = backend.health_snapshots();
    let decision =
        health_monitor::degrade_decision(&snapshots, &health_monitor::HealthThresholds::default());
    let mut degrade_events = Vec::new();
    if decision.should_degrade {
        for (i, mode) in decision.modes.iter().enumerate() {
            let evt_id = format!("evt-degrade-{:04}", i + 1);
            degrade_events.push(EventRecord {
                event_id: evt_id,
                show_id: compiled.show_id.clone(),
                revision: compiled.revision,
                kind: String::from("degrade.activated"),
                phase: String::from("executed"),
                source: String::from("health_monitor"),
                musical_time: MusicalTime::at_bar(
                    compiled.final_bar(),
                    1,
                    final_show_state.time.section.clone(),
                    128.0,
                ),
                scheduler_time_ms,
                wallclock_time_ms: scheduler_time_ms,
                causation_id: mode.mode.clone(),
                payload: RuntimePayload::Degrade(DegradeEvent {
                    degrade_id: mode.mode.clone(),
                    mode: mode.mode.clone(),
                    reason: mode.reason.clone(),
                    affected_backends: mode.affected_backends.clone(),
                    fallback_action: mode.fallback_action.clone(),
                }),
                ack: None,
            });
        }
    }

    let _ = run_id;

    ScheduledRun {
        events,
        summary,
        final_show_state,
        patch_decisions,
        resource_samples,
        degrade_events,
    }
}

// ShowState construction logic lives in show_state module.

/// Run simulation with fault injection.
///
/// Evaluates the injector at each section's start bar.  If faults are
/// produced, the injected snapshots are merged with the backend's own
/// snapshots and fed through [`health_monitor::degrade_decision`] to
/// produce `DegradeEvent` records mid-run.  The scheduler continues
/// executing after any injected fault — it never panics.
pub fn simulate_run_with_fault_injector(
    compiled: &CompiledRevision,
    run_id: &str,
    backend: &dyn BackendClient,
    injector: &dyn fault_injection::FaultInjector,
) -> ScheduledRun {
    let mut base = simulate_run_with_backend(compiled, run_id, backend);

    // Walk sections and evaluate the fault injector at each bar.
    let thresholds = health_monitor::HealthThresholds::default();
    let mut degrade_idx = base.degrade_events.len();
    for section in &compiled.structure_ir.sections {
        let bar = section.span.start_bar;
        let injected = injector.inject(bar);
        if injected.is_empty() {
            continue;
        }
        // Merge real + injected snapshots
        let mut all = backend.health_snapshots();
        all.extend(injected);
        let decision = health_monitor::degrade_decision(&all, &thresholds);
        if decision.should_degrade {
            for mode in &decision.modes {
                let evt_id = format!("evt-fault-degrade-{:04}", degrade_idx + 1);
                base.degrade_events.push(EventRecord {
                    event_id: evt_id,
                    show_id: compiled.show_id.clone(),
                    revision: compiled.revision,
                    kind: String::from("degrade.activated"),
                    phase: String::from("executed"),
                    source: String::from("health_monitor"),
                    musical_time: MusicalTime::at_bar(bar, 1, section.section_id.clone(), 128.0),
                    scheduler_time_ms: (bar as u64) * 1_000,
                    wallclock_time_ms: (bar as u64) * 1_000,
                    causation_id: mode.mode.clone(),
                    payload: RuntimePayload::Degrade(DegradeEvent {
                        degrade_id: mode.mode.clone(),
                        mode: mode.mode.clone(),
                        reason: mode.reason.clone(),
                        affected_backends: mode.affected_backends.clone(),
                        fallback_action: mode.fallback_action.clone(),
                    }),
                    ack: None,
                });
                degrade_idx += 1;
            }
        }
    }

    base.summary.event_count = base.events.len();
    base
}

/// Run simulation with an external control adapter.
///
/// Polls the adapter once per section tick. Each external control event is
/// mapped to a `RuntimePayload::ExternalControl` event record and appended
/// to the event stream (and consequently the trace).
pub fn simulate_run_with_controls(
    compiled: &CompiledRevision,
    run_id: &str,
    backend: &dyn BackendClient,
    control: &mut dyn ExternalControlAdapter,
) -> ScheduledRun {
    let mut base = simulate_run_with_backend(compiled, run_id, backend);

    // Poll external control events and append them to the event stream.
    let control_events = control.poll_events();
    let base_time_ms = base.events.last().map(|e| e.scheduler_time_ms).unwrap_or(0) + 1;
    let section = &base.final_show_state.time.section;
    for (i, ctrl_evt) in control_events.into_iter().enumerate() {
        let event_id = format!("evt-ctrl-{:04}", i + 1);
        let kind = match &ctrl_evt {
            ExternalControlEvent::MidiCc { .. } => "external_control.midi_cc",
            ExternalControlEvent::MidiNote { .. } => "external_control.midi_note",
            ExternalControlEvent::OscMessage { .. } => "external_control.osc_message",
        };
        base.events.push(EventRecord {
            event_id,
            show_id: compiled.show_id.clone(),
            revision: compiled.revision,
            kind: kind.to_string(),
            phase: String::from("executed"),
            source: String::from("external_control"),
            musical_time: MusicalTime::at_bar(
                base.final_show_state.time.bar,
                1,
                section.clone(),
                128.0,
            ),
            scheduler_time_ms: base_time_ms + i as u64,
            wallclock_time_ms: base_time_ms + i as u64,
            causation_id: String::from("control_adapter"),
            payload: RuntimePayload::ExternalControl(ctrl_evt),
            ack: None,
        });
    }

    // Update event count in summary
    base.summary.event_count = base.events.len();
    base
}

/// Realtime-mode simulation.
///
/// Produces the same events as offline `simulate_run_with_backend`, but
/// assigns wall-clock timestamps scaled by the show's tempo so that the
/// trace reflects real-time durations:
///
/// `wallclock_time_ms = (bar - 1) × beats_per_bar × ms_per_beat`
///
/// This allows trace analysis to verify that events are spaced according
/// to tempo rather than the synthetic `+1000ms` steps used by the offline
/// scheduler.
pub fn simulate_run_realtime(
    compiled: &CompiledRevision,
    run_id: &str,
    backend: &dyn BackendClient,
) -> ScheduledRun {
    let mut base = simulate_run_with_backend(compiled, run_id, backend);

    // Rewrite wallclock timestamps using tempo-derived timing.
    let tempo = compiled.structure_ir.sections.first().map(|_| 128.0_f64).unwrap_or(120.0);
    let beats_per_bar = 4.0_f64;
    let ms_per_beat = 60_000.0 / tempo;

    for event in &mut base.events {
        let bar = event.musical_time.bar;
        // beat_in_bar is 1-based in at_bar(), so subtract 1 for offset.
        let beat_offset = (event.musical_time.beat_in_bar - 1.0).max(0.0);
        let bar_ms = (bar.saturating_sub(1) as f64) * beats_per_bar * ms_per_beat;
        let beat_ms = beat_offset * ms_per_beat;
        event.wallclock_time_ms = (bar_ms + beat_ms) as u64;
    }

    base
}

#[cfg(test)]
mod tests {
    use super::{
        BackendClient, FakeBackendClient, simulate_run, simulate_run_realtime,
        simulate_run_with_backend, simulate_run_with_controls, simulate_run_with_fault_injector,
    };
    use vidodo_compiler::compile_plan;
    use vidodo_ir::{
        AudioEvent, BackendAck, ExternalControlEvent, MidiCC, OscMessage, PlanBundle,
        RuntimePayload, VisualEvent,
    };
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
            fn dispatch_lighting(&self, event: &vidodo_ir::LightingEvent) -> BackendAck {
                FakeBackendClient.dispatch_lighting(event)
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

    #[test]
    fn emits_lighting_events_from_cue_sets() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let run = simulate_run(&compiled, "run-lighting-test");

        let lighting_events: Vec<_> =
            run.events.iter().filter(|e| e.kind == "lighting.cue.enter").collect();
        assert!(
            !lighting_events.is_empty(),
            "expected at least one lighting event from cue_sets in minimal plan"
        );
        // The minimal plan has a cue with source_ref="intro", which matches the first section
        assert!(lighting_events.iter().any(|e| {
            if let RuntimePayload::Lighting(l) = &e.payload {
                l.source_ref == "intro" && !l.fixture_group.is_empty()
            } else {
                false
            }
        }));
    }

    #[test]
    fn emits_degrade_events_from_degraded_backend() {
        use vidodo_ir::{BackendHealthSnapshot, LightingEvent};

        struct DegradedBackend;
        impl BackendClient for DegradedBackend {
            fn dispatch_audio(&self, event: &AudioEvent) -> BackendAck {
                FakeBackendClient.dispatch_audio(event)
            }
            fn dispatch_visual(&self, event: &VisualEvent) -> BackendAck {
                FakeBackendClient.dispatch_visual(event)
            }
            fn dispatch_lighting(&self, event: &LightingEvent) -> BackendAck {
                FakeBackendClient.dispatch_lighting(event)
            }
            fn health_snapshots(&self) -> Vec<BackendHealthSnapshot> {
                vec![BackendHealthSnapshot {
                    backend_ref: String::from("audio_backend_1"),
                    plugin_ref: String::from("plugin-daw-bridge"),
                    status: String::from("degraded"),
                    timestamp: String::from("2025-01-01T00:00:00Z"),
                    latency_ms: Some(300.0),
                    error_count: Some(0),
                    last_ack_lag_ms: None,
                    degrade_reason: Some(String::from("bypass_audio_backend_1")),
                }]
            }
        }

        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let run = simulate_run_with_backend(&compiled, "run-degrade-test", &DegradedBackend);

        assert!(!run.degrade_events.is_empty(), "expected degrade events from degraded backend");
        let degrade = &run.degrade_events[0];
        assert_eq!(degrade.kind, "degrade.activated");
        assert_eq!(degrade.source, "health_monitor");
        if let RuntimePayload::Degrade(ref d) = degrade.payload {
            assert!(d.affected_backends.contains(&String::from("audio_backend_1")));
        } else {
            panic!("expected Degrade payload");
        }
    }

    #[test]
    fn no_degrade_events_from_healthy_backend() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let run = simulate_run(&compiled, "run-healthy-test");

        assert!(
            run.degrade_events.is_empty(),
            "healthy FakeBackendClient should produce no degrade events"
        );
    }

    // --- WST-03: Scheduler consumes external control events ---

    #[test]
    fn midi_cc_control_event_appears_in_run() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let mut adapter = crate::null_control_adapter::NullControlAdapter::new();
        adapter.inject(vec![ExternalControlEvent::MidiCc {
            source_id: String::from("midi-1"),
            midi_cc: MidiCC { channel: 1, cc: 7, value: 100 },
        }]);
        let run = simulate_run_with_controls(
            &compiled,
            "run-ctrl-midi",
            &FakeBackendClient,
            &mut adapter,
        );
        let ctrl_events: Vec<_> =
            run.events.iter().filter(|e| e.kind.starts_with("external_control.")).collect();
        assert_eq!(ctrl_events.len(), 1);
        assert_eq!(ctrl_events[0].kind, "external_control.midi_cc");
        assert_eq!(ctrl_events[0].source, "external_control");
    }

    #[test]
    fn osc_control_event_appears_in_run() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let mut adapter = crate::null_control_adapter::NullControlAdapter::new();
        adapter.inject(vec![ExternalControlEvent::OscMessage {
            source_id: String::from("osc-1"),
            osc_message: OscMessage {
                address: String::from("/fader/1"),
                args: vec![serde_json::json!(0.75)],
            },
        }]);
        let run =
            simulate_run_with_controls(&compiled, "run-ctrl-osc", &FakeBackendClient, &mut adapter);
        let ctrl_events: Vec<_> =
            run.events.iter().filter(|e| e.kind.starts_with("external_control.")).collect();
        assert_eq!(ctrl_events.len(), 1);
        assert_eq!(ctrl_events[0].kind, "external_control.osc_message");
    }

    #[test]
    fn no_control_events_without_adapter_input() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let mut adapter = crate::null_control_adapter::NullControlAdapter::new();
        let run = simulate_run_with_controls(
            &compiled,
            "run-ctrl-empty",
            &FakeBackendClient,
            &mut adapter,
        );
        let ctrl_events: Vec<_> =
            run.events.iter().filter(|e| e.kind.starts_with("external_control.")).collect();
        assert!(ctrl_events.is_empty());
    }

    #[test]
    fn multiple_control_events_all_tracked() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let mut adapter = crate::null_control_adapter::NullControlAdapter::new();
        adapter.inject(vec![
            ExternalControlEvent::MidiCc {
                source_id: String::from("midi-1"),
                midi_cc: MidiCC { channel: 1, cc: 7, value: 50 },
            },
            ExternalControlEvent::MidiCc {
                source_id: String::from("midi-1"),
                midi_cc: MidiCC { channel: 1, cc: 1, value: 127 },
            },
            ExternalControlEvent::OscMessage {
                source_id: String::from("osc-1"),
                osc_message: OscMessage {
                    address: String::from("/tempo"),
                    args: vec![serde_json::json!(130)],
                },
            },
        ]);
        let run = simulate_run_with_controls(
            &compiled,
            "run-ctrl-multi",
            &FakeBackendClient,
            &mut adapter,
        );
        let ctrl_events: Vec<_> =
            run.events.iter().filter(|e| e.kind.starts_with("external_control.")).collect();
        assert_eq!(ctrl_events.len(), 3);
    }

    // --- WSW-02: Fault injection tests ---

    #[test]
    fn fault_at_bar_triggers_degrade_event() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let injector = crate::fault_injection::FailAtBarInjector::new(
            compiled.structure_ir.sections[0].span.start_bar,
            "audio_backend_1",
        );
        let run = simulate_run_with_fault_injector(
            &compiled,
            "run-fault-bar",
            &FakeBackendClient,
            &injector,
        );
        assert!(!run.degrade_events.is_empty(), "expected degrade events from fault injection");
        let degrade = &run.degrade_events[0];
        assert_eq!(degrade.kind, "degrade.activated");
        assert_eq!(degrade.source, "health_monitor");
        if let RuntimePayload::Degrade(ref d) = degrade.payload {
            assert!(d.affected_backends.contains(&String::from("audio_backend_1")));
        } else {
            panic!("expected Degrade payload");
        }
    }

    #[test]
    fn null_injector_produces_no_degrade_events() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let injector = crate::fault_injection::NullFaultInjector;
        let run = simulate_run_with_fault_injector(
            &compiled,
            "run-null-injector",
            &FakeBackendClient,
            &injector,
        );
        assert!(
            run.degrade_events.is_empty(),
            "NullFaultInjector should produce no degrade events"
        );
    }

    #[test]
    fn scheduler_continues_after_fault_injection() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        // Put fault at the first section bar
        let injector = crate::fault_injection::FailAtBarInjector::new(
            compiled.structure_ir.sections[0].span.start_bar,
            "visual_backend_1",
        );
        let run = simulate_run_with_fault_injector(
            &compiled,
            "run-fault-continue",
            &FakeBackendClient,
            &injector,
        );
        // Scheduler must still produce all normal events (no panic, no short-circuit)
        let non_degrade: Vec<_> = run.events.iter().collect();
        assert!(!non_degrade.is_empty(), "scheduler must still produce events after fault");
        assert!(run.summary.event_count > 0, "summary event_count must be positive after fault");
    }

    #[test]
    fn fault_degrade_event_has_correct_bar_in_musical_time() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let target_bar = compiled.structure_ir.sections[0].span.start_bar;
        let injector =
            crate::fault_injection::FailAtBarInjector::new(target_bar, "audio_backend_1");
        let run = simulate_run_with_fault_injector(
            &compiled,
            "run-fault-bar-time",
            &FakeBackendClient,
            &injector,
        );
        let degrade = &run.degrade_events[0];
        assert_eq!(
            degrade.musical_time.bar, target_bar,
            "degrade event bar should match trigger bar"
        );
    }

    #[test]
    fn realtime_mode_wallclock_reflects_tempo() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let run = simulate_run_realtime(&compiled, "run-realtime-smoke", &FakeBackendClient);
        assert!(!run.events.is_empty());

        // In offline mode, wallclock_time_ms == scheduler_time_ms (synthetic +1000 steps).
        // In realtime mode, wallclock_time_ms is derived from bar position + tempo.
        // At 128 BPM, ms_per_beat = 468.75, ms_per_bar = 1875.
        let first = &run.events[0];
        assert_eq!(first.musical_time.bar, 1);
        assert_eq!(first.wallclock_time_ms, 0); // bar 1 → 0 ms offset

        // Any event at bar > 1 should have wallclock > 0 and proportional to
        // bar offset.
        if run.events.len() > 2 {
            let later = run.events.iter().find(|e| e.musical_time.bar > 1);
            if let Some(evt) = later {
                assert!(evt.wallclock_time_ms > 0);
            }
        }
    }

    #[test]
    fn offline_mode_regression_after_realtime_addition() {
        // Ensure offline simulate_run is unchanged.
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let run = simulate_run(&compiled, "run-offline-regression");
        // Offline: wallclock == scheduler_time (synthetic 1000ms steps)
        for event in &run.events {
            assert_eq!(event.wallclock_time_ms, event.scheduler_time_ms);
        }
    }
}
