use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::{fs, process};

use serde_json::{Value, json};
use vidodo_adapter_registry::loader::load_adapters;
use vidodo_adapter_registry::persistence::PersistentAdapterRegistry;
use vidodo_capability::{
    CapabilityRegistry, RouteTarget, mcp_tool_mappings, resolve_mcp_tool, route,
};
use vidodo_compiler::compile_plan;
use vidodo_compiler::revision::{archive_revision, publish_revision};
use vidodo_evaluation::evaluate_run;
use vidodo_ir::{
    AssetRecord, AudioDsl, CompiledRevision, ConstraintSet, CueSet, Diagnostic, LightingTopology,
    LivePatchProposal, PatchDecision, PlanBundle, ResponseEnvelope, SetPlan, VisualDsl,
};
use vidodo_patch_manager::{apply_patch, check_patch, deferred_rollback, rollback_patch};
use vidodo_resource_hub::persistence::PersistentHubRegistry;
use vidodo_scheduler::{RunStatusRecord, simulate_run};
use vidodo_storage::{
    ArtifactLayout, AssetIngestRequest, AssetQuery, discover_repo_root, get_asset, ingest_assets,
    list_asset_analysis, list_asset_jobs, list_assets, list_compile_assets, read_json, slug,
    write_json,
};
use vidodo_trace::{
    append_degrade_events, export_audio, filter_events_by_bar, load_events, load_manifest,
    manifest_path, write_trace,
};
use vidodo_validator::validate_plan;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct McpState {
    registry: CapabilityRegistry,
    layout: ArtifactLayout,
    repo_root: PathBuf,
}

// ---------------------------------------------------------------------------
// Entry point — stdio JSON-RPC loop
// ---------------------------------------------------------------------------

fn main() {
    let repo_root = match discover_repo_root() {
        Ok(root) => root,
        Err(msg) => {
            eprintln!("mcp-adapter: cannot find repo root: {msg}");
            process::exit(1);
        }
    };
    let layout = ArtifactLayout::new(repo_root.join("artifacts"));
    if let Err(msg) = layout.ensure() {
        eprintln!("mcp-adapter: cannot initialise artifact store: {msg}");
        process::exit(1);
    }

    let state = McpState { registry: CapabilityRegistry::default(), layout, repo_root };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                write_jsonrpc_error(&mut out, Value::Null, -32700, &format!("parse error: {e}"));
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));

        let response = handle_method(&state, method, &params);
        let jsonrpc = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": response
        });
        let _ = writeln!(out, "{}", serde_json::to_string(&jsonrpc).unwrap_or_default());
        let _ = out.flush();
    }
}

// ---------------------------------------------------------------------------
// Method dispatch
// ---------------------------------------------------------------------------

fn handle_method(state: &McpState, method: &str, params: &Value) -> Value {
    match method {
        "initialize" => handle_initialize(),
        "notifications/initialized" => json!({}),
        "tools/list" => handle_tools_list(state),
        "tools/call" => handle_tools_call(state, params),
        _ => json!({"error": format!("unknown method: {method}")}),
    }
}

fn handle_initialize() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "vidodo-mcp-adapter",
            "version": "0.1"
        }
    })
}

fn handle_tools_list(state: &McpState) -> Value {
    let mappings = mcp_tool_mappings();
    let tools: Vec<Value> = mappings
        .iter()
        .filter_map(|m| {
            let descriptor = state.registry.lookup(&m.capability)?;
            let input_schema: Value = serde_json::from_str(&descriptor.input_schema)
                .unwrap_or_else(|_| json!({"type": "object"}));
            Some(json!({
                "name": m.tool_name,
                "description": descriptor.description,
                "inputSchema": input_schema,
                "annotations": {
                    "readOnlyHint": m.read_only,
                    "idempotency": descriptor.idempotency,
                    "async": m.is_async,
                    "authorization": descriptor.authorization.first().unwrap_or(&String::new())
                }
            }))
        })
        .collect();
    json!({ "tools": tools })
}

fn handle_tools_call(state: &McpState, params: &Value) -> Value {
    let tool_name = match params.get("name").and_then(Value::as_str) {
        Some(name) => name,
        None => {
            return json!({"content": [{"type": "text", "text": "missing tool name"}], "isError": true});
        }
    };
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let capability = match resolve_mcp_tool(tool_name) {
        Some(cap) => cap,
        None => {
            return json!({"content": [{"type": "text", "text": format!("unknown tool: {tool_name}")}], "isError": true});
        }
    };

    let target = match route(capability) {
        Ok(t) => t,
        Err(diagnostic) => {
            let msg = serde_json::to_string(&*diagnostic).unwrap_or_default();
            return json!({"content": [{"type": "text", "text": msg}], "isError": true});
        }
    };

    let request_id =
        arguments.get("request_id").and_then(Value::as_str).unwrap_or("mcp-request").to_string();

    match dispatch(state, target, capability, &request_id, &arguments) {
        Ok(envelope) => {
            let text = serde_json::to_string_pretty(&envelope).unwrap_or_default();
            json!({"content": [{"type": "text", "text": text}], "isError": false})
        }
        Err(envelope) => {
            let text = serde_json::to_string_pretty(&envelope).unwrap_or_default();
            json!({"content": [{"type": "text", "text": text}], "isError": true})
        }
    }
}

