use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};
use vidodo_adapter_registry::loader::load_adapters;
use vidodo_adapter_registry::persistence::PersistentAdapterRegistry;
use vidodo_capability::CapabilityRegistry;
use vidodo_compiler::compile_plan;
use vidodo_compiler::revision::{
    archive_revision, publish_revision, query_revisions, register_candidate,
};
use vidodo_evaluation::evaluate_run;
use vidodo_ir::{
    AssetRecord, AudioDsl, CompiledRevision, ConstraintSet, CueSet, Diagnostic, LightingTopology,
    LivePatchProposal, PatchDecision, PlanBundle, ResponseEnvelope, SetPlan, VisualDsl,
};
use vidodo_patch_manager::{apply_patch, check_patch, deferred_rollback, rollback_patch};
use vidodo_resource_hub::persistence::PersistentHubRegistry;
use vidodo_scheduler::{RunStatusRecord, simulate_run, simulate_run_with_backend};
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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

struct CommandContext {
    repo_root: PathBuf,
    layout: ArtifactLayout,
}

fn run() -> Result<(), ExitCode> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() || args.len() == 1 && args[0] == "help" {
        print_usage();
        return Ok(());
    }

    let context = command_context().map_err(|message| {
        eprintln!("{message}");
        ExitCode::from(1)
    })?;

    match args[0].as_str() {
        "doctor" => handle_doctor(&context),
        "asset" => handle_asset(&context, &args[1..]),
        "plan" => handle_plan(&context, &args[1..]),
        "compile" => handle_compile(&context, &args[1..]),
        "revision" => handle_revision(&context, &args[1..]),
        "run" => handle_run(&context, &args[1..]),
        "patch" => handle_patch(&context, &args[1..]),
        "trace" => handle_trace(&context, &args[1..]),
        "eval" => handle_eval(&context, &args[1..]),
        "export" => handle_export(&context, &args[1..]),
        "system" => handle_system(&context, &args[1..]),
        "adapter" => handle_adapter(&context, &args[1..]),
        "hub" => handle_hub(&context, &args[1..]),
        "control" => handle_control(&context, &args[1..]),
        "template" => handle_template(&context, &args[1..]),
        "scene" => handle_scene(&context, &args[1..]),
        "demo" => handle_demo(&context, &args[1..]),
        _ => {
            print_usage();
            Err(ExitCode::from(2))
        }
    }
}

fn command_context() -> Result<CommandContext, String> {
    let repo_root = discover_repo_root()?;
    let layout = ArtifactLayout::discover()?;
    layout.ensure()?;
    Ok(CommandContext { repo_root, layout })
}

fn handle_doctor(context: &CommandContext) -> Result<(), ExitCode> {
    let capability = "system.doctor";
    let request_id = "req-doctor";
    let plan_dir = default_plan_dir(&context.repo_root);
    let assets_file = default_assets_file(context)
        .map_err(|message| emit_error(capability, request_id, "CLI-008", message))?;
    let patch_file = default_patch_file(&context.repo_root);

    let plan = load_plan_bundle(&plan_dir, &assets_file)
        .map_err(|message| emit_error(capability, request_id, "CLI-001", message))?;
    let diagnostics = validate_plan(&plan);
    if diagnostics.iter().any(|diagnostic| diagnostic.severity == "error") {
        return print_response(
            capability,
            request_id,
            "error",
            json!({}),
            diagnostics,
            vec![],
            vec![],
        );
    }

    let compiled = compile_plan(&plan).map_err(|diagnostics| {
        let _ =
            print_response(capability, request_id, "error", json!({}), diagnostics, vec![], vec![]);
        ExitCode::from(1)
    })?;
    let mut artifacts = persist_revision(context, &compiled)
        .map_err(|message| emit_error(capability, request_id, "CLI-002", message))?;

    let patch = load_patch(&patch_file)
        .map_err(|message| emit_error(capability, request_id, "CLI-003", message))?;
    let patch_diagnostics = check_patch(&compiled, &patch);
    if !patch_diagnostics.is_empty() {
        return print_response(
            capability,
            request_id,
            "error",
            json!({}),
            patch_diagnostics,
            artifacts,
            vec![],
        );
    }

    let patched = apply_patch(&compiled, &patch).map_err(|diagnostics| {
        let _ = print_response(
            capability,
            request_id,
            "error",
            json!({}),
            diagnostics,
            artifacts.clone(),
            vec![],
        );
        ExitCode::from(1)
    })?;
    artifacts.extend(
        persist_revision(context, &patched)
            .map_err(|message| emit_error(capability, request_id, "CLI-004", message))?,
    );

    let run_id = deterministic_run_id(&patched.show_id, patched.revision);
    let simulated_run = simulate_run(&patched, &run_id);
    let manifest = write_trace(
        &context.layout,
        &run_id,
        &patched,
        "offline",
        &simulated_run.summary,
        &simulated_run.final_show_state,
        &simulated_run.events,
        &simulated_run.patch_decisions,
        &simulated_run.resource_samples,
    )
    .map_err(|message| emit_error(capability, request_id, "CLI-005", message))?;
    if !simulated_run.degrade_events.is_empty() {
        append_degrade_events(&context.layout, &run_id, &simulated_run.degrade_events)
            .map_err(|message| emit_error(capability, request_id, "CLI-005", message))?;
    }
    let manifest_file = manifest_path(&context.layout, &run_id);
    let trace_manifest_ref = relative_to_repo(context, &manifest_file);
    artifacts.push(trace_manifest_ref.clone());

    let rollback = rollback_patch(&patched, &patch.patch_id).map_err(|diagnostic| {
        let _ = print_response(
            capability,
            request_id,
            "error",
            json!({}),
            vec![*diagnostic],
            artifacts.clone(),
            vec![],
        );
        ExitCode::from(1)
    })?;
    let rollback_path = context
        .layout
        .revisions
        .join(slug(&patched.show_id))
        .join(format!("rollback-{}.json", patch.patch_id));
    write_json(&rollback_path, &rollback)
        .map_err(|message| emit_error(capability, request_id, "CLI-006", message))?;
    artifacts.push(relative_to_repo(context, &rollback_path));

    let status_record = RunStatusRecord {
        show_id: patched.show_id.clone(),
        run_id: run_id.clone(),
        revision: patched.revision,
        status: String::from("completed"),
        trace_manifest: trace_manifest_ref.clone(),
        summary: simulated_run.summary.clone(),
        final_show_state: simulated_run.final_show_state.clone(),
    };
    write_json(&context.layout.run_status_path(&patched.show_id), &status_record)
        .map_err(|message| emit_error(capability, request_id, "CLI-007", message))?;

    print_response(
        capability,
        request_id,
        "ok",
        json!({
            "show_id": patched.show_id,
            "asset_source": relative_to_repo(context, &assets_file),
            "compiled_revision": compiled.revision,
            "patched_revision": patched.revision,
            "run_id": run_id,
            "trace_bundle_id": manifest.trace_bundle_id,
            "event_count": simulated_run.summary.event_count,
            "rollback_fallback_revision": rollback.fallback_revision
        }),
        diagnostics,
        artifacts,
        vec![String::from(
            "run `avctl trace show --run-id <run-id>` to inspect the generated trace",
        )],
    )
}

