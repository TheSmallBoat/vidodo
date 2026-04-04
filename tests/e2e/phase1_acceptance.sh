#!/usr/bin/env bash
# M10: Phase 1 Acceptance — CLI / HTTP / MCP semantic equivalence + triple-runtime
# Verifies the same capability semantics across all three entry points and that
# audio + visual + lighting runtimes all process the same compiled revision.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
plan_dir="$repo_root/tests/fixtures/plans/minimal-show"
assets_file="$repo_root/tests/fixtures/assets/asset-records.json"
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
visual_rt="$repo_root/vidodo-src/target/debug/visual-runtime"
lighting_rt="$repo_root/vidodo-src/target/debug/lighting-runtime"

echo "=== Phase 1 Acceptance: CLI Entry Point ==="

cli_validate=$("$avctl" plan validate --plan-dir "$plan_dir" --assets-file "$assets_file" 2>/dev/null)
check "CLI plan.validate succeeds" test -n "$cli_validate"
cli_show_id=$(echo "$cli_validate" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['show_id'])" 2>/dev/null || echo "")
check "CLI plan.validate show_id" test "$cli_show_id" = "$show_id"

cli_compile=$("$avctl" compile run --plan-dir "$plan_dir" --assets-file "$assets_file" 2>/dev/null)
check "CLI compile.run succeeds" test -n "$cli_compile"
cli_revision=$(echo "$cli_compile" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['revision'])" 2>/dev/null || echo "0")
cli_timeline=$(echo "$cli_compile" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['timeline_entries'])" 2>/dev/null || echo "0")
check "CLI compile.run produces revision >= 1" test "$cli_revision" -ge 1
check "CLI compile.run produces timeline entries" test "$cli_timeline" -ge 1

cli_caps=$("$avctl" system capabilities 2>/dev/null)
cli_cap_count=$(echo "$cli_caps" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('data',{}).get('count', len(d) if isinstance(d,list) else 0))" 2>/dev/null || echo "0")
check "CLI system.capabilities returns 39" test "$cli_cap_count" = "39"

echo ""
echo "=== Phase 1 Acceptance: HTTP Entry Point ==="

# Start core-service in background
"$core_svc" &
core_pid=$!
# Wait for it to start
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

http_health=$(curl -sf http://127.0.0.1:7400/health 2>/dev/null || echo "{}")
check "HTTP /health returns ok" \
  test "$(echo "$http_health" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")" = "ok"

http_caps=$(curl -sf http://127.0.0.1:7400/capabilities 2>/dev/null || echo "{}")
http_cap_count=$(echo "$http_caps" | python3 -c "import sys,json; print(len(json.load(sys.stdin).get('capabilities',[])))" 2>/dev/null || echo "0")
check "HTTP /capabilities returns 39" test "$http_cap_count" = "39"

# Reset artifacts for HTTP round
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

http_validate=$(curl -sf -X POST http://127.0.0.1:7400/capability/plan.validate \
  -H "Content-Type: application/json" \
  -d "{\"plan_dir\":\"${plan_dir}\",\"assets_file\":\"${assets_file}\"}" 2>/dev/null || echo "{}")
http_validate_status=$(echo "$http_validate" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
http_validate_show=$(echo "$http_validate" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('show_id',''))" 2>/dev/null || echo "")
check "HTTP plan.validate status=ok" test "$http_validate_status" = "ok"
check "HTTP plan.validate show_id matches CLI" test "$http_validate_show" = "$cli_show_id"

http_compile=$(curl -sf -X POST http://127.0.0.1:7400/capability/compile.run \
  -H "Content-Type: application/json" \
  -d "{\"plan_dir\":\"${plan_dir}\",\"assets_file\":\"${assets_file}\"}" 2>/dev/null || echo "{}")
http_compile_status=$(echo "$http_compile" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
http_revision=$(echo "$http_compile" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('revision',0))" 2>/dev/null || echo "0")
http_timeline=$(echo "$http_compile" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('timeline_entries',0))" 2>/dev/null || echo "0")
check "HTTP compile.run status=ok" test "$http_compile_status" = "ok"
check "HTTP compile.run revision >= 1" test "$http_revision" -ge 1
check "HTTP compile.run timeline entries match CLI" test "$http_timeline" = "$cli_timeline"

http_sys_caps=$(curl -sf -X POST http://127.0.0.1:7400/capability/system.capabilities \
  -H "Content-Type: application/json" \
  -d "{}" 2>/dev/null || echo "{}")
http_sys_cap_count=$(echo "$http_sys_caps" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('count',0))" 2>/dev/null || echo "0")
check "HTTP system.capabilities count=39" test "$http_sys_cap_count" = "39"

# HTTP: run start + run status + trace + eval + export
http_run=$(curl -sf -X POST http://127.0.0.1:7400/capability/run.start \
  -H "Content-Type: application/json" \
  -d "{\"show_id\":\"${show_id}\",\"revision\":1}" 2>/dev/null || echo "{}")
check "HTTP run.start status=ok" \
  test "$(echo "$http_run" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")" = "ok"

http_run_id=$(echo "$http_run" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('run_id',''))" 2>/dev/null || echo "")
check "HTTP run.start produces run_id" test -n "$http_run_id"

http_trace=$(curl -sf -X POST http://127.0.0.1:7400/capability/trace.show \
  -H "Content-Type: application/json" \
  -d "{\"run_id\":\"${http_run_id}\"}" 2>/dev/null || echo "{}")
check "HTTP trace.show status=ok" \
  test "$(echo "$http_trace" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")" = "ok"

http_eval=$(curl -sf -X POST http://127.0.0.1:7400/capability/eval.run \
  -H "Content-Type: application/json" \
  -d "{\"show_id\":\"${show_id}\",\"run_id\":\"${http_run_id}\"}" 2>/dev/null || echo "{}")
check "HTTP eval.run status=ok" \
  test "$(echo "$http_eval" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")" = "ok"

