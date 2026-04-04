#!/usr/bin/env bash
# WSW-03  Regression suite — runs all E2E test scripts in order.
# Exit on first failure unless VIDODO_REGRESSION_CONTINUE=1 is set.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO"

TOTAL=0; PASSED=0; FAILED=0; FAILED_LIST=""

run_suite() {
    local script="$1"
    local name
    name="$(basename "$script" .sh)"
    TOTAL=$((TOTAL + 1))
    echo ""
    echo "════════════════════════════════════════════════════════════"
    echo "  Running: $name"
    echo "════════════════════════════════════════════════════════════"
    if bash "$script"; then
        PASSED=$((PASSED + 1))
        echo "  ── $name: OK"
    else
        FAILED=$((FAILED + 1))
        FAILED_LIST="$FAILED_LIST $name"
        echo "  ── $name: FAILED"
        if [[ "${VIDODO_REGRESSION_CONTINUE:-0}" != "1" ]]; then
            echo ""
            echo "Regression suite aborted ($PASSED passed, $FAILED failed out of $TOTAL attempted)"
            exit 1
        fi
    fi
}

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Vidodo Regression Suite                                     ║"
echo "╚══════════════════════════════════════════════════════════════╝"

# Build once
echo "── Building workspace..."
(cd vidodo-src && cargo build --workspace --quiet 2>&1)

# Phase 0 – core smoke
run_suite tests/e2e/phase0_smoke.sh

# Asset ingest
run_suite tests/e2e/asset_ingest_smoke.sh

# MCP E2E
run_suite tests/e2e/mcp_e2e.sh

# Negative paths
run_suite tests/e2e/negative_paths.sh

# Phase 1 acceptance
run_suite tests/e2e/phase1_acceptance.sh

# Phase 2 acceptance
run_suite tests/e2e/phase2_acceptance.sh

# Phase 3 acceptance
run_suite tests/e2e/phase3_acceptance.sh

# Patch rollback + checkpoint
run_suite tests/e2e/patch_rollback_checkpoint.sh

# External control
run_suite tests/e2e/external_control_smoke.sh

# Adapter ecosystem
run_suite tests/e2e/adapter_ecosystem_smoke.sh

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Regression Summary: $PASSED/$TOTAL passed, $FAILED failed"
if [[ $FAILED -gt 0 ]]; then
    echo "║  Failed:$FAILED_LIST"
fi
echo "╚══════════════════════════════════════════════════════════════╝"

[[ $FAILED -eq 0 ]] || exit 1