fn write_jsonrpc_error(out: &mut impl Write, id: Value, code: i32, message: &str) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message}
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&response).unwrap_or_default());
    let _ = out.flush();
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

fn dispatch(
    state: &McpState,
    target: RouteTarget,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    match target {
        RouteTarget::AssetIngest => dispatch_asset_ingest(state, capability, request_id, body),
        RouteTarget::AssetList => dispatch_asset_list(state, capability, request_id, body),
        RouteTarget::AssetShow => dispatch_asset_show(state, capability, request_id, body),
        RouteTarget::PlanValidate => dispatch_plan_validate(state, capability, request_id, body),
        RouteTarget::PlanSubmit => dispatch_plan_submit(state, capability, request_id, body),
        RouteTarget::CompileRun => dispatch_compile_run(state, capability, request_id, body),
        RouteTarget::RevisionList => dispatch_revision_list(state, capability, request_id, body),
        RouteTarget::RevisionPublish => {
            dispatch_revision_publish(state, capability, request_id, body)
        }
        RouteTarget::RevisionArchive => {
            dispatch_revision_archive(state, capability, request_id, body)
        }
        RouteTarget::RunStart => dispatch_run_start(state, capability, request_id, body),
        RouteTarget::RunStatus => dispatch_run_status(state, capability, request_id, body),
        RouteTarget::PatchCheck => dispatch_patch_check(state, capability, request_id, body),
        RouteTarget::PatchSubmit => dispatch_patch_submit(state, capability, request_id, body),
        RouteTarget::PatchRollback => dispatch_patch_rollback(state, capability, request_id, body),
        RouteTarget::PatchDeferredRollback => {
            dispatch_patch_deferred_rollback(state, capability, request_id, body)
        }
        RouteTarget::TraceShow => dispatch_trace_show(state, capability, request_id, body),
        RouteTarget::TraceEvents => dispatch_trace_events(state, capability, request_id, body),
        RouteTarget::EvalRun => dispatch_eval_run(state, capability, request_id, body),
        RouteTarget::ExportAudio => dispatch_export_audio(state, capability, request_id, body),
        RouteTarget::SystemDoctor => {
            Ok(ok_envelope(capability, request_id, json!({"status": "healthy"})))
        }
        RouteTarget::SystemCapabilities => {
            let list: Vec<_> = state
                .registry
                .list()
                .iter()
                .map(|d| {
                    json!({
                        "capability": d.capability,
                        "version": d.version,
                        "execution_mode": d.execution_mode,
                        "idempotency": d.idempotency,
                        "authorization": d.authorization,
                        "description": d.description,
                        "input_schema": d.input_schema,
                        "output_schema": d.output_schema
                    })
                })
                .collect();
            Ok(ok_envelope(
                capability,
                request_id,
                json!({"count": list.len(), "capabilities": list}),
            ))
        }
        RouteTarget::SystemAdapters => {
            let db_path = state.layout.root.join("adapters.db");
            match PersistentAdapterRegistry::open(&db_path) {
                Ok(registry) => {
                    let list = registry.list().unwrap_or_default();
                    Ok(ok_envelope(
                        capability,
                        request_id,
                        json!({"count": list.len(), "adapters": list}),
                    ))
                }
                Err(e) => Err(err_str(capability, request_id, &e)),
            }
        }
        RouteTarget::SystemHubs => {
            let db_path = state.layout.root.join("hubs.db");
            match PersistentHubRegistry::open(&db_path) {
                Ok(registry) => {
                    let list = registry.list_hubs().unwrap_or_default();
                    Ok(ok_envelope(
                        capability,
                        request_id,
                        json!({"count": list.len(), "hubs": list}),
                    ))
                }
                Err(e) => Err(err_str(capability, request_id, &e)),
            }
        }
        RouteTarget::AdapterLoad => dispatch_adapter_load(state, capability, request_id, body),
        RouteTarget::AdapterShutdown => {
            dispatch_adapter_shutdown(state, capability, request_id, body)
        }
        RouteTarget::AdapterStatus => dispatch_adapter_status(state, capability, request_id, body),
        RouteTarget::HubRegister => dispatch_hub_register(state, capability, request_id, body),
        RouteTarget::HubResolve => dispatch_hub_resolve(state, capability, request_id, body),
        RouteTarget::HubStatus => dispatch_hub_status(state, capability, request_id, body),
        RouteTarget::ControlBind => dispatch_control_bind(state, capability, request_id, body),
        RouteTarget::ControlUnbind => dispatch_control_unbind(state, capability, request_id, body),
        RouteTarget::ControlList => dispatch_control_list(state, capability, request_id, body),
        RouteTarget::ControlStatus => dispatch_control_status(state, capability, request_id, body),
        RouteTarget::TemplateList => dispatch_template_list(state, capability, request_id, body),
        RouteTarget::TemplateLoad => dispatch_template_load(state, capability, request_id, body),
        RouteTarget::SceneList => dispatch_scene_list(state, capability, request_id, body),
        RouteTarget::SceneActivate => dispatch_scene_activate(state, capability, request_id, body),
        RouteTarget::DemoList => dispatch_demo_list(state, capability, request_id, body),
        RouteTarget::DemoRun => dispatch_demo_run(state, capability, request_id, body),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn dispatch_asset_ingest(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let source_dir = require_str(body, "source_dir")?;
    let declared_kind = require_str(body, "declared_kind")?;
    let tags: Vec<String> = body
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();
    let asset_namespace = body.get("asset_namespace").and_then(Value::as_str).map(String::from);
    let source_path = resolve(&state.repo_root, &source_dir);
    let report = ingest_assets(
        &state.layout,
        &AssetIngestRequest {
            source: source_path,
            declared_kind,
            tags,
            asset_namespace: asset_namespace.clone(),
            asset_id_overrides: BTreeMap::new(),
        },
    )
    .map_err(|diags| error_envelope(capability, request_id, diags))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "ingestion_run_id": report.run.ingestion_run_id,
            "discovered": report.run.discovered,
            "published": report.run.published,
            "assets": report.assets.iter().map(|a| &a.asset_id).collect::<Vec<_>>()
        }),
    ))
}