fn handle_asset(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "ingest" => {
            let capability = "asset.ingest";
            let request_id = "req-asset-ingest";
            let source_dir = required_flag(rest, "--source-dir")
                .map_err(|message| emit_error(capability, request_id, "CLI-070", message))?;
            let source_path = resolve_path(context, &source_dir);
            let declared_kind = required_flag(rest, "--declared-kind")
                .map_err(|message| emit_error(capability, request_id, "CLI-071", message))?;
            let tags = optional_flag(rest, "--tags")
                .map(|value| parse_csv_list(&value))
                .unwrap_or_default();
            let asset_namespace = optional_flag(rest, "--asset-namespace");
            let naming_manifest = asset_pack_manifest_path(&source_path);

            let report = ingest_assets(
                &context.layout,
                &AssetIngestRequest {
                    source: source_path,
                    declared_kind,
                    tags,
                    asset_namespace: asset_namespace.clone(),
                    asset_id_overrides: BTreeMap::new(),
                },
            )
            .map_err(|diagnostics| {
                let _ = print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    diagnostics,
                    vec![],
                    vec![],
                );
                ExitCode::from(1)
            })?;

            let report_path = context.layout.ingestion_report_path(&report.run.ingestion_run_id);
            let registry_path = context.layout.asset_registry_file();
            let mut artifacts = vec![
                relative_to_repo(context, &report_path),
                relative_to_repo(context, &registry_path),
            ];
            let naming_manifest_ref = if naming_manifest.exists() {
                let manifest_ref = relative_to_repo(context, &naming_manifest);
                artifacts.push(manifest_ref.clone());
                Some(manifest_ref)
            } else {
                None
            };
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "ingestion_run_id": report.run.ingestion_run_id,
                    "source": report.run.source,
                    "asset_namespace_override": asset_namespace,
                    "asset_naming_manifest": naming_manifest_ref,
                    "discovered": report.run.discovered,
                    "published": report.run.published,
                    "reused": report.run.reused,
                    "analysis_jobs": report.analysis_jobs.len(),
                    "assets": report
                        .assets
                        .iter()
                        .map(|asset| asset.asset_id.clone())
                        .collect::<Vec<_>>()
                }),
                vec![],
                artifacts,
                vec![String::from(
                    "run `avctl asset list [--kind <kind>] [--tag <tag>]` to inspect published assets",
                )],
            )
        }
        [command, rest @ ..] if command == "list" => {
            let capability = "asset.list";
            let request_id = "req-asset-list";
            let assets = list_assets(
                &context.layout,
                &AssetQuery {
                    asset_kind: optional_flag(rest, "--kind"),
                    tag: optional_flag(rest, "--tag"),
                },
            )
            .map_err(|message| emit_error(capability, request_id, "CLI-072", message))?;

            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "count": assets.len(),
                    "assets": assets
                }),
                vec![],
                vec![relative_to_repo(context, &context.layout.asset_registry_file())],
                vec![],
            )
        }
        [command, rest @ ..] if command == "show" => {
            let capability = "asset.show";
            let request_id = "req-asset-show";
            let asset_id = required_flag(rest, "--asset-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-073", message))?;
            let asset = get_asset(&context.layout, &asset_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-074", message))?
                .ok_or_else(|| {
                    emit_error(
                        capability,
                        request_id,
                        "CLI-075",
                        format!("asset {} was not found", asset_id),
                    )
                })?;
            let analysis_entries = list_asset_analysis(&context.layout, &asset_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-076", message))?;
            let analysis_jobs = list_asset_jobs(&context.layout, &asset_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-077", message))?;
            let analysis_payloads = analysis_entries
                .iter()
                .map(|entry| {
                    let payload: Value = read_json(&resolve_path(context, &entry.payload_ref))?;
                    Ok(json!({
                        "payload_ref": entry.payload_ref,
                        "payload": payload,
                    }))
                })
                .collect::<Result<Vec<_>, String>>()
                .map_err(|message| emit_error(capability, request_id, "CLI-078", message))?;

            let mut artifacts =
                vec![relative_to_repo(context, &context.layout.asset_registry_file())];
            if let Some(raw_locator) = &asset.raw_locator {
                artifacts.push(raw_locator.clone());
            }
            if let Some(normalized_locator) = &asset.normalized_locator {
                artifacts.push(normalized_locator.clone());
            }
            for entry in &analysis_entries {
                artifacts.push(entry.payload_ref.clone());
            }

            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "asset": asset,
                    "analysis_entries": analysis_entries,
                    "analysis_jobs": analysis_jobs,
                    "analysis_payloads": analysis_payloads
                }),
                vec![],
                artifacts,
                vec![],
            )
        }
        _ => Err(emit_usage_error(
            "asset",
            "req-asset",
            "usage: avctl asset ingest --source-dir <path> --declared-kind <kind> [--tags tag1,tag2] [--asset-namespace <namespace>] | avctl asset list [--kind <kind>] [--tag <tag>] | avctl asset show --asset-id <id>",
        )),
    }
}

fn handle_plan(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    let capability = "plan.validate";
    let request_id = "req-plan-validate";
    match args {
        [command, rest @ ..] if command == "validate" => {
            let plan_dir = required_flag(rest, "--plan-dir")
                .map_err(|message| emit_error(capability, request_id, "CLI-010", message))?;
            let asset_file = match optional_flag(rest, "--assets-file") {
                Some(value) => resolve_path(context, &value),
                None => default_assets_file(context)
                    .map_err(|message| emit_error(capability, request_id, "CLI-012", message))?,
            };
            let plan = load_plan_bundle(&resolve_path(context, &plan_dir), &asset_file)
                .map_err(|message| emit_error(capability, request_id, "CLI-011", message))?;
            let diagnostics = validate_plan(&plan);
            let status = if diagnostics.iter().any(|diagnostic| diagnostic.severity == "error") {
                "error"
            } else {
                "ok"
            };
            print_response(
                capability,
                request_id,
                status,
                json!({
                    "show_id": plan.show_id,
                    "asset_source": relative_to_repo(context, &asset_file),
                    "section_count": plan.set_plan.sections.len(),
                    "audio_layer_count": plan.audio_dsl.layers.len(),
                    "visual_scene_count": plan.visual_dsl.scenes.len()
                }),
                diagnostics,
                vec![relative_to_repo(context, &asset_file)],
                vec![String::from(
                    "run `avctl compile run --plan-dir <path>` when validation is clean",
                )],
            )
        }
        _ => Err(emit_usage_error(
            capability,
            request_id,
            "usage: avctl plan validate --plan-dir <path> [--assets-file <path>]",
        )),
    }
}

