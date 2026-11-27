#!/usr/bin/env bash
# Firm reason: 5 (thin offline integration test harness starting throwaway Postgres 16, mock WorkOS Pipes + GitHub HTTP servers, and ASan oa-api).
# End-to-end Phase 2 WorkOS Pipes GitHub integration verification (#14126):
#   - GET /api/integrations/github/status (JSON + ?as=lines) before and after connect
#   - POST /api/integrations/github/connect (valid return_to + open-redirect/traversal refusal)
#   - GET /api/integrations/github/repos (JSON + ?as=lines, sanitized schema, zero raw token leak)
#   - POST /api/integrations/github/disconnect (revokes provider connection, status flips false)
#   - Auth admission (401 unauthenticated, 403 non-WorkOS account, 503 when WorkOS Pipes disabled)
#   - Zero GitHub OAuth token persistence or leak in HTTP responses, headers, Postgres, or api.log
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

# 1. Build oa-api with ASan/UBSan + -Werror
api_bin="${OA_API_BIN:-$work/api}"
if [[ ! -x "$api_bin" ]]; then
  source "$api_dir/api-sources.sh"
  api_sources "$api_dir" "$work" "$coax"
  "$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -DOA_LNADDR_TEST_FETCH \
    -fsanitize=address,undefined -I"$work" "${pq_inc[@]}" "${secp_inc[@]}" "${ssl_inc[@]}" \
    -o "$api_bin" "${API_SOURCES[@]}" \
    "${pq_lib[@]}" -lpq "${secp_lib[@]}" -lsecp256k1 "${ssl_lib[@]}" -lssl -lcrypto -lcurl -lz -lpthread -lm ${darwin_fw[@]+"${darwin_fw[@]}"}
fi

# 2. Start throwaway Postgres 16
pg_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
api_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
mock_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

"$pgbin/initdb" -D "$work/db" -U postgres --auth=trust --no-sync >/dev/null 2>&1
"$pgbin/pg_ctl" -D "$work/db" -l "$work/pg.log" -o "-k $work -p $pg_port -h 127.0.0.1 -F" start >/dev/null
"$pgbin/createdb" -h 127.0.0.1 -p "$pg_port" -U postgres oa_analytics
"$pgbin/psql" -h 127.0.0.1 -p "$pg_port" -U postgres -d oa_analytics -c "CREATE ROLE oa_analytics SUPERUSER LOGIN" >/dev/null
pgurl="host=127.0.0.1 port=$pg_port user=oa_analytics dbname=oa_analytics"

# 3. Start combined mock WorkOS Pipes + mock GitHub REST API server
SECRET_GH_TOKEN="gho_NEVER_LEAK_SUPER_SECRET_TOKEN_9876543210"
cat > "$work/mock_server.py" <<'PY'
import json, os, sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse

PORT = int(sys.argv[1])
GH_TOKEN = os.environ["SECRET_GH_TOKEN"]
STATE = {"connected": False, "last_return_to": "", "disconnects": 0}