fn dispatch_asset_list(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let query = AssetQuery {
        asset_kind: body.get("kind").and_then(Value::as_str).map(String::from),
        tag: body.get("tag").and_then(Value::as_str).map(String::from),
    };
    let assets =
        list_assets(&state.layout, &query).map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(capability, request_id, json!({"count": assets.len(), "assets": assets})))
}

fn dispatch_asset_show(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let asset_id = require_str(body, "asset_id")?;
    let asset = get_asset(&state.layout, &asset_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?
        .ok_or_else(|| err_str(capability, request_id, &format!("asset {asset_id} not found")))?;
    let analysis = list_asset_analysis(&state.layout, &asset_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let jobs = list_asset_jobs(&state.layout, &asset_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"asset": asset, "analysis_entries": analysis, "analysis_jobs": jobs}),
    ))
}

fn dispatch_plan_validate(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let plan =
        load_plan_from_body(state, body).map_err(|msg| err_str(capability, request_id, &msg))?;
    let diagnostics = validate_plan(&plan);
    let status = if diagnostics.iter().any(|d| d.severity == "error") { "error" } else { "ok" };
    Ok(envelope_value(
        status,
        capability,
        request_id,
        json!({
            "show_id": plan.show_id,
            "section_count": plan.set_plan.sections.len(),
            "audio_layer_count": plan.audio_dsl.layers.len(),
            "visual_scene_count": plan.visual_dsl.scenes.len()
        }),
        diagnostics,
        vec![],
        vec![],
    ))
}

fn dispatch_plan_submit(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let plan =
        load_plan_from_body(state, body).map_err(|msg| err_str(capability, request_id, &msg))?;
    let compiled =
        compile_plan(&plan).map_err(|diags| error_envelope(capability, request_id, diags))?;
    persist_revision(&state.layout, &compiled)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "show_id": compiled.show_id, "revision": compiled.revision,
            "compile_run_id": compiled.compile_run_id,
            "timeline_entries": compiled.timeline.len()
        }),
    ))
}

fn dispatch_compile_run(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let plan =
        load_plan_from_body(state, body).map_err(|msg| err_str(capability, request_id, &msg))?;
    let compiled =
        compile_plan(&plan).map_err(|diags| error_envelope(capability, request_id, diags))?;
    persist_revision(&state.layout, &compiled)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "show_id": compiled.show_id, "revision": compiled.revision,
            "compile_run_id": compiled.compile_run_id,
            "timeline_entries": compiled.timeline.len()
        }),
    ))
}

fn dispatch_revision_list(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let records = query_revisions(&state.layout, &show_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(capability, request_id, json!({"show_id": show_id, "revisions": records})))
}

fn dispatch_revision_publish(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let revision = require_u64(body, "revision")?;
    publish_revision(&state.layout, &show_id, revision)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"show_id": show_id, "revision": revision, "status": "published"}),
    ))
}

fn dispatch_revision_archive(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let revision = require_u64(body, "revision")?;
    archive_revision(&state.layout, &show_id, revision)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"show_id": show_id, "revision": revision, "status": "archived"}),
    ))
}

fn dispatch_run_start(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let revision = require_u64(body, "revision")?;
    let compiled = load_revision(&state.layout, &show_id, revision)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let run_id = format!("run-{}-rev-{revision}", slug(&show_id));
    let scheduled = simulate_run(&compiled, &run_id);
    let manifest = write_trace(
        &state.layout,
        &run_id,
        &compiled,
        "offline",
        &scheduled.summary,
        &scheduled.final_show_state,
        &scheduled.events,
        &scheduled.patch_decisions,
        &scheduled.resource_samples,
    )
    .map_err(|msg| err_str(capability, request_id, &msg))?;
    if !scheduled.degrade_events.is_empty() {
        append_degrade_events(&state.layout, &run_id, &scheduled.degrade_events)
            .map_err(|msg| err_str(capability, request_id, &msg))?;
    }
    let status_record = RunStatusRecord {
        show_id: show_id.clone(),
        run_id: run_id.clone(),
        revision,
        status: String::from("completed"),
        trace_manifest: manifest_path(&state.layout, &run_id).display().to_string(),
        summary: scheduled.summary.clone(),
        final_show_state: scheduled.final_show_state.clone(),
    };
    write_json(&state.layout.run_status_path(&show_id), &status_record)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "run_id": run_id, "show_id": show_id, "revision": revision,
            "event_count": scheduled.summary.event_count,
            "trace_bundle_id": manifest.trace_bundle_id
        }),
    ))
}