fn handle_compile(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    let capability = "compile.run";
    let request_id = "req-compile-run";
    match args {
        [command, rest @ ..] if command == "run" => {
            let plan_dir = required_flag(rest, "--plan-dir")
                .map_err(|message| emit_error(capability, request_id, "CLI-020", message))?;
            let asset_file = match optional_flag(rest, "--assets-file") {
                Some(value) => resolve_path(context, &value),
                None => default_assets_file(context)
                    .map_err(|message| emit_error(capability, request_id, "CLI-023", message))?,
            };
            let plan = load_plan_bundle(&resolve_path(context, &plan_dir), &asset_file)
                .map_err(|message| emit_error(capability, request_id, "CLI-021", message))?;
            let compiled = compile_plan(&plan).map_err(|diagnostics| {
                let _ = print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    diagnostics,
                    vec![],
                    vec![],
                );
                ExitCode::from(1)
            })?;
            let mut artifacts = persist_revision(context, &compiled)
                .map_err(|message| emit_error(capability, request_id, "CLI-022", message))?;
            artifacts.insert(0, relative_to_repo(context, &asset_file));

            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "show_id": compiled.show_id,
                    "asset_source": relative_to_repo(context, &asset_file),
                    "revision": compiled.revision,
                    "compile_run_id": compiled.compile_run_id,
                    "timeline_entries": compiled.timeline.len()
                }),
                vec![],
                artifacts,
                vec![String::from(
                    "run `avctl run start --show-id <show-id> --revision <revision>` to generate trace artifacts",
                )],
            )
        }
        _ => Err(emit_usage_error(
            capability,
            request_id,
            "usage: avctl compile run --plan-dir <path> [--assets-file <path>]",
        )),
    }
}

fn handle_revision(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "list" => {
            let capability = "revision.list";
            let request_id = "req-revision-list";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-080", message))?;
            let records = query_revisions(&context.layout, &show_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-081", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "show_id": show_id,
                    "count": records.len(),
                    "revisions": records
                }),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "publish" => {
            let capability = "revision.publish";
            let request_id = "req-revision-publish";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-082", message))?;
            let revision = required_flag(rest, "--revision")
                .and_then(|v| parse_u64(&v, "--revision"))
                .map_err(|message| emit_error(capability, request_id, "CLI-083", message))?;
            publish_revision(&context.layout, &show_id, revision)
                .map_err(|message| emit_error(capability, request_id, "CLI-084", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({ "show_id": show_id, "revision": revision, "status": "published" }),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "archive" => {
            let capability = "revision.archive";
            let request_id = "req-revision-archive";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-085", message))?;
            let revision = required_flag(rest, "--revision")
                .and_then(|v| parse_u64(&v, "--revision"))
                .map_err(|message| emit_error(capability, request_id, "CLI-086", message))?;
            archive_revision(&context.layout, &show_id, revision)
                .map_err(|message| emit_error(capability, request_id, "CLI-087", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({ "show_id": show_id, "revision": revision, "status": "archived" }),
                vec![],
                vec![],
                vec![],
            )
        }
        _ => Err(ExitCode::from(2)),
    }
}

fn handle_run(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "start" => {
            let capability = "run.start";
            let request_id = "req-run-start";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-030", message))?;
            let revision = required_flag(rest, "--revision")
                .and_then(|value| parse_u64(&value, "--revision"))
                .map_err(|message| emit_error(capability, request_id, "CLI-031", message))?;
            let compiled = load_revision(context, &show_id, revision)
                .map_err(|message| emit_error(capability, request_id, "CLI-032", message))?;
            let run_id = deterministic_run_id(&show_id, revision);
            let backend_flag = optional_flag(rest, "--backend");
            let scheduled = if backend_flag.as_deref() == Some("reference") {
                let backend = vidodo_scheduler::reference_backend::ReferenceBackendClient::new();
                simulate_run_with_backend(&compiled, &run_id, &backend)
            } else {
                simulate_run(&compiled, &run_id)
            };
            let manifest = write_trace(
                &context.layout,
                &run_id,
                &compiled,
                "offline",
                &scheduled.summary,
                &scheduled.final_show_state,
                &scheduled.events,
                &scheduled.patch_decisions,
                &scheduled.resource_samples,
            )
            .map_err(|message| emit_error(capability, request_id, "CLI-033", message))?;
            if !scheduled.degrade_events.is_empty() {
                append_degrade_events(&context.layout, &run_id, &scheduled.degrade_events)
                    .map_err(|message| emit_error(capability, request_id, "CLI-033", message))?;
            }
            let trace_manifest =
                relative_to_repo(context, &manifest_path(&context.layout, &run_id));
            let status_record = RunStatusRecord {
                show_id: show_id.clone(),
                run_id: run_id.clone(),
                revision,
                status: String::from("completed"),
                trace_manifest: trace_manifest.clone(),
                summary: scheduled.summary.clone(),
                final_show_state: scheduled.final_show_state.clone(),
            };
            write_json(&context.layout.run_status_path(&show_id), &status_record)
                .map_err(|message| emit_error(capability, request_id, "CLI-034", message))?;

            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "run_id": run_id,
                    "show_id": show_id,
                    "revision": revision,
                    "event_count": scheduled.summary.event_count,
                    "final_section": scheduled.summary.final_section,
                    "trace_bundle_id": manifest.trace_bundle_id
                }),
                vec![],
                vec![trace_manifest],
                vec![String::from(
                    "run `avctl trace events --run-id <run-id>` to inspect runtime events",
                )],
            )
        }
        [command, rest @ ..] if command == "status" => {
            let capability = "run.status";
            let request_id = "req-run-status";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-035", message))?;
            let status_path = context.layout.run_status_path(&show_id);
            let status_record: RunStatusRecord = read_json(&status_path)
                .map_err(|message| emit_error(capability, request_id, "CLI-036", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                serde_json::to_value(status_record).unwrap_or_else(|_| json!({})),
                vec![],
                vec![relative_to_repo(context, &status_path)],
                vec![],
            )
        }
        _ => Err(ExitCode::from(2)),
    }
}

