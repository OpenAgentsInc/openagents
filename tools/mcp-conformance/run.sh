#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: builds Coax MCP binaries via coax/web/tool.sh and invokes external Node/npx @modelcontextprotocol/{server-everything,inspector,conformance} oracles)
# Phase 2–5 Coax MCP conformance & interop runner.
# Builds the Coax MCP stdio harnesses via coax/web/tool.sh and verifies:
#   1. Coax stdio client <-> Coax stdio server (2026-07-28 mode)
#   2. Coax stdio client <-> Coax stdio server (--2025-only fallback probe)
#   3. Coax stdio client <-> official @modelcontextprotocol/server-everything (if available)
#   4. Official @modelcontextprotocol/inspector --cli <-> Coax stdio server (if available)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
tool="$repo/coax/web/tool.sh"

echo "==> Building Coax MCP stdio server & client binaries"
COAX_TOOL_BUILD_ONLY=1 "$tool" "$here/stdio-server.coax"
COAX_TOOL_BUILD_ONLY=1 "$tool" "$here/stdio-client.coax"

coax="${COAX:-$repo/coax/bin/coax}"
chash="$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"
srv_bin="$repo/coax/web/build/tools/stdio-server-$chash"
cli_bin="$repo/coax/web/build/tools/stdio-client-$chash"

echo "==> [1/4] Coax stdio client <-> Coax stdio server (2026-07-28 stateless)"
out_2026="$("$cli_bin" \
  --call echo --args '{"message":"hello-2026"}' \
  --resource memo://hello \
  --prompt greet --prompt-args '{"name":"Coax"}' \
  --test-cancel \
  -- "$srv_bin")"
echo "$out_2026"
grep -q "CONNECTED mode=2026-07-28 version=2026-07-28 server=coax-mcp-stdio/0.2.0" <<<"$out_2026"
grep -q "TOOLS count=3 valid_headers=3" <<<"$out_2026"
grep -q "TOOL_CALL name=echo isError=false text=hello-2026" <<<"$out_2026"
grep -q "RESOURCE_READ uri=memo://hello text=Hello from Coax MCP stdio server!" <<<"$out_2026"
grep -q "PROMPT_GET name=greet messages=1" <<<"$out_2026"
grep -q "CANCEL ok=true" <<<"$out_2026"

echo "==> [2/4] Coax stdio client <-> Coax stdio server (2025-11-25 discover->initialize fallback)"
out_2025="$("$cli_bin" \
  --call add --args '{"a":19,"b":23}' \
  --resource memo://42 \
  --prompt greet --prompt-args '{"name":"Legacy"}' \
  --test-cancel \
  -- "$srv_bin" --2025-only)"
echo "$out_2025"
grep -q "CONNECTED mode=2025-11-25 version=2025-11-25 server=coax-mcp-stdio/0.2.0" <<<"$out_2025"
grep -q "TOOL_CALL name=add isError=false text=42" <<<"$out_2025"
grep -q "RESOURCE_READ uri=memo://42 text=Memo item: memo://42" <<<"$out_2025"
grep -q "PING ok=true" <<<"$out_2025"
grep -q "CANCEL ok=true" <<<"$out_2025"

echo "==> [3/4] Coax stdio client <-> @modelcontextprotocol/server-everything"
everything_dir="${MCP_EVERYTHING_DIR:-$HOME/work/projects/repos/mcp/servers/src/everything}"
everything_cmd=()
if [[ -f "$everything_dir/dist/index.js" ]]; then
  everything_cmd=(node "$everything_dir/dist/index.js")
elif command -v npx >/dev/null 2>&1 && [[ "${MCP_ALLOW_NPX:-0}" == "1" ]]; then
  everything_cmd=(npx -y @modelcontextprotocol/server-everything)
