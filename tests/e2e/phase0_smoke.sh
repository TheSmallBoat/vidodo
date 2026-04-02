#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
plan_dir="$repo_root/tests/fixtures/plans/minimal-show"
assets_file="$repo_root/tests/fixtures/assets/asset-records.json"
patch_file="$repo_root/tests/fixtures/patches/minimal-local-content-patch.json"
show_id="show-phase0-minimal"
run_id="run-show-phase0-minimal-rev-2"

rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

cd "$repo_root/vidodo-src"

cargo run -p avctl -- plan validate --plan-dir "$plan_dir" --assets-file "$assets_file" >/dev/null
cargo run -p avctl -- compile run --plan-dir "$plan_dir" --assets-file "$assets_file" >/dev/null
cargo run -p avctl -- patch check --show-id "$show_id" --patch-file "$patch_file" >/dev/null
cargo run -p avctl -- patch submit --show-id "$show_id" --patch-file "$patch_file" >/dev/null
cargo run -p avctl -- run start --show-id "$show_id" --revision 2 >/dev/null
cargo run -p avctl -- run status --show-id "$show_id" >/dev/null
cargo run -p avctl -- trace show --run-id "$run_id" >/dev/null
cargo run -p avctl -- trace events --run-id "$run_id" >/dev/null
cargo run -p avctl -- trace events --run-id "$run_id" --from-bar 1 --to-bar 8 >/dev/null
cargo run -p avctl -- eval run --show-id "$show_id" --run-id "$run_id" >/dev/null
cargo run -p avctl -- export audio --run-id "$run_id" >/dev/null
cargo run -p avctl -- revision list --show-id "$show_id" >/dev/null
cargo run -p avctl -- patch rollback --show-id "$show_id" --patch-id patch-phase0-pad-swap >/dev/null
cargo run -p avctl -- patch deferred-rollback --show-id "$show_id" --patch-id patch-phase0-pad-swap --anomaly "gpu_overload" --run-id "$run_id" >/dev/null
cargo run -p visual-runtime -- --run-id "$run_id" >/dev/null
cargo run -p lighting-runtime -- --run-id "$run_id" >/dev/null
cargo run -p avctl -- system capabilities >/dev/null

test -f "$repo_root/artifacts/revisions/show-phase0-minimal/revision-2/patch-decision.json"
test -f "$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/manifest.json"
test -f "$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/events.jsonl"
test -f "$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/evaluation.json"
test -f "$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/patch-decisions.jsonl"
test -f "$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/resource-samples.jsonl"
test -f "$repo_root/artifacts/exports/run-show-phase0-minimal-rev-2/mix.wav"
test -f "$repo_root/artifacts/exports/run-show-phase0-minimal-rev-2/export-record.json"
test -f "$repo_root/artifacts/revisions/show-phase0-minimal/rollback-patch-phase0-pad-swap.json"
test -f "$repo_root/artifacts/revisions/show-phase0-minimal/deferred-rollback-patch-phase0-pad-swap.json"
test -f "$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/visual-acks.json"
test -f "$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/lighting-acks.json"

# WSJ-04: verify lighting-acks.json contains cue_executed and synced entries
lighting_acks="$repo_root/artifacts/traces/run-show-phase0-minimal-rev-2/lighting-acks.json"
if ! grep -q '"cue_executed"' "$lighting_acks"; then
  echo "FAIL: lighting-acks.json missing cue_executed" >&2
  exit 1
fi
if ! grep -q '"synced"' "$lighting_acks"; then
  echo "FAIL: lighting-acks.json missing synced" >&2
  exit 1
fi

echo "phase0_smoke: all checks passed"