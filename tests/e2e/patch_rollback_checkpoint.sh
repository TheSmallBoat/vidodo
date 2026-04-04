#!/usr/bin/env bash
# WSS-04: Patch rollback complete E2E — multi-backend patch → rollback → checkpoint verification
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

# 1. Compile and patch
cargo run -p avctl -- compile run --plan-dir "$plan_dir" --assets-file "$assets_file" >/dev/null
cargo run -p avctl -- patch submit --show-id "$show_id" --patch-file "$patch_file" >/dev/null

# 2. Run with patched revision
cargo run -p avctl -- run start --show-id "$show_id" --revision 2 >/dev/null

# 3. Rollback the patch
cargo run -p avctl -- patch rollback --show-id "$show_id" --patch-id patch-phase0-pad-swap >/dev/null

# 4. Deferred rollback (writes checkpoint-like artifact)
cargo run -p avctl -- patch deferred-rollback --show-id "$show_id" --patch-id patch-phase0-pad-swap --anomaly "gpu_overload" --run-id "$run_id" >/dev/null

# --- Assertions ---

# A1: Rollback decision artifact exists
test -f "$repo_root/artifacts/revisions/show-phase0-minimal/rollback-patch-phase0-pad-swap.json"

# A2: Deferred rollback artifact exists
test -f "$repo_root/artifacts/revisions/show-phase0-minimal/deferred-rollback-patch-phase0-pad-swap.json"

# A3: Rollback decision contains "rolled_back"
rollback_file="$repo_root/artifacts/revisions/show-phase0-minimal/rollback-patch-phase0-pad-swap.json"
if ! grep -q '"rolled_back"' "$rollback_file"; then
  echo "FAIL: rollback decision missing 'rolled_back'" >&2
  exit 1
fi

# A4: Deferred rollback decision contains "deferred_rollback"
deferred_file="$repo_root/artifacts/revisions/show-phase0-minimal/deferred-rollback-patch-phase0-pad-swap.json"
if ! grep -q '"deferred_rollback"' "$deferred_file"; then
  echo "FAIL: deferred rollback missing 'deferred_rollback'" >&2
  exit 1
fi

# A5: Trace bundle was written
test -f "$repo_root/artifacts/traces/$run_id/manifest.json"
test -f "$repo_root/artifacts/traces/$run_id/events.jsonl"
test -f "$repo_root/artifacts/traces/$run_id/patch-decisions.jsonl"

# A6: Patch decisions in trace contain the patch id
if ! grep -q 'patch-phase0-pad-swap' "$repo_root/artifacts/traces/$run_id/patch-decisions.jsonl"; then
  echo "FAIL: patch-decisions.jsonl missing patch id" >&2
  exit 1
fi

# A7: Rollback decision has fallback_revision == 1
if ! grep -q '"fallback_revision":1' "$rollback_file" && ! grep -q '"fallback_revision": 1' "$rollback_file"; then
  echo "FAIL: rollback fallback_revision != 1" >&2
  exit 1
fi

# A8: Deferred rollback contains anomaly reason
if ! grep -q 'gpu_overload' "$deferred_file"; then
  echo "FAIL: deferred rollback missing anomaly reason" >&2
  exit 1
fi

echo "patch_rollback_checkpoint: all checks passed"
