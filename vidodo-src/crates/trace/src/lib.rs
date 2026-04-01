use std::path::PathBuf;

use vidodo_ir::{CompiledRevision, EventRecord, RunSummary, ShowState, TraceManifest};
use vidodo_storage::{ArtifactLayout, read_json, read_jsonl, slug, write_json, write_jsonl};

pub fn write_trace(
    layout: &ArtifactLayout,
    run_id: &str,
    compiled: &CompiledRevision,
    mode: &str,
    summary: &RunSummary,
    final_show_state: &ShowState,
    events: &[EventRecord],
) -> Result<TraceManifest, String> {
    layout.ensure()?;
    let trace_dir = layout.trace_dir(run_id);
    let events_path = trace_dir.join("events.jsonl");
    let manifest_path = trace_dir.join("manifest.json");
    let summary_path = trace_dir.join("summary.json");
    let show_state_path = trace_dir.join("show-state.json");

    write_jsonl(&events_path, events)?;
    write_json(&summary_path, summary)?;
    write_json(&show_state_path, final_show_state)?;

    let trace_manifest = TraceManifest {
        trace_bundle_id: format!("trace-{}", slug(run_id)),
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
        event_log_ref: Some(format!("artifacts/traces/{}/events.jsonl", slug(run_id))),
        metrics_ref: Some(format!("artifacts/traces/{}/summary.json", slug(run_id))),
        evaluation_ref: None,
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

pub fn manifest_path(layout: &ArtifactLayout, run_id: &str) -> PathBuf {
    layout.trace_dir(run_id).join("manifest.json")
}