fn handle_patch(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "check" => {
            let capability = "patch.check";
            let request_id = "req-patch-check";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-040", message))?;
            let patch_file = required_flag(rest, "--patch-file")
                .map_err(|message| emit_error(capability, request_id, "CLI-041", message))?;
            let revision = load_latest_revision(context, &show_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-042", message))?;
            let patch = load_patch(&resolve_path(context, &patch_file))
                .map_err(|message| emit_error(capability, request_id, "CLI-043", message))?;
            let diagnostics = check_patch(&revision, &patch);
            let status = if diagnostics.is_empty() { "ok" } else { "error" };
            print_response(
                capability,
                request_id,
                status,
                json!({
                    "show_id": show_id,
                    "base_revision": revision.revision,
                    "patch_id": patch.patch_id
                }),
                diagnostics,
                vec![],
                vec![String::from(
                    "run `avctl patch submit --show-id <show-id> --patch-file <path>` when the patch is accepted",
                )],
            )
        }
        [command, rest @ ..] if command == "submit" => {
            let capability = "patch.submit";
            let request_id = "req-patch-submit";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-044", message))?;
            let patch_file = required_flag(rest, "--patch-file")
                .map_err(|message| emit_error(capability, request_id, "CLI-045", message))?;
            let revision = load_latest_revision(context, &show_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-046", message))?;
            let patch = load_patch(&resolve_path(context, &patch_file))
                .map_err(|message| emit_error(capability, request_id, "CLI-047", message))?;
            let patched = apply_patch(&revision, &patch).map_err(|diagnostics| {
                let _ = print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    diagnostics,
                    vec![],
                    vec![],
                );
                ExitCode::from(1)
            })?;
            let artifacts = persist_revision(context, &patched)
                .map_err(|message| emit_error(capability, request_id, "CLI-048", message))?;
            let decision = patched.patch_history.last().cloned().ok_or_else(|| {
                emit_error(
                    capability,
                    request_id,
                    "CLI-049",
                    String::from("patch was applied but no decision was recorded"),
                )
            })?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "show_id": show_id,
                    "patch_id": patch.patch_id,
                    "effective_revision": patched.revision,
                    "fallback_revision": decision.fallback_revision
                }),
                vec![],
                artifacts,
                vec![format!(
                    "run `avctl run start --show-id {} --revision {}` to execute the patched revision",
                    show_id, patched.revision
                )],
            )
        }
        [command, rest @ ..] if command == "rollback" => {
            let capability = "patch.rollback";
            let request_id = "req-patch-rollback";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-049", message))?;
            let patch_id = required_flag(rest, "--patch-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-050", message))?;
            let revision = load_latest_revision(context, &show_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-051", message))?;
            let rollback = rollback_patch(&revision, &patch_id).map_err(|diagnostic| {
                let _ = print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    vec![*diagnostic],
                    vec![],
                    vec![],
                );
                ExitCode::from(1)
            })?;
            let rollback_path = context
                .layout
                .revisions
                .join(slug(&show_id))
                .join(format!("rollback-{patch_id}.json"));
            write_json(&rollback_path, &rollback)
                .map_err(|message| emit_error(capability, request_id, "CLI-052", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "show_id": show_id,
                    "patch_id": patch_id,
                    "fallback_revision": rollback.fallback_revision
                }),
                vec![],
                vec![relative_to_repo(context, &rollback_path)],
                vec![format!(
                    "run `avctl run start --show-id {} --revision {}` to resume from the fallback revision",
                    show_id, rollback.fallback_revision
                )],
            )
        }
        [command, rest @ ..] if command == "deferred-rollback" => {
            let capability = "patch.deferred_rollback";
            let request_id = "req-patch-deferred-rollback";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-053", message))?;
            let patch_id = required_flag(rest, "--patch-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-054", message))?;
            let anomaly = required_flag(rest, "--anomaly")
                .map_err(|message| emit_error(capability, request_id, "CLI-055", message))?;
            let run_id = optional_flag(rest, "--run-id");
            let revision = load_latest_revision(context, &show_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-056", message))?;
            let decision =
                deferred_rollback(&revision, &patch_id, &anomaly).map_err(|diagnostic| {
                    let _ = print_response(
                        capability,
                        request_id,
                        "error",
                        json!({}),
                        vec![*diagnostic],
                        vec![],
                        vec![],
                    );
                    ExitCode::from(1)
                })?;
            let rollback_path = context
                .layout
                .revisions
                .join(slug(&show_id))
                .join(format!("deferred-rollback-{patch_id}.json"));
            write_json(&rollback_path, &decision)
                .map_err(|message| emit_error(capability, request_id, "CLI-057", message))?;

            // If a run-id is provided, append the rollback decision to that run's trace
            let mut artifacts = vec![relative_to_repo(context, &rollback_path)];
            if let Some(ref rid) = run_id {
                let trace_decisions_path =
                    context.layout.trace_dir(rid).join("patch-decisions.jsonl");
                if trace_decisions_path.exists() {
                    let mut existing: Vec<PatchDecision> =
                        vidodo_storage::read_jsonl(&trace_decisions_path)
                            .unwrap_or_else(|_| Vec::new());
                    existing.push(decision.clone());
                    vidodo_storage::write_jsonl(&trace_decisions_path, &existing).map_err(
                        |message| emit_error(capability, request_id, "CLI-058", message),
                    )?;
                    artifacts.push(relative_to_repo(context, &trace_decisions_path));
                }
            }

            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "show_id": show_id,
                    "patch_id": patch_id,
                    "decision": decision.decision,
                    "fallback_revision": decision.fallback_revision,
                    "anomaly": anomaly
                }),
                vec![],
                artifacts,
                vec![format!(
                    "run `avctl run start --show-id {} --revision {}` to resume from the fallback revision",
                    show_id, decision.fallback_revision
                )],
            )
        }
        _ => Err(ExitCode::from(2)),
    }
}

