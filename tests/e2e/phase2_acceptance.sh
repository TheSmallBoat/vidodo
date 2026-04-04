#!/usr/bin/env bash
# M14: Phase 2 Acceptance — Adapter/Hub/Deployment/Health full-chain + triple entry equivalence
# Verifies Phase 2 capabilities (system.adapters, system.hubs) across CLI/HTTP/MCP,
# revision lifecycle (publish/archive), and that degrade trace pipeline is wired end-to-end.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
plan_dir="$repo_root/tests/fixtures/plans/minimal-show"
assets_file="$repo_root/tests/fixtures/assets/asset-records.json"
patch_file="$repo_root/tests/fixtures/patches/minimal-local-content-patch.json"
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
# Section 1: CLI Entry Point — Phase 2 Capabilities
# ═══════════════════════════════════════════════════════════
echo "=== Phase 2 Acceptance: CLI Entry Point ==="

# 1.1: system.capabilities returns 29
cli_caps=$("$avctl" system capabilities 2>/dev/null)
cli_cap_count=$(echo "$cli_caps" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "CLI system.capabilities returns 29" test "$cli_cap_count" = "29"

# 1.2: system.adapters endpoint is reachable (empty registry is valid)
cli_adapters=$("$avctl" system adapters 2>/dev/null)
cli_adapter_count=$(echo "$cli_adapters" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('data',{}).get('count',0))" 2>/dev/null || echo "-1")
check "CLI system.adapters succeeds" test "$cli_adapter_count" != "-1"
cli_adapter_status=$(echo "$cli_adapters" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI system.adapters status=ok" test "$cli_adapter_status" = "ok"

# 1.3: system.hubs endpoint is reachable
cli_hubs=$("$avctl" system hubs 2>/dev/null)
cli_hub_count=$(echo "$cli_hubs" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('data',{}).get('count',0))" 2>/dev/null || echo "-1")
check "CLI system.hubs succeeds" test "$cli_hub_count" != "-1"
cli_hub_status=$(echo "$cli_hubs" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI system.hubs status=ok" test "$cli_hub_status" = "ok"

# 1.4: Full pipeline — compile + patch + run
cli_compile=$("$avctl" compile run --plan-dir "$plan_dir" --assets-file "$assets_file" 2>/dev/null)
check "CLI compile.run succeeds" test -n "$cli_compile"
cli_revision=$(echo "$cli_compile" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['revision'])" 2>/dev/null || echo "0")
cli_timeline=$(echo "$cli_compile" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['timeline_entries'])" 2>/dev/null || echo "0")
check "CLI compile.run revision >= 1" test "$cli_revision" -ge 1

# 1.5: Patch submit to create revision 2
"$avctl" patch check --show-id "$show_id" --patch-file "$patch_file" >/dev/null 2>&1
"$avctl" patch submit --show-id "$show_id" --patch-file "$patch_file" >/dev/null 2>&1

# 1.6: Revision publish
cli_publish=$("$avctl" revision publish --show-id "$show_id" --revision 1 2>/dev/null)
cli_publish_status=$(echo "$cli_publish" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI revision.publish status=ok" test "$cli_publish_status" = "ok"

# 1.7: Revision archive
cli_archive=$("$avctl" revision archive --show-id "$show_id" --revision 1 2>/dev/null)
cli_archive_status=$(echo "$cli_archive" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI revision.archive status=ok" test "$cli_archive_status" = "ok"

# 1.8: Revision list shows lifecycle
cli_rev_list=$("$avctl" revision list --show-id "$show_id" 2>/dev/null)
cli_rev_count=$(echo "$cli_rev_list" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['count'])" 2>/dev/null || echo "0")
check "CLI revision.list count >= 2" test "$cli_rev_count" -ge 2

# 1.9: Run start on patched revision
cli_run=$("$avctl" run start --show-id "$show_id" --revision 2 2>/dev/null)
cli_run_status=$(echo "$cli_run" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "CLI run.start status=ok" test "$cli_run_status" = "ok"
cli_run_id=$(echo "$cli_run" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['run_id'])" 2>/dev/null || echo "")
check "CLI run.start produces run_id" test -n "$cli_run_id"

# 1.10: Trace events are queryable and include expected event types
cli_events=$("$avctl" trace events --run-id "$cli_run_id" 2>/dev/null)
cli_event_count=$(echo "$cli_events" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['event_count'])" 2>/dev/null || echo "0")
check "CLI trace.events event_count > 0" test "$cli_event_count" -gt 0

# Verify timing, audio, visual, patch event types present
cli_has_timing=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('kind','').startswith('timing') for e in events) else 'no')
" 2>/dev/null || echo "no")
check "CLI trace has timing events" test "$cli_has_timing" = "yes"

cli_has_audio=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('kind','').startswith('audio') for e in events) else 'no')
" 2>/dev/null || echo "no")
check "CLI trace has audio events" test "$cli_has_audio" = "yes"

cli_has_visual=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('kind','').startswith('visual') for e in events) else 'no')
" 2>/dev/null || echo "no")
check "CLI trace has visual events" test "$cli_has_visual" = "yes"

