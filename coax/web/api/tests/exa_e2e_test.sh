#!/usr/bin/env bash
# Firm reason: 5 (end-to-end shell test for the metered Exa web search & MCP proxy route)
# End-to-end integration test for central metered Exa search, fetch, and MCP
# (/api/v1/search, /api/v1/fetch, /api/v1/mcp/exa) — Issues #14114, #14115.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/../../.." && pwd)"
coax="$root/coax/bin/coax"

if [[ "$(uname -s)" == Darwin ]]; then
  pgbin=/opt/homebrew/opt/postgresql@16/bin
  cc=/usr/bin/clang
  pq_inc=(-I/opt/homebrew/opt/postgresql@16/include); pq_lib=(-L/opt/homebrew/opt/postgresql@16/lib)
  secp="$(brew --prefix secp256k1)"; secp_inc=(-I"$secp/include"); secp_lib=(-L"$secp/lib")
  ssl="$(brew --prefix openssl@3)"; ssl_inc=(-I"$ssl/include"); ssl_lib=(-L"$ssl/lib")
  sec_fw=(-framework CoreFoundation -framework Security)
else
  pgbin=/usr/lib/postgresql/16/bin
  cc="${CC:-clang}"
  pq_inc=(-I"$(pg_config --includedir 2>/dev/null || echo /usr/include/postgresql)"); pq_lib=()
  secp_inc=(); secp_lib=(); ssl_inc=(); ssl_lib=(); sec_fw=()
fi