fn dispatch_run_status(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let status: RunStatusRecord = read_json(&state.layout.run_status_path(&show_id))
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(capability, request_id, serde_json::to_value(status).unwrap_or_default()))
}

fn dispatch_patch_check(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let patch: LivePatchProposal = serde_json::from_value(
        body.get("patch")
            .cloned()
            .ok_or_else(|| err_str(capability, request_id, "missing field: patch"))?,
    )
    .map_err(|e| err_str(capability, request_id, &e.to_string()))?;
    let revision = load_latest_revision(&state.layout, &show_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let diagnostics = check_patch(&revision, &patch);
    let status = if diagnostics.is_empty() { "ok" } else { "error" };
    Ok(envelope_value(
        status,
        capability,
        request_id,
        json!({"show_id": show_id, "base_revision": revision.revision, "patch_id": patch.patch_id}),
        diagnostics,
        vec![],
        vec![],
    ))
}

fn dispatch_patch_submit(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let patch: LivePatchProposal = serde_json::from_value(
        body.get("patch")
            .cloned()
            .ok_or_else(|| err_str(capability, request_id, "missing field: patch"))?,
    )
    .map_err(|e| err_str(capability, request_id, &e.to_string()))?;
    let revision = load_latest_revision(&state.layout, &show_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let patched = apply_patch(&revision, &patch)
        .map_err(|diags| error_envelope(capability, request_id, diags))?;
    persist_revision(&state.layout, &patched)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let decision = patched.patch_history.last().cloned();
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "show_id": show_id, "patch_id": patch.patch_id,
            "effective_revision": patched.revision,
            "fallback_revision": decision.map(|d| d.fallback_revision)
        }),
    ))
}

fn dispatch_patch_rollback(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let patch_id = require_str(body, "patch_id")?;
    let revision = load_latest_revision(&state.layout, &show_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let rollback = rollback_patch(&revision, &patch_id)
        .map_err(|d| error_envelope(capability, request_id, vec![*d]))?;
    let path =
        state.layout.revisions.join(slug(&show_id)).join(format!("rollback-{patch_id}.json"));
    write_json(&path, &rollback).map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "show_id": show_id, "patch_id": patch_id,
            "fallback_revision": rollback.fallback_revision
        }),
    ))
}

fn dispatch_patch_deferred_rollback(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let patch_id = require_str(body, "patch_id")?;
    let anomaly = require_str(body, "anomaly")?;
    let run_id = body.get("run_id").and_then(Value::as_str).map(String::from);
    let revision = load_latest_revision(&state.layout, &show_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let decision = deferred_rollback(&revision, &patch_id, &anomaly)
        .map_err(|d| error_envelope(capability, request_id, vec![*d]))?;
    let path = state
        .layout
        .revisions
        .join(slug(&show_id))
        .join(format!("deferred-rollback-{patch_id}.json"));
    write_json(&path, &decision).map_err(|msg| err_str(capability, request_id, &msg))?;
    if let Some(ref rid) = run_id {
        let trace_path = state.layout.trace_dir(rid).join("patch-decisions.jsonl");
        if trace_path.exists() {
            let mut existing: Vec<PatchDecision> =
                vidodo_storage::read_jsonl(&trace_path).unwrap_or_default();
            existing.push(decision.clone());
            let _ = vidodo_storage::write_jsonl(&trace_path, &existing);
        }
    }
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "show_id": show_id, "patch_id": patch_id,
            "decision": decision.decision,
            "fallback_revision": decision.fallback_revision,
            "anomaly": anomaly
        }),
    ))
}

fn dispatch_trace_show(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let run_id = require_str(body, "run_id")?;
    let manifest = load_manifest(&state.layout, &run_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(capability, request_id, serde_json::to_value(manifest).unwrap_or_default()))
}

fn dispatch_trace_events(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let run_id = require_str(body, "run_id")?;
    let all_events =
        load_events(&state.layout, &run_id).map_err(|msg| err_str(capability, request_id, &msg))?;
    let from_bar = body.get("from_bar").and_then(Value::as_u64).map(|v| v as u32);
    let to_bar = body.get("to_bar").and_then(Value::as_u64).map(|v| v as u32);
    let events = match (from_bar, to_bar) {
        (Some(f), Some(t)) => filter_events_by_bar(&all_events, f, t),
        (Some(f), None) => filter_events_by_bar(&all_events, f, u32::MAX),
        (None, Some(t)) => filter_events_by_bar(&all_events, 0, t),
        (None, None) => all_events,
    };
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"run_id": run_id, "event_count": events.len(), "events": events}),
    ))
}

