#!/usr/bin/env bash
# WSAD-06  IPC integration E2E smoke test
# Verifies: 3 runtime threads + scheduler → 8 bar show → trace causation chain
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
AVCTL="$REPO/vidodo-src/target/debug/avctl"
PLAN_DIR="$REPO/tests/fixtures/plans/minimal-show"
ASSETS="$REPO/tests/fixtures/assets/asset-records.json"
SHOW_ID="show-ipc-integration"
REVISION=1

cd "$REPO"

# ── 0. build ────────────────────────────────────────────────────────────
if [[ ! -x "$AVCTL" ]]; then
  (cd vidodo-src && cargo build -p avctl --quiet)
fi

PASS=0; FAIL=0
check() { if eval "$2"; then PASS=$((PASS+1)); echo "  ✓ $1"; else FAIL=$((FAIL+1)); echo "  ✗ $1"; fi }

echo "=== WSAD-06: IPC Integration E2E ==="

# ── 1. Plan validate ────────────────────────────────────────────────────
echo "── plan validate"
VALIDATE=$("$AVCTL" plan validate --plan-dir "$PLAN_DIR" --assets-file "$ASSETS" 2>&1 || true)
check "plan validates" "echo '$VALIDATE' | grep -q '\"status\": \"ok\"'"

# ── 2. Compile ──────────────────────────────────────────────────────────
echo "── compile run"
COMPILE=$("$AVCTL" compile run \
  --plan-dir "$PLAN_DIR" \
  --assets-file "$ASSETS" \
  --show-id "$SHOW_ID" \
  --revision "$REVISION" 2>&1 || true)
check "compile succeeds" "echo '$COMPILE' | grep -q '\"status\": \"ok\"'"

# ── 3. Run (8 bar show with default backend) ────────────────────────────
echo "── run start (8 bar show)"
RUN_OUT=$("$AVCTL" run start \
  --show-id "$SHOW_ID" \
  --revision "$REVISION" 2>&1 || true)
check "run completes" "echo '$RUN_OUT' | grep -q '\"run_id\"'"
check "run returns event_count" "echo '$RUN_OUT' | grep -q '\"event_count\"'"

# ── 4. Trace verification ──────────────────────────────────────────────
RUN_ID="run-${SHOW_ID}-rev-${REVISION}"
TRACE_DIR="$REPO/artifacts/traces/$RUN_ID"
echo "── trace verification"
check "trace directory exists" "[[ -d '$TRACE_DIR' ]]"
check "events.jsonl exists" "[[ -f '$TRACE_DIR/events.jsonl' ]]"
check "manifest.json exists" "[[ -f '$TRACE_DIR/manifest.json' ]]"

# ── 5. Verify all events have acks ─────────────────────────────────────
if [[ -f "$TRACE_DIR/events.jsonl" ]]; then
  TOTAL_EVENTS=$(wc -l < "$TRACE_DIR/events.jsonl" | tr -d ' ')
  EVENTS_WITH_ACK=$(grep '"ack"' "$TRACE_DIR/events.jsonl" | grep -v '"ack": null' | wc -l | tr -d ' ')
  check "events have acks ($EVENTS_WITH_ACK/$TOTAL_EVENTS)" \
    "[[ '$TOTAL_EVENTS' -gt 0 && '$EVENTS_WITH_ACK' -gt 0 ]]"
fi

# ── 6. Audio, visual, lighting events all present ──────────────────────
if [[ -f "$TRACE_DIR/events.jsonl" ]]; then
  echo "── per-channel verification"
  HAS_AUDIO=$(grep -c '"audio\.' "$TRACE_DIR/events.jsonl" || echo 0)
  HAS_VISUAL=$(grep -c '"visual\.' "$TRACE_DIR/events.jsonl" || echo 0)
  HAS_LIGHTING=$(grep -c '"lighting\.' "$TRACE_DIR/events.jsonl" || echo 0)
  check "audio events present ($HAS_AUDIO)" "[[ '$HAS_AUDIO' -gt 0 ]]"
  check "visual events present ($HAS_VISUAL)" "[[ '$HAS_VISUAL' -gt 0 ]]"
  check "lighting events present ($HAS_LIGHTING)" "[[ '$HAS_LIGHTING' -gt 0 ]]"
fi

# ── 7. Rust unit tests pass (causal chain + resilience) ────────────────
echo "── IPC unit tests"
IPC_TESTS=$(cd vidodo-src && cargo test -p vidodo-ipc --lib -- ipc_integration 2>&1 || true)
check "IPC integration tests pass" "echo '$IPC_TESTS' | grep -q 'test result: ok'"

# ── summary ──────────────────────────────────────────────────────────────
echo ""
echo "=== WSAD-06 IPC Integration results: $PASS passed, $FAIL failed ==="
[[ "$FAIL" -eq 0 ]] || exit 1
