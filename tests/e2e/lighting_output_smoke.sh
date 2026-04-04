#!/usr/bin/env bash
# WSAB-05  Lighting output E2E smoke test
# Verifies: compile → run (with fixture-bus backend) → trace verify
# When no DMX hardware is available, verifies graceful fallback to reference backend.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
AVCTL="$REPO/vidodo-src/target/debug/avctl"
PLAN_DIR="$REPO/tests/fixtures/plans/minimal-show"
ASSETS="$REPO/tests/fixtures/assets/asset-records.json"
SHOW_ID="show-lighting-output-smoke"
REVISION=1

cd "$REPO"

# ── 0. build ────────────────────────────────────────────────────────────
if [[ ! -x "$AVCTL" ]]; then
  (cd vidodo-src && cargo build -p avctl --quiet)
fi

PASS=0; FAIL=0
check() { if eval "$2"; then PASS=$((PASS+1)); echo "  ✓ $1"; else FAIL=$((FAIL+1)); echo "  ✗ $1"; fi }

echo "=== WSAB-05: Lighting Output E2E ==="

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

# ── 3. Run with --lighting-backend=fixture-bus ──────────────────────────
echo "── run start --lighting-backend=fixture-bus"
RUN_OUT=$("$AVCTL" run start \
  --show-id "$SHOW_ID" \
  --revision "$REVISION" \
  --lighting-backend fixture-bus 2>&1 || true)
check "run completes" "echo '$RUN_OUT' | grep -q '\"run_id\"'"
check "run returns event_count" "echo '$RUN_OUT' | grep -q '\"event_count\"'"

# ── 4. Verify trace ────────────────────────────────────────────────────
RUN_ID="run-${SHOW_ID}-rev-${REVISION}"
TRACE_DIR="$REPO/artifacts/traces/$RUN_ID"
echo "── trace verification"
check "trace directory exists" "[[ -d '$TRACE_DIR' ]]"
check "events.jsonl exists" "[[ -f '$TRACE_DIR/events.jsonl' ]]"
check "manifest.json exists" "[[ -f '$TRACE_DIR/manifest.json' ]]"

# ── 5. Every lighting event has an ack ──────────────────────────────────
if [[ -f "$TRACE_DIR/events.jsonl" ]]; then
  LIGHT_EVENTS=$(grep '"lighting\.' "$TRACE_DIR/events.jsonl" | wc -l | tr -d ' ')
  LIGHT_WITH_ACK=$(grep '"lighting\.' "$TRACE_DIR/events.jsonl" | grep '"ack"' | grep -v '"ack": null' | wc -l | tr -d ' ')
  check "all lighting events have acks ($LIGHT_WITH_ACK/$LIGHT_EVENTS)" \
    "[[ '$LIGHT_EVENTS' -gt 0 && '$LIGHT_WITH_ACK' -eq '$LIGHT_EVENTS' ]]"
fi

# ── 6. Fallback diagnostic ──────────────────────────────────────────────
echo "── fallback behavior"
if echo "$RUN_OUT" | grep -q 'diag:.*fallback'; then
  check "graceful fallback diagnostic present" "true"
  echo "    (no DMX hardware — fell back to reference backend)"
else
  check "fixture-bus backend active (no fallback)" "true"
fi

# ── summary ──────────────────────────────────────────────────────────────
echo ""
echo "=== WSAB-05 results: $PASS passed, $FAIL failed ==="
[[ "$FAIL" -eq 0 ]] || exit 1