http_export=$(curl -sf -X POST http://127.0.0.1:7400/capability/export.audio \
  -H "Content-Type: application/json" \
  -d "{\"run_id\":\"${http_run_id}\"}" 2>/dev/null || echo "{}")
check "HTTP export.audio status=ok" \
  test "$(echo "$http_export" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status',''))")" = "ok"

# Stop core-service for next phase
kill "$core_pid" 2>/dev/null || true
wait "$core_pid" 2>/dev/null || true
trap - EXIT

echo ""
echo "=== Phase 1 Acceptance: MCP Entry Point ==="

# Reset artifacts for MCP round
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

mcp_input=$(cat <<EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"system.capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"plan.validate","arguments":{"plan_dir":"${plan_dir}","assets_file":"${assets_file}"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"compile.run","arguments":{"plan_dir":"${plan_dir}","assets_file":"${assets_file}"}}}
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

mcp_init=$(get_mcp_response 1)
check "MCP initialize returns protocolVersion" \
  test "$(echo "$mcp_init" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['protocolVersion'])")" = "2024-11-05"

mcp_tools=$(get_mcp_response 2)
mcp_tool_count=$(echo "$mcp_tools" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['result']['tools']))")
check "MCP tools/list returns 39 tools" test "$mcp_tool_count" = "39"

mcp_caps=$(get_mcp_response 3)
mcp_caps_text=$(echo "$mcp_caps" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
mcp_cap_count=$(echo "$mcp_caps_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['count'])")
check "MCP system.capabilities returns 39" test "$mcp_cap_count" = "39"

mcp_validate=$(get_mcp_response 4)
mcp_validate_text=$(echo "$mcp_validate" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
mcp_validate_show=$(echo "$mcp_validate_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['show_id'])")
check "MCP plan.validate show_id matches CLI" test "$mcp_validate_show" = "$cli_show_id"

mcp_compile=$(get_mcp_response 5)
mcp_compile_text=$(echo "$mcp_compile" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
mcp_compile_status=$(echo "$mcp_compile_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['status'])")
mcp_timeline=$(echo "$mcp_compile_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['timeline_entries'])")
check "MCP compile.run status=ok" test "$mcp_compile_status" = "ok"
check "MCP compile.run timeline entries match CLI" test "$mcp_timeline" = "$cli_timeline"

echo ""
echo "=== Phase 1 Acceptance: Triple-Runtime Unified Scheduling ==="

# Run all three runtimes against the MCP-compiled revision
# (the most recently compiled revision lives under artifacts from the MCP round)
mcp_run_revision=$(echo "$mcp_compile_text" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['revision'])")
triple_run_id="run-${show_id}-rev-${mcp_run_revision}"

# Execute run.start via CLI on the MCP-compiled artifacts
"$avctl" run start --show-id "$show_id" --revision "$mcp_run_revision" >/dev/null 2>&1

# audio runtime is handled inside run start; verify visual and lighting
"$visual_rt" --run-id "$triple_run_id" 2>/dev/null
"$lighting_rt" --run-id "$triple_run_id" 2>/dev/null

# Check all three runtimes produced output
traces="$repo_root/artifacts/traces/$triple_run_id"
check "Triple-runtime: events.jsonl exists" test -f "$traces/events.jsonl"
check "Triple-runtime: visual-acks.json exists" test -f "$traces/visual-acks.json"
check "Triple-runtime: lighting-acks.json exists" test -f "$traces/lighting-acks.json"

# Verify visual-acks has rendered entries
check "Triple-runtime: visual-acks has rendered" \
  grep -q '"rendered"' "$traces/visual-acks.json"

# Verify lighting-acks has cue_executed and synced entries
check "Triple-runtime: lighting-acks has cue_executed" \
  grep -q '"cue_executed"' "$traces/lighting-acks.json"
check "Triple-runtime: lighting-acks has synced" \
  grep -q '"synced"' "$traces/lighting-acks.json"

echo ""
echo "=== Phase 1 Acceptance: Semantic Equivalence Summary ==="

# All three entry points returned the same show_id
check "Equivalence: CLI/HTTP/MCP show_id all match" \
  test "$cli_show_id" = "$http_validate_show" -a "$cli_show_id" = "$mcp_validate_show"

# All three entry points produced the same timeline_entries count
check "Equivalence: CLI/HTTP/MCP timeline_entries all match" \
  test "$cli_timeline" = "$http_timeline" -a "$cli_timeline" = "$mcp_timeline"

# All three entry points returned the same capability count
check "Equivalence: CLI/HTTP/MCP capability count all 39" \
  test "$cli_cap_count" = "39" -a "$http_sys_cap_count" = "39" -a "$mcp_cap_count" = "39"

echo ""
echo "Results: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