fn handle_trace(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "show" => {
            let capability = "trace.show";
            let request_id = "req-trace-show";
            let run_id = required_flag(rest, "--run-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-060", message))?;
            let manifest = load_manifest(&context.layout, &run_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-061", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                serde_json::to_value(manifest).unwrap_or_else(|_| json!({})),
                vec![],
                vec![relative_to_repo(context, &manifest_path(&context.layout, &run_id))],
                vec![],
            )
        }
        [command, rest @ ..] if command == "events" => {
            let capability = "trace.events";
            let request_id = "req-trace-events";
            let run_id = required_flag(rest, "--run-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-062", message))?;
            let all_events = load_events(&context.layout, &run_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-063", message))?;
            let from_bar = optional_flag(rest, "--from-bar").and_then(|v| v.parse::<u32>().ok());
            let to_bar = optional_flag(rest, "--to-bar").and_then(|v| v.parse::<u32>().ok());
            let events = match (from_bar, to_bar) {
                (Some(from), Some(to)) => filter_events_by_bar(&all_events, from, to),
                (Some(from), None) => filter_events_by_bar(&all_events, from, u32::MAX),
                (None, Some(to)) => filter_events_by_bar(&all_events, 0, to),
                (None, None) => all_events,
            };
            let event_log = context.layout.trace_dir(&run_id).join("events.jsonl");
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "run_id": run_id,
                    "event_count": events.len(),
                    "events": events
                }),
                vec![],
                vec![relative_to_repo(context, &event_log)],
                vec![],
            )
        }
        _ => Err(ExitCode::from(2)),
    }
}

fn handle_export(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "audio" => {
            let capability = "export.audio";
            let request_id = "req-export-audio";
            let run_id = required_flag(rest, "--run-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-070", message))?;
            let manifest = load_manifest(&context.layout, &run_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-071", message))?;
            let compiled = load_revision(context, &manifest.show_id, manifest.revision)
                .map_err(|message| emit_error(capability, request_id, "CLI-072", message))?;
            let record = export_audio(
                &context.layout,
                &run_id,
                &manifest.show_id,
                manifest.revision,
                compiled.final_bar(),
                128.0,
            )
            .map_err(|message| emit_error(capability, request_id, "CLI-073", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "artifact_id": record.artifact_id,
                    "artifact_type": record.artifact_type,
                    "locator": record.locator,
                    "content_hash": record.content_hash,
                    "duration_sec": record.duration_sec
                }),
                vec![],
                vec![record.locator.clone()],
                vec![String::from("export complete — WAV file linked to trace bundle")],
            )
        }
        _ => Err(ExitCode::from(2)),
    }
}

fn handle_eval(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "run" => {
            let capability = "eval.run";
            let request_id = "req-eval-run";
            let show_id = required_flag(rest, "--show-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-070", message))?;
            let run_id = match optional_flag(rest, "--run-id") {
                Some(id) => id,
                None => {
                    let status_record: RunStatusRecord = read_json(
                        &context.layout.run_status_path(&show_id),
                    )
                    .map_err(|message| emit_error(capability, request_id, "CLI-071", message))?;
                    if status_record.run_id.is_empty() {
                        return Err(emit_error(
                            capability,
                            request_id,
                            "CLI-071",
                            String::from(
                                "no run found; provide --run-id or run `avctl run start` first",
                            ),
                        ));
                    }
                    status_record.run_id
                }
            };
            let status_record: RunStatusRecord =
                read_json(&context.layout.run_status_path(&show_id))
                    .map_err(|message| emit_error(capability, request_id, "CLI-072", message))?;
            let report = evaluate_run(
                &context.layout,
                &run_id,
                &status_record.summary,
                &status_record.final_show_state,
            )
            .map_err(|message| emit_error(capability, request_id, "CLI-073", message))?;

            let eval_path = context.layout.trace_dir(&run_id).join("evaluation.json");
            write_json(&eval_path, &report)
                .map_err(|message| emit_error(capability, request_id, "CLI-074", message))?;

            print_response(
                capability,
                request_id,
                "ok",
                serde_json::to_value(&report).unwrap_or_else(|_| json!({})),
                vec![],
                vec![relative_to_repo(context, &eval_path)],
                vec![],
            )
        }
        _ => Err(ExitCode::from(2)),
    }
}

fn handle_system(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command] if command == "capabilities" => {
            let capability = "system.capabilities";
            let request_id = "req-system-capabilities";
            let registry = CapabilityRegistry::default();
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "count": registry.len(),
                    "capabilities": registry.list()
                }),
                vec![],
                vec![],
                vec![],
            )
        }
        [command] if command == "adapters" => {
            let capability = "system.adapters";
            let request_id = "req-system-adapters";
            let db_path = context.layout.root.join("adapters.db");
            let registry = PersistentAdapterRegistry::open(&db_path)
                .map_err(|e| emit_error(capability, request_id, "SVC-100", e))?;
            let list =
                registry.list().map_err(|e| emit_error(capability, request_id, "SVC-100", e))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "count": list.len(),
                    "adapters": list
                }),
                vec![],
                vec![],
                vec![],
            )
        }
        [command] if command == "hubs" => {
            let capability = "system.hubs";
            let request_id = "req-system-hubs";
            let db_path = context.layout.root.join("hubs.db");
            let registry = PersistentHubRegistry::open(&db_path)
                .map_err(|e| emit_error(capability, request_id, "SVC-100", e))?;
            let list = registry
                .list_hubs()
                .map_err(|e| emit_error(capability, request_id, "SVC-100", e))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "count": list.len(),
                    "hubs": list
                }),
                vec![],
                vec![],
                vec![],
            )
        }
        _ => Err(emit_usage_error(
            "system",
            "req-system",
            "usage: avctl system <capabilities|adapters|hubs>",
        )),
    }
}

// ---------------------------------------------------------------------------
// WSR-01 — adapter lifecycle CLI
// ---------------------------------------------------------------------------

