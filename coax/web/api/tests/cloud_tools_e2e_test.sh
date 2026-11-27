#!/usr/bin/env bash
# Firm reason: 5 (thin offline integration test harness starting throwaway Postgres 16, mock WorkOS Pipes/GitHub/Exa/Web servers, and ASan oa-api).
# End-to-end Phase 3 Tier 1 Cloud Tools verification (#14127):
#   - POST /api/tools/cloud/v1: web_search (Exa), web_fetch (SSRF-guarded),
#     github_tree, github_read_file (with line slicing), github_search_code,
#     github_list_issues, github_get_pr
#   - Closed allowlist enforcement (refuses bash/unknown tools, path traversal, SSRF metadata/RFC1918)
#   - Auth admission (401 unauth) and zero Exa/GitHub token leaks in responses, headers, or api.log
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
api_dir="$(cd "$here/.." && pwd)"
repo="$(cd "$api_dir/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"

if [[ "$(uname -s)" == Darwin ]]; then
  pgbin=/opt/homebrew/opt/postgresql@16/bin
  cc=/usr/bin/clang
  pq_inc=(-I/opt/homebrew/opt/postgresql@16/include); pq_lib=(-L/opt/homebrew/opt/postgresql@16/lib)
  secp="$(brew --prefix secp256k1)"; secp_inc=(-I"$secp/include"); secp_lib=(-L"$secp/lib")
  ssl="$(brew --prefix openssl@3)"; ssl_inc=(-I"$ssl/include"); ssl_lib=(-L"$ssl/lib")
  darwin_fw=(-framework Security -framework CoreFoundation)
else
  pgbin=/usr/lib/postgresql/16/bin
  cc="${CC:-clang}"
  pq_inc=(-I"$(pg_config --includedir 2>/dev/null || echo /usr/include/postgresql)"); pq_lib=()
  secp_inc=(); secp_lib=(); ssl_inc=(); ssl_lib=()
  darwin_fw=()
fi