fn dispatch_eval_run(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let show_id = require_str(body, "show_id")?;
    let run_id = match body.get("run_id").and_then(Value::as_str) {
        Some(id) => id.to_string(),
        None => {
            let s: RunStatusRecord = read_json(&state.layout.run_status_path(&show_id))
                .map_err(|msg| err_str(capability, request_id, &msg))?;
            s.run_id
        }
    };
    let status: RunStatusRecord = read_json(&state.layout.run_status_path(&show_id))
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let report = evaluate_run(&state.layout, &run_id, &status.summary, &status.final_show_state)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let eval_path = state.layout.trace_dir(&run_id).join("evaluation.json");
    write_json(&eval_path, &report).map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(capability, request_id, serde_json::to_value(&report).unwrap_or_default()))
}

fn dispatch_export_audio(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let run_id = require_str(body, "run_id")?;
    let manifest = load_manifest(&state.layout, &run_id)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let compiled = load_revision(&state.layout, &manifest.show_id, manifest.revision)
        .map_err(|msg| err_str(capability, request_id, &msg))?;
    let record = export_audio(
        &state.layout,
        &run_id,
        &manifest.show_id,
        manifest.revision,
        compiled.final_bar(),
        128.0,
    )
    .map_err(|msg| err_str(capability, request_id, &msg))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({
            "artifact_id": record.artifact_id, "locator": record.locator,
            "content_hash": record.content_hash, "duration_sec": record.duration_sec
        }),
    ))
}

// ---------------------------------------------------------------------------
// Plan loading
// ---------------------------------------------------------------------------

fn load_plan_from_body(state: &McpState, body: &Value) -> Result<PlanBundle, String> {
    if let Some(plan_dir) = body.get("plan_dir").and_then(Value::as_str) {
        let plan_path = resolve(&state.repo_root, plan_dir);
        let assets_path = match body.get("assets_file").and_then(Value::as_str) {
            Some(p) => resolve(&state.repo_root, p),
            None => default_assets_file(state)?,
        };
        load_plan_bundle(&plan_path, &assets_path)
    } else if body.get("plan").is_some() {
        let pv = body.get("plan").unwrap();
        let set_plan: SetPlan =
            serde_json::from_value(pv.get("set_plan").cloned().ok_or("missing plan.set_plan")?)
                .map_err(|e| e.to_string())?;
        let audio_dsl: AudioDsl =
            serde_json::from_value(pv.get("audio_dsl").cloned().ok_or("missing plan.audio_dsl")?)
                .map_err(|e| e.to_string())?;
        let visual_dsl: VisualDsl =
            serde_json::from_value(pv.get("visual_dsl").cloned().ok_or("missing plan.visual_dsl")?)
                .map_err(|e| e.to_string())?;
        let constraint_set: ConstraintSet = serde_json::from_value(
            pv.get("constraint_set").cloned().ok_or("missing plan.constraint_set")?,
        )
        .map_err(|e| e.to_string())?;
        let asset_records: Vec<AssetRecord> = serde_json::from_value(
            pv.get("asset_records").cloned().ok_or("missing plan.asset_records")?,
        )
        .map_err(|e| e.to_string())?;
        let lighting_topology: Option<LightingTopology> =
            pv.get("lighting_topology").and_then(|v| serde_json::from_value(v.clone()).ok());
        let cue_sets: Vec<CueSet> = pv
            .get("cue_sets")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let show_id = set_plan.show_id.clone();
        Ok(PlanBundle {
            show_id,
            base_revision: 0,
            set_plan,
            audio_dsl,
            visual_dsl,
            constraint_set,
            asset_records,
            lighting_topology,
            cue_sets,
        })
    } else {
        Err(String::from("provide either 'plan_dir' or 'plan' (inline object)"))
    }
}

fn load_plan_bundle(plan_dir: &Path, assets_file: &Path) -> Result<PlanBundle, String> {
    let set_plan: SetPlan = read_json(&plan_dir.join("set-plan.json"))?;
    let audio_dsl: AudioDsl = read_json(&plan_dir.join("audio-dsl.json"))?;
    let visual_dsl: VisualDsl = read_json(&plan_dir.join("visual-dsl.json"))?;
    let constraint_set: ConstraintSet = read_json(&plan_dir.join("constraint-set.json"))?;
    let asset_records: Vec<AssetRecord> = read_json(assets_file)?;
    let lt_path = plan_dir.join("lighting-topology.json");
    let lighting_topology: Option<LightingTopology> =
        if lt_path.exists() { Some(read_json(&lt_path)?) } else { None };
    let cs_path = plan_dir.join("cue-set.json");
    let cue_sets: Vec<CueSet> = if cs_path.exists() { read_json(&cs_path)? } else { Vec::new() };
    let show_id = set_plan.show_id.clone();
    Ok(PlanBundle {
        show_id,
        base_revision: 0,
        set_plan,
        audio_dsl,
        visual_dsl,
        constraint_set,
        asset_records,
        lighting_topology,
        cue_sets,
    })
}

// ---------------------------------------------------------------------------
// Revision helpers
// ---------------------------------------------------------------------------

fn load_revision(
    layout: &ArtifactLayout,
    show_id: &str,
    revision: u64,
) -> Result<CompiledRevision, String> {
    read_json(&layout.revision_dir(show_id, revision).join("revision.json"))
}