fn handle_adapter(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "load" => {
            let capability = "adapter.load";
            let request_id = "req-adapter-load";
            let manifest_path = required_flag(rest, "--manifest")
                .map_err(|message| emit_error(capability, request_id, "CLI-200", message))?;
            let path = resolve_path(context, &manifest_path);
            let raw = fs::read_to_string(&path).map_err(|e| {
                emit_error(capability, request_id, "CLI-201", format!("cannot read manifest: {e}"))
            })?;
            let manifests: Vec<vidodo_ir::AdapterPluginManifest> = serde_json::from_str(&raw)
                .map_err(|e| {
                    emit_error(
                        capability,
                        request_id,
                        "CLI-202",
                        format!("invalid manifest JSON: {e}"),
                    )
                })?;
            let loaded = load_adapters(&manifests).map_err(|d| {
                let _ = print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    vec![*d],
                    vec![],
                    vec![],
                );
                ExitCode::from(1)
            })?;
            let db_path = context.layout.root.join("adapters.db");
            let registry = PersistentAdapterRegistry::open(&db_path)
                .map_err(|e| emit_error(capability, request_id, "CLI-203", e))?;
            let mut loaded_ids = Vec::new();
            for adapter in &loaded {
                if let Err(e) = registry.register(&adapter.manifest)
                    && !e.contains("already registered")
                {
                    return Err(emit_error(capability, request_id, "CLI-204", e));
                }
                loaded_ids.push(adapter.manifest.plugin_id.clone());
            }
            print_response(
                capability,
                request_id,
                "ok",
                json!({"loaded_count": loaded_ids.len(), "adapters": loaded_ids}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "shutdown" => {
            let capability = "adapter.shutdown";
            let request_id = "req-adapter-shutdown";
            let plugin_id = required_flag(rest, "--plugin-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-210", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"plugin_id": plugin_id, "status": "shutdown"}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "status" => {
            let capability = "adapter.status";
            let request_id = "req-adapter-status";
            let plugin_id = required_flag(rest, "--plugin-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-220", message))?;
            let db_path = context.layout.root.join("adapters.db");
            let registry = PersistentAdapterRegistry::open(&db_path)
                .map_err(|e| emit_error(capability, request_id, "CLI-221", e))?;
            let manifest = registry
                .lookup(&plugin_id)
                .map_err(|e| emit_error(capability, request_id, "CLI-222", e))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"plugin_id": plugin_id, "manifest": manifest, "status": manifest.status}),
                vec![],
                vec![],
                vec![],
            )
        }
        _ => Err(emit_usage_error(
            "adapter",
            "req-adapter",
            "usage: avctl adapter <load|shutdown|status> [flags]",
        )),
    }
}

// ---------------------------------------------------------------------------
// WSR-02 — hub lifecycle CLI
// ---------------------------------------------------------------------------

fn handle_hub(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "register" => {
            let capability = "hub.register";
            let request_id = "req-hub-register";
            let hub_id = required_flag(rest, "--hub-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-300", message))?;
            let resource_kind = required_flag(rest, "--kind")
                .map_err(|message| emit_error(capability, request_id, "CLI-301", message))?;
            let locator = required_flag(rest, "--locator")
                .map_err(|message| emit_error(capability, request_id, "CLI-302", message))?;
            let provides: Vec<String> = rest
                .iter()
                .zip(rest.iter().skip(1))
                .filter(|(k, _)| k.as_str() == "--provides")
                .map(|(_, v)| v.clone())
                .collect();
            let descriptor = vidodo_ir::ResourceHubDescriptor {
                hub_id: hub_id.clone(),
                resource_kind,
                version: String::from("1.0.0"),
                locator,
                provides,
                compatibility: None,
                status: Some(String::from("available")),
                tags: Vec::new(),
            };
            let db_path = context.layout.root.join("hubs.db");
            let registry = PersistentHubRegistry::open(&db_path)
                .map_err(|e| emit_error(capability, request_id, "CLI-303", e))?;
            registry
                .register_hub(&descriptor)
                .map_err(|e| emit_error(capability, request_id, "CLI-304", e))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"hub_id": hub_id, "status": "registered"}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "resolve" => {
            let capability = "hub.resolve";
            let request_id = "req-hub-resolve";
            let resource_name = required_flag(rest, "--resource")
                .map_err(|message| emit_error(capability, request_id, "CLI-310", message))?;
            let db_path = context.layout.root.join("hubs.db");
            let registry = PersistentHubRegistry::open(&db_path)
                .map_err(|e| emit_error(capability, request_id, "CLI-311", e))?;
            let resolved = registry
                .resolve_resource(&resource_name)
                .map_err(|e| emit_error(capability, request_id, "CLI-312", e))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"hub_id": resolved.hub_id, "locator": resolved.locator, "resource_kind": resolved.resource_kind}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "status" => {
            let capability = "hub.status";
            let request_id = "req-hub-status";
            let hub_id = required_flag(rest, "--hub-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-320", message))?;
            let db_path = context.layout.root.join("hubs.db");
            let registry = PersistentHubRegistry::open(&db_path)
                .map_err(|e| emit_error(capability, request_id, "CLI-321", e))?;
            let descriptor = registry
                .lookup(&hub_id)
                .map_err(|e| emit_error(capability, request_id, "CLI-322", e))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"hub_id": hub_id, "descriptor": descriptor, "status": descriptor.status}),
                vec![],
                vec![],
                vec![],
            )
        }
        _ => Err(emit_usage_error(
            "hub",
            "req-hub",
            "usage: avctl hub <register|resolve|status> [flags]",
        )),
    }
}

fn handle_control(_context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, rest @ ..] if command == "bind" => {
            let capability = "control.bind";
            let request_id = "req-control-bind";
            let source_id = required_flag(rest, "--source-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-400", message))?;
            let protocol = required_flag(rest, "--protocol")
                .map_err(|message| emit_error(capability, request_id, "CLI-401", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"source_id": source_id, "protocol": protocol, "status": "bound"}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "unbind" => {
            let capability = "control.unbind";
            let request_id = "req-control-unbind";
            let source_id = required_flag(rest, "--source-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-410", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"source_id": source_id, "status": "unbound"}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, ..] if command == "list" => {
            let capability = "control.list";
            let request_id = "req-control-list";
            let bindings: Vec<serde_json::Value> = vec![];
            print_response(
                capability,
                request_id,
                "ok",
                json!({"count": bindings.len(), "bindings": bindings}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "status" => {
            let capability = "control.status";
            let request_id = "req-control-status";
            let source_id = required_flag(rest, "--source-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-420", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"source_id": source_id, "protocol": "unknown", "status": "not_bound"}),
                vec![],
                vec![],
                vec![],
            )
        }
        _ => Err(emit_usage_error(
            "control",
            "req-control",
            "usage: avctl control <bind|unbind|list|status> [flags]",
        )),
    }
}

fn handle_template(_context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, ..] if command == "list" => {
            let capability = "template.list";
            let request_id = "req-template-list";
            let templates: Vec<serde_json::Value> = vec![];
            print_response(
                capability,
                request_id,
                "ok",
                json!({"count": templates.len(), "templates": templates}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "load" => {
            let capability = "template.load";
            let request_id = "req-template-load";
            let template_id = required_flag(rest, "--template-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-500", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"template_id": template_id, "template": {}}),
                vec![],
                vec![],
                vec![],
            )
        }
        _ => Err(emit_usage_error(
            "template",
            "req-template",
            "usage: avctl template <list|load> [flags]",
        )),
    }
}

