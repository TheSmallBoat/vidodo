use std::env;
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use vidodo_ir::{EventRecord, RuntimePayload};
use vidodo_storage::{ArtifactLayout, discover_repo_root, write_json};
use vidodo_trace::load_events;

mod lighting_backend;
pub use lighting_backend::LightingReferenceBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CueAck {
    event_id: String,
    cue_set_id: String,
    source_ref: String,
    bar: u32,
    status: String,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("lighting-runtime error: {message}");
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

    let output_path = layout.trace_dir(&run_id).join("lighting-acks.json");
    write_json(&output_path, &acks)?;

    let lighting_count =
        events.iter().filter(|e| matches!(&e.payload, RuntimePayload::Lighting(_))).count();

    println!(
        "{{\"status\":\"ok\",\"run_id\":\"{run_id}\",\"lighting_events_processed\":{lighting_count},\"acks_written\":{}}}",
        acks.len()
    );

    Ok(())
}

fn process_events(events: &[EventRecord]) -> Vec<CueAck> {
    let mut acks = Vec::new();
    let mut active_cue_set = String::from("none");

    for event in events {
        match &event.payload {
            RuntimePayload::Lighting(lighting) => {
                active_cue_set = lighting.cue_set_id.clone();
                acks.push(CueAck {
                    event_id: event.event_id.clone(),
                    cue_set_id: lighting.cue_set_id.clone(),
                    source_ref: lighting.source_ref.clone(),
                    bar: event.musical_time.bar,
                    status: String::from("cue_executed"),
                });
            }
            RuntimePayload::Timing(timing) => {
                if timing.downbeat && !active_cue_set.is_empty() && active_cue_set != "none" {
                    acks.push(CueAck {
                        event_id: event.event_id.clone(),
                        cue_set_id: active_cue_set.clone(),
                        source_ref: String::from("timing-sync"),
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

fn required_arg(args: &[String], name: &str) -> Result<String, String> {
    for i in 0..args.len() {
        if args[i] == name {
            if i + 1 < args.len() {
                return Ok(args[i + 1].clone());
            }
            return Err(format!("missing value for {name}"));
        }
    }
    Err(format!("missing required argument: {name}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{LightingEvent, MusicalTime, TimingEvent};

    fn make_lighting_event(
        event_id: &str,
        bar: u32,
        cue_set_id: &str,
        source_ref: &str,
    ) -> EventRecord {
        EventRecord {
            event_id: String::from(event_id),
            show_id: String::from("show-test"),
            revision: 1,
            kind: String::from("Lighting"),
            phase: String::from("steady"),
            source: String::from("test"),
            musical_time: MusicalTime::at_bar(bar, 1, "A", 120.0),
            scheduler_time_ms: 0,
            wallclock_time_ms: 0,
            causation_id: String::from("cause-01"),
            payload: RuntimePayload::Lighting(LightingEvent {
                cue_set_id: String::from(cue_set_id),
                source_ref: String::from(source_ref),
                fixture_group: vec![String::from("fx-01")],
                intensity: Some(0.8),
                color: None,
                fade_beats: Some(2.0),
            }),
            ack: None,
        }
    }

    fn make_timing_event(event_id: &str, bar: u32, downbeat: bool) -> EventRecord {
        EventRecord {
            event_id: String::from(event_id),
            show_id: String::from("show-test"),
            revision: 1,
            kind: String::from("Timing"),
            phase: String::from("steady"),
            source: String::from("test"),
            musical_time: MusicalTime::at_bar(bar, 1, "A", 120.0),
            scheduler_time_ms: 0,
            wallclock_time_ms: 0,
            causation_id: String::from("cause-02"),
            payload: RuntimePayload::Timing(TimingEvent {
                downbeat,
                phrase: 1,
                section: String::from("A"),
                tempo: 120.0,
                bar: Some(bar),
                beat: None,
                time_signature: Some([4, 4]),
                transition_window_open: None,
            }),
            ack: None,
        }
    }

    #[test]
    fn lighting_event_produces_cue_executed_ack() {
        let events = vec![make_lighting_event("evt-1", 1, "cue-set-a", "scene/intro")];
        let acks = process_events(&events);
        assert_eq!(acks.len(), 1);
        assert_eq!(acks[0].status, "cue_executed");
        assert_eq!(acks[0].cue_set_id, "cue-set-a");
        assert_eq!(acks[0].source_ref, "scene/intro");
    }

    #[test]
    fn timing_downbeat_after_lighting_produces_synced_ack() {
        let events = vec![
            make_lighting_event("evt-1", 1, "cue-set-a", "scene/drop"),
            make_timing_event("evt-2", 2, true),
        ];
        let acks = process_events(&events);
        assert_eq!(acks.len(), 2);
        assert_eq!(acks[0].status, "cue_executed");
        assert_eq!(acks[1].status, "synced");
        assert_eq!(acks[1].cue_set_id, "cue-set-a");
    }

    #[test]
    fn timing_downbeat_without_active_cue_produces_no_ack() {
        let events = vec![make_timing_event("evt-1", 1, true)];
        let acks = process_events(&events);
        assert!(acks.is_empty());
    }

    #[test]
    fn non_downbeat_timing_produces_no_ack() {
        let events = vec![
            make_lighting_event("evt-1", 1, "cue-a", "scene/intro"),
            make_timing_event("evt-2", 1, false), // not a downbeat
        ];
        let acks = process_events(&events);
        assert_eq!(acks.len(), 1); // only the lighting cue_executed
    }
}