work="$(mktemp -d)"
pg_pid=""; mock_pid=""; api_pid=""
cleanup() {
  [[ -n "$api_pid" ]] && kill "$api_pid" 2>/dev/null || true
  [[ -n "$mock_pid" ]] && kill "$mock_pid" 2>/dev/null || true
  "$pgbin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

api_bin="${OA_API_BIN:-$work/api}"
if [[ ! -x "$api_bin" ]]; then
  source "$api_dir/api-sources.sh"
  api_sources "$api_dir" "$work" "$coax"
  "$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -DOA_LNADDR_TEST_FETCH \
    -fsanitize=address,undefined -I"$work" "${pq_inc[@]}" "${secp_inc[@]}" "${ssl_inc[@]}" \
    -o "$api_bin" "${API_SOURCES[@]}" \
    "${pq_lib[@]}" -lpq "${secp_lib[@]}" -lsecp256k1 "${ssl_lib[@]}" -lssl -lcrypto -lcurl -lz -lpthread -lm ${darwin_fw[@]+"${darwin_fw[@]}"}
fi

pg_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
api_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
mock_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

"$pgbin/initdb" -D "$work/db" -U postgres --auth=trust --no-sync >/dev/null 2>&1
"$pgbin/pg_ctl" -D "$work/db" -l "$work/pg.log" -o "-k $work -p $pg_port -h 127.0.0.1 -F" start >/dev/null
"$pgbin/createdb" -h 127.0.0.1 -p "$pg_port" -U postgres oa_analytics
"$pgbin/psql" -h 127.0.0.1 -p "$pg_port" -U postgres -d oa_analytics -c "CREATE ROLE oa_analytics SUPERUSER LOGIN" >/dev/null
pgurl="host=127.0.0.1 port=$pg_port user=oa_analytics dbname=oa_analytics"

SECRET_GH="gho_NEVER_LEAK_CLOUD_TOOLS_GH_001"
SECRET_EXA="exa_NEVER_LEAK_CLOUD_TOOLS_EXA_002"

cat > "$work/mock_upstreams.py" <<'PY'
import json, os, sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse

PORT = int(sys.argv[1])
GH = os.environ["SECRET_GH"]
EXA = os.environ["SECRET_EXA"]

class H(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args): pass
    def _send(self, code, data, ctype="application/json"):
        body = data.encode("utf-8") if isinstance(data, str) else json.dumps(data).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        p = urlparse(self.path).path
        if p == "/health": return self._send(200, {"ok": True})
        if p == "/page":
            return self._send(200, "<html><head><script>evil()</script></head><body><h1>OpenAgents Docs</h1><p>Cloud execution works.</p></body></html>", "text/html")
        if p == "/api-commits":
            acc = (self.headers.get("Accept") or "").lower()
            if "application/json" not in acc and "*/*" not in acc:
                return self._send(415, {"message": "Unsupported Media Type"})
            return self._send(200, [{"sha": "abc1234", "commit": {"message": "Fix thought_signature <dev@openagents.com>"}}])
        if p.startswith("/gh/"):
            assert self.headers.get("Authorization") == f"Bearer {GH}", self.headers.get("Authorization")
            if p == "/gh/repos/OpenAgentsInc/openagents/contents":
                return self._send(200, [{"type": "dir", "path": "coax"}, {"type": "file", "path": "AGENTS.md"}])
            if p == "/gh/repos/OpenAgentsInc/openagents/contents/AGENTS.md":
                return self._send(200, "Line one\nLine two\nLine three\nLine four\n", "text/plain")
            if p == "/gh/search/code":
                return self._send(200, {"items": [{"path": "coax/web/api/cloud-tools.c"}]})
            if p == "/gh/repos/OpenAgentsInc/openagents/issues":
                return self._send(200, [{"number": 14127, "state": "open", "title": "Phase 3 Cloud Tools"}])
            if p == "/gh/repos/OpenAgentsInc/openagents/pulls/42":
                return self._send(200, {"number": 42, "state": "open", "title": "Add tools", "additions": 100, "deletions": 5, "changed_files": 4, "body": "Implements Tier 1."})
        return self._send(404, {"error": "not_found", "path": p})
    def do_POST(self):
        ln = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(ln).decode("utf-8") if ln > 0 else "{}"
        p = urlparse(self.path).path
        if p == "/data-integrations/github/token":
            assert self.headers.get("Authorization") == "Bearer sk_test_tools"
            return self._send(200, {"active": True, "access_token": {"access_token": GH, "scopes": ["repo"]}})
        if p == "/exa/search":
            assert self.headers.get("x-api-key") == EXA
            return self._send(200, {"results": [{"title": "Coax Lang", "url": "https://openagents.com/coax", "text": "Pure Coax execution."}]})
        return self._send(404, {"error": "not_found", "path": p})

HTTPServer(("127.0.0.1", PORT), H).serve_forever()
PY

SECRET_GH="$SECRET_GH" SECRET_EXA="$SECRET_EXA" python3 "$work/mock_upstreams.py" "$mock_port" >"$work/mock.log" 2>&1 &
mock_pid=$!
mock_base="http://127.0.0.1:$mock_port"
for _ in $(seq 1 40); do curl -fsS "$mock_base/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "$mock_base/health" >/dev/null

api_origin="http://127.0.0.1:$api_port"
ASAN_OPTIONS=detect_leaks=0 \
OA_API_DATABASE="$pgurl" \
OA_API_ORIGIN="$api_origin" \
OA_API_WORKERS=1 \
OA_AUTH_SEAL_KEY="1:0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20" \
OA_AUTH_LOOKUP_PEPPER="1:201f1e1d1c1b1a191817161514131211100f0e0d0c0b0a090807060504030201" \
OA_WORKOS_ENABLED=1 \
OA_WORKOS_BASE_URL="$mock_base" \
OA_WORKOS_API_KEY="sk_test_tools" \
OA_WORKOS_CLIENT_ID="client_01TOOLS_E2E" \
OA_GITHUB_API_BASE="$mock_base/gh" \
OA_EXA_API_BASE="$mock_base/exa" \
OA_EXA_API_KEY="$SECRET_EXA" \
OA_CLOUD_TOOLS_ALLOW_LOCAL=1 \
  "$api_bin" --listen "127.0.0.1:$api_port" --migrations "$api_dir/migrations" >"$work/api.log" 2>&1 &
api_pid=$!

for _ in $(seq 1 60); do curl -fsS "$api_origin/api/playtest/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "$api_origin/api/playtest/health" >/dev/null || { cat "$work/api.log" >&2; exit 1; }

python3 - "$pgurl" "$api_origin" "$mock_base" "$SECRET_GH" "$SECRET_EXA" "$work/api.log" "$pgbin/psql" <<'PY'
import base64, hashlib, json, subprocess, sys, urllib.request, urllib.error, zlib

pgurl, api, mock, s_gh, s_exa, api_log, psql = sys.argv[1:8]
B32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
def make_tok(raw32):
    head = "oas_" + base64.b32encode(raw32).rstrip(b"=").decode()
    crc = (zlib.crc32(head.encode()) & 0xFFFFFFFF) >> 2
    return head + "".join(B32[(crc >> (25 - 5 * i)) & 31] for i in range(6))

tok = make_tok(b"\x33" * 32)
h = hashlib.sha256(tok.encode()).hexdigest()

subprocess.run([psql, pgurl, "-v", "ON_ERROR_STOP=1", "-c", f"""
INSERT INTO auth.accounts (id, kind, verse_account, team) VALUES ('acct_cccccccccccccccccccccccccc', 'person', 'v_tools', true);
INSERT INTO auth.sessions (token_sha256, id, account, via_kind, client, level, l1_at, epoch)
  VALUES ('{h}', 'ses_tools', 'acct_cccccccccccccccccccccccccc', 'workos', 'web', 'L1', now(), 0);
INSERT INTO auth.workos_identities (workos_user_id, account, provider)
  VALUES ('user_01TOOLS_TEST', 'acct_cccccccccccccccccccccccccc', 'github');
INSERT INTO chat.environment_connections_v1 (account, host_pubkey, display_name, grant_id, epoch, relay)
  VALUES ('acct_cccccccccccccccccccccccccc', '{"a"*64}', 'Mac Studio Lab', '{"1"*64}', 1, 'wss://relay.openagents.com');
"""], check=True, stdout=subprocess.DEVNULL)

def call(body, cookie=tok):
    hdrs = {"Content-Type": "application/json"}
    if cookie:
        hdrs["Cookie"] = f"__Host-oa_session={cookie}"
        hdrs["Origin"] = api
    r = urllib.request.Request(api + "/api/tools/cloud/v1", data=json.dumps(body).encode(), headers=hdrs, method="POST")
    try:
        with urllib.request.urlopen(r, timeout=10) as resp:
            txt = resp.read().decode()
            assert s_gh not in txt and s_exa not in txt, "LEAK in body!"
            assert s_gh not in str(resp.headers) and s_exa not in str(resp.headers), "LEAK in headers!"
            return resp.status, json.loads(txt)
    except urllib.error.HTTPError as e:
        txt = e.read().decode()
        assert s_gh not in txt and s_exa not in txt, "LEAK in error!"
        return e.code, txt

# 1. Auth gate
st, _ = call({"tool": "web_search", "arguments": {"query": "hi"}}, cookie=None)
assert st == 401, st

# 2. Allowlist / traversal / SSRF refusals -> 400
for bad in [
    {"tool": "bash", "arguments": {"cmd": "id"}},
    {"tool": "github_read_file", "arguments": {"repo": "OpenAgentsInc/openagents", "path": "../etc/passwd"}},
    {"tool": "web_fetch", "arguments": {"url": "https://169.254.169.254/latest"}},
    {"tool": "web_fetch", "arguments": {"url": "https://metadata.google.internal/x"}},
]:
    st, txt = call(bad)
    assert st == 400, (bad, st, txt)

# 3. All 7 Tier 1 Cloud Tools work end-to-end
st, j = call({"tool": "web_search", "arguments": {"query": "Coax", "num_results": 3}})
assert st == 200 and j["ok"] and "Coax Lang" in j["output"], j

st, j = call({"tool": "web_fetch", "arguments": {"url": f"{mock}/page"}})
assert st == 200 and j["ok"] and "OpenAgents Docs" in j["output"] and "evil()" not in j["output"], j

st, j = call({"tool": "web_fetch", "arguments": {"url": f"{mock}/api-commits"}})
assert st == 200 and j["ok"] and "abc1234" in j["output"] and "<dev@openagents.com>" in j["output"], j

st, j = call({"tool": "github_tree", "arguments": {"repo": "OpenAgentsInc/openagents"}})
assert st == 200 and j["ok"] and "d coax\nf AGENTS.md\n" == j["output"], j

st, j = call({"tool": "github_read_file", "arguments": {"repo": "OpenAgentsInc/openagents", "path": "AGENTS.md", "from_line": 2, "to_line": 3}})
assert st == 200 and j["ok"] and j["output"] == "2: Line two\n3: Line three\n", j

st, j = call({"tool": "github_search_code", "arguments": {"repo": "OpenAgentsInc/openagents", "query": "cloud_tools_route"}})
assert st == 200 and j["ok"] and "coax/web/api/cloud-tools.c" in j["output"], j

st, j = call({"tool": "github_list_issues", "arguments": {"repo": "OpenAgentsInc/openagents"}})
assert st == 200 and j["ok"] and "#14127 [open] Phase 3 Cloud Tools" in j["output"], j

st, j = call({"tool": "github_get_pr", "arguments": {"repo": "OpenAgentsInc/openagents", "number": 42}})
assert st == 200 and j["ok"] and "PR #42 [open] Add tools" in j["output"] and "Implements Tier 1." in j["output"], j

# 4. Tier 2 Computer Pinning on POST /api/chat/submit/v2alpha1?host=<64hex>
import re
UUID4_RE = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
def is_uuid4(s):
    return isinstance(s, str) and UUID4_RE.match(s) is not None

def submit_pin(op_id, host_q):
    hdrs = {"Content-Type": "application/json", "Cookie": f"__Host-oa_session={tok}", "Origin": api}
    payload = {"schema": "openagents.chat.submit.v2alpha1", "operation_id": op_id, "conversation_id": "", "expected_revision": 0, "project_id": None, "expected_project_revision": None, "legacy_thread_id": None, "legacy_thread_revision": None, "content": [{"type": "text", "text": "Run tests on Mac Studio"}]}
    r = urllib.request.Request(f"{api}/api/chat/submit/v2alpha1?host={host_q}", data=json.dumps(payload).encode(), headers=hdrs, method="POST")
    try:
        with urllib.request.urlopen(r, timeout=10) as resp:
            return resp.status, json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()

st, _ = submit_pin("op_pin_bad", "not-a-64-hex-host")
assert st == 400, st
st, _ = submit_pin("op_pin_missing", "b" * 64)
assert st == 404, st
st, j = submit_pin("op_pin_ok", "a" * 64)
assert st == 202 and is_uuid4(j.get("turn_id", "")) and is_uuid4(j.get("conversation_id", "")), (st, j)
row = subprocess.check_output([psql, pgurl, "-tAc", f"SELECT compute||'|'||source_device||'|'||workspace FROM chat.turns_v2 WHERE id='{j['turn_id']}'"]).decode().strip()
assert row == f"local|Mac Studio Lab|host:{'a'*64}", row

# 5. Tier 3 Cloud Sandbox submit on POST /api/chat/submit/v2alpha1?target=sandbox&size=small
def submit_qs(op_id, qs):
    hdrs = {"Content-Type": "application/json", "Cookie": f"__Host-oa_session={tok}", "Origin": api}
    payload = {"schema": "openagents.chat.submit.v2alpha1", "operation_id": op_id, "conversation_id": "", "expected_revision": 0, "project_id": None, "expected_project_revision": None, "legacy_thread_id": None, "legacy_thread_revision": None, "content": [{"type": "text", "text": "Run tests in cloud sandbox"}]}
    r = urllib.request.Request(f"{api}/api/chat/submit/v2alpha1?{qs}", data=json.dumps(payload).encode(), headers=hdrs, method="POST")
    try:
        with urllib.request.urlopen(r, timeout=10) as resp:
            return resp.status, json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()

st, _ = submit_qs("op_sb_bad_tgt", "target=bogus")
assert st == 400, st
st, _ = submit_qs("op_sb_bad_sz", "target=sandbox&size=huge")
assert st == 400, st
st, j = submit_qs("op_sb_ok", "target=sandbox&size=small")
assert st == 202 and is_uuid4(j.get("turn_id", "")) and is_uuid4(j.get("conversation_id", "")), (st, j)
srow = subprocess.check_output([psql, pgurl, "-tAc", f"SELECT compute||'|'||source_device||'|'||workspace FROM chat.turns_v2 WHERE id='{j['turn_id']}'"]).decode().strip()
assert srow == "sandbox|cloud-sandbox|sandbox:small", srow

# 6. Phase 5 (#14129): Metered Sandbox Compute Billing (402 at 0 sats, exact msat debit, $0 local) & Auto Placement
tok_user = make_tok(b"\x44" * 32)
h_user = hashlib.sha256(tok_user.encode()).hexdigest()
now_ms = int(__import__("time").time() * 1000)
subprocess.run([psql, pgurl, "-v", "ON_ERROR_STOP=1", "-c", f"""
INSERT INTO inference.btc_usd (one, usd_per_btc, at) VALUES (true, 100000, now())
  ON CONFLICT (one) DO UPDATE SET usd_per_btc = 100000, at = now();
INSERT INTO auth.accounts (id, kind, verse_account, team) VALUES ('acct_dddddddddddddddddddddddddd', 'person', 'v_paid', false);
INSERT INTO auth.sessions (token_sha256, id, account, via_kind, client, level, l1_at, epoch)
  VALUES ('{h_user}', 'ses_paid', 'acct_dddddddddddddddddddddddddd', 'workos', 'web', 'L1', now(), 0);
INSERT INTO chat.environment_connections_v1 (account, host_pubkey, display_name, grant_id, epoch, relay, last_seen_ms)
  VALUES ('acct_dddddddddddddddddddddddddd', '{"c"*64}', 'MacBook Pro M4', '{"2"*64}', 1, 'wss://relay.openagents.com', {now_ms});
"""], check=True, stdout=subprocess.DEVNULL)

def api_req(method, path, body=None, cookie=tok_user):
    hdrs = {}
    data = None
    if body is not None:
        hdrs["Content-Type"] = "application/json"
        data = json.dumps(body).encode()
    if cookie:
        hdrs["Cookie"] = f"__Host-oa_session={cookie}"
        hdrs["Origin"] = api
    r = urllib.request.Request(api + path, data=data, headers=hdrs, method=method)
    try:
        with urllib.request.urlopen(r, timeout=10) as resp:
            return resp.status, json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()

# 6a. GET /api/v1/inference/pricing exposes transparent retail sandbox_compute rates & $0 local
st, pr = api_req("GET", "/api/v1/inference/pricing", cookie=None)
assert st == 200 and pr["sandbox_compute"]["local_cost_usd_micros"] == 0, pr
rates = {s["name"]: s["usd_micros_hr"] for s in pr["sandbox_compute"]["sizes"]}
assert rates == {"small": 22500, "default": 45000, "large": 90000}, rates

# 6b. Unfunded non-team user (0 sats) gets 402 on ?target=sandbox
def submit_user(op_id, qs, text="Run build in sandbox"):
    payload = {"schema": "openagents.chat.submit.v2alpha1", "operation_id": op_id, "conversation_id": "", "expected_revision": 0, "project_id": None, "expected_project_revision": None, "legacy_thread_id": None, "legacy_thread_revision": None, "content": [{"type": "text", "text": text}]}
    return api_req("POST", f"/api/chat/submit/v2alpha1?{qs}", body=payload, cookie=tok_user)

st, err_402 = submit_user("op_u_unpaid", "target=sandbox&size=small")
assert st == 402, (st, err_402)

# 6c. Top up 10,000 msat -> sandbox submit succeeds (202) and metering debits exact 5,625 msat (leaving 4,375 msat)
subprocess.run([psql, pgurl, "-v", "ON_ERROR_STOP=1", "-c",
    "INSERT INTO inference.credit (pubkey, msat) VALUES ('acct_dddddddddddddddddddddddddd', 10000) ON CONFLICT (pubkey) DO UPDATE SET msat = 10000;"
], check=True, stdout=subprocess.DEVNULL)

st, sb_turn = submit_user("op_u_paid_sb", "target=sandbox&size=small")
assert st == 202 and is_uuid4(sb_turn.get("turn_id", "")), (st, sb_turn)
st, mres = api_req("POST", "/api/chat/sandbox/meter/v1", body={"turn_id": sb_turn["turn_id"], "size": "small", "seconds": 900})
assert st == 200 and mres["debit_msat"] == 5625 and mres["total_cost_msat"] == 5625 and mres["credit_msat"] == 4375, (st, mres)

# 6d. User's own enrolled computer run strictly costs 0 msat ($0 local compute)
st, loc_turn = submit_user("op_u_local", f"host={'c'*64}", text="Run coax test locally")
assert st == 202 and is_uuid4(loc_turn.get("turn_id", "")), (st, loc_turn)
st, lres = api_req("POST", "/api/chat/sandbox/meter/v1", body={"turn_id": loc_turn["turn_id"], "size": "large", "seconds": 3600})
assert st == 200 and lres["compute"] == "local" and lres["debit_msat"] == 0 and lres["total_cost_msat"] == 0 and lres["credit_msat"] == 4375, (st, lres)

# 6e. GET /api/v1/inference/usage reports sandbox compute seconds & msat alongside credit & local $0 turns
st, usg = api_req("GET", "/api/v1/inference/usage")
assert st == 200 and usg["credit_msat"] == 4375 and usg["sandbox"] == {"turns": 1, "seconds": 900, "msat": 5625} and usg["local"] == {"turns": 1, "msat": 0}, (st, usg)

# 6f. POST /api/chat/placement/v1 across read-only, online, offline, busy, keep-clear station fixtures
st, p_ro = api_req("POST", "/api/chat/placement/v1", body={"prompt": "Explain AGENTS.md"}, cookie=tok)
assert st == 200 and p_ro["tier"] == 1 and p_ro["compute"] == "cloud", p_ro
st, p_on = api_req("POST", "/api/chat/placement/v1", body={"prompt": "Run coax test", "station": {"online": True, "repo_matched": True, "keep_clear": False, "active_turns": 0, "max_turns": 2, "cpu_pct": 15, "free_ram_mb": 8192, "free_disk_mb": 20480}}, cookie=tok)
assert st == 200 and p_on["tier"] == 2 and p_on["compute"] == "local" and p_on["host_pubkey"] == "a" * 64, p_on
st, p_off = api_req("POST", "/api/chat/placement/v1", body={"prompt": "Run coax test", "station": {"online": False, "repo_matched": True}}, cookie=tok)
assert st == 200 and p_off["tier"] == 3 and p_off["compute"] == "sandbox" and "offline" in p_off["reason"].lower(), p_off
st, p_busy = api_req("POST", "/api/chat/placement/v1", body={"prompt": "Build project", "station": {"online": True, "repo_matched": True, "cpu_pct": 95}}, cookie=tok)
assert st == 200 and p_busy["tier"] == 3 and p_busy["compute"] == "sandbox" and "busy" in p_busy["reason"].lower(), p_busy
st, p_kc = api_req("POST", "/api/chat/placement/v1", body={"prompt": "Fix bug", "station": {"online": True, "repo_matched": True, "keep_clear": True}}, cookie=tok)
assert st == 200 and p_kc["tier"] == 3 and p_kc["compute"] == "sandbox" and "keep-clear" in p_kc["reason"].lower(), p_kc

# 6g. Live POST /api/chat/submit/v2alpha1?target=auto routes code turn to online enrolled computer ($0 local) and records placement_reason
st, auto_turn = submit_user("op_u_auto_local", "target=auto", text="Run coax test on this repo")
assert st == 202 and is_uuid4(auto_turn.get("turn_id", "")), (st, auto_turn)
arow = subprocess.check_output([psql, pgurl, "-tAc", f"SELECT compute||'|'||source_device||'|'||workspace||'|'||placement_reason FROM chat.turns_v2 WHERE id='{auto_turn['turn_id']}'"]).decode().strip()
assert arow.startswith(f"local|MacBook Pro M4|host:{'c'*64}|Matched enrolled computer"), arow

with open(api_log, "r", encoding="utf-8", errors="replace") as f:
    log = f.read()
assert s_gh not in log and s_exa not in log, "LEAK in api.log!"
print("cloud_tools_e2e_test: TIER 1 + TIER 2 + TIER 3 SANDBOX + PHASE 5 METERING (402/DEBIT/$0 LOCAL) + AUTO PLACEMENT PASSED (ASan/UBSan clean, zero token leaks)")
PY