fn handle_scene(_context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    match args {
        [command, ..] if command == "list" => {
            let capability = "scene.list";
            let request_id = "req-scene-list";
            let scenes: Vec<serde_json::Value> = vec![];
            print_response(
                capability,
                request_id,
                "ok",
                json!({"count": scenes.len(), "scenes": scenes}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "activate" => {
            let capability = "scene.activate";
            let request_id = "req-scene-activate";
            let scene_id = required_flag(rest, "--scene-id")
                .map_err(|message| emit_error(capability, request_id, "CLI-510", message))?;
            print_response(
                capability,
                request_id,
                "ok",
                json!({"scene_id": scene_id, "status": "activated"}),
                vec![],
                vec![],
                vec![],
            )
        }
        _ => Err(emit_usage_error(
            "scene",
            "req-scene",
            "usage: avctl scene <list|activate> [flags]",
        )),
    }
}

fn handle_demo(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    let examples_dir = context.repo_root.join("examples");
    match args {
        [command, ..] if command == "list" => {
            let capability = "demo.list";
            let request_id = "req-demo-list";
            let mut names: Vec<String> = Vec::new();
            if examples_dir.is_dir() {
                let entries = fs::read_dir(&examples_dir)
                    .map_err(|e| emit_error(capability, request_id, "DEMO-001", e.to_string()))?;
                for entry in entries.flatten() {
                    if entry.path().is_dir()
                        && let Some(name) = entry.file_name().to_str()
                    {
                        names.push(name.to_string());
                    }
                }
            }
            names.sort();
            print_response(
                capability,
                request_id,
                "ok",
                json!({"count": names.len(), "examples": names}),
                vec![],
                vec![],
                vec![],
            )
        }
        [command, rest @ ..] if command == "run" => {
            let capability = "demo.run";
            let request_id = "req-demo-run";
            let name = rest.first().cloned().ok_or_else(|| {
                emit_error(
                    capability,
                    request_id,
                    "CLI-600",
                    String::from("usage: avctl demo run <name>"),
                )
            })?;
            let plan_dir = examples_dir.join(&name);
            if !plan_dir.is_dir() {
                return Err(emit_error(
                    capability,
                    request_id,
                    "DEMO-002",
                    format!("example not found: {name}"),
                ));
            }
            let assets_file = plan_dir.join("asset-records.json");
            let bundle = load_plan_bundle(&plan_dir, &assets_file)
                .map_err(|msg| emit_error(capability, request_id, "DEMO-003", msg))?;
            let _show_id = bundle.show_id.clone();
            // validate
            let _report = validate_plan(&bundle);
            // compile
            let compiled = compile_plan(&bundle).map_err(|diagnostics| {
                let _ = print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    diagnostics,
                    vec![],
                    vec![],
                );
                ExitCode::from(1)
            })?;
            let timeline_entries = compiled.timeline.len();
            let revision = compiled.revision;
            let _rev_record = register_candidate(&context.layout, &compiled)
                .map_err(|msg| emit_error(capability, request_id, "DEMO-004", msg))?;
            // run
            let run_id = deterministic_run_id(&compiled.show_id, revision);
            let scheduled = simulate_run(&compiled, &run_id);
            let event_count = scheduled.summary.event_count;
            // trace
            let manifest = write_trace(
                &context.layout,
                &run_id,
                &compiled,
                "offline",
                &scheduled.summary,
                &scheduled.final_show_state,
                &scheduled.events,
                &scheduled.patch_decisions,
                &scheduled.resource_samples,
            )
            .map_err(|msg| emit_error(capability, request_id, "DEMO-005", msg))?;
            let trace_path = relative_to_repo(context, &manifest_path(&context.layout, &run_id));
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "name": name,
                    "timeline_entries": timeline_entries,
                    "total_events": event_count,
                    "revision": revision,
                    "trace_bundle_id": manifest.trace_bundle_id,
                    "trace_path": trace_path
                }),
                vec![],
                vec![trace_path.clone()],
                vec![],
            )
        }
        _ => Err(emit_usage_error("demo", "req-demo", "usage: avctl demo <list|run> [name]")),
    }
}

fn load_plan_bundle(plan_dir: &Path, assets_file: &Path) -> Result<PlanBundle, String> {
    let set_plan: SetPlan = read_json(&plan_dir.join("set-plan.json"))?;
    let audio_dsl: AudioDsl = read_json(&plan_dir.join("audio-dsl.json"))?;
    let visual_dsl: VisualDsl = read_json(&plan_dir.join("visual-dsl.json"))?;
    let constraint_set: ConstraintSet = read_json(&plan_dir.join("constraint-set.json"))?;
    let asset_records: Vec<AssetRecord> = read_json(assets_file)?;

    let lighting_topology_path = plan_dir.join("lighting-topology.json");
    let lighting_topology: Option<LightingTopology> = if lighting_topology_path.exists() {
        Some(read_json(&lighting_topology_path)?)
    } else {
        None
    };

    let cue_set_path = plan_dir.join("cue-set.json");
    let cue_sets: Vec<CueSet> =
        if cue_set_path.exists() { read_json(&cue_set_path)? } else { Vec::new() };

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

fn load_patch(path: &Path) -> Result<LivePatchProposal, String> {
    read_json(path)
}

fn persist_revision(
    context: &CommandContext,
    revision: &CompiledRevision,
) -> Result<Vec<String>, String> {
    let show_root = context.layout.revisions.join(slug(&revision.show_id));
    if revision.revision == 1 && show_root.exists() {
        fs::remove_dir_all(&show_root)
            .map_err(|error| format!("failed to reset {}: {error}", show_root.display()))?;
    }

    let revision_dir = context.layout.revision_dir(&revision.show_id, revision.revision);
    fs::create_dir_all(&revision_dir)
        .map_err(|error| format!("failed to create {}: {error}", revision_dir.display()))?;

    let mut artifacts = Vec::new();
    let revision_artifacts: Vec<(&str, Value)> = vec![
        (
            "revision.json",
            serde_json::to_value(revision)
                .map_err(|e| format!("failed to serialize revision: {e}"))?,
        ),
        (
            "set-plan.json",
            serde_json::to_value(&revision.set_plan)
                .map_err(|e| format!("failed to serialize set-plan: {e}"))?,
        ),
        (
            "audio-dsl.json",
            serde_json::to_value(&revision.audio_dsl)
                .map_err(|e| format!("failed to serialize audio-dsl: {e}"))?,
        ),
        (
            "visual-dsl.json",
            serde_json::to_value(&revision.visual_dsl)
                .map_err(|e| format!("failed to serialize visual-dsl: {e}"))?,
        ),
        (
            "constraint-set.json",
            serde_json::to_value(&revision.constraint_set)
                .map_err(|e| format!("failed to serialize constraint-set: {e}"))?,
        ),
        (
            "asset-records.json",
            serde_json::to_value(&revision.asset_records)
                .map_err(|e| format!("failed to serialize asset-records: {e}"))?,
        ),
        (
            "structure-ir.json",
            serde_json::to_value(&revision.structure_ir)
                .map_err(|e| format!("failed to serialize structure-ir: {e}"))?,
        ),
        (
            "performance-ir.json",
            serde_json::to_value(&revision.performance_ir)
                .map_err(|e| format!("failed to serialize performance-ir: {e}"))?,
        ),
        (
            "visual-ir.json",
            serde_json::to_value(&revision.visual_ir)
                .map_err(|e| format!("failed to serialize visual-ir: {e}"))?,
        ),
        (
            "timeline.json",
            serde_json::to_value(&revision.timeline)
                .map_err(|e| format!("failed to serialize timeline: {e}"))?,
        ),
    ];
    for (filename, value) in revision_artifacts {
        let path = revision_dir.join(filename);
        write_json(&path, &value)?;
        artifacts.push(relative_to_repo(context, &path));
    }

    if let Some(decision) = revision.patch_history.last() {
        let path = revision_dir.join("patch-decision.json");
        write_json(&path, decision)?;
        artifacts.push(relative_to_repo(context, &path));
    }

    // Register in SQLite revision catalog
    let _ = register_candidate(&context.layout, revision);

    Ok(artifacts)
}

fn load_revision(
    context: &CommandContext,
    show_id: &str,
    revision: u64,
) -> Result<CompiledRevision, String> {
    read_json(&context.layout.revision_dir(show_id, revision).join("revision.json"))
}

fn load_latest_revision(
    context: &CommandContext,
    show_id: &str,
) -> Result<CompiledRevision, String> {
    let show_root = context.layout.revisions.join(slug(show_id));
    let entries = fs::read_dir(&show_root)
        .map_err(|error| format!("failed to read {}: {error}", show_root.display()))?;
    let latest_revision = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.strip_prefix("revision-")?.parse::<u64>().ok()
        })
        .max()
        .ok_or_else(|| format!("no revision artifacts found for show {}", show_id))?;

    load_revision(context, show_id, latest_revision)
}

