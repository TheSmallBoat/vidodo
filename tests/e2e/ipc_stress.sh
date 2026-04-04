#!/usr/bin/env bash
# WSAD-06  IPC stress test
# Verifies: 10000 events → zero loss → memory stable
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO"

PASS=0; FAIL=0
check() { if eval "$2"; then PASS=$((PASS+1)); echo "  ✓ $1"; else FAIL=$((FAIL+1)); echo "  ✗ $1"; fi }

echo "=== WSAD-06: IPC Stress Test ==="

# ── 1. Run the stress tests ────────────────────────────────────────────
echo "── stress tests (10000 events × 3 runtimes)"
STRESS_OUT=$(cd vidodo-src && cargo test -p vidodo-ipc --lib -- ipc_integration::stress 2>&1 || true)
check "stress tests compile" "echo '$STRESS_OUT' | grep -q 'Running'"
check "stress_ten_thousand passed" "echo '$STRESS_OUT' | grep -q 'stress_ten_thousand_events_zero_loss ... ok'"
check "stress_memory_stability passed" "echo '$STRESS_OUT' | grep -q 'stress_memory_stability ... ok'"
check "all stress tests ok" "echo '$STRESS_OUT' | grep -q 'test result: ok'"

# ── 2. Run all IPC tests to verify no regressions ──────────────────────
echo "── full IPC test suite"
FULL_OUT=$(cd vidodo-src && cargo test -p vidodo-ipc --lib 2>&1 || true)
TOTAL=$(echo "$FULL_OUT" | grep 'test result:' | head -1)
check "full IPC test suite passes" "echo '$TOTAL' | grep -q 'ok'"

# Count total tests
TEST_COUNT=$(echo "$FULL_OUT" | grep -oE '[0-9]+ passed' | head -1 | grep -oE '[0-9]+')
echo "    Total IPC tests: $TEST_COUNT"

# ── summary ──────────────────────────────────────────────────────────────
echo ""
echo "=== WSAD-06 Stress results: $PASS passed, $FAIL failed ==="
[[ "$FAIL" -eq 0 ]] || exit 1
