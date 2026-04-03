use std::fs;
use std::io::Write;
use std::path::PathBuf;

use vidodo_ir::{
    CompiledRevision, EventRecord, ExportArtifactRecord, PatchDecision, ResourceSample, RunSummary,
    ShowState, TraceManifest,
};
use vidodo_storage::{ArtifactLayout, read_json, read_jsonl, slug, write_json, write_jsonl};

#[allow(clippy::too_many_arguments)]
pub fn write_trace(
    layout: &ArtifactLayout,
    run_id: &str,
    compiled: &CompiledRevision,
    mode: &str,
    summary: &RunSummary,
    final_show_state: &ShowState,
    events: &[EventRecord],
    patch_decisions: &[PatchDecision],
    resource_samples: &[ResourceSample],
) -> Result<TraceManifest, String> {
    layout.ensure()?;
    let trace_dir = layout.trace_dir(run_id);
    let events_path = trace_dir.join("events.jsonl");
    let manifest_path = trace_dir.join("manifest.json");
    let summary_path = trace_dir.join("summary.json");
    let show_state_path = trace_dir.join("show-state.json");
    let patch_decisions_path = trace_dir.join("patch-decisions.jsonl");
    let resource_samples_path = trace_dir.join("resource-samples.jsonl");

    write_jsonl(&events_path, events)?;
    write_json(&summary_path, summary)?;
    write_json(&show_state_path, final_show_state)?;
    write_jsonl(&patch_decisions_path, patch_decisions)?;
    write_jsonl(&resource_samples_path, resource_samples)?;

    let slug_run = slug(run_id);
    let trace_manifest = TraceManifest {
        trace_bundle_id: format!("trace-{slug_run}"),
        show_id: compiled.show_id.clone(),
        revision: compiled.revision,
        run_id: run_id.to_string(),
        mode: mode.to_string(),
        started_at: Some(format!("simulated:{run_id}:start")),
        completed_at: Some(format!("simulated:{run_id}:end")),
        status: String::from("completed"),
        input_refs: vec![format!(
            "artifacts/revisions/{}/revision-{}",
            slug(&compiled.show_id),
            compiled.revision
        )],
        event_log_ref: Some(format!("artifacts/traces/{slug_run}/events.jsonl")),
        metrics_ref: Some(format!("artifacts/traces/{slug_run}/summary.json")),
        evaluation_ref: None,
        patch_decisions_ref: if patch_decisions.is_empty() {
            None
        } else {
            Some(format!("artifacts/traces/{slug_run}/patch-decisions.jsonl"))
        },
        resource_samples_ref: Some(format!("artifacts/traces/{slug_run}/resource-samples.jsonl")),
        export_ref: None,
    };
    write_json(&manifest_path, &trace_manifest)?;

    Ok(trace_manifest)
}

pub fn load_manifest(layout: &ArtifactLayout, run_id: &str) -> Result<TraceManifest, String> {
    read_json(&layout.trace_dir(run_id).join("manifest.json"))
}

pub fn load_events(layout: &ArtifactLayout, run_id: &str) -> Result<Vec<EventRecord>, String> {
    read_jsonl(&layout.trace_dir(run_id).join("events.jsonl"))
}

/// Filter events to those whose `musical_time.bar` falls within `[from_bar, to_bar]` (inclusive).
pub fn filter_events_by_bar(
    events: &[EventRecord],
    from_bar: u32,
    to_bar: u32,
) -> Vec<EventRecord> {
    events
        .iter()
        .filter(|event| event.musical_time.bar >= from_bar && event.musical_time.bar <= to_bar)
        .cloned()
        .collect()
}

pub fn manifest_path(layout: &ArtifactLayout, run_id: &str) -> PathBuf {
    layout.trace_dir(run_id).join("manifest.json")
}

/// Append degrade event records to an existing trace's events.jsonl file.
///
/// The events are serialized as individual JSON lines and appended to the
/// existing file.  If the trace directory or events file does not exist,
/// an error is returned.
pub fn append_degrade_events(
    layout: &ArtifactLayout,
    run_id: &str,
    events: &[EventRecord],
) -> Result<(), String> {
    let events_path = layout.trace_dir(run_id).join("events.jsonl");
    if !events_path.exists() {
        return Err(format!("events.jsonl not found for run '{run_id}'"));
    }
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&events_path)
        .map_err(|e| format!("failed to open events.jsonl for append: {e}"))?;
    for event in events {
        let line =
            serde_json::to_string(event).map_err(|e| format!("failed to serialize event: {e}"))?;
        writeln!(file, "{line}").map_err(|e| format!("failed to write event: {e}"))?;
    }
    Ok(())
}