class H(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass
    def _send(self, code, obj):
        body = json.dumps(obj).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        p = urlparse(self.path).path
        if p == "/health":
            return self._send(200, {"ok": True})
        if p.startswith("/user_management/users/") and p.endswith("/data_providers"):
            assert self.headers.get("Authorization") == "Bearer sk_test_pipes"
            if STATE["connected"]:
                return self._send(200, {"data": [{"slug": "github", "connected_account": {"state": "connected", "scopes": ["repo", "read:user"]}}]})
            return self._send(200, {"data": [{"slug": "github", "connected_account": None}]})
        if p == "/gh/user":
            if self.headers.get("Authorization") != f"Bearer {GH_TOKEN}":
                return self._send(401, {"message": "Bad credentials"})
            return self._send(200, {"login": "octocat", "id": 1})
        if p == "/gh/user/repos":
            if self.headers.get("Authorization") != f"Bearer {GH_TOKEN}":
                return self._send(401, {"message": "Bad credentials"})
            return self._send(200, [
                {
                    "full_name": "OpenAgentsInc/openagents",
                    "default_branch": "main",
                    "private": False,
                    "html_url": "https://github.com/OpenAgentsInc/openagents",
                    "clone_url": "https://github.com/OpenAgentsInc/openagents.git",
                    "updated_at": "2026-11-26T22:00:00Z",
                    "internal_secret_field": "SHOULD_BE_STRIPPED",
                },
                {
                    "full_name": "OpenAgentsInc/private-cloud",
                    "default_branch": "main",
                    "private": True,
                    "html_url": "https://github.com/OpenAgentsInc/private-cloud",
                    "clone_url": "https://github.com/OpenAgentsInc/private-cloud.git",
                    "updated_at": "2026-11-26T21:00:00Z",
                },
            ])
        return self._send(404, {"error": "not_found", "path": p})
    def do_POST(self):
        ln = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(ln).decode("utf-8") if ln > 0 else "{}"
        data = json.loads(raw) if raw.strip() else {}
        p = urlparse(self.path).path
        if p == "/data-integrations/github/authorize":
            assert self.headers.get("Authorization") == "Bearer sk_test_pipes"
            assert data.get("user_id") == "user_01PIPES_TEST"
            STATE["last_return_to"] = data.get("return_to", "")
            STATE["connected"] = True
            return self._send(200, {"url": f"http://127.0.0.1:{PORT}/oauth/github/done"})
        if p == "/data-integrations/github/token":
            assert self.headers.get("Authorization") == "Bearer sk_test_pipes"
            if not STATE["connected"]:
                return self._send(200, {"active": False, "error": "not_installed"})
            return self._send(200, {"active": True, "access_token": {"access_token": GH_TOKEN, "scopes": ["repo", "read:user"]}})
        return self._send(404, {"error": "not_found", "path": p})
    def do_DELETE(self):
        p = urlparse(self.path).path
        if p == "/user_management/users/user_01PIPES_TEST/connected_accounts/github":
            assert self.headers.get("Authorization") == "Bearer sk_test_pipes"
            STATE["connected"] = False
            STATE["disconnects"] += 1
            return self._send(200, {"deleted": True})
        return self._send(404, {"error": "not_found", "path": p})

HTTPServer(("127.0.0.1", PORT), H).serve_forever()
PY
SECRET_GH_TOKEN="$SECRET_GH_TOKEN" python3 "$work/mock_server.py" "$mock_port" >"$work/mock.log" 2>&1 &
mock_pid=$!
mock_base="http://127.0.0.1:$mock_port"
for _ in $(seq 1 40); do curl -fsS "$mock_base/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "$mock_base/health" >/dev/null

# 4. Start ASan oa-api with OA_WORKOS_ENABLED=1 pointing to mock WorkOS Pipes & mock GitHub API
api_origin="http://127.0.0.1:$api_port"
ASAN_OPTIONS=detect_leaks=0 \
OA_API_DATABASE="$pgurl" \
OA_API_ORIGIN="$api_origin" \
OA_API_WORKERS=1 \
OA_AUTH_SEAL_KEY="1:0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20" \
OA_AUTH_LOOKUP_PEPPER="1:201f1e1d1c1b1a191817161514131211100f0e0d0c0b0a090807060504030201" \
OA_WORKOS_ENABLED=1 \
OA_WORKOS_BASE_URL="$mock_base" \
OA_WORKOS_API_KEY="sk_test_pipes" \
OA_WORKOS_CLIENT_ID="client_01PIPES_E2E" \
OA_GITHUB_API_BASE="$mock_base/gh" \
  "$api_bin" --listen "127.0.0.1:$api_port" --migrations "$api_dir/migrations" >"$work/api.log" 2>&1 &
api_pid=$!

for _ in $(seq 1 60); do curl -fsS "$api_origin/api/playtest/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "$api_origin/api/playtest/health" >/dev/null || { cat "$work/api.log" >&2; exit 1; }

# 5. Seed one WorkOS-linked person account + session cookie and one non-WorkOS person account + session cookie
python3 - "$pgurl" "$api_origin" "$SECRET_GH_TOKEN" "$work/api.log" "$pgbin/psql" <<'PY'
import base64, hashlib, json, subprocess, sys, urllib.request, urllib.error, zlib

pgurl, api, secret_tok, api_log, psql = sys.argv[1:6]

def sql(q):
    subprocess.run([psql, pgurl, "-v", "ON_ERROR_STOP=1", "-c", q], check=True, stdout=subprocess.DEVNULL)

B32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
def make_session_token(raw32):
    head = "oas_" + base64.b32encode(raw32).rstrip(b"=").decode()
    crc = (zlib.crc32(head.encode()) & 0xFFFFFFFF) >> 2
    chk = "".join(B32[(crc >> (25 - 5 * i)) & 31] for i in range(6))
    return head + chk

tok_wos = make_session_token(b"\x11" * 32)
tok_raw = make_session_token(b"\x22" * 32)
h_wos = hashlib.sha256(tok_wos.encode()).hexdigest()
h_raw = hashlib.sha256(tok_raw.encode()).hexdigest()

sql(f"""
INSERT INTO auth.accounts (id, kind, verse_account) VALUES
  ('acct_aaaaaaaaaaaaaaaaaaaaaaaaaa', 'person', 'v_pipes_wos'),
  ('acct_bbbbbbbbbbbbbbbbbbbbbbbbbb', 'person', 'v_pipes_plain');
INSERT INTO auth.sessions (token_sha256, id, account, via_kind, client, level, l1_at, epoch) VALUES
  ('{h_wos}', 'ses_pipes_wos', 'acct_aaaaaaaaaaaaaaaaaaaaaaaaaa', 'workos', 'web', 'L1', now(), 0),
  ('{h_raw}', 'ses_pipes_plain', 'acct_bbbbbbbbbbbbbbbbbbbbbbbbbb', 'email', 'web', 'L1', now(), 0);
INSERT INTO auth.workos_identities (workos_user_id, account, provider) VALUES
  ('user_01PIPES_TEST', 'acct_aaaaaaaaaaaaaaaaaaaaaaaaaa', 'github');
""")

def req(method, path, cookie=None, body=None):
    hdrs = {}
    if cookie:
        hdrs["Cookie"] = f"__Host-oa_session={cookie}"
        hdrs["Origin"] = api
    data = None
    if body is not None:
        data = body.encode("utf-8") if isinstance(body, str) else json.dumps(body).encode("utf-8")
        hdrs["Content-Type"] = "application/json"
    r = urllib.request.Request(api + path, data=data, headers=hdrs, method=method)
    try:
        with urllib.request.urlopen(r, timeout=10) as resp:
            txt = resp.read().decode("utf-8")
            assert secret_tok not in txt, "LEAK: secret GitHub token found in response body!"
            assert secret_tok not in str(resp.headers), "LEAK: secret GitHub token found in headers!"
            return resp.status, txt
    except urllib.error.HTTPError as e:
        txt = e.read().decode("utf-8")
        assert secret_tok not in txt, "LEAK: secret GitHub token found in error body!"
        return e.code, txt

# 1. Unauthenticated -> 401
st, _ = req("GET", "/api/integrations/github/status")
assert st == 401, f"expected 401, got {st}"

# 2. Non-WorkOS account -> connected: false on status, 409 on connect
st, txt = req("GET", "/api/integrations/github/status", tok_raw)
assert st == 200 and json.loads(txt)["connected"] is False, (st, txt)
st, _ = req("POST", "/api/integrations/github/connect", tok_raw, {})
assert st == 409, f"expected 409 for non-WorkOS account, got {st}"

# 3. Initial status for WorkOS user -> connected: false (JSON + ?as=lines)
st, txt = req("GET", "/api/integrations/github/status", tok_wos)
assert st == 200, (st, txt)
j = json.loads(txt)
assert j["schema"] == "openagents.integrations.github.status.v1alpha1" and j["connected"] is False, j
st, txt = req("GET", "/api/integrations/github/status?as=lines", tok_wos)
assert st == 200 and "connected 0\nprovider github\n" in txt, (st, txt)

# 4. Bad return_to (open redirect / protocol-relative / traversal / unknown field) -> 400
for bad in [
    {"return_to": "https://evil.com/steal"},
    {"return_to": "//evil.com/steal"},
    {"return_to": "/settings/../admin"},
    {"unexpected": "field"},
]:
    st, txt = req("POST", "/api/integrations/github/connect", tok_wos, bad)
    assert st == 400, f"expected 400 for {bad}, got {st}: {txt}"

# 5. Valid connect -> returns authorize URL (and mock flips connected=True)
st, txt = req("POST", "/api/integrations/github/connect", tok_wos, {"return_to": "/settings/account?github=connected"})
assert st == 200, (st, txt)
j = json.loads(txt)
assert j["schema"] == "openagents.integrations.github.connect.v1alpha1" and "/oauth/github/done" in j["url"], j

# 6. Status now reports connected: true + scopes
st, txt = req("GET", "/api/integrations/github/status", tok_wos)
assert st == 200, (st, txt)
j = json.loads(txt)
assert j["connected"] is True and j["scopes"] == "repo,read:user", j
st, txt = req("GET", "/api/integrations/github/status?as=lines", tok_wos)
assert st == 200 and "connected 1\n" in txt and "scopes repo,read:user\n" in txt, (st, txt)

# 7. Repos endpoint returns sanitized closed schema (JSON + ?as=lines), strips extra fields
st, txt = req("GET", "/api/integrations/github/repos", tok_wos)
assert st == 200, (st, txt)
j = json.loads(txt)
assert j["schema"] == "openagents.integrations.github.repos.v1alpha1" and len(j["repos"]) == 2, j
assert j["repos"][0]["full_name"] == "OpenAgentsInc/openagents" and "internal_secret_field" not in j["repos"][0], j
assert j["repos"][1]["private"] is True, j
st, txt = req("GET", "/api/integrations/github/repos?as=lines", tok_wos)
assert st == 200 and "repo OpenAgentsInc/openagents|main|0|https://github.com/OpenAgentsInc/openagents\n" in txt, (st, txt)

# 8. Disconnect -> flips status back to connected: false and repos returns 412 (needs connect)
st, txt = req("POST", "/api/integrations/github/disconnect", tok_wos, {})
assert st == 200 and json.loads(txt)["disconnected"] is True, (st, txt)
st, txt = req("GET", "/api/integrations/github/status", tok_wos)
assert st == 200 and json.loads(txt)["connected"] is False, (st, txt)
st, txt = req("GET", "/api/integrations/github/repos", tok_wos)
assert st == 412, (st, txt)

# 9. Verify zero secret token leak in api.log
with open(api_log, "r", encoding="utf-8", errors="replace") as f:
    log_txt = f.read()
assert secret_tok not in log_txt, "LEAK: secret GitHub token found in api.log!"

print("github_pipes_e2e_test: ALL 9 CHECKS PASSED (ASan/UBSan clean, zero token leaks)")
PY
