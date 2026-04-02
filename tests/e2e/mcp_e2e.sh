#!/usr/bin/env bash
# WSI-04: MCP end-to-end integration test
# Pipes JSON-RPC requests through mcp-adapter stdin and validates responses.
# Workflow: initialize → tools/list → system.capabilities → plan.validate → compile.run
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
plan_dir="$repo_root/tests/fixtures/plans/minimal-show"
assets_file="$repo_root/tests/fixtures/assets/asset-records.json"

# Reset artifact store
rm -rf "$repo_root/artifacts"
"$repo_root/scripts/init-artifact-store.sh"

cd "$repo_root/vidodo-src"

# Build the binary once
cargo build -p mcp-adapter 2>/dev/null

adapter="$repo_root/vidodo-src/target/debug/mcp-adapter"

# Compose a multi-line input with 5 JSON-RPC requests
input=$(cat <<EOF
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}
{"jsonrpc":"2.0","id":2,"method":"notifications/initialized","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"system.capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"plan.validate","arguments":{"plan_dir":"${plan_dir}","assets_file":"${assets_file}"}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"compile.run","arguments":{"plan_dir":"${plan_dir}","assets_file":"${assets_file}"}}}
EOF
)

# Run the adapter and capture all output lines
output=$(echo "$input" | "$adapter" 2>/dev/null)

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

# Parse responses by id
get_response() {
  local id="$1"
  echo "$output" | while IFS= read -r line; do
    rid=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id',''))" 2>/dev/null || true)
    if [ "$rid" = "$id" ]; then
      echo "$line"
      break
    fi
  done
}

# --- 1. initialize ---
resp1=$(get_response 1)
check "initialize returns protocolVersion" \
  test "$(echo "$resp1" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['protocolVersion'])")" = "2024-11-05"

check "initialize returns serverInfo.name" \
  test "$(echo "$resp1" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['serverInfo']['name'])")" = "vidodo-mcp-adapter"

# --- 2. tools/list ---
resp3=$(get_response 3)
tool_count=$(echo "$resp3" | python3 -c "import sys,json; print(len(json.load(sys.stdin)['result']['tools']))")
check "tools/list returns 21 tools" test "$tool_count" = "21"

# Verify each tool has name, description, inputSchema
all_have_fields=$(echo "$resp3" | python3 -c "
import sys, json
tools = json.load(sys.stdin)['result']['tools']
ok = all('name' in t and 'description' in t and 'inputSchema' in t for t in tools)
print('yes' if ok else 'no')
")
check "tools/list each tool has name+description+inputSchema" test "$all_have_fields" = "yes"

# Verify inputSchema is object type (not stub)
schemas_valid=$(echo "$resp3" | python3 -c "
import sys, json
tools = json.load(sys.stdin)['result']['tools']
ok = all(t['inputSchema'].get('type') == 'object' for t in tools)
print('yes' if ok else 'no')
")
check "tools/list inputSchemas have type=object" test "$schemas_valid" = "yes"

# --- 3. tools/call system.capabilities ---
resp4=$(get_response 4)
check "system.capabilities isError=false" \
  test "$(echo "$resp4" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['isError'])")" = "False"

cap_envelope=$(echo "$resp4" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
cap_count=$(echo "$cap_envelope" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['count'])")
check "system.capabilities returns 21 capabilities" test "$cap_count" = "21"

# Verify schema fields present
schemas_present=$(echo "$cap_envelope" | python3 -c "
import sys, json
caps = json.load(sys.stdin)['data']['capabilities']
ok = all('input_schema' in c and 'output_schema' in c for c in caps)
print('yes' if ok else 'no')
")
check "system.capabilities entries have input/output schema" test "$schemas_present" = "yes"

# Verify response envelope has required fields
envelope_valid=$(echo "$cap_envelope" | python3 -c "
import sys, json
env = json.load(sys.stdin)
fields = ['status', 'capability', 'request_id', 'data', 'diagnostics']
ok = all(f in env for f in fields)
print('yes' if ok else 'no')
")
check "system.capabilities response envelope structure" test "$envelope_valid" = "yes"

# --- 4. tools/call plan.validate ---
resp5=$(get_response 5)
check "plan.validate isError=false" \
  test "$(echo "$resp5" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['isError'])")" = "False"

plan_envelope=$(echo "$resp5" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
check "plan.validate status=ok" \
  test "$(echo "$plan_envelope" | python3 -c "import sys,json; print(json.load(sys.stdin)['status'])")" = "ok"
check "plan.validate show_id correct" \
  test "$(echo "$plan_envelope" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['show_id'])")" = "show-phase0-minimal"

# --- 5. tools/call compile.run ---
resp6=$(get_response 6)
check "compile.run isError=false" \
  test "$(echo "$resp6" | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['isError'])")" = "False"

compile_envelope=$(echo "$resp6" | python3 -c "import sys,json; r=json.load(sys.stdin)['result']; print(r['content'][0]['text'])")
check "compile.run status=ok" \
  test "$(echo "$compile_envelope" | python3 -c "import sys,json; print(json.load(sys.stdin)['status'])")" = "ok"
check "compile.run produces revision" \
  test "$(echo "$compile_envelope" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']; print('yes' if d.get('revision',0)>=1 else 'no')")" = "yes"
check "compile.run produces timeline" \
  test "$(echo "$compile_envelope" | python3 -c "import sys,json; d=json.load(sys.stdin)['data']; print('yes' if d.get('timeline_entries',0)>=1 else 'no')")" = "yes"

# Verify compiled revision artifact exists on disk
check "compile.run creates revision artifact" \
  test -f "$repo_root/artifacts/revisions/show-phase0-minimal/revision-1/revision.json"

echo ""
echo "Results: $passed passed, $failed failed"
[ "$failed" -eq 0 ]