/// Generate a minimal deterministic WAV file representing the offline mix.
///
/// Produces a 16-bit mono PCM silence WAV whose duration matches the show
/// length (derived from bar count and tempo).  The file is written to
/// `artifacts/exports/{run-slug}/mix.wav` and an `ExportArtifactRecord` is
/// persisted alongside it.  The trace manifest is updated with an `export_ref`.
pub fn export_audio(
    layout: &ArtifactLayout,
    run_id: &str,
    _show_id: &str,
    revision: u64,
    final_bar: u32,
    tempo: f64,
) -> Result<ExportArtifactRecord, String> {
    layout.ensure()?;
    let slug_run = slug(run_id);
    let export_dir = layout.exports.join(&slug_run);
    fs::create_dir_all(&export_dir).map_err(|e| format!("failed to create export dir: {e}"))?;

    // Duration in seconds: bars * beats_per_bar / tempo * 60
    let beats_per_bar = 4_u32;
    let duration_sec = ((final_bar as f64) * (beats_per_bar as f64) / tempo * 60.0).ceil() as u32;
    let sample_rate: u32 = 44_100;
    let bits_per_sample: u16 = 16;
    let channels: u16 = 1;
    let wav_bytes = generate_silence_wav(sample_rate, bits_per_sample, channels, duration_sec);

    let wav_path = export_dir.join("mix.wav");
    fs::write(&wav_path, &wav_bytes).map_err(|e| format!("failed to write WAV: {e}"))?;

    let content_hash = deterministic_hash(&wav_bytes);

    let record = ExportArtifactRecord {
        artifact_id: format!("export-audio-{slug_run}"),
        artifact_type: String::from("audio/wav"),
        locator: format!("artifacts/exports/{slug_run}/mix.wav"),
        content_hash,
        derived_from_run_id: run_id.to_string(),
        revision,
        duration_sec: Some(duration_sec),
    };

    let record_path = export_dir.join("export-record.json");
    write_json(&record_path, &record)?;

    // Update trace manifest with export_ref
    let manifest_file = layout.trace_dir(run_id).join("manifest.json");
    if manifest_file.exists() {
        let mut manifest: TraceManifest = read_json(&manifest_file)?;
        manifest.export_ref = Some(record.locator.clone());
        write_json(&manifest_file, &manifest)?;
    }

    Ok(record)
}

/// Generate a minimal RIFF/WAVE file with silence (PCM 16-bit).
fn generate_silence_wav(
    sample_rate: u32,
    bits_per_sample: u16,
    channels: u16,
    duration_sec: u32,
) -> Vec<u8> {
    let byte_rate = sample_rate * (channels as u32) * (bits_per_sample as u32) / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = byte_rate * duration_sec;
    let file_size = 36 + data_size; // total - 8 bytes for RIFF header

    let mut buf = Vec::with_capacity(44 + data_size as usize);
    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt  sub-chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16_u32.to_le_bytes()); // sub-chunk size
    buf.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    // data sub-chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    buf.resize(44 + data_size as usize, 0); // silence
    buf
}

