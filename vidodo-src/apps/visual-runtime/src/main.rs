use std::env;
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use vidodo_ir::{EventRecord, RuntimePayload};
use vidodo_storage::{ArtifactLayout, discover_repo_root, write_json};
use vidodo_trace::load_events;

mod visual_backend;
pub use visual_backend::VisualReferenceBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SceneAck {
    event_id: String,
    scene_id: String,
    shader_program: String,
    bar: u32,
    status: String,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("visual-runtime error: {message}");
            ExitCode::from(1)
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let run_id = required_arg(args, "--run-id")?;
    let repo_root = discover_repo_root()?;
    let layout = ArtifactLayout::new(repo_root.join("artifacts"));

    let events = load_events(&layout, &run_id)?;
    let acks = process_events(&events);

    let output_path = layout.trace_dir(&run_id).join("visual-acks.json");
    write_json(&output_path, &acks)?;

    println!(
        "{{\"status\":\"ok\",\"run_id\":\"{run_id}\",\"visual_events_processed\":{},\"acks_written\":{}}}",
        events.iter().filter(|e| matches!(&e.payload, RuntimePayload::Visual(_))).count(),
        acks.len()
    );

    Ok(())
}

fn process_events(events: &[EventRecord]) -> Vec<SceneAck> {
    let mut acks = Vec::new();
    let mut current_scene = String::from("none");

    for event in events {
        match &event.payload {
            RuntimePayload::Visual(visual) => {
                current_scene = visual.scene_id.clone();
                acks.push(SceneAck {
                    event_id: event.event_id.clone(),
                    scene_id: visual.scene_id.clone(),
                    shader_program: visual.shader_program.clone(),
                    bar: event.musical_time.bar,
                    status: String::from("rendered"),
                });
            }
            RuntimePayload::Timing(timing) => {
                if timing.downbeat && !current_scene.is_empty() && current_scene != "none" {
                    acks.push(SceneAck {
                        event_id: event.event_id.clone(),
                        scene_id: current_scene.clone(),
                        shader_program: String::from("timing-sync"),
                        bar: event.musical_time.bar,
                        status: String::from("synced"),
                    });
                }
            }
            _ => {}
        }
    }
    acks
}

fn required_arg(args: &[String], flag: &str) -> Result<String, String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .map(|value| value.to_string())
        .ok_or_else(|| format!("missing required argument: {flag}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{MusicalTime, RuntimePayload, TimingEvent, VisualEvent};

    fn make_visual_event(event_id: &str, scene_id: &str, bar: u32) -> EventRecord {
        EventRecord {
            event_id: event_id.into(),
            show_id: "test-show".into(),
            revision: 1,
            kind: "visual.scene.enter".into(),
            phase: "executed".into(),
            source: "scheduler".into(),
            musical_time: MusicalTime::at_bar(bar, 1, "intro", 128.0),
            scheduler_time_ms: 0,
            wallclock_time_ms: 0,
            causation_id: "cause".into(),
            payload: RuntimePayload::Visual(VisualEvent {
                scene_id: scene_id.into(),
                shader_program: "shader-a".into(),
                output_backend: "fake".into(),
                view_group: None,
                display_topology: None,
                calibration_profile: Some("default".into()),
                uniforms: std::collections::BTreeMap::new(),
                views: Vec::new(),
                duration_beats: Some(8),
                blend: None,
            }),
            ack: None,
        }
    }

    fn make_timing_event(event_id: &str, bar: u32, downbeat: bool) -> EventRecord {
        EventRecord {
            event_id: event_id.into(),
            show_id: "test-show".into(),
            revision: 1,
            kind: "timing.section.enter".into(),
            phase: "executed".into(),
            source: "scheduler".into(),
            musical_time: MusicalTime::at_bar(bar, 1, "intro", 128.0),
            scheduler_time_ms: 0,
            wallclock_time_ms: 0,
            causation_id: "cause".into(),
            payload: RuntimePayload::Timing(TimingEvent {
                phrase: 1,
                section: "intro".into(),
                tempo: 128.0,
                downbeat,
                bar: Some(bar),
                beat: Some(1.0),
                time_signature: Some([4, 4]),
                transition_window_open: Some(false),
            }),
            ack: None,
        }
    }

    #[test]
    fn processes_visual_events_into_acks() {
        let events = vec![
            make_visual_event("evt-01", "scene_intro", 1),
            make_visual_event("evt-02", "scene_drop", 9),
        ];
        let acks = process_events(&events);
        assert_eq!(acks.len(), 2);
        assert_eq!(acks[0].scene_id, "scene_intro");
        assert_eq!(acks[0].status, "rendered");
        assert_eq!(acks[1].scene_id, "scene_drop");
    }

    #[test]
    fn timing_events_produce_sync_acks_after_scene() {
        let events = vec![
            make_visual_event("evt-01", "scene_intro", 1),
            make_timing_event("evt-02", 5, true),
        ];
        let acks = process_events(&events);
        assert_eq!(acks.len(), 2);
        assert_eq!(acks[1].status, "synced");
        assert_eq!(acks[1].scene_id, "scene_intro");
    }

    #[test]
    fn timing_before_any_scene_does_not_produce_sync() {
        let events = vec![make_timing_event("evt-01", 1, true)];
        let acks = process_events(&events);
        assert!(acks.is_empty());
    }
}
