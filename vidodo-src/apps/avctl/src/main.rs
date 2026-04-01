use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};
use vidodo_compiler::compile_plan;
use vidodo_ir::{
    AssetRecord, AudioDsl, CompiledRevision, ConstraintSet, Diagnostic, LivePatchProposal,
    PlanBundle, ResponseEnvelope, SetPlan, VisualDsl,
};
use vidodo_patch_manager::{apply_patch, check_patch, rollback_patch};
use vidodo_scheduler::{RunStatusRecord, simulate_run};
use vidodo_storage::{ArtifactLayout, discover_repo_root, read_json, slug, write_json};
use vidodo_trace::{load_events, load_manifest, manifest_path, write_trace};
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
        "plan" => handle_plan(&context, &args[1..]),
        "compile" => handle_compile(&context, &args[1..]),
        "run" => handle_run(&context, &args[1..]),
        "patch" => handle_patch(&context, &args[1..]),
        "trace" => handle_trace(&context, &args[1..]),
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
    let assets_file = default_assets_file(&context.repo_root);
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
        print_response(capability, request_id, "error", json!({}), diagnostics, vec![], vec![])
            .err()
            .unwrap()
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
        print_response(
            capability,
            request_id,
            "error",
            json!({}),
            diagnostics,
            artifacts.clone(),
            vec![],
        )
        .err()
        .unwrap()
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
    )
    .map_err(|message| emit_error(capability, request_id, "CLI-005", message))?;
    let manifest_file = manifest_path(&context.layout, &run_id);
    let trace_manifest_ref = relative_to_repo(context, &manifest_file);
    artifacts.push(trace_manifest_ref.clone());

    let rollback = rollback_patch(&patched, &patch.patch_id).map_err(|diagnostic| {
        print_response(
            capability,
            request_id,
            "error",
            json!({}),
            vec![*diagnostic],
            artifacts.clone(),
            vec![],
        )
        .err()
        .unwrap()
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

fn handle_plan(context: &CommandContext, args: &[String]) -> Result<(), ExitCode> {
    let capability = "plan.validate";
    let request_id = "req-plan-validate";
    match args {
        [command, rest @ ..] if command == "validate" => {
            let plan_dir = required_flag(rest, "--plan-dir")
                .map_err(|message| emit_error(capability, request_id, "CLI-010", message))?;
            let asset_file = optional_flag(rest, "--assets-file")
                .map(|value| resolve_path(context, &value))
                .unwrap_or_else(|| default_assets_file(&context.repo_root));
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
                    "section_count": plan.set_plan.sections.len(),
                    "audio_layer_count": plan.audio_dsl.layers.len(),
                    "visual_scene_count": plan.visual_dsl.scenes.len()
                }),
                diagnostics,
                vec![],
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
            let asset_file = optional_flag(rest, "--assets-file")
                .map(|value| resolve_path(context, &value))
                .unwrap_or_else(|| default_assets_file(&context.repo_root));
            let plan = load_plan_bundle(&resolve_path(context, &plan_dir), &asset_file)
                .map_err(|message| emit_error(capability, request_id, "CLI-021", message))?;
            let compiled = compile_plan(&plan).map_err(|diagnostics| {
                print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    diagnostics,
                    vec![],
                    vec![],
                )
                .err()
                .unwrap()
            })?;
            let artifacts = persist_revision(context, &compiled)
                .map_err(|message| emit_error(capability, request_id, "CLI-022", message))?;

            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "show_id": compiled.show_id,
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
            let scheduled = simulate_run(&compiled, &run_id);
            let manifest = write_trace(
                &context.layout,
                &run_id,
                &compiled,
                "offline",
                &scheduled.summary,
                &scheduled.final_show_state,
                &scheduled.events,
            )
            .map_err(|message| emit_error(capability, request_id, "CLI-033", message))?;
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
                print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    diagnostics,
                    vec![],
                    vec![],
                )
                .err()
                .unwrap()
            })?;
            let artifacts = persist_revision(context, &patched)
                .map_err(|message| emit_error(capability, request_id, "CLI-048", message))?;
            let decision = patched.patch_history.last().cloned().unwrap();
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
                print_response(
                    capability,
                    request_id,
                    "error",
                    json!({}),
                    vec![*diagnostic],
                    vec![],
                    vec![],
                )
                .err()
                .unwrap()
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
            let events = load_events(&context.layout, &run_id)
                .map_err(|message| emit_error(capability, request_id, "CLI-063", message))?;
            let event_log = context.layout.trace_dir(&run_id).join("events.jsonl");
            print_response(
                capability,
                request_id,
                "ok",
                json!({
                    "run_id": run_id,
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

fn load_plan_bundle(plan_dir: &Path, assets_file: &Path) -> Result<PlanBundle, String> {
    let set_plan: SetPlan = read_json(&plan_dir.join("set-plan.json"))?;
    let audio_dsl: AudioDsl = read_json(&plan_dir.join("audio-dsl.json"))?;
    let visual_dsl: VisualDsl = read_json(&plan_dir.join("visual-dsl.json"))?;
    let constraint_set: ConstraintSet = read_json(&plan_dir.join("constraint-set.json"))?;
    let asset_records: Vec<AssetRecord> = read_json(assets_file)?;
    let show_id = set_plan.show_id.clone();
    Ok(PlanBundle {
        show_id,
        base_revision: 0,
        set_plan,
        audio_dsl,
        visual_dsl,
        constraint_set,
        asset_records,
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
    for (filename, value) in [
        ("revision.json", serde_json::to_value(revision).unwrap()),
        ("set-plan.json", serde_json::to_value(&revision.set_plan).unwrap()),
        ("audio-dsl.json", serde_json::to_value(&revision.audio_dsl).unwrap()),
        ("visual-dsl.json", serde_json::to_value(&revision.visual_dsl).unwrap()),
        ("constraint-set.json", serde_json::to_value(&revision.constraint_set).unwrap()),
        ("asset-records.json", serde_json::to_value(&revision.asset_records).unwrap()),
        ("structure-ir.json", serde_json::to_value(&revision.structure_ir).unwrap()),
        ("performance-ir.json", serde_json::to_value(&revision.performance_ir).unwrap()),
        ("visual-ir.json", serde_json::to_value(&revision.visual_ir).unwrap()),
        ("timeline.json", serde_json::to_value(&revision.timeline).unwrap()),
    ] {
        let path = revision_dir.join(filename);
        write_json(&path, &value)?;
        artifacts.push(relative_to_repo(context, &path));
    }

    if let Some(decision) = revision.patch_history.last() {
        let path = revision_dir.join("patch-decision.json");
        write_json(&path, decision)?;
        artifacts.push(relative_to_repo(context, &path));
    }

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

fn parse_u64(value: &str, flag: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|error| format!("{} expects an unsigned integer: {error}", flag))
}

fn resolve_path(context: &CommandContext, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() { candidate } else { context.repo_root.join(candidate) }
}

fn relative_to_repo(context: &CommandContext, path: &Path) -> String {
    path.strip_prefix(&context.repo_root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn default_plan_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tests/fixtures/plans/minimal-show")
}

fn default_assets_file(repo_root: &Path) -> PathBuf {
    repo_root.join("tests/fixtures/assets/asset-records.json")
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
    eprintln!("avctl plan validate --plan-dir <path> [--assets-file <path>]");
    eprintln!("avctl compile run --plan-dir <path> [--assets-file <path>]");
    eprintln!("avctl run start --show-id <show-id> --revision <revision>");
    eprintln!("avctl run status --show-id <show-id>");
    eprintln!("avctl patch check --show-id <show-id> --patch-file <path>");
    eprintln!("avctl patch submit --show-id <show-id> --patch-file <path>");
    eprintln!("avctl patch rollback --show-id <show-id> --patch-id <patch-id>");
    eprintln!("avctl trace show --run-id <run-id>");
    eprintln!("avctl trace events --run-id <run-id>");
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
