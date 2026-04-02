#!/usr/bin/env bash
set -euo pipefail

# E2E negative-path tests: verify that avctl returns non-zero exit codes
# and structured error responses for invalid inputs.

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
plan_dir="$repo_root/tests/fixtures/plans/minimal-show"
assets_file="$repo_root/tests/fixtures/assets/asset-records.json"
show_id="show-phase0-minimal"

rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

cd "$repo_root/vidodo-src"

pass=0
fail=0

expect_failure() {
    local label="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        echo "FAIL  $label (expected non-zero exit)"
        fail=$((fail + 1))
    else
        echo "PASS  $label"
        pass=$((pass + 1))
    fi
}

expect_success() {
    local label="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        echo "PASS  $label"
        pass=$((pass + 1))
    else
        echo "FAIL  $label (expected zero exit)"
        fail=$((fail + 1))
    fi
}

# --- Missing flags ---
expect_failure "compile without plan-dir" \
    cargo run -p avctl -- compile run

expect_failure "run start without show-id" \
    cargo run -p avctl -- run start

expect_failure "patch check without show-id" \
    cargo run -p avctl -- patch check

# --- Non-existent show ---
expect_failure "run start with unknown show" \
    cargo run -p avctl -- run start --show-id nonexistent-show --revision 1

expect_failure "run status with unknown show" \
    cargo run -p avctl -- run status --show-id nonexistent-show

# --- Invalid patch: base revision mismatch ---
# First compile a valid revision so we have something to patch against
expect_success "setup: compile valid plan" \
    cargo run -p avctl -- compile run --plan-dir "$plan_dir" --assets-file "$assets_file"

expect_failure "patch check with mismatched base revision" \
    cargo run -p avctl -- patch check --show-id "$show_id" \
        --patch-file "$repo_root/tests/fixtures/patches/invalid-base-revision-mismatch.json"

# --- Invalid patch: out-of-range scope ---
expect_failure "patch check with out-of-range scope" \
    cargo run -p avctl -- patch check --show-id "$show_id" \
        --patch-file "$repo_root/tests/fixtures/patches/invalid-scope-out-of-range.json"

# --- Rollback of non-existent patch ---
expect_failure "rollback unknown patch id" \
    cargo run -p avctl -- patch rollback --show-id "$show_id" --patch-id nonexistent-patch

# --- Eval without a prior run ---
expect_failure "eval run without prior run" \
    cargo run -p avctl -- eval run --show-id "$show_id"

# --- Trace show for unknown run ---
expect_failure "trace show for unknown run" \
    cargo run -p avctl -- trace show --run-id nonexistent-run

# --- Export without prior run ---
expect_failure "export audio for unknown run" \
    cargo run -p avctl -- export audio --run-id nonexistent-run

# --- Revision commands without flags ---
expect_failure "revision list without show-id" \
    cargo run -p avctl -- revision list

expect_failure "revision publish without show-id" \
    cargo run -p avctl -- revision publish

expect_failure "revision archive for unknown revision" \
    cargo run -p avctl -- revision archive --show-id nonexistent --revision 999

# --- Deferred rollback without required flags ---
expect_failure "deferred-rollback without show-id" \
    cargo run -p avctl -- patch deferred-rollback

expect_failure "deferred-rollback without anomaly" \
    cargo run -p avctl -- patch deferred-rollback --show-id "$show_id" --patch-id some-patch

expect_failure "deferred-rollback for unknown patch" \
    cargo run -p avctl -- patch deferred-rollback --show-id "$show_id" --patch-id nonexistent --anomaly "test"

# --- trace events with invalid bar range (should still succeed if run exists) ---
# First do a full pipeline to have a run
expect_success "setup: submit patch and run" \
    cargo run -p avctl -- patch submit --show-id "$show_id" \
        --patch-file "$repo_root/tests/fixtures/patches/minimal-local-content-patch.json"
expect_success "setup: run patched revision" \
    cargo run -p avctl -- run start --show-id "$show_id" --revision 2

expect_success "trace events with --from-bar --to-bar returns filtered" \
    cargo run -p avctl -- trace events --run-id run-show-phase0-minimal-rev-2 --from-bar 999 --to-bar 1000

# --- Visual runtime with unknown run ---
expect_failure "visual-runtime with unknown run" \
    cargo run -p visual-runtime -- --run-id nonexistent-run

# --- System capabilities: always succeeds ---
expect_success "system capabilities list" \
    cargo run -p avctl -- system capabilities

# --- System: invalid subcommand ---
expect_failure "system with invalid subcommand" \
    cargo run -p avctl -- system unknown

echo ""
echo "Results: $pass passed, $fail failed"
if [ "$fail" -gt 0 ]; then
    exit 1
fi
