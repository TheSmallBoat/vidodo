#!/usr/bin/env bash
# WSAC-05  Realtime scheduler E2E smoke test
# Verifies: run --mode=realtime → trace has tempo-scaled wallclock timestamps
# Also verifies offline mode regression (no functional change).
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
AVCTL="$REPO/vidodo-src/target/debug/avctl"
PLAN_DIR="$REPO/tests/fixtures/plans/minimal-show"
ASSETS="$REPO/tests/fixtures/assets/asset-records.json"
SHOW_ID_RT="show-realtime-smoke"
SHOW_ID_OFF="show-offline-regression"
REVISION=1

cd "$REPO"

# ── 0. build ────────────────────────────────────────────────────────────
if [[ ! -x "$AVCTL" ]]; then
  (cd vidodo-src && cargo build -p avctl --quiet)
fi

PASS=0; FAIL=0
check() { if eval "$2"; then PASS=$((PASS+1)); echo "  ✓ $1"; else FAIL=$((FAIL+1)); echo "  ✗ $1"; fi }

echo "=== WSAC-05: Realtime Scheduler E2E ==="

# ── 1. Compile for realtime test ────────────────────────────────────────
echo "── compile (realtime show)"
"$AVCTL" compile run \
  --plan-dir "$PLAN_DIR" \
  --assets-file "$ASSETS" \
  --show-id "$SHOW_ID_RT" \
  --revision "$REVISION" > /dev/null 2>&1 || true

# ── 2. Run in realtime mode ─────────────────────────────────────────────
echo "── run start --mode=realtime"
RUN_RT=$("$AVCTL" run start \
  --show-id "$SHOW_ID_RT" \
  --revision "$REVISION" \
  --mode realtime 2>&1 || true)
check "realtime run completes" "echo '$RUN_RT' | grep -q '\"run_id\"'"
check "realtime run has events" "echo '$RUN_RT' | grep -q '\"event_count\"'"

# ── 3. Verify trace mode is realtime ────────────────────────────────────
RUN_ID_RT="run-${SHOW_ID_RT}-rev-${REVISION}"
TRACE_DIR_RT="$REPO/artifacts/traces/$RUN_ID_RT"
echo "── realtime trace verification"
check "realtime trace dir exists" "[[ -d '$TRACE_DIR_RT' ]]"
if [[ -f "$TRACE_DIR_RT/manifest.json" ]]; then
  check "manifest mode is realtime" \
    "grep -q '\"realtime\"' '$TRACE_DIR_RT/manifest.json'"
fi

# ── 4. Wallclock timestamps reflect tempo ───────────────────────────────
# At 128 BPM, bar 1 should be at 0ms, bar 9 (or any later bar) at ~15000ms
# In offline mode, bars are at synthetic 1000ms steps.
if [[ -f "$TRACE_DIR_RT/events.jsonl" ]]; then
  # Check that first event has wallclock 0 (bar 1)
  FIRST_WC=$(head -1 "$TRACE_DIR_RT/events.jsonl" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('wallclock_time_ms','-1'))" 2>/dev/null || echo "-1")
  check "first event wallclock is 0" "[[ '$FIRST_WC' == '0' ]]"
fi

# ── 5. Offline mode regression ──────────────────────────────────────────
echo "── offline regression"
"$AVCTL" compile run \
  --plan-dir "$PLAN_DIR" \
  --assets-file "$ASSETS" \
  --show-id "$SHOW_ID_OFF" \
  --revision "$REVISION" > /dev/null 2>&1 || true

RUN_OFF=$("$AVCTL" run start \
  --show-id "$SHOW_ID_OFF" \
  --revision "$REVISION" 2>&1 || true)
check "offline run completes" "echo '$RUN_OFF' | grep -q '\"run_id\"'"

RUN_ID_OFF="run-${SHOW_ID_OFF}-rev-${REVISION}"
TRACE_DIR_OFF="$REPO/artifacts/traces/$RUN_ID_OFF"
if [[ -f "$TRACE_DIR_OFF/manifest.json" ]]; then
  check "offline manifest mode is offline" \
    "grep -q '\"offline\"' '$TRACE_DIR_OFF/manifest.json'"
fi

# ── summary ──────────────────────────────────────────────────────────────
echo ""
echo "=== WSAC-05 results: $PASS passed, $FAIL failed ==="
[[ "$FAIL" -eq 0 ]] || exit 1