/// Simple deterministic hash (FNV-1a 64-bit) for content verification.
fn deterministic_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_compiler::compile_plan;
    use vidodo_ir::PlanBundle;
    use vidodo_scheduler::simulate_run;

    #[test]
    fn generate_silence_wav_has_valid_header() {
        let wav = generate_silence_wav(44_100, 16, 1, 1);
        // RIFF header
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        // fmt  sub-chunk
        assert_eq!(&wav[12..16], b"fmt ");
        // data sub-chunk
        assert_eq!(&wav[36..40], b"data");
        // 1 second of 44100 Hz, 16-bit mono = 88200 bytes of data
        let data_size = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]);
        assert_eq!(data_size, 88_200);
        // Total file size: 44 header + 88200 data
        assert_eq!(wav.len(), 44 + 88_200);
    }

    #[test]
    fn generate_silence_wav_is_deterministic() {
        let a = generate_silence_wav(44_100, 16, 1, 2);
        let b = generate_silence_wav(44_100, 16, 1, 2);
        assert_eq!(a, b);
    }

    #[test]
    fn deterministic_hash_is_stable() {
        let h1 = deterministic_hash(b"hello");
        let h2 = deterministic_hash(b"hello");
        assert_eq!(h1, h2);
        // Different input → different hash
        let h3 = deterministic_hash(b"world");
        assert_ne!(h1, h3);
    }

    #[test]
    fn export_audio_creates_wav_and_record() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = ArtifactLayout::new(tmp.path());
        layout.ensure().expect("ensure layout");

        // compile + simulate to get a real trace
        let compiled = compile_plan(&PlanBundle::minimal("show-export-test")).expect("compile");
        let run = simulate_run(&compiled, "test-run-export");

        write_trace(
            &layout,
            "test-run-export",
            &compiled,
            "offline",
            &run.summary,
            &run.final_show_state,
            &run.events,
            &run.patch_decisions,
            &run.resource_samples,
        )
        .expect("write_trace");

        let record = export_audio(
            &layout,
            "test-run-export",
            &compiled.show_id,
            compiled.revision,
            compiled.final_bar(),
            128.0,
        )
        .expect("export_audio");

        assert_eq!(record.artifact_type, "audio/wav");
        assert!(record.duration_sec.is_some());
        assert!(!record.content_hash.is_empty());

        let slug_run = slug("test-run-export");
        let wav_path = layout.exports.join(&slug_run).join("mix.wav");
        assert!(wav_path.exists(), "WAV file must exist");

        let record_path = layout.exports.join(&slug_run).join("export-record.json");
        assert!(record_path.exists(), "export-record.json must exist");

        // Trace manifest should now have export_ref
        let manifest: TraceManifest =
            read_json(&layout.trace_dir("test-run-export").join("manifest.json"))
                .expect("read manifest");
        assert!(manifest.export_ref.is_some());
        assert!(manifest.export_ref.unwrap().contains("mix.wav"));
    }

    #[test]
    fn filter_events_by_bar_selects_range() {
        let compiled = compile_plan(&PlanBundle::minimal("show-filter")).expect("compile");
        let run = simulate_run(&compiled, "run-filter");
        // Ensure we have events spanning multiple bars
        assert!(!run.events.is_empty());
        let all_bars: Vec<u32> = run.events.iter().map(|e| e.musical_time.bar).collect();
        let min_bar = *all_bars.iter().min().unwrap();
        let max_bar = *all_bars.iter().max().unwrap();
        assert!(max_bar > min_bar, "need events across multiple bars");

        // Filter a subset
        let filtered = filter_events_by_bar(&run.events, min_bar, min_bar);
        assert!(!filtered.is_empty());
        assert!(filtered.iter().all(|e| e.musical_time.bar == min_bar));

        // Filtering beyond range returns empty
        let empty = filter_events_by_bar(&run.events, max_bar + 100, max_bar + 200);
        assert!(empty.is_empty());
    }

    #[test]
    fn append_degrade_events_adds_to_trace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = ArtifactLayout::new(tmp.path());
        layout.ensure().expect("ensure layout");

        let compiled = compile_plan(&PlanBundle::minimal("show-degrade")).expect("compile");
        let run = simulate_run(&compiled, "run-degrade");
        let initial_count = run.events.len();

        write_trace(
            &layout,
            "run-degrade",
            &compiled,
            "offline",
            &run.summary,
            &run.final_show_state,
            &run.events,
            &run.patch_decisions,
            &run.resource_samples,
        )
        .expect("write_trace");

        // Append a degrade event
        let degrade_event = EventRecord {
            event_id: String::from("evt-degrade-001"),
            show_id: compiled.show_id.clone(),
            revision: compiled.revision,
            kind: String::from("degrade"),
            phase: String::from("runtime"),
            source: String::from("health_monitor"),
            musical_time: vidodo_ir::MusicalTime::at_bar(1, 1, "intro", 128.0),
            scheduler_time_ms: 0,
            wallclock_time_ms: 0,
            causation_id: String::from("health-check-001"),
            payload: vidodo_ir::RuntimePayload::Degrade(vidodo_ir::DegradeEvent {
                degrade_id: String::from("deg-001"),
                mode: String::from("degrade_audio"),
                reason: String::from("backend offline"),
                affected_backends: vec![String::from("audio-1")],
                fallback_action: Some(String::from("bypass_audio")),
            }),
            ack: None,
        };

        append_degrade_events(&layout, "run-degrade", &[degrade_event]).expect("append");

        // Reload and verify
        let all_events = load_events(&layout, "run-degrade").expect("load");
        assert_eq!(all_events.len(), initial_count + 1);
        let last = all_events.last().unwrap();
        assert_eq!(last.kind, "degrade");
        assert!(matches!(last.payload, vidodo_ir::RuntimePayload::Degrade(_)));
    }
}
