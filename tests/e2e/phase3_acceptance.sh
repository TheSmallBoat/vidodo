#!/usr/bin/env bash
# M18: Phase 3 Acceptance — Adapter lifecycle, Hub persistence, Reference backends,
#      Authorization, triple entry-point equivalence, trace audit chain.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
plan_dir="$repo_root/tests/fixtures/plans/minimal-show"
assets_file="$repo_root/tests/fixtures/assets/asset-records.json"
patch_file="$repo_root/tests/fixtures/patches/minimal-local-content-patch.json"
adapter_manifest="$repo_root/tests/fixtures/adapters/reference-adapters.json"
show_id="show-phase0-minimal"

# Reset artifact store
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

cd "$repo_root/vidodo-src"

passed=0
failed=0

check() {
  local label="$1"
  shift
  if "$@"; then
    echo "PASS  $label"
    passed=$((passed + 1))
  else
    echo "FAIL  $label" >&2
    failed=$((failed + 1))
  fi
}

# Build everything once
cargo build --workspace 2>/dev/null

avctl="$repo_root/vidodo-src/target/debug/avctl"
core_svc="$repo_root/vidodo-src/target/debug/core-service"
mcp="$repo_root/vidodo-src/target/debug/mcp-adapter"

# ═══════════════════════════════════════════════════════════
# Section 1: CLI — Adapter Lifecycle (WSR-01)
# ═══════════════════════════════════════════════════════════
echo "=== Phase 3 Acceptance: CLI Adapter Lifecycle ==="