fn load_latest_revision(
    layout: &ArtifactLayout,
    show_id: &str,
) -> Result<CompiledRevision, String> {
    let show_root = layout.revisions.join(slug(show_id));
    let entries = fs::read_dir(&show_root)
        .map_err(|e| format!("failed to read {}: {e}", show_root.display()))?;
    let latest = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry.file_name().to_string_lossy().strip_prefix("revision-")?.parse::<u64>().ok()
        })
        .max()
        .ok_or_else(|| format!("no revision artifacts for show {show_id}"))?;
    load_revision(layout, show_id, latest)
}

fn persist_revision(layout: &ArtifactLayout, revision: &CompiledRevision) -> Result<(), String> {
    let show_root = layout.revisions.join(slug(&revision.show_id));
    if revision.revision == 1 && show_root.exists() {
        fs::remove_dir_all(&show_root)
            .map_err(|e| format!("failed to reset {}: {e}", show_root.display()))?;
    }
    let dir = layout.revision_dir(&revision.show_id, revision.revision);
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    write_json(&dir.join("revision.json"), revision)?;
    write_json(&dir.join("set-plan.json"), &revision.set_plan)?;
    write_json(&dir.join("audio-dsl.json"), &revision.audio_dsl)?;
    write_json(&dir.join("visual-dsl.json"), &revision.visual_dsl)?;
    write_json(&dir.join("constraint-set.json"), &revision.constraint_set)?;
    write_json(&dir.join("asset-records.json"), &revision.asset_records)?;
    write_json(&dir.join("structure-ir.json"), &revision.structure_ir)?;
    write_json(&dir.join("performance-ir.json"), &revision.performance_ir)?;
    write_json(&dir.join("visual-ir.json"), &revision.visual_ir)?;
    write_json(&dir.join("timeline.json"), &revision.timeline)?;
    if let Some(decision) = revision.patch_history.last() {
        write_json(&dir.join("patch-decision.json"), decision)?;
    }
    let _ = vidodo_compiler::revision::register_candidate(layout, revision);
    Ok(())
}

fn query_revisions(layout: &ArtifactLayout, show_id: &str) -> Result<Vec<Value>, String> {
    let show_root = layout.revisions.join(slug(show_id));
    let entries = fs::read_dir(&show_root)
        .map_err(|e| format!("failed to read {}: {e}", show_root.display()))?;
    let mut revisions: Vec<u64> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry.file_name().to_string_lossy().strip_prefix("revision-")?.parse::<u64>().ok()
        })
        .collect();
    revisions.sort();
    Ok(revisions.iter().map(|r| json!({"revision": r})).collect())
}

fn default_assets_file(state: &McpState) -> Result<PathBuf, String> {
    let selection = list_compile_assets(&state.layout)?;
    if selection.published_asset_count == 0 {
        return Ok(state.repo_root.join("tests/fixtures/assets/asset-records.json"));
    }
    if selection.eligible_assets.is_empty() {
        return Err(String::from("asset registry has published assets but none are compile-ready"));
    }
    let path = state.layout.exports.join("compile-ready-asset-records.json");
    write_json(&path, &selection.eligible_assets)?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// WSR-01 — adapter lifecycle handlers
// ---------------------------------------------------------------------------

fn dispatch_adapter_load(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let manifest_path = require_str(body, "manifest_path")?;
    let path = resolve(&state.repo_root, &manifest_path);
    let raw = fs::read_to_string(&path)
        .map_err(|e| err_str(capability, request_id, &format!("cannot read manifest: {e}")))?;
    let manifests: Vec<vidodo_ir::AdapterPluginManifest> = serde_json::from_str(&raw)
        .map_err(|e| err_str(capability, request_id, &format!("invalid manifest JSON: {e}")))?;
    let loaded =
        load_adapters(&manifests).map_err(|d| err_str(capability, request_id, &d.message))?;
    let db_path = state.layout.root.join("adapters.db");
    let registry = PersistentAdapterRegistry::open(&db_path)
        .map_err(|e| err_str(capability, request_id, &e))?;
    let mut loaded_ids = Vec::new();
    for adapter in &loaded {
        if let Err(e) = registry.register(&adapter.manifest)
            && !e.contains("already registered")
        {
            return Err(err_str(capability, request_id, &e));
        }
        loaded_ids.push(adapter.manifest.plugin_id.clone());
    }
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"loaded_count": loaded_ids.len(), "adapters": loaded_ids}),
    ))
}

fn dispatch_adapter_shutdown(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let plugin_id = require_str(body, "plugin_id")?;
    Ok(ok_envelope(capability, request_id, json!({"plugin_id": plugin_id, "status": "shutdown"})))
}

fn dispatch_adapter_status(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let plugin_id = require_str(body, "plugin_id")?;
    let db_path = state.layout.root.join("adapters.db");
    let registry = PersistentAdapterRegistry::open(&db_path)
        .map_err(|e| err_str(capability, request_id, &e))?;
    let manifest = registry.lookup(&plugin_id).map_err(|e| err_str(capability, request_id, &e))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"plugin_id": plugin_id, "manifest": manifest, "status": manifest.status}),
    ))
}

// ---------------------------------------------------------------------------
// WSR-02 — hub lifecycle handlers
// ---------------------------------------------------------------------------

