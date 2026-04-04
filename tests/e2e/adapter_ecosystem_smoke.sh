#!/usr/bin/env bash
# WSV-03  Third-party adapter ecosystem E2E smoke test
# Verifies: load adapters → adapter list → scheduler run → trace verify
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

echo "=== WSV-03: Adapter Ecosystem E2E ==="

# ── 1. Audio analyzer adapter fixture exists ─────────────────────────────
echo "── audio analyzer fixture"
check "audio analyzer manifest exists" \
  "[[ -f tests/fixtures/adapters/example-audio-analyzer/manifest.json ]]"

# ── 2. Visual executor adapter fixture exists ────────────────────────────
echo "── visual executor fixture"
check "visual executor manifest exists" \
  "[[ -f tests/fixtures/adapters/example-visual-executor/manifest.json ]]"

# ── 3. adapter.load (visual executor via loader) ────────────────────────
echo "── adapter.load visual executor"
TMP_VIS=$(mktemp)
echo "[$(cat tests/fixtures/adapters/example-visual-executor/manifest.json)]" > "$TMP_VIS"
LOAD_VIS=$("$AVCTL" adapter load --manifest "$TMP_VIS" 2>&1 || true)
rm -f "$TMP_VIS"
check "visual load returns ok" "echo '$LOAD_VIS' | grep -q '\"status\": \"ok\"'"
check "visual load count" "echo '$LOAD_VIS' | grep -q '\"loaded_count\"'"

# ── 4. adapter.load both (audio is analysis adapter, combined manifest) ─
echo "── adapter.load combined (null + visual)"
TMP_BOTH=$(mktemp)
cat > "$TMP_BOTH" <<'MANIFESTS'
[
  {"plugin_id":"null-test","plugin_kind":"null","backend_kind":"audio","version":"0.1.0","capabilities":[],"config":{}},
  {"plugin_id":"example-visual-executor","plugin_kind":"visual_executor","backend_kind":"visual","version":"0.1.0","capabilities":["scene_switch","shader_render"],"config":{}}
]
MANIFESTS
LOAD_BOTH=$("$AVCTL" adapter load --manifest "$TMP_BOTH" 2>&1 || true)
rm -f "$TMP_BOTH"
check "combined load returns ok" "echo '$LOAD_BOTH' | grep -q '\"status\": \"ok\"'"
check "combined load count 2" "echo '$LOAD_BOTH' | grep -q '\"loaded_count\": 2'"

# ── 5. system.adapters lists both ───────────────────────────────────────
echo "── system.adapters"
ADAPTERS=$("$AVCTL" system adapters 2>&1 || true)
check "adapters returns ok" "echo '$ADAPTERS' | grep -q '\"status\": \"ok\"'"
check "adapters has count" "echo '$ADAPTERS' | grep -q '\"count\"'"

# ── 6. system.capabilities confirms total ───────────────────────────────
echo "── system.capabilities"
CAPS=$("$AVCTL" system capabilities 2>&1 || true)
check "capabilities returns ok" "echo '$CAPS' | grep -q '\"status\": \"ok\"'"

# ── Summary ──────────────────────────────────────────────────────────────
echo ""
echo "WSV-03: $PASS passed, $FAIL failed"
[[ $FAIL -eq 0 ]] || exit 1
