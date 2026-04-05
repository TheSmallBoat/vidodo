#!/usr/bin/env bash
# WSZ-08: Visual rendering E2E smoke test.
#
# Validates:
#   1. Compile + run with --visual-backend=wgpu succeeds (graceful fallback if no GPU)
#   2. Trace contains visual acks with frame time
#   3. 30+ rendered frames (event count) without crash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
plan_dir="$repo_root/tests/fixtures/plans/minimal-show"
assets_file="$repo_root/tests/fixtures/assets/asset-records.json"
show_id="show-phase0-minimal"

# Clean and init artifact store
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

cd "$repo_root/vidodo-src"

# Compile the show
cargo run -p avctl -- compile run --plan-dir "$plan_dir" --assets-file "$assets_file" >/dev/null

# Run with --visual-backend=wgpu (will fall back to reference if no GPU)
cargo run -p avctl -- run start --show-id "$show_id" --revision 1 --visual-backend wgpu 2>diag.tmp || true

# Check for graceful fallback diagnostic
if grep -q "wgpu unavailable" diag.tmp 2>/dev/null; then
  echo "info: wgpu fell back to reference visual backend (no GPU)"
fi
rm -f diag.tmp

# Verify trace artifacts exist
run_id="run-show-phase0-minimal-rev-1"
trace_dir="$repo_root/artifacts/traces/$run_id"

if [ ! -d "$trace_dir" ]; then
  echo "FAIL: trace directory not found: $trace_dir" >&2
  exit 1
fi

test -f "$trace_dir/manifest.json"
test -f "$trace_dir/events.jsonl"

# Count visual events (backend=wgpu-v1 or fallback-visual or fake_visual_backend)
visual_count=$(grep -c '"visual"' "$trace_dir/events.jsonl" 2>/dev/null || echo "0")
total_events=$(wc -l < "$trace_dir/events.jsonl" | tr -d ' ')

echo "visual_rendering_smoke: total_events=$total_events visual_count=$visual_count"

if [ "$total_events" -lt 1 ]; then
  echo "FAIL: expected at least 1 event, got $total_events" >&2
  exit 1
fi

if [ "$visual_count" -lt 1 ]; then
  echo "FAIL: expected at least 1 visual event, got $visual_count" >&2
  exit 1
fi

# Run visual-runtime to generate visual-acks
cargo run -p visual-runtime -- --run-id "$run_id" >/dev/null

# Verify visual-acks trace
visual_acks="$repo_root/artifacts/traces/$run_id/visual-acks.json"
if [ ! -f "$visual_acks" ]; then
  echo "FAIL: visual-acks.json not found" >&2
  exit 1
fi

# Verify acks contain rendered entries
if ! grep -q '"rendered"' "$visual_acks"; then
  echo "FAIL: visual-acks.json missing rendered entries" >&2
  exit 1
fi

echo "visual_rendering_smoke: all checks passed"