fn dispatch_hub_register(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let hub_id = require_str(body, "hub_id")?;
    let resource_kind = require_str(body, "resource_kind")?;
    let locator = require_str(body, "locator")?;
    let provides: Vec<String> = body
        .get("provides")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();
    let version = body.get("version").and_then(Value::as_str).unwrap_or("1.0.0").to_string();
    let status = body.get("status").and_then(Value::as_str).unwrap_or("available").to_string();
    let tags: Vec<String> = body
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();
    let descriptor = vidodo_ir::ResourceHubDescriptor {
        hub_id: hub_id.clone(),
        resource_kind,
        version,
        locator,
        provides,
        compatibility: None,
        status: Some(status),
        tags,
    };
    let db_path = state.layout.root.join("hubs.db");
    let registry =
        PersistentHubRegistry::open(&db_path).map_err(|e| err_str(capability, request_id, &e))?;
    registry.register_hub(&descriptor).map_err(|e| err_str(capability, request_id, &e))?;
    Ok(ok_envelope(capability, request_id, json!({"hub_id": hub_id, "status": "registered"})))
}

fn dispatch_hub_resolve(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let resource_name = require_str(body, "resource_name")?;
    let db_path = state.layout.root.join("hubs.db");
    let registry =
        PersistentHubRegistry::open(&db_path).map_err(|e| err_str(capability, request_id, &e))?;
    let resolved = registry
        .resolve_resource(&resource_name)
        .map_err(|e| err_str(capability, request_id, &e))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"hub_id": resolved.hub_id, "locator": resolved.locator, "resource_kind": resolved.resource_kind}),
    ))
}

fn dispatch_hub_status(
    state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let hub_id = require_str(body, "hub_id")?;
    let db_path = state.layout.root.join("hubs.db");
    let registry =
        PersistentHubRegistry::open(&db_path).map_err(|e| err_str(capability, request_id, &e))?;
    let descriptor = registry.lookup(&hub_id).map_err(|e| err_str(capability, request_id, &e))?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"hub_id": hub_id, "descriptor": descriptor, "status": descriptor.status}),
    ))
}

fn dispatch_control_bind(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let source_id = require_str(body, "source_id")?;
    let protocol = require_str(body, "protocol")?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"source_id": source_id, "protocol": protocol, "status": "bound"}),
    ))
}

fn dispatch_control_unbind(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let source_id = require_str(body, "source_id")?;
    Ok(ok_envelope(capability, request_id, json!({"source_id": source_id, "status": "unbound"})))
}

fn dispatch_control_list(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    _body: &Value,
) -> Result<Value, Value> {
    let bindings: Vec<Value> = vec![];
    Ok(ok_envelope(capability, request_id, json!({"count": bindings.len(), "bindings": bindings})))
}

fn dispatch_control_status(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let source_id = require_str(body, "source_id")?;
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"source_id": source_id, "protocol": "unknown", "status": "not_bound"}),
    ))
}

fn dispatch_template_list(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    _body: &Value,
) -> Result<Value, Value> {
    let templates: Vec<Value> = vec![];
    Ok(ok_envelope(
        capability,
        request_id,
        json!({"count": templates.len(), "templates": templates}),
    ))
}

fn dispatch_template_load(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let template_id = require_str(body, "template_id")?;
    Ok(ok_envelope(capability, request_id, json!({"template_id": template_id, "template": {}})))
}

fn dispatch_scene_list(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    _body: &Value,
) -> Result<Value, Value> {
    let scenes: Vec<Value> = vec![];
    Ok(ok_envelope(capability, request_id, json!({"count": scenes.len(), "scenes": scenes})))
}

fn dispatch_scene_activate(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let scene_id = require_str(body, "scene_id")?;
    Ok(ok_envelope(capability, request_id, json!({"scene_id": scene_id, "status": "activated"})))
}

fn dispatch_demo_list(
    state: &McpState,
    capability: &str,
    request_id: &str,
    _body: &Value,
) -> Result<Value, Value> {
    let examples_dir = state.repo_root.join("examples");
    let mut names: Vec<String> = Vec::new();
    if examples_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&examples_dir)
    {
        for entry in entries.flatten() {
            if entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(ok_envelope(capability, request_id, json!({"count": names.len(), "examples": names})))
}

fn dispatch_demo_run(
    _state: &McpState,
    capability: &str,
    request_id: &str,
    body: &Value,
) -> Result<Value, Value> {
    let name = require_str(body, "name")?;
    Ok(ok_envelope(capability, request_id, json!({"name": name, "status": "stub"})))
}

// ---------------------------------------------------------------------------
// Envelope helpers
// ---------------------------------------------------------------------------

fn resolve(repo_root: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() { candidate } else { repo_root.join(candidate) }
}

fn require_str(body: &Value, field: &str) -> Result<String, Value> {
    body.get(field)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| err_str("unknown", "unknown", &format!("missing required field: {field}")))
}

fn require_u64(body: &Value, field: &str) -> Result<u64, Value> {
    body.get(field).and_then(Value::as_u64).ok_or_else(|| {
        err_str("unknown", "unknown", &format!("missing required field: {field} (integer)"))
    })
}

fn ok_envelope(capability: &str, request_id: &str, data: Value) -> Value {
    envelope_value("ok", capability, request_id, data, vec![], vec![], vec![])
}