cli_has_patch=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('kind','').startswith('patch') for e in events) else 'no')
" 2>/dev/null || echo "no")
check "CLI trace has patch events" test "$cli_has_patch" = "yes"

cli_has_lighting=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('kind','').startswith('lighting') for e in events) else 'no')
" 2>/dev/null || echo "no")
check "CLI trace has lighting events" test "$cli_has_lighting" = "yes"

# 1.11: Healthy backend produces no degrade events (FakeBackendClient returns empty snapshots)
cli_has_degrade=$(echo "$cli_events" | python3 -c "
import sys,json
events = json.load(sys.stdin)['data']['events']
print('yes' if any(e.get('kind','').startswith('degrade') for e in events) else 'no')
" 2>/dev/null || echo "no")
check "CLI trace: no degrade events from healthy backend" test "$cli_has_degrade" = "no"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 2: HTTP Entry Point — Phase 2 Capabilities
# ═══════════════════════════════════════════════════════════
echo "=== Phase 2 Acceptance: HTTP Entry Point ==="

# Reset artifacts for HTTP round
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

# Start core-service in background
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

# 2.1: HTTP capability count = 29
http_caps=$(curl -sf http://127.0.0.1:7400/capabilities 2>/dev/null || echo "{}")
http_cap_count=$(echo "$http_caps" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('capabilities',[])))" 2>/dev/null || echo "0")
check "HTTP /capabilities returns 29" test "$http_cap_count" = "29"

# 2.2: system.adapters via HTTP
http_adapters=$(curl -sf -X POST http://127.0.0.1:7400/capability/system.adapters \
  -H "Content-Type: application/json" -d '{}' 2>/dev/null || echo "{}")
http_adapter_status=$(echo "$http_adapters" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
http_adapter_count=$(echo "$http_adapters" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count','-1'))" 2>/dev/null || echo "-1")
check "HTTP system.adapters status=ok" test "$http_adapter_status" = "ok"
check "HTTP system.adapters count matches CLI" test "$http_adapter_count" = "$cli_adapter_count"

# 2.3: system.hubs via HTTP
http_hubs=$(curl -sf -X POST http://127.0.0.1:7400/capability/system.hubs \
  -H "Content-Type: application/json" -d '{}' 2>/dev/null || echo "{}")
http_hub_status=$(echo "$http_hubs" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
http_hub_count=$(echo "$http_hubs" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count','-1'))" 2>/dev/null || echo "-1")
check "HTTP system.hubs status=ok" test "$http_hub_status" = "ok"
check "HTTP system.hubs count matches CLI" test "$http_hub_count" = "$cli_hub_count"

# 2.4: system.capabilities count
http_sys_caps=$(curl -sf -X POST http://127.0.0.1:7400/capability/system.capabilities \
  -H "Content-Type: application/json" -d '{}' 2>/dev/null || echo "{}")
http_sys_cap_count=$(echo "$http_sys_caps" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "HTTP system.capabilities count=29" test "$http_sys_cap_count" = "29"

# 2.5: Compile + run pipeline
http_compile=$(curl -sf -X POST http://127.0.0.1:7400/capability/compile.run \
  -H "Content-Type: application/json" \
  -d "{\"plan_dir\":\"${plan_dir}\",\"assets_file\":\"${assets_file}\"}" 2>/dev/null || echo "{}")
http_compile_status=$(echo "$http_compile" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
http_revision=$(echo "$http_compile" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('revision',0))" 2>/dev/null || echo "0")
http_timeline=$(echo "$http_compile" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('timeline_entries',0))" 2>/dev/null || echo "0")
check "HTTP compile.run status=ok" test "$http_compile_status" = "ok"
check "HTTP compile.run timeline matches CLI" test "$http_timeline" = "$cli_timeline"

# 2.6: Run start via HTTP
http_run=$(curl -sf -X POST http://127.0.0.1:7400/capability/run.start \
  -H "Content-Type: application/json" \
  -d "{\"show_id\":\"${show_id}\",\"revision\":${http_revision}}" 2>/dev/null || echo "{}")
http_run_status=$(echo "$http_run" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "HTTP run.start status=ok" test "$http_run_status" = "ok"
http_run_id=$(echo "$http_run" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('run_id',''))" 2>/dev/null || echo "")

# 2.7: Trace events via HTTP
http_events=$(curl -sf -X POST http://127.0.0.1:7400/capability/trace.events \
  -H "Content-Type: application/json" \
  -d "{\"run_id\":\"${http_run_id}\"}" 2>/dev/null || echo "{}")
http_event_count=$(echo "$http_events" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('event_count',0))" 2>/dev/null || echo "0")
check "HTTP trace.events event_count > 0" test "$http_event_count" -gt 0

# 2.8: Revision publish + archive via HTTP
http_publish=$(curl -sf -X POST http://127.0.0.1:7400/capability/revision.publish \
  -H "Content-Type: application/json" \
  -d "{\"show_id\":\"${show_id}\",\"revision\":${http_revision}}" 2>/dev/null || echo "{}")
http_publish_status=$(echo "$http_publish" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "HTTP revision.publish status=ok" test "$http_publish_status" = "ok"

http_archive=$(curl -sf -X POST http://127.0.0.1:7400/capability/revision.archive \
  -H "Content-Type: application/json" \
  -d "{\"show_id\":\"${show_id}\",\"revision\":${http_revision}}" 2>/dev/null || echo "{}")
http_archive_status=$(echo "$http_archive" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
check "HTTP revision.archive status=ok" test "$http_archive_status" = "ok"

# Stop core-service
kill "$core_pid" 2>/dev/null || true
wait "$core_pid" 2>/dev/null || true
trap - EXIT

echo ""

# ═══════════════════════════════════════════════════════════
# Section 3: MCP Entry Point — Phase 2 Capabilities
# ═══════════════════════════════════════════════════════════
echo "=== Phase 2 Acceptance: MCP Entry Point ==="

# Reset artifacts for MCP round
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

mcp_input=$(cat <<EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"system.capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"system.adapters","arguments":{}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"system.hubs","arguments":{}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"compile.run","arguments":{"plan_dir":"${plan_dir}","assets_file":"${assets_file}"}}}
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

# 3.1: MCP initialize
mcp_init=$(get_mcp_response 1)
check "MCP initialize returns protocolVersion" \
  test "$(echo "$mcp_init" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['protocolVersion'])")" = "2024-11-05"

# 3.2: MCP tools/list returns 29
mcp_tools=$(get_mcp_response 2)
mcp_tool_count=$(echo "$mcp_tools" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['result']['tools']))")
check "MCP tools/list returns 29 tools" test "$mcp_tool_count" = "29"

# 3.3: MCP system.capabilities returns 29
mcp_caps=$(get_mcp_response 3)
mcp_caps_text=$(echo "$mcp_caps" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
mcp_cap_count=$(echo "$mcp_caps_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['count'])")
check "MCP system.capabilities count=29" test "$mcp_cap_count" = "29"

# 3.4: MCP system.adapters succeeds
mcp_adapters=$(get_mcp_response 4)
mcp_adapters_text=$(echo "$mcp_adapters" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
mcp_adapter_status=$(echo "$mcp_adapters_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")
mcp_adapter_count=$(echo "$mcp_adapters_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count','-1'))")
check "MCP system.adapters status=ok" test "$mcp_adapter_status" = "ok"
check "MCP system.adapters count matches CLI" test "$mcp_adapter_count" = "$cli_adapter_count"

# 3.5: MCP system.hubs succeeds
mcp_hubs=$(get_mcp_response 5)
mcp_hubs_text=$(echo "$mcp_hubs" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
mcp_hub_status=$(echo "$mcp_hubs_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")
mcp_hub_count=$(echo "$mcp_hubs_text" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count','-1'))")
check "MCP system.hubs status=ok" test "$mcp_hub_status" = "ok"
check "MCP system.hubs count matches CLI" test "$mcp_hub_count" = "$cli_hub_count"

# 3.6: MCP compile.run succeeds
mcp_compile=$(get_mcp_response 6)
mcp_compile_text=$(echo "$mcp_compile" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
mcp_compile_status=$(echo "$mcp_compile_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['status'])")
mcp_timeline=$(echo "$mcp_compile_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['timeline_entries'])")
check "MCP compile.run status=ok" test "$mcp_compile_status" = "ok"
check "MCP compile.run timeline matches CLI" test "$mcp_timeline" = "$cli_timeline"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 4: Semantic Equivalence Across Three Entry Points
# ═══════════════════════════════════════════════════════════
echo "=== Phase 2 Acceptance: Semantic Equivalence ==="

check "Equivalence: capability count CLI=HTTP=MCP=29" \
  test "$cli_cap_count" = "29" -a "$http_sys_cap_count" = "29" -a "$mcp_cap_count" = "29"

check "Equivalence: adapter count CLI=HTTP=MCP" \
  test "$cli_adapter_count" = "$http_adapter_count" -a "$cli_adapter_count" = "$mcp_adapter_count"

check "Equivalence: hub count CLI=HTTP=MCP" \
  test "$cli_hub_count" = "$http_hub_count" -a "$cli_hub_count" = "$mcp_hub_count"

check "Equivalence: timeline entries CLI=HTTP=MCP" \
  test "$cli_timeline" = "$http_timeline" -a "$cli_timeline" = "$mcp_timeline"

echo ""

# ═══════════════════════════════════════════════════════════
# Section 5: Artifact Integrity Checks
# ═══════════════════════════════════════════════════════════
echo "=== Phase 2 Acceptance: Artifact Integrity ==="

# Schema fixture validation (all 87 pass)
schema_result=$("$repo_root/scripts/schema-validate.sh" 2>&1)
schema_count=$(echo "$schema_result" | grep -o '[0-9]*' | head -1)
check "Schema validation: 87 fixtures pass" test "$schema_count" = "87"

# Rust test count
test_result=$(cd "$repo_root/vidodo-src" && cargo test --workspace --all-targets 2>&1 | grep 'test result:' | awk '{sum += $4} END {print sum}')
check "Rust tests: 127+ pass" test "$test_result" -ge 127

echo ""
echo "=== Phase 2 Acceptance: Results ==="
echo "Results: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
