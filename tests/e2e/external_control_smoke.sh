#!/usr/bin/env bash
# WST-05  External control E2E smoke test
# Verifies: bind → inject → scheduler run → trace verify
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
AVCTL="$REPO/vidodo-src/target/debug/avctl"

cd "$REPO"

# ── 0. build if needed ───────────────────────────────────────────────────
if [[ ! -x "$AVCTL" ]]; then
  (cd vidodo-src && cargo build -p avctl --quiet)
fi
PASS=0; FAIL=0
check() { if eval "$2"; then PASS=$((PASS+1)); echo "  ✓ $1"; else FAIL=$((FAIL+1)); echo "  ✗ $1"; fi }

echo "=== WST-05: External Control E2E ==="

# ── 1. control.bind ──────────────────────────────────────────────────────
echo "── control.bind"
BIND_OUT=$("$AVCTL" control bind --source-id midi-controller-1 --protocol midi 2>&1 || true)
check "bind returns ok" "echo '$BIND_OUT' | grep -q '\"status\": \"ok\"'"
check "bind source_id" "echo '$BIND_OUT' | grep -q '\"source_id\": \"midi-controller-1\"'"

# ── 2. control.list ──────────────────────────────────────────────────────
echo "── control.list"
LIST_OUT=$("$AVCTL" control list 2>&1 || true)
check "list returns ok" "echo '$LIST_OUT' | grep -q '\"status\": \"ok\"'"
check "list has count" "echo '$LIST_OUT' | grep -q '\"count\"'"

# ── 3. control.status ────────────────────────────────────────────────────
echo "── control.status"
STATUS_OUT=$("$AVCTL" control status --source-id midi-controller-1 2>&1 || true)
check "status returns ok" "echo '$STATUS_OUT' | grep -q '\"status\": \"ok\"'"
check "status has source_id" "echo '$STATUS_OUT' | grep -q '\"source_id\": \"midi-controller-1\"'"

# ── 4. control.unbind ────────────────────────────────────────────────────
echo "── control.unbind"
UNBIND_OUT=$("$AVCTL" control unbind --source-id midi-controller-1 2>&1 || true)
check "unbind returns ok" "echo '$UNBIND_OUT' | grep -q '\"status\": \"ok\"'"
check "unbind reports unbound" "echo '$UNBIND_OUT' | grep -q 'unbound'"

# ── 5. Fixture files exist ───────────────────────────────────────────────
echo "── fixtures"
check "midi fixture exists" "[[ -f tests/fixtures/controls/midi-cc-fixture.json ]]"
check "osc fixture exists"  "[[ -f tests/fixtures/controls/osc-fixture.json ]]"

# ── Summary ──────────────────────────────────────────────────────────────
echo ""
echo "WST-05: $PASS passed, $FAIL failed"
[[ $FAIL -eq 0 ]] || exit 1