fn envelope_value(
    status: &str,
    capability: &str,
    request_id: &str,
    data: Value,
    diagnostics: Vec<Diagnostic>,
    artifacts: Vec<String>,
    next_actions: Vec<String>,
) -> Value {
    let envelope = ResponseEnvelope::new(
        status,
        capability,
        request_id,
        data,
        diagnostics,
        artifacts,
        next_actions,
    );
    serde_json::to_value(envelope).unwrap_or_default()
}

fn err_str(capability: &str, request_id: &str, message: &str) -> Value {
    envelope_value(
        "error",
        capability,
        request_id,
        json!(null),
        vec![Diagnostic::error("MCP-ERR", message.to_string())],
        vec![],
        vec![],
    )
}

fn error_envelope(capability: &str, request_id: &str, diagnostics: Vec<Diagnostic>) -> Value {
    envelope_value("error", capability, request_id, json!(null), diagnostics, vec![], vec![])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> McpState {
        let repo_root = discover_repo_root().expect("repo root");
        let layout = ArtifactLayout::new(repo_root.join("artifacts"));
        let _ = layout.ensure();
        McpState { registry: CapabilityRegistry::default(), layout, repo_root }
    }

    #[test]
    fn initialize_returns_protocol_version() {
        let result = handle_initialize();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], "vidodo-mcp-adapter");
    }

    #[test]
    fn tools_list_returns_39_tools() {
        let state = test_state();
        let result = handle_tools_list(&state);
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 39, "expected 39 tools, got {}", tools.len());
        for tool in tools {
            assert!(tool["name"].is_string(), "tool missing name");
            assert!(tool["description"].is_string(), "tool missing description");
            assert!(tool["inputSchema"].is_object(), "tool missing inputSchema");
            assert!(tool["annotations"].is_object(), "tool missing annotations");
        }
    }

    #[test]
    fn tools_call_unknown_tool_returns_error() {
        let state = test_state();
        let params = json!({"name": "nonexistent.tool", "arguments": {}});
        let result = handle_tools_call(&state, &params);
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn tools_call_system_capabilities_returns_list() {
        let state = test_state();
        let params = json!({"name": "system.capabilities", "arguments": {}});
        let result = handle_tools_call(&state, &params);
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let envelope: Value = serde_json::from_str(text).unwrap();
        assert_eq!(envelope["status"], "ok");
        assert!(envelope["data"]["count"].as_u64().unwrap() >= 12);
    }

    #[test]
    fn tools_call_plan_validate() {
        let state = test_state();
        let plan_dir = state.repo_root.join("tests/fixtures/plans/minimal-show");
        let assets_file = state.repo_root.join("tests/fixtures/assets/asset-records.json");
        let params = json!({
            "name": "plan.validate",
            "arguments": {
                "plan_dir": plan_dir.display().to_string(),
                "assets_file": assets_file.display().to_string()
            }
        });
        let result = handle_tools_call(&state, &params);
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let envelope: Value = serde_json::from_str(text).unwrap();
        assert_eq!(envelope["status"], "ok");
        assert_eq!(envelope["data"]["show_id"], "show-phase0-minimal");
    }

    // --- WSI-03: system.describe_capabilities tests ---

    #[test]
    fn describe_capabilities_returns_12_plus_entries_with_schemas() {
        let state = test_state();
        let params = json!({"name": "system.capabilities", "arguments": {}});
        let result = handle_tools_call(&state, &params);
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        let envelope: Value = serde_json::from_str(text).unwrap();
        assert_eq!(envelope["status"], "ok");

        let caps = envelope["data"]["capabilities"].as_array().unwrap();
        assert!(caps.len() >= 12, "expected 12+ capabilities, got {}", caps.len());

        for cap in caps {
            assert!(cap["capability"].is_string(), "missing capability name");
            assert!(
                cap["input_schema"].is_string(),
                "missing input_schema for {}",
                cap["capability"]
            );
            assert!(
                cap["output_schema"].is_string(),
                "missing output_schema for {}",
                cap["capability"]
            );
        }
    }

    #[test]
    fn describe_capabilities_schemas_are_valid_json() {
        let state = test_state();
        let params = json!({"name": "system.capabilities", "arguments": {}});
        let result = handle_tools_call(&state, &params);
        let text = result["content"][0]["text"].as_str().unwrap();
        let envelope: Value = serde_json::from_str(text).unwrap();
        let caps = envelope["data"]["capabilities"].as_array().unwrap();

        let mut checked = 0;
        for cap in caps {
            let input = cap["input_schema"].as_str().unwrap_or("");
            let output = cap["output_schema"].as_str().unwrap_or("");
            if !input.is_empty() {
                let parsed: Value = serde_json::from_str(input).unwrap_or_else(|e| {
                    panic!("invalid input_schema JSON for {}: {e}", cap["capability"])
                });
                assert_eq!(parsed["type"], "object", "input_schema.type must be 'object'");
                checked += 1;
            }
            if !output.is_empty() {
                let parsed: Value = serde_json::from_str(output).unwrap_or_else(|e| {
                    panic!("invalid output_schema JSON for {}: {e}", cap["capability"])
                });
                assert_eq!(parsed["type"], "object", "output_schema.type must be 'object'");
                checked += 1;
            }
        }
        assert!(checked >= 20, "expected 20+ schema entries checked, got {checked}");
    }
}