fi
if [[ ${#everything_cmd[@]} -gt 0 ]]; then
  out_ev="$("$cli_bin" --call echo --args '{"message":"from-coax"}' --test-cancel -- "${everything_cmd[@]}")"
  echo "$out_ev"
  grep -Eq "CONNECTED mode=(2026-07-28|2025-11-25)" <<<"$out_ev"
  grep -q "TOOL_CALL name=echo isError=false text=Echo: from-coax" <<<"$out_ev"
  grep -q "CANCEL ok=true" <<<"$out_ev"
  out_ev_legacy="$("$cli_bin" --legacy-2025 --call echo --args '{"message":"from-coax-2025"}' --test-cancel -- "${everything_cmd[@]}")"
  echo "$out_ev_legacy"
  grep -q "CONNECTED mode=2025-11-25" <<<"$out_ev_legacy"
  grep -q "TOOL_CALL name=echo isError=false text=Echo: from-coax-2025" <<<"$out_ev_legacy"
  grep -q "PING ok=true" <<<"$out_ev_legacy"
  grep -q "CANCEL ok=true" <<<"$out_ev_legacy"
else
  echo "SKIP: $everything_dir/dist/index.js not built (set MCP_EVERYTHING_DIR or MCP_ALLOW_NPX=1)"
fi

echo "==> [4/4] @modelcontextprotocol/inspector --cli <-> Coax stdio server"
inspector_cli="${MCP_INSPECTOR_CLI:-$HOME/work/projects/repos/mcp/inspector/cli/build/cli.js}"
inspector_cmd=()
if [[ -f "$inspector_cli" ]]; then
  inspector_cmd=(node "$inspector_cli")
elif command -v npx >/dev/null 2>&1 && [[ "${MCP_ALLOW_NPX:-0}" == "1" ]]; then
  inspector_cmd=(npx -y @modelcontextprotocol/inspector)
fi
if [[ ${#inspector_cmd[@]} -gt 0 ]]; then
  "${inspector_cmd[@]}" --cli "$srv_bin" --method tools/list | grep -q '"echo"'
  "${inspector_cmd[@]}" --cli "$srv_bin" --method tools/call --tool-name add --tool-arg a=7 --tool-arg b=8 | grep -q '15'
  "${inspector_cmd[@]}" --cli "$srv_bin" --method resources/list | grep -q 'memo://hello'
  "${inspector_cmd[@]}" --cli "$srv_bin" --method prompts/list | grep -q '"greet"'
  echo "INSPECTOR_CLI ok=true"
else
  echo "SKIP: $inspector_cli not built (set MCP_INSPECTOR_CLI or MCP_ALLOW_NPX=1)"
fi

echo "ALL PHASE 2 STDIO INTEROP CHECKS PASSED"

echo "==> Building Coax MCP Streamable HTTP conformance server (Phase 3)"
COAX_TOOL_BUILD_ONLY=1 "$tool" "$here/everything-server.coax"
http_srv_bin="$repo/coax/web/build/tools/everything-server-$chash"
http_port="${MCP_HTTP_PORT:-18421}"
"$http_srv_bin" --port "$http_port" >/tmp/coax-mcp-http-srv.log 2>&1 &
http_pid=$!
trap 'kill "$http_pid" 2>/dev/null || true' EXIT
sleep 1

echo "==> [5/6] Coax Streamable HTTP server 2026-07-28 stateless & Origin/Host checks"
curl -fsS -X POST "http://127.0.0.1:$http_port/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "MCP-Protocol-Version: 2026-07-28" \
  -d '{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{}}' | grep -q '"supportedVersions":\["2026-07-28"'
curl -fsS -X POST "http://127.0.0.1:$http_port/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "MCP-Protocol-Version: 2026-07-28" \
  -H "Mcp-Method: tools/call" \
  -H "Mcp-Name: test_simple_text" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"test_simple_text","arguments":{}}}' | grep -q 'This is a simple text response for testing\.'
bad_origin_code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$http_port/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "Origin: https://evil.example.com" \
  -d '{"jsonrpc":"2.0","id":3,"method":"ping"}')"
[[ "$bad_origin_code" == "403" ]]
echo "HTTP_2026_STATELESS ok=true"

echo "==> [6/6] @modelcontextprotocol/conformance server --suite all <-> Coax Streamable HTTP server"
if command -v npx >/dev/null 2>&1 && [[ "${MCP_ALLOW_NPX:-0}" == "1" ]]; then
  npx -y @modelcontextprotocol/conformance server --url "http://127.0.0.1:$http_port/mcp" --suite all
else
  echo "SKIP: set MCP_ALLOW_NPX=1 to run @modelcontextprotocol/conformance server --suite all"
fi

echo "==> Building Coax MCP Streamable HTTP conformance client (Phase 4)"
COAX_TOOL_BUILD_ONLY=1 "$tool" "$here/everything-client.coax"
http_cli_bin="$repo/coax/web/build/tools/everything-client-$chash"

echo "==> [7/8] Coax Streamable HTTP client <-> Coax Streamable HTTP server (2026-07-28 stateless)"
out_hc_2026="$("$http_cli_bin" --2026-suite "http://127.0.0.1:$http_port/mcp")"
echo "$out_hc_2026"
grep -q "CONNECTED mode=2026-07-28 version=2026-07-28 server=coax-mcp-conformance-server/0.3.0" <<<"$out_hc_2026"
grep -q "TOOLS count=14 valid_headers=14 schema_2020_12_preserved=true" <<<"$out_hc_2026"
grep -q "CALL_SIMPLE_TEXT ok=true" <<<"$out_hc_2026"
grep -q "RESOURCE_READ ok=true" <<<"$out_hc_2026"
grep -q "PROMPT_GET ok=true" <<<"$out_hc_2026"

echo "==> [8/8] @modelcontextprotocol/conformance client (non-auth suite) <-> Coax Streamable HTTP client"
if command -v npx >/dev/null 2>&1 && [[ "${MCP_ALLOW_NPX:-0}" == "1" ]]; then
  for scen in initialize tools_call elicitation-sep1034-client-defaults sse-retry; do
    echo "--> client scenario: $scen"
    npx -y @modelcontextprotocol/conformance client --command "$http_cli_bin" --scenario "$scen"
  done
else
  echo "SKIP: set MCP_ALLOW_NPX=1 to run @modelcontextprotocol/conformance client scenarios"
fi

echo "ALL PHASE 2 + PHASE 3 + PHASE 4 CONFORMANCE CHECKS PASSED"
