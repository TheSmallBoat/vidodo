use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let Some(command) = env::args().nth(1) else {
        return Err(String::from("usage: cargo xtask <fmt|fmt-check|clippy|audit|test|bench|ci>"));
    };

    match command.as_str() {
        "fmt" => cargo(&["fmt", "--all"]),
        "fmt-check" => cargo(&["fmt", "--all", "--check"]),
        "clippy" => cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ]),
        "audit" => cargo_audit(&["audit"]),
        "test" => cargo(&["test", "--workspace", "--all-targets"]),
        "bench" => cargo(&["bench", "--workspace"]),
        "ci" => {
            cargo(&["fmt", "--all", "--check"])?;
            cargo(&[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ])?;
            cargo(&["test", "--workspace", "--all-targets"])?;
            cargo_audit(&["audit"])
        }
        _ => Err(String::from("unknown xtask command")),
    }
}

fn cargo(args: &[&str]) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(args)
        .current_dir(workspace_root())
        .status()
        .map_err(|error| format!("failed to invoke cargo {args:?}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo command failed: cargo {}", args.join(" ")))
    }
}

fn cargo_audit(args: &[&str]) -> Result<(), String> {
    let status = Command::new("cargo-audit")
        .args(args)
        .current_dir(workspace_root())
        .status()
        .map_err(|error| {
            format!(
                "failed to invoke cargo-audit: {error}. Install it with `cargo install cargo-audit --locked`"
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(String::from("cargo-audit reported at least one finding"))
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask workspace should have a parent directory")
        .to_path_buf()
}