# 1.1 System capabilities count = 29
cli_caps=$("$avctl" system capabilities 2>/dev/null)
cli_cap_count=$(echo "$cli_caps" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "CLI system.capabilities returns 29" test "$cli_cap_count" = "29"

# 1.2 Adapter load from manifest
cli_adapter_load=$("$avctl" adapter load --manifest "$adapter_manifest" 2>/dev/null)
cli_load_status=$(echo "$cli_adapter_load" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
cli_loaded_count=$(echo "$cli_adapter_load" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('loaded_count',0))" 2>/dev/null || echo "0")
check "CLI adapter.load status=ok" test "$cli_load_status" = "ok"
check "CLI adapter.load loaded 3 adapters" test "$cli_loaded_count" = "3"

# 1.3 Adapter status for loaded adapter
cli_adapter_status=$("$avctl" adapter status --plugin-id ref-audio-1 2>/dev/null)
cli_astatus=$(echo "$cli_adapter_status" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI adapter.status returns ok" test "$cli_astatus" = "ok"
cli_aplugin=$(echo "$cli_adapter_status" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('plugin_id',''))" 2>/dev/null || echo "")
check "CLI adapter.status plugin_id=ref-audio-1" test "$cli_aplugin" = "ref-audio-1"

# 1.4 System adapters lists 3 registered adapters
cli_adapters=$("$avctl" system adapters 2>/dev/null)
cli_adapter_count=$(echo "$cli_adapters" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "CLI system.adapters shows 3" test "$cli_adapter_count" = "3"

# 1.5 Adapter shutdown
cli_adapter_shut=$("$avctl" adapter shutdown --plugin-id ref-audio-1 2>/dev/null)
cli_shut_status=$(echo "$cli_adapter_shut" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI adapter.shutdown status=ok" test "$cli_shut_status" = "ok"

# 1.6 Persistence — adapters survive re-read
cli_adapters2=$("$avctl" system adapters 2>/dev/null)
cli_adapter_count2=$(echo "$cli_adapters2" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "CLI adapter persistence: still 3 after reload" test "$cli_adapter_count2" = "3"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 2: CLI — Hub Lifecycle (WSR-02)
# ═══════════════════════════════════════════════════════════
echo "=== Phase 3 Acceptance: CLI Hub Lifecycle ==="

# 2.1 Hub register
cli_hub_reg=$("$avctl" hub register --hub-id hub-sample-pack \
  --kind sample_pack --locator "/mnt/samples/drums" --provides kick.wav --provides snare.wav 2>/dev/null)
cli_hub_reg_status=$(echo "$cli_hub_reg" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI hub.register status=ok" test "$cli_hub_reg_status" = "ok"

# 2.2 Hub status
cli_hub_st=$("$avctl" hub status --hub-id hub-sample-pack 2>/dev/null)
cli_hub_st_status=$(echo "$cli_hub_st" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI hub.status status=ok" test "$cli_hub_st_status" = "ok"

# 2.3 Hub resolve
cli_hub_res=$("$avctl" hub resolve --resource kick.wav 2>/dev/null)
cli_hub_res_status=$(echo "$cli_hub_res" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
cli_hub_res_locator=$(echo "$cli_hub_res" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('locator',''))" 2>/dev/null || echo "")
check "CLI hub.resolve status=ok" test "$cli_hub_res_status" = "ok"
check "CLI hub.resolve locator matches" test "$cli_hub_res_locator" = "/mnt/samples/drums"

# 2.4 Hub list
cli_hubs=$("$avctl" system hubs 2>/dev/null)
cli_hub_count=$(echo "$cli_hubs" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "CLI system.hubs shows 1 hub" test "$cli_hub_count" = "1"

# 2.5 Hub persistence — re-reading shows hub still present
cli_hubs2=$("$avctl" system hubs 2>/dev/null)
cli_hub_count2=$(echo "$cli_hubs2" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "CLI hub persistence: still 1 after reload" test "$cli_hub_count2" = "1"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 3: CLI — Reference Backend Run (WSR-03)
# ═══════════════════════════════════════════════════════════
echo "=== Phase 3 Acceptance: CLI Reference Backend ==="

# 3.1 Compile + run with reference backend
"$avctl" compile run --plan-dir "$plan_dir" --assets-file "$assets_file" >/dev/null 2>&1
"$avctl" patch check --show-id "$show_id" --patch-file "$patch_file" >/dev/null 2>&1
"$avctl" patch submit --show-id "$show_id" --patch-file "$patch_file" >/dev/null 2>&1
cli_run=$("$avctl" run start --show-id "$show_id" --revision 2 --backend reference 2>/dev/null)
cli_run_status=$(echo "$cli_run" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI run.start --backend=reference status=ok" test "$cli_run_status" = "ok"
cli_run_id=$(echo "$cli_run" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('run_id',''))" 2>/dev/null || echo "")
check "CLI reference run produces run_id" test -n "$cli_run_id"

# 3.2 Trace events include all 5 event types
cli_events=$("$avctl" trace events --run-id "$cli_run_id" 2>/dev/null)
cli_event_count=$(echo "$cli_events" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['event_count'])" 2>/dev/null || echo "0")
check "CLI reference run event_count > 0" test "$cli_event_count" -gt 0

for kind in timing audio visual patch lighting; do
  has_kind=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('kind','').startswith('$kind') for e in events) else 'no')
" 2>/dev/null || echo "no")
  check "CLI reference run has $kind events" test "$has_kind" = "yes"
done

# 3.3 Reference backend acks have ref-audio / ref-visual / ref-lighting backend names
has_ref_audio=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('ack',{}).get('backend','')=='ref-audio' for e in events if e.get('ack')) else 'no')
" 2>/dev/null || echo "no")
check "CLI reference run: audio acks from ref-audio" test "$has_ref_audio" = "yes"

has_ref_visual=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('ack',{}).get('backend','')=='ref-visual' for e in events if e.get('ack')) else 'no')
" 2>/dev/null || echo "no")
check "CLI reference run: visual acks from ref-visual" test "$has_ref_visual" = "yes"

has_ref_lighting=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('ack',{}).get('backend','')=='ref-lighting' for e in events if e.get('ack')) else 'no')
" 2>/dev/null || echo "no")
check "CLI reference run: lighting acks from ref-lighting" test "$has_ref_lighting" = "yes"

# 3.4 Trace manifest and artifacts exist
check "Trace manifest exists" test -f "$repo_root/artifacts/traces/$cli_run_id/manifest.json"
check "Trace events file exists" test -f "$repo_root/artifacts/traces/$cli_run_id/events.jsonl"
check "Trace patch-decisions exists" test -f "$repo_root/artifacts/traces/$cli_run_id/patch-decisions.jsonl"
check "Trace resource-samples exists" test -f "$repo_root/artifacts/traces/$cli_run_id/resource-samples.jsonl"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 4: HTTP Entry Point — Phase 3 Capabilities
# ═══════════════════════════════════════════════════════════
echo "=== Phase 3 Acceptance: HTTP Entry Point ==="

# Reset artifacts for HTTP round
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

"$core_svc" &
core_pid=$!
for i in $(seq 1 30); do
  if curl -sf http://127.0.0.1:7400/health >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

cleanup_http() {
  kill "$core_pid" 2>/dev/null || true
  wait "$core_pid" 2>/dev/null || true
}
trap cleanup_http EXIT

# 4.1 HTTP capability count = 29
http_caps=$(curl -sf http://127.0.0.1:7400/capabilities 2>/dev/null || echo "{}")
http_cap_count=$(echo "$http_caps" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('capabilities',[])))" 2>/dev/null || echo "0")
check "HTTP /capabilities returns 29" test "$http_cap_count" = "29"

# 4.2 HTTP adapter.load
http_adapter_load=$(curl -sf -X POST http://127.0.0.1:7400/capability/adapter.load \
  -H "Content-Type: application/json" \
  -d "{\"manifest_path\":\"${adapter_manifest}\"}" 2>/dev/null || echo "{}")
http_load_status=$(echo "$http_adapter_load" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
http_loaded_count=$(echo "$http_adapter_load" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('loaded_count',0))" 2>/dev/null || echo "0")
check "HTTP adapter.load status=ok" test "$http_load_status" = "ok"
check "HTTP adapter.load loaded 3" test "$http_loaded_count" = "3"

# 4.3 HTTP adapter.status
http_adapter_st=$(curl -sf -X POST http://127.0.0.1:7400/capability/adapter.status \
  -H "Content-Type: application/json" \
  -d '{"plugin_id":"ref-audio-1"}' 2>/dev/null || echo "{}")
http_astatus=$(echo "$http_adapter_st" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "HTTP adapter.status status=ok" test "$http_astatus" = "ok"

# 4.4 HTTP system.adapters count = 3
http_adapters=$(curl -sf -X POST http://127.0.0.1:7400/capability/system.adapters \
  -H "Content-Type: application/json" -d '{}' 2>/dev/null || echo "{}")
http_adapter_count=$(echo "$http_adapters" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "HTTP system.adapters count=3" test "$http_adapter_count" = "3"

# 4.5 HTTP hub.register
http_hub_reg=$(curl -sf -X POST http://127.0.0.1:7400/capability/hub.register \
  -H "Content-Type: application/json" \
  -d '{"hub_id":"hub-sample-pack","resource_kind":"sample_pack","locator":"/mnt/samples/drums","provides":["kick.wav","snare.wav"]}' 2>/dev/null || echo "{}")
http_hub_reg_status=$(echo "$http_hub_reg" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "HTTP hub.register status=ok" test "$http_hub_reg_status" = "ok"

# 4.6 HTTP hub.resolve
http_hub_res=$(curl -sf -X POST http://127.0.0.1:7400/capability/hub.resolve \
  -H "Content-Type: application/json" \
  -d '{"resource_name":"kick.wav"}' 2>/dev/null || echo "{}")
http_hub_res_status=$(echo "$http_hub_res" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "HTTP hub.resolve status=ok" test "$http_hub_res_status" = "ok"

# 4.7 HTTP system.hubs count = 1
http_hubs=$(curl -sf -X POST http://127.0.0.1:7400/capability/system.hubs \
  -H "Content-Type: application/json" -d '{}' 2>/dev/null || echo "{}")
http_hub_count=$(echo "$http_hubs" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "HTTP system.hubs count=1" test "$http_hub_count" = "1"

# Stop core-service
kill "$core_pid" 2>/dev/null || true
wait "$core_pid" 2>/dev/null || true
trap - EXIT

echo ""

# ═══════════════════════════════════════════════════════════
# Section 5: MCP Entry Point — Phase 3 Capabilities
# ═══════════════════════════════════════════════════════════
echo "=== Phase 3 Acceptance: MCP Entry Point ==="

# Reset artifacts for MCP round
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

mcp_input=$(cat <<EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"system.capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"adapter.load","arguments":{"manifest_path":"${adapter_manifest}"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"system.adapters","arguments":{}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"hub.register","arguments":{"hub_id":"hub-mcp","resource_kind":"sample_pack","locator":"/mnt/mcp","provides":["bass.wav"]}}}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"system.hubs","arguments":{}}}
EOF
)

mcp_output=$(echo "$mcp_input" | "$mcp" 2>/dev/null)

get_mcp_response() {
  local id="$1"
  echo "$mcp_output" | while IFS= read -r line; do
    rid=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id',''))" 2>/dev/null || true)
    if [ "$rid" = "$id" ]; then
      echo "$line"
      break
    fi
  done
}

# 5.1 MCP tools/list returns 29
mcp_tools=$(get_mcp_response 2)
mcp_tool_count=$(echo "$mcp_tools" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['result']['tools']))" 2>/dev/null || echo "0")
check "MCP tools/list returns 29 tools" test "$mcp_tool_count" = "29"

# 5.2 MCP system.capabilities = 29
mcp_caps=$(get_mcp_response 3)
mcp_caps_text=$(echo "$mcp_caps" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])" 2>/dev/null || echo "{}")
mcp_cap_count=$(echo "$mcp_caps_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['count'])" 2>/dev/null || echo "0")
check "MCP system.capabilities count=29" test "$mcp_cap_count" = "29"

# 5.3 MCP adapter.load = 3
mcp_aload=$(get_mcp_response 4)
mcp_aload_text=$(echo "$mcp_aload" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])" 2>/dev/null || echo "{}")
mcp_aload_count=$(echo "$mcp_aload_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('loaded_count',0))" 2>/dev/null || echo "0")
check "MCP adapter.load loaded 3" test "$mcp_aload_count" = "3"

# 5.4 MCP system.adapters = 3
mcp_adapters=$(get_mcp_response 5)
mcp_adapters_text=$(echo "$mcp_adapters" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])" 2>/dev/null || echo "{}")
mcp_adapter_count=$(echo "$mcp_adapters_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "MCP system.adapters count=3" test "$mcp_adapter_count" = "3"

# 5.5 MCP hub.register ok
mcp_hreg=$(get_mcp_response 6)
mcp_hreg_text=$(echo "$mcp_hreg" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])" 2>/dev/null || echo "{}")
mcp_hreg_status=$(echo "$mcp_hreg_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "MCP hub.register status=ok" test "$mcp_hreg_status" = "ok"

# 5.6 MCP system.hubs = 1
mcp_hubs=$(get_mcp_response 7)
mcp_hubs_text=$(echo "$mcp_hubs" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])" 2>/dev/null || echo "{}")
mcp_hub_count=$(echo "$mcp_hubs_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "MCP system.hubs count=1" test "$mcp_hub_count" = "1"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 6: Semantic Equivalence Across Three Entry Points
# ═══════════════════════════════════════════════════════════
echo "=== Phase 3 Acceptance: Semantic Equivalence ==="

check "Equivalence: capability count CLI=HTTP=MCP=29" \
  test "$cli_cap_count" = "29" -a "$http_cap_count" = "29" -a "$mcp_cap_count" = "29"

check "Equivalence: MCP tool count = 29" test "$mcp_tool_count" = "29"

check "Equivalence: adapter count CLI=HTTP after load" \
  test "$cli_adapter_count" = "$http_adapter_count"

check "Equivalence: hub count CLI=HTTP after register" \
  test "$cli_hub_count" = "$http_hub_count"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 7: Artifact Integrity & Quality Gates
# ═══════════════════════════════════════════════════════════
echo "=== Phase 3 Acceptance: Artifact Integrity ==="

# Schema fixture validation
schema_result=$("$repo_root/scripts/schema-validate.sh" 2>&1)
schema_count=$(echo "$schema_result" | grep -o '[0-9]*' | head -1)
check "Schema validation: 89 fixtures pass" test "$schema_count" = "89"

# Rust test count
test_result=$(cd "$repo_root/vidodo-src" && cargo test --workspace --all-targets 2>&1 | grep 'test result:' | awk '{sum += $4} END {print sum}')
check "Rust tests: 200+ pass" test "$test_result" -ge 200

# SQLite persistence files created
check "adapters.db exists" test -f "$repo_root/artifacts/adapters.db"
check "hubs.db exists" test -f "$repo_root/artifacts/hubs.db"

echo ""
echo "=== Phase 3 Acceptance: Results ==="
echo "Results: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
