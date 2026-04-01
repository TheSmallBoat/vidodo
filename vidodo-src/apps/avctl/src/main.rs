use std::env;
use std::process::ExitCode;

use serde_json::json;
use vidodo_compiler::compile_plan;
use vidodo_ir::PlanBundle;
use vidodo_scheduler::prepare_run_summary;
use vidodo_storage::ArtifactLayout;
use vidodo_trace::manifest_from_plan;
use vidodo_validator::validate_plan;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run() -> Result<(), ExitCode> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() || args.len() == 1 && args[0] == "help" {
        print_usage();
        return Ok(());
    }

    match args.as_slice() {
        [command] if command == "doctor" => {
            let plan = PlanBundle::minimal("doctor-smoke");
            let compiled = compile_plan(&plan).map_err(|diagnostics| {
                print_json(&diagnostics);
                ExitCode::from(1)
            })?;
            let response = json!({
                "workspace": "vidodo-src",
                "run_summary": prepare_run_summary(&compiled),
                "trace_manifest": manifest_from_plan("doctor-run", &compiled),
                "artifact_layout": ArtifactLayout::new("artifacts").root.display().to_string(),
                "diagnostics": validate_plan(&plan),
            });
            print_json(&response);
            Ok(())
        }
        [namespace, command, show_id] if namespace == "plan" && command == "validate" => {
            let plan = PlanBundle::minimal(show_id);
            match compile_plan(&plan) {
                Ok(compiled) => {
                    print_json(&compiled);
                    Ok(())
                }
                Err(diagnostics) => {
                    print_json(&diagnostics);
                    Err(ExitCode::from(1))
                }
            }
        }
        _ => {
            print_usage();
            Err(ExitCode::from(2))
        }
    }
}

fn print_usage() {
    eprintln!("avctl doctor");
    eprintln!("avctl plan validate <show-id>");
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