fn required_flag(args: &[String], flag: &str) -> Result<String, String> {
    optional_flag(args, flag).ok_or_else(|| format!("missing required flag {}", flag))
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find(|window| window[0] == flag).map(|window| window[1].clone())
}

fn parse_csv_list(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|item| !item.is_empty()).map(String::from).collect()
}

fn parse_u64(value: &str, flag: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|error| format!("{} expects an unsigned integer: {error}", flag))
}

fn resolve_path(context: &CommandContext, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() { candidate } else { context.repo_root.join(candidate) }
}

fn asset_pack_manifest_path(source_path: &Path) -> PathBuf {
    source_path.join("vidodo-asset-pack.json")
}

fn relative_to_repo(context: &CommandContext, path: &Path) -> String {
    path.strip_prefix(&context.repo_root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn default_plan_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tests/fixtures/plans/minimal-show")
}

fn default_assets_file(context: &CommandContext) -> Result<PathBuf, String> {
    let selection = list_compile_assets(&context.layout)?;
    if selection.published_asset_count == 0 {
        return Ok(context.repo_root.join("tests/fixtures/assets/asset-records.json"));
    }

    if selection.eligible_assets.is_empty() {
        return Err(String::from(
            "asset registry contains published assets, but none are compile_ready or warmed; publish eligible assets or pass --assets-file to override",
        ));
    }

    let snapshot_path = context.layout.exports.join("compile-ready-asset-records.json");
    write_json(&snapshot_path, &selection.eligible_assets)?;
    Ok(snapshot_path)
}

fn default_patch_file(repo_root: &Path) -> PathBuf {
    repo_root.join("tests/fixtures/patches/minimal-local-content-patch.json")
}

fn deterministic_run_id(show_id: &str, revision: u64) -> String {
    format!("run-{}-rev-{revision}", slug(show_id))
}

fn emit_error(capability: &str, request_id: &str, code: &str, message: String) -> ExitCode {
    let _ = print_response(
        capability,
        request_id,
        "error",
        json!({}),
        vec![Diagnostic::error(code, message)],
        vec![],
        vec![],
    );
    ExitCode::from(1)
}

fn emit_usage_error(capability: &str, request_id: &str, usage: &str) -> ExitCode {
    emit_error(capability, request_id, "CLI-USAGE", usage.to_string())
}

fn print_usage() {
    eprintln!("avctl doctor");
    eprintln!(
        "avctl asset ingest --source-dir <path> --declared-kind <kind> [--tags tag1,tag2] [--asset-namespace <namespace>]"
    );
    eprintln!("avctl asset list [--kind <kind>] [--tag <tag>]");
    eprintln!("avctl asset show --asset-id <id>");
    eprintln!("avctl plan validate --plan-dir <path> [--assets-file <path>]");
    eprintln!("avctl compile run --plan-dir <path> [--assets-file <path>]");
    eprintln!("avctl run start --show-id <show-id> --revision <revision>");
    eprintln!("avctl run status --show-id <show-id>");
    eprintln!("avctl patch check --show-id <show-id> --patch-file <path>");
    eprintln!("avctl patch submit --show-id <show-id> --patch-file <path>");
    eprintln!("avctl patch rollback --show-id <show-id> --patch-id <patch-id>");
    eprintln!(
        "avctl patch deferred-rollback --show-id <show-id> --patch-id <id> --anomaly <reason> [--run-id <run-id>]"
    );
    eprintln!("avctl trace show --run-id <run-id>");
    eprintln!("avctl trace events --run-id <run-id> [--from-bar N] [--to-bar N]");
    eprintln!("avctl eval run --show-id <show-id> [--run-id <run-id>]");
    eprintln!("avctl export audio --run-id <run-id>");
    eprintln!("avctl system capabilities");
}

fn print_response(
    capability: &str,
    request_id: &str,
    status: &str,
    data: Value,
    diagnostics: Vec<Diagnostic>,
    artifacts: Vec<String>,
    next_actions: Vec<String>,
) -> Result<(), ExitCode> {
    let response = ResponseEnvelope::new(
        status,
        capability,
        request_id,
        data,
        diagnostics,
        artifacts,
        next_actions,
    );
    print_json(&response);
    if status == "error" { Err(ExitCode::from(1)) } else { Ok(()) }
}

fn print_json<T>(value: &T)
where
    T: serde::Serialize,
{
    match serde_json::to_string_pretty(value) {
        Ok(serialized) => println!("{serialized}"),
        Err(error) => eprintln!("failed to serialize JSON output: {error}"),
    }
}