work="$(mktemp -d)"
pg_pid=""
api_pid=""
cleanup() {
  [[ -n "$api_pid" ]] && kill "$api_pid" 2>/dev/null || true
  [[ -n "$pg_pid" ]] && "$pgbin/pg_ctl" -D "$work/pg" -m immediate stop >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

# 1. Compile oa-api and sign helper
source "$here/api-sources.sh"
api_sources "$here" "$work" "$coax"
"$cc" -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE \
  -I"$here" -I"$work" -I"$root/coax/platform" "${pq_inc[@]}" "${secp_inc[@]}" "${ssl_inc[@]}" \
  -o "$work/api" "${API_SOURCES[@]}" \
  "${pq_lib[@]}" "${secp_lib[@]}" "${ssl_lib[@]}" \
  -lpthread -lpq -lsecp256k1 -lssl -lcrypto -lcurl -lz -lm "${sec_fw[@]}"

"$cc" -std=c11 -O2 -D_DEFAULT_SOURCE "${secp_inc[@]}" "${ssl_inc[@]}" \
  -o "$work/sign" "$here/sign.c" "${secp_lib[@]}" "${ssl_lib[@]}" -lsecp256k1 -lcrypto

# 2. Start throwaway Postgres 16
"$pgbin/initdb" -D "$work/pg" -U oa_analytics --auth=trust --no-sync >/dev/null 2>&1
pg_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
"$pgbin/pg_ctl" -D "$work/pg" -o "-k $work -p $pg_port -h 127.0.0.1" -l "$work/pg.log" start >/dev/null
pg_pid="running"
"$pgbin/createdb" -h "$work" -p "$pg_port" -U oa_analytics oa_analytics
export OA_API_DATABASE="host=$work port=$pg_port user=oa_analytics dbname=oa_analytics"

# 3. Run all migrations (including 0220_exa_search_billing.sql)
"$work/api" --migrations "$here/migrations" --migrate-only

# 4. Generate test Nostr keys
FREE_KEY="$work/free.key"; FREE_PUB="$("$work/sign" --new "$FREE_KEY")"
PAID_KEY="$work/paid.key"; PAID_PUB="$("$work/sign" --new "$PAID_KEY")"
BROKE_KEY="$work/broke.key"; BROKE_PUB="$("$work/sign" --new "$BROKE_KEY")"

api_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
exa_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

export OA_API_ORIGIN="http://127.0.0.1:$api_port"
export OA_INFERENCE_PUBKEYS="$FREE_PUB"
export EXA_API_KEY="exa-test-central-secret-key"
export OA_EXA_BASE_URL="http://127.0.0.1:$exa_port"
export OA_EXA_DAILY_REQUESTS="50"
export OA_EXA_DAILY_USD_MICROS="500000"
export OA_API_WORKERS="1"

"$work/api" --listen "127.0.0.1:$api_port" >"$work/api.log" 2>&1 &
api_pid=$!

for _ in $(seq 1 50); do
  if curl -sf "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null 2>&1; then break; fi
  sleep 0.1
done

# 5. Run end-to-end Python verification against fake Exa + live oa-api + Postgres
python3 - "$work/sign" "$FREE_KEY" "$FREE_PUB" "$PAID_KEY" "$PAID_PUB" "$BROKE_KEY" "$BROKE_PUB" "$api_port" "$exa_port" "$pgbin/psql" "$work" "$pg_port" <<'PY'
import http.server, json, subprocess, sys, threading, urllib.request, urllib.error

SIGN, FREE_KEY, FREE_PUB, PAID_KEY, PAID_PUB, BROKE_KEY, BROKE_PUB, API_PORT, EXA_PORT, PSQL, PGHOST, PGPORT = sys.argv[1:]
API = f"http://127.0.0.1:{API_PORT}"

seen = []
mode = {"status": 200, "cost": 0.005, "omit_cost": False}

class FakeExa(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        body = json.loads(raw)
        seen.append({
            "path": self.path,
            "api_key": self.headers.get("x-api-key"),
            "exa_beta": self.headers.get("Exa-Beta"),
            "body": body,
        })
        if mode["status"] != 200:
            out = json.dumps({"error": "upstream down"}).encode()
            self.send_response(mode["status"])
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            self.wfile.write(out)
            return
        if self.path == "/search":
            resp = {
                "results": [{
                    "title": "Coax Compiler Guide",
                    "url": "https://docs.openagents.com/coax",
                    "publishedDate": "2026-11-10T00:00:00Z",
                    "author": "OpenAgents",
                    "highlights": ["Coax uses explicit effect sets and #authority modules."],
                }],
                "searchTime": 123.4,
            }
        else:
            resp = {
                "results": [{
                    "title": "Exa Reference",
                    "url": body["urls"][0],
                    "publishedDate": "2026-11-10T00:00:00Z",
                    "author": "Exa",
                    "text": "Clean extracted markdown content from Exa /contents.",
                }],
                "statuses": [{"id": body["urls"][0], "status": "success"}],
                "searchTime": 88.2,
            }
        if not mode["omit_cost"]:
            resp["costDollars"] = {"total": mode["cost"]}
        out = json.dumps(resp).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass

srv = http.server.ThreadingHTTPServer(("127.0.0.1", int(EXA_PORT)), FakeExa)
threading.Thread(target=srv.serve_forever, daemon=True).start()

def sql(q):
    r = subprocess.run([PSQL, "-h", PGHOST, "-p", PGPORT, "-U", "oa_analytics", "-d", "oa_analytics", "-At", "-c", q],
                       capture_output=True, text=True, check=True)
    return r.stdout.strip()

def nip98(keyfile, method, path):
    return "Nostr " + subprocess.run([SIGN, "--key", keyfile, method, API + path], capture_output=True, text=True, check=True).stdout.strip()

def req(method, path, payload=None, keyfile=None):
    headers = {"Content-Type": "application/json"}
    if keyfile:
        headers["Authorization"] = nip98(keyfile, method, path)
    data = json.dumps(payload).encode() if payload is not None else None
    r = urllib.request.Request(API + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(r, timeout=10) as resp:
            return resp.status, resp.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()

# Seed BTC/USD rate ($100,000/BTC => 1 USD micro = 1 msat!) and 100,000 msat credit for PAID_PUB
sql("SELECT inference.rate_set(100000);")
sql(f"INSERT INTO inference.credit (pubkey, msat) VALUES ('{PAID_PUB}', 100000);")

# --- 1. Auth & Validation on /api/v1/search ---
st, _ = req("POST", "/api/v1/search", {"query": "coax"}, keyfile=None)
assert st == 401, f"expected 401 unsigned, got {st}"
st, _ = req("GET", "/api/v1/search", None, keyfile=FREE_KEY)
assert st == 405, f"expected 405 GET, got {st}"
st, b = req("POST", "/api/v1/search", {"query": "coax", "type": "deep"}, keyfile=FREE_KEY)
assert st == 400 and "auto or fast" in b, (st, b)
st, b = req("POST", "/api/v1/search", {"query": "coax", "allowed_domains": ["a.com"], "blocked_domains": ["b.com"]}, keyfile=FREE_KEY)
assert st == 400 and "cannot both" in b, (st, b)
st, b = req("POST", "/api/v1/search", {"query": "coax", "allowed_domains": ["https://bad.com"]}, keyfile=FREE_KEY)
assert st == 400 and "invalid domain" in b, (st, b)
st, b = req("POST", "/api/v1/search", {"query": "coax"}, keyfile=BROKE_KEY)
assert st == 402 and "/api/v1/inference/credit" in b, (st, b)
assert len(seen) == 0, f"invalid/unfunded requests must never reach Exa: {seen}"

# --- 2. Free key /api/v1/search with clamping + Dynamic Highlights + inline category ---
st, b = req("POST", "/api/v1/search", {"query": "category:github coax compiler", "num_results": 50, "allowed_domains": ["github.com"]}, keyfile=FREE_KEY)
assert st == 200, (st, b)
j = json.loads(b)
assert j["results"][0]["title"] == "Coax Compiler Guide"
assert j["billing"]["upstream_usd_micros"] == 5000
assert j["billing"]["usd_micros"] == 6250
assert j["billing"]["msat"] == 0 and j["billing"]["free"] is True
assert seen[-1]["api_key"] == "exa-test-central-secret-key"
assert seen[-1]["exa_beta"] == "dynamic-highlights-2026-08-28"
assert seen[-1]["body"]["numResults"] == 10
assert seen[-1]["body"]["category"] == "github"
assert seen[-1]["body"]["query"] == "coax compiler"
assert seen[-1]["body"]["includeDomains"] == ["github.com"]

# --- 3. Paid key /api/v1/search with exact costDollars.total = 0.0125 -> 15625 micros / 15625 msat ---
mode["cost"] = 0.0125
st, b = req("POST", "/api/v1/search", {"query": "openagents exa"}, keyfile=PAID_KEY)
assert st == 200, (st, b)
j = json.loads(b)
assert j["billing"]["upstream_usd_micros"] == 12500
assert j["billing"]["usd_micros"] == 15625
assert j["billing"]["msat"] == 15625 and j["billing"]["free"] is False
assert int(sql(f"SELECT msat FROM inference.credit WHERE pubkey = '{PAID_PUB}'")) == 100000 - 15625

# --- 4. Fallback per_call_micros when costDollars omitted ---
mode["omit_cost"] = True
st, b = req("POST", "/api/v1/search", {"query": "fallback test"}, keyfile=PAID_KEY)
assert st == 200, (st, b)
j = json.loads(b)
assert j["billing"]["usd_micros"] == 6250 and j["billing"]["msat"] == 6250
mode["omit_cost"] = False

# --- 5. Upstream 502 never charged ---
mode["status"] = 502
bal_before = int(sql(f"SELECT msat FROM inference.credit WHERE pubkey = '{PAID_PUB}'"))
st, _ = req("POST", "/api/v1/search", {"query": "fail test"}, keyfile=PAID_KEY)
assert st == 502
assert int(sql(f"SELECT msat FROM inference.credit WHERE pubkey = '{PAID_PUB}'")) == bal_before
mode["status"] = 200

# --- 6. /api/v1/fetch SSRF guard + batch cap + metering ---
st, b = req("POST", "/api/v1/fetch", {"urls": ["http://127.0.0.1/secret"]}, keyfile=PAID_KEY)
assert st == 400 and "blocked or invalid URL" in b, (st, b)
st, b = req("POST", "/api/v1/fetch", {"urls": ["https://a.com/1","https://a.com/2","https://a.com/3","https://a.com/4","https://a.com/5","https://a.com/6"]}, keyfile=PAID_KEY)
assert st == 400 and "1 to 5" in b, (st, b)
mode["cost"] = 0.002
st, b = req("POST", "/api/v1/fetch", {"urls": ["https://docs.exa.ai/reference/search"], "max_characters": 99999}, keyfile=PAID_KEY)
assert st == 200, (st, b)
j = json.loads(b)
assert j["billing"]["model"] == "exa/contents"
assert j["billing"]["upstream_usd_micros"] == 2000
assert j["billing"]["usd_micros"] == 2500 and j["billing"]["msat"] == 2500
assert seen[-1]["body"]["text"]["maxCharacters"] == 24000

# --- 7. First-party metered Streamable HTTP MCP (/api/v1/mcp/exa) ---
st, b = req("POST", "/api/v1/mcp/exa", {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}, keyfile=PAID_KEY)
assert st == 200, (st, b)
j = json.loads(b)
assert j["result"]["protocolVersion"] == "2025-03-26" and j["result"]["serverInfo"]["name"] == "openagents-exa"

st, _ = req("POST", "/api/v1/mcp/exa", {"jsonrpc": "2.0", "method": "notifications/initialized"}, keyfile=PAID_KEY)
assert st == 204, st

st, b = req("POST", "/api/v1/mcp/exa", {"jsonrpc": "2.0", "id": 2, "method": "tools/list"}, keyfile=PAID_KEY)
assert st == 200, (st, b)
tool_names = [t["name"] for t in json.loads(b)["result"]["tools"]]
assert tool_names == ["web_search_exa", "web_fetch_exa"], tool_names

mode["cost"] = 0.004
st, b = req("POST", "/api/v1/mcp/exa", {
    "jsonrpc": "2.0", "id": 3, "method": "tools/call",
    "params": {"name": "web_search_exa", "arguments": {"query": "mcp search", "numResults": 3}}
}, keyfile=PAID_KEY)
assert st == 200, (st, b)
j = json.loads(b)
assert j["result"]["isError"] is False
assert "Title: Coax Compiler Guide" in j["result"]["content"][0]["text"]
assert j["result"]["_meta"]["billing"]["usd_micros"] == 5000

st, b = req("POST", "/api/v1/mcp/exa", {
    "jsonrpc": "2.0", "id": 4, "method": "tools/call",
    "params": {"name": "web_search_exa", "arguments": {"query": "no credit"}}
}, keyfile=BROKE_KEY)
assert st == 402 and json.loads(b)["error"]["code"] == -32002, (st, b)

# --- 8. /api/v1/inference/pricing & /api/v1/inference/usage ---
st, b = req("GET", "/api/v1/inference/pricing")
assert st == 200
models = {m["name"]: m for m in json.loads(b)["models"]}
assert "exa/search" in models and models["exa/search"]["kind"] == "search" and models["exa/search"]["markup_bps"] == 2500
assert "exa/contents" in models and models["exa/contents"]["kind"] == "search"

st, b = req("GET", "/api/v1/inference/usage", None, keyfile=PAID_KEY)
assert st == 200, (st, b)
u = json.loads(b)
expected_spend = 15625 + 6250 + 2500 + 5000
assert u["search"]["calls"] == 5, u["search"]  # includes the 502 logged at $0
assert u["search"]["usd_micros"] == expected_spend, u["search"]
assert u["search"]["msat"] == expected_spend, u["search"]
assert u["credit_msat"] == 100000 - expected_spend, u

print("ALL EXA E2E CHECKS PASSED! (REST /search, /fetch, MCP /api/v1/mcp/exa, billing, pricing, usage)")
PY
