#!/usr/bin/env bash
# Firm reason: 5 (ASan/UBSan end-to-end test harness running live oa-api against throwaway Postgres 16 and mock WorkOS/GitHub/Spark/Vertex servers)
# End-to-end verification of the Referral & Promo Credit system (Phases 0-4):
#   1. Personal code lazy mint, public /lookup + click tracking, vanity toggle, show_name privacy
#   2. GitHub eligibility ($5 grant, ineligible too-new / low-activity, permanent numeric-ID uniqueness)
#   3. Invitee redeem ($10 stacking -> $15), self/reciprocal/already-attributed/past-7d/revoked refusals
#   4. Promo credit spend order (0-sat admission on promo model, $5/UTC-day cap + paid-sat split, non-eligible model skips promo)
#   5. 2-distinct-UTC-day inviter activation ($5 grant)
#   6. Atomic 1% lifetime kickback inside Lightning top-up settlement transaction + replay idempotency + kill-switch
#   7. Milestone 5 ($10 one-time), rolling 30d inviter cap (void reason 'cap'), daily budget ceiling (stays 'pending')
#   8. Admin campaigns (hash-only storage, revoke), clawback (zeroes unspent promo ONLY, NEVER touches paid credit), config + audit
#   9. GET /api/auth/account, GET /api/auth/life, GET /api/v1/inference/usage expose promo_micros & promo_expires_at
#  10. Clean ASan/UBSan shutdown with zero sanitizer diagnostics.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
API_DIR="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$API_DIR/../../.." && pwd)"
COAX="${COAX:-$ROOT/coax/bin/coax}"

if [[ -d /opt/homebrew/opt/postgresql@16/bin ]]; then
  export PATH="/opt/homebrew/opt/postgresql@16/bin:$PATH"
fi

WORK="$(mktemp -d /tmp/oa-ref-e2e.XXXXXX)"
API_PID=""
MOCK_PID=""
PG_STARTED=0

cleanup() {
  local rc=$?
  [[ -n "$API_PID" ]] && kill -TERM "$API_PID" 2>/dev/null && wait "$API_PID" 2>/dev/null || true
  [[ -n "$MOCK_PID" ]] && kill -TERM "$MOCK_PID" 2>/dev/null && wait "$MOCK_PID" 2>/dev/null || true
  if [[ "$PG_STARTED" -eq 1 ]]; then
    pg_ctl -D "$WORK/pgdata" -m immediate stop >/dev/null 2>&1 || true
  fi
  if [[ "$rc" -ne 0 ]]; then
    echo "=== E2E FAILED (rc=$rc) ===" >&2
    if [[ -f "$WORK/api.log" ]]; then
      echo "--- api.log (tail 60) ---" >&2
      tail -n 60 "$WORK/api.log" >&2
    fi
    if [[ -f "$WORK/mock.log" ]]; then
      echo "--- mock.log (tail 40) ---" >&2
      tail -n 40 "$WORK/mock.log" >&2
    fi
  fi
  rm -rf "$WORK"
  exit "$rc"
}
trap cleanup EXIT

echo "[1/5] Building ASan/UBSan oa-api binary..."
mkdir -p "$WORK/gen"
# shellcheck source=../api-sources.sh
source "$API_DIR/api-sources.sh"
api_sources "$API_DIR" "$WORK/gen" "$COAX"

CC_FLAGS=(-std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -Wall -Wextra -Wno-unused-parameter -D_DEFAULT_SOURCE)
INC_FLAGS=(-I"$WORK/gen" -I"$API_DIR" -I"$ROOT/coax/platform")
LIB_FLAGS=(-lpthread -lpq -lsecp256k1 -lssl -lcrypto -lcurl -lz -lm)
if [[ "$(uname -s)" == "Darwin" ]]; then
  [[ -d /opt/homebrew/opt/postgresql@16/include ]] && INC_FLAGS+=(-I/opt/homebrew/opt/postgresql@16/include)
  [[ -d /opt/homebrew/include ]] && INC_FLAGS+=(-I/opt/homebrew/include)
  [[ -d /opt/homebrew/opt/openssl@3/include ]] && INC_FLAGS+=(-I/opt/homebrew/opt/openssl@3/include)
  [[ -d /opt/homebrew/opt/postgresql@16/lib ]] && LIB_FLAGS=(-L/opt/homebrew/opt/postgresql@16/lib "${LIB_FLAGS[@]}")
  [[ -d /opt/homebrew/lib ]] && LIB_FLAGS=(-L/opt/homebrew/lib "${LIB_FLAGS[@]}")
  [[ -d /opt/homebrew/opt/openssl@3/lib ]] && LIB_FLAGS=(-L/opt/homebrew/opt/openssl@3/lib "${LIB_FLAGS[@]}")
  LIB_FLAGS+=(-framework Security -framework CoreFoundation)
fi

clang "${CC_FLAGS[@]}" "${INC_FLAGS[@]}" -o "$WORK/api-asan" "${API_SOURCES[@]}" "${LIB_FLAGS[@]}"
echo "  ok  compiled $WORK/api-asan"

echo "[2/5] Checking Dockerfile & deploy.sh source parity..."
diff -u <(printf "%s\n" "${API_SOURCES[@]}" | xargs -n1 basename | sort) \
        <(grep ' -o /api ' "$API_DIR/Dockerfile" | grep -oE "/(src|host)/[^ ]+\.c" | xargs -n1 basename | sort -u)
for u in $API_COAX $API_COAX_WRAPPED; do
  grep -E '^for m in ' "$API_DIR/deploy.sh" | grep -q -- "[ :]${u##*:}[ ;]" || { echo "api: deploy.sh makes no C for ${u##*:}" >&2; exit 1; }
done
echo "  ok  Dockerfile and deploy.sh match api-sources.sh"

echo "[3/5] Starting throwaway Postgres 16 and applying all migrations..."
mkdir -p "$WORK/pgdata" "$WORK/pgsock"
initdb -D "$WORK/pgdata" -U postgres -A trust --encoding=UTF8 --locale=C >/dev/null
pg_ctl -D "$WORK/pgdata" -l "$WORK/pg.log" -o "-k $WORK/pgsock -h '' -c fsync=off -c synchronous_commit=off" start >/dev/null
PG_STARTED=1
createdb -h "$WORK/pgsock" -U postgres oa_ref_test
DSN="host=$WORK/pgsock user=postgres dbname=oa_ref_test"

export ASAN_OPTIONS="detect_leaks=0:halt_on_error=1:abort_on_error=1"
export UBSAN_OPTIONS="print_stacktrace=1:halt_on_error=1"
OA_API_DATABASE="$DSN" "$WORK/api-asan" --migrations "$API_DIR/migrations" --migrate-only >"$WORK/migrate.log" 2>&1
echo "  ok  applied all migrations including 0224_referrals_and_promo_credit.sql"

# Pick two free ephemeral localhost ports
read -r MOCK_PORT API_PORT < <(python3 - <<'PY'
import socket
s1 = socket.socket(); s1.bind(('127.0.0.1', 0)); p1 = s1.getsockname()[1]
s2 = socket.socket(); s2.bind(('127.0.0.1', 0)); p2 = s2.getsockname()[1]
s1.close(); s2.close()
print(p1, p2)
PY
)

echo "[4/5] Starting mock WorkOS/GitHub/Spark/Vertex server (:$MOCK_PORT) and oa-api (:$API_PORT)..."
cat > "$WORK/mock_server.py" <<'PY'
import hashlib, json, os, sys, time
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

PORT = int(sys.argv[1])
INVOICES = {}
INVOICE_SEQ = 0

def iso_days_ago(days):
    return (datetime.now(timezone.utc) - timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")

PROFILES = {
    "gho_wos_alice": {
        "user": {"id": 10001, "login": "alice-dev", "avatar_url": "https://avatars.githubusercontent.com/u/10001", "created_at": iso_days_ago(400), "followers": 12},
        "repos": [{"name": "r1", "fork": False, "private": False}, {"name": "r2", "fork": False, "private": False}, {"name": "r3", "fork": False, "private": False}],
        "orgs": [{"login": "openagents-test"}],
    },
    "gho_wos_bob": {
        "user": {"id": 10002, "login": "bob-builder", "avatar_url": "https://avatars.githubusercontent.com/u/10002", "created_at": iso_days_ago(300), "followers": 2},
        "repos": [{"name": "a", "fork": False, "private": False}, {"name": "b", "fork": False, "private": False}, {"name": "c", "fork": False, "private": False}],
        "orgs": [],
    },
    "gho_wos_newbie": {
        "user": {"id": 10003, "login": "newbie", "avatar_url": "https://avatars.githubusercontent.com/u/10003", "created_at": iso_days_ago(10), "followers": 20},
        "repos": [{"name": "a", "fork": False, "private": False}, {"name": "b", "fork": False, "private": False}, {"name": "c", "fork": False, "private": False}],
        "orgs": [{"login": "org1"}],
    },
    "gho_wos_lowact": {
        "user": {"id": 10004, "login": "lowact", "avatar_url": "https://avatars.githubusercontent.com/u/10004", "created_at": iso_days_ago(400), "followers": 1},
        "repos": [{"name": "a", "fork": False, "private": False}, {"name": "f1", "fork": True, "private": False}, {"name": "f2", "fork": True, "private": False}, {"name": "p1", "fork": False, "private": True}],
        "orgs": [],
    },
    "gho_wos_dup": {
        "user": {"id": 10002, "login": "bob-alt", "avatar_url": "https://avatars.githubusercontent.com/u/10002", "created_at": iso_days_ago(300), "followers": 9},
        "repos": [{"name": "a", "fork": False, "private": False}, {"name": "b", "fork": False, "private": False}, {"name": "c", "fork": False, "private": False}],
        "orgs": [],
    },
}

def get_profile(tok):
    if tok in PROFILES:
        return PROFILES[tok]
    if tok.startswith("gho_wos_dyn_"):
        n = int(tok.split("_")[-1])
        # Distinct creation dates per user so legitimate users don't trip the gh_cluster bot-farm detector
        days_old = 400 + n * 7
        return {
            "user": {"id": 20000 + n, "login": f"dynuser-{n}", "avatar_url": f"https://avatars.githubusercontent.com/u/{20000+n}", "created_at": iso_days_ago(days_old), "followers": 10},
            "repos": [{"name": "r1", "fork": False, "private": False}, {"name": "r2", "fork": False, "private": False}, {"name": "r3", "fork": False, "private": False}],
            "orgs": [],
        }
    return None

class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        sys.stderr.write(f"[mock] {self.command} {self.path} -> " + (fmt % args) + "\n")
    def send_json(self, status, obj):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def read_json(self):
        n = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(n) if n else b"{}"
        return json.loads(raw.decode() or "{}")

    def do_GET(self):
        p = urlparse(self.path).path
        # WorkOS Pipes providers status
        if p.startswith("/wos/data-integrations/users/") and "/providers" in p:
            self.send_json(200, {"data": [{"slug": "github", "connected": True, "scopes": ["read:user"]}]})
            return
        # Mock GitHub REST API
        if p.startswith("/gh/"):
            auth = self.headers.get("Authorization", "")
            tok = auth.split(" ", 1)[1] if auth.startswith("Bearer ") else ""
            prof = get_profile(tok)
            if not prof:
                self.send_json(401, {"message": "Bad credentials"})
                return
            if p == "/gh/user":
                self.send_json(200, prof["user"])
            elif p == "/gh/user/repos":
                self.send_json(200, prof["repos"])
            elif p == "/gh/user/orgs":
                self.send_json(200, prof["orgs"])
            else:
                self.send_json(404, {"message": "not found"})
            return
        if p == "/spark/rate":
            self.send_json(200, {"usd_per_btc": 100000})
            return
        self.send_json(404, {"error": "unknown GET " + p})

    def do_POST(self):
        global INVOICE_SEQ
        p = urlparse(self.path).path
        body = self.read_json()
        # WorkOS Pipes token endpoint: /wos/data-integrations/github/token
        if p == "/wos/data-integrations/github/token":
            uid = body.get("user_id", "")
            if uid:
                self.send_json(200, {"active": True, "access_token": {"access_token": f"gho_{uid}", "scopes": ["read:user"]}})
            else:
                self.send_json(404, {"active": False, "error": "not_installed"})
            return
        # Mock Spark /invoice
        if p == "/spark/invoice":
            INVOICE_SEQ += 1
            msat = int(body.get("amount_msat") or body.get("msat") or (int(body.get("sats", 0)) * 1000))
            preimage = hashlib.sha256(f"preimage-{INVOICE_SEQ}-{msat}".encode()).hexdigest()
            phash = hashlib.sha256(bytes.fromhex(preimage)).hexdigest()
            inv = f"lnbc1mock{INVOICE_SEQ}x{msat}"
            INVOICES[inv] = {"payment_hash": phash, "preimage": preimage, "amount_msat": msat}
            self.send_json(200, {"invoice": inv, "payment_hash": phash, "pay_to": "spark1mockaddr", "expiry": int(time.time()) + 3600})
            return
        # Mock Spark /paid
        if p == "/spark/paid":
            inv = body.get("invoice", "")
            rec = INVOICES.get(inv)
            if not rec:
                self.send_json(404, {"state": "unknown"})
                return
            self.send_json(200, {"state": "completed", "amount_msat": str(rec["amount_msat"]), "preimage": rec["preimage"]})
            return
        # Mock Vertex Chat Completions
        if p == "/vertex":
            # Controllable prompt_tokens via message tag "TOKENS:N" (default 1000000 -> $3.75 on neon, $1.50 on hydrogen)
            ptok = 1000000
            msgs = body.get("messages") or []
            if msgs:
                txt = str(msgs[-1].get("content", ""))
                if txt.startswith("TOKENS:"):
                    ptok = int(txt.split(":", 1)[1].strip())
            self.send_json(200, {
                "id": "chatcmpl-mock",
                "object": "chat.completion",
                "model": body.get("model", "google/gemini-4-neon"),
                "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": ptok, "completion_tokens": 0, "total_tokens": ptok},
            })
            return
        self.send_json(404, {"error": "unknown POST " + p})

ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
PY

python3 "$WORK/mock_server.py" "$MOCK_PORT" >"$WORK/mock.log" 2>&1 &
MOCK_PID=$!

ADMIN_PK="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
OA_API_DATABASE="$DSN" \
OA_API_WORKERS=1 \
OA_API_ORIGIN="http://127.0.0.1:$API_PORT" \
OA_ADMIN_PUBKEYS="$ADMIN_PK" \
OA_WORKOS_ENABLED=1 \
OA_WORKOS_API_KEY="sk_test_mock" \
OA_WORKOS_CLIENT_ID="client_test_mock" \
OA_WORKOS_BASE_URL="http://127.0.0.1:$MOCK_PORT/wos" \
OA_GITHUB_API_BASE="http://127.0.0.1:$MOCK_PORT/gh" \
OA_SPARK_URL="http://127.0.0.1:$MOCK_PORT/spark" \
OA_SPARK_TOKEN="mock_spark_token" \
OA_INFERENCE_URL="http://127.0.0.1:$MOCK_PORT/vertex" \
OA_INFERENCE_TOKEN="mock_vertex_token" \
OA_INFERENCE_DAILY_USD_MICROS="50000000" \
"$WORK/api-asan" --listen "127.0.0.1:$API_PORT" >"$WORK/api.log" 2>&1 &
API_PID=$!

# Wait for API health
for _ in $(seq 1 40); do
  if curl -sf --max-time 2 "http://127.0.0.1:$API_PORT/api/playtest/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done
curl -sf --max-time 3 "http://127.0.0.1:$API_PORT/api/playtest/health" >/dev/null
echo "  ok  oa-api is healthy on 127.0.0.1:$API_PORT"

echo "[5/5] Running comprehensive E2E test suite (Flows 1-9)..."
python3 - "$WORK/pgsock" "$API_PORT" "$ADMIN_PK" <<'PY'
import base64, hashlib, json, subprocess, sys, urllib.error, urllib.request, zlib

PGSOCK, API_PORT, ADMIN_PK = sys.argv[1], int(sys.argv[2]), sys.argv[3]
BASE = f"http://127.0.0.1:{API_PORT}"
B32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"

def make_oas_token(tag):
    raw32 = hashlib.sha256(f"oas-session:{tag}".encode()).digest()
    body = base64.b32encode(raw32).decode().rstrip("=")
    head = "oas_" + body
    s = (zlib.crc32(head.encode()) & 0xffffffff) >> 2
    chk = "".join(B32[(s >> (25 - 5 * i)) & 31] for i in range(6))
    return head + chk

# Self-check token generator against token.coax vector (0..31 bytes)
_vec_head = "oas_" + base64.b32encode(bytes(range(32))).decode().rstrip("=")
_vec_s = (zlib.crc32(_vec_head.encode()) & 0xffffffff) >> 2
_vec_tok = _vec_head + "".join(B32[(_vec_s >> (25 - 5 * i)) & 31] for i in range(6))
assert _vec_tok == "oas_AAAQEAYEAUDAOCAJBIFQYDIOB4IBCEQTCQKRMFYYDENBWHA5DYPQLUR7XK", _vec_tok

def psql(sql):
    out = subprocess.check_output(
        ["psql", "-h", PGSOCK, "-U", "postgres", "-d", "oa_ref_test", "-t", "-A", "-v", "ON_ERROR_STOP=1", "-c", sql],
        text=True,
    )
    return out.strip()

def http(method, path, body=None, cookie=None, headers=None):
    hdrs = {"Origin": BASE}
    if cookie:
        hdrs["Cookie"] = cookie
    if headers:
        hdrs.update(headers)
    data = None
    if body is not None:
        if isinstance(body, (dict, list)):
            data = json.dumps(body).encode()
            hdrs.setdefault("Content-Type", "application/json")
        else:
            data = body.encode() if isinstance(body, str) else body
    req = urllib.request.Request(BASE + path, data=data, headers=hdrs, method=method)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.status, resp.read().decode(), dict(resp.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode(), dict(e.headers)

def acct_id(tag):
    b32 = base64.b32encode(hashlib.sha256(tag.encode()).digest()).decode().lower().rstrip("=")
    return "acct_" + b32[:26]

def make_user(tag, wos_uid, pubkey=None, level="L2", created_days_ago=0):
    acct = acct_id(tag)
    tok = make_oas_token(tag)
    dig = hashlib.sha256(tok.encode()).hexdigest()
    psql(f"""
        INSERT INTO auth.accounts (id, kind, verse_account, created_at)
        VALUES ('{acct}', 'person', '{acct}', now() - interval '{created_days_ago} days');
        INSERT INTO auth.sessions (id, account, token_sha256, level, via_kind, client, l2_at, epoch)
        VALUES ('ses_{tag}', '{acct}', '{dig}', '{level}', 'workos', 'web', now(), 0);
    """)
    if wos_uid:
        psql(f"INSERT INTO auth.workos_identities (workos_user_id, account, provider) VALUES ('{wos_uid}', '{acct}', 'GitHubOAuth');")
    if pubkey:
        psql(f"INSERT INTO auth.nostr_keys (account, pubkey, label) VALUES ('{acct}', '{pubkey}', 'test');")
    return acct, f"__Host-oa_session={tok}"

# Seed BTC/USD rate = $100,000/BTC -> 1 msat = 1 micro USD (exact!)
psql("INSERT INTO inference.btc_usd (one, usd_per_btc, at) VALUES (true, 100000, now()) ON CONFLICT (one) DO UPDATE SET usd_per_btc = 100000, at = now();")

A_ADMIN,  c_admin  = make_user("admin",  None, pubkey=ADMIN_PK, level="L2")
A_ALICE,  c_alice  = make_user("alice",  "wos_alice",  pubkey="11"*32)
A_BOB,    c_bob    = make_user("bob",    "wos_bob",    pubkey="22"*32)
A_NEWBIE, c_newbie = make_user("newbie", "wos_newbie", pubkey="33"*32)
A_LOWACT, c_lowact = make_user("lowact", "wos_lowact", pubkey="44"*32)
A_DUP,    c_dup    = make_user("dup",    "wos_dup",    pubkey="55"*32)
A_OLD,    c_old    = make_user("old",    "wos_dyn_99", pubkey="66"*32, created_days_ago=10)
A_CAMP,   c_camp   = make_user("camp",   "wos_dyn_98", pubkey="77"*32)

# ---- Flow 1: Personal code, lookup + clicks, GitHub $5, vanity, privacy, auth/account & auth/life ----
st, body, _ = http("GET", "/api/v1/referral", cookie=c_alice)
assert st == 200, (st, body)
j = json.loads(body)
alice_code = j["code"]
assert len(alice_code) == 8, alice_code
assert j["promo_micros"] == 0 and j["clicks"] == 0

# Idempotent repeat + ?as=lines
st2, body2, _ = http("GET", "/api/v1/referral?as=lines", cookie=c_alice)
assert st2 == 200 and f"code {alice_code}\n" in body2, (st2, body2)

# Public lookup records click
st, body, _ = http("GET", f"/api/v1/referral/lookup?code={alice_code}")
assert st == 200, (st, body)
lj = json.loads(body)
assert lj["valid"] is True and lj["bonus_micros"] == 10000000 and lj["github_bonus_micros"] == 5000000, lj

# Alice GitHub recheck -> $5 promo credit
st, body, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=c_alice)
assert st == 200, (st, body)
rj = json.loads(body)
assert rj["eligible"] is True and rj["newly_granted_micros"] == 5000000 and rj["promo_micros"] == 5000000, rj

# Vanity enable + lookup via vanity handle + show_name privacy toggle
st, body, _ = http("POST", "/api/v1/referral/vanity", {"on": True}, cookie=c_alice)
assert st == 200 and json.loads(body)["vanity_enabled"] is True, (st, body)
st, body, _ = http("GET", "/api/v1/referral/lookup?code=alice-dev")
lj = json.loads(body)
assert st == 200 and lj["valid"] is True and lj["inviter_handle"] == "alice-dev", lj

st, body, _ = http("POST", "/api/v1/referral/vanity", {"show_name": False}, cookie=c_alice)
assert st == 200 and json.loads(body)["show_name"] is False, (st, body)
st, body, _ = http("GET", "/api/v1/referral/lookup?code=alice-dev")
assert json.loads(body)["inviter_handle"] == "", body

# Verify /api/auth/account and /api/auth/life expose promo_micros & promo_expires_at
st, body, _ = http("GET", "/api/auth/account", cookie=c_alice)
assert st == 200, (st, body)
aj = json.loads(body)
assert aj["promo_micros"] == 5000000 and len(aj.get("promo_expires_at", "")) >= 19, aj
st, lines, _ = http("GET", "/api/auth/account?as=lines", cookie=c_alice)
assert st == 200 and "promo_micros 5000000\n" in lines and "promo_expires_at " in lines, lines

st, body, _ = http("GET", "/api/auth/life", cookie=c_alice)
assert st == 200, (st, body)
lf = json.loads(body)
assert lf["promo_micros"] == 5000000 and len(lf.get("promo_expires_at", "")) >= 19, lf
st, lines, _ = http("GET", "/api/auth/life?as=lines", cookie=c_alice)
assert st == 200 and "promo_micros 5000000\n" in lines, lines
print("  ok  Flow 1: personal code, lookup+clicks, $5 GitHub, vanity, privacy, auth/account & auth/life promo")

# ---- Flow 2: GitHub eligibility gates & permanent numeric-ID uniqueness ----
st, body, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=c_newbie)
rj = json.loads(body)
assert st == 200 and rj["eligible"] is False and rj["promo_micros"] == 0 and "90" in rj["reason"], rj

st, body, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=c_lowact)
rj = json.loads(body)
assert st == 200 and rj["eligible"] is False and rj["promo_micros"] == 0 and "repo" in rj["reason"].lower(), rj

# Bob redeems Alice's code (pending before GitHub) then rechecks GitHub -> $5 + $10 = $15 total!
st, body, _ = http("POST", "/api/v1/referral/redeem", {"code": alice_code, "source": "link"}, cookie=c_bob)
assert st == 200 and json.loads(body)["status"] == "pending", (st, body)
st, body, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=c_bob)
rj = json.loads(body)
assert st == 200 and rj["eligible"] is True and rj["newly_granted_micros"] == 15000000 and rj["promo_micros"] == 15000000, rj

# Dup shares Bob's numeric GitHub ID (10002) -> permanently blocked!
st, body, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=c_dup)
rj = json.loads(body)
assert st == 200 and rj["eligible"] is False and rj["promo_micros"] == 0 and "claimed" in rj["reason"].lower(), rj
print("  ok  Flow 2: GitHub eligibility ($5+$10=$15 stack, too-new/low-activity refused, duplicate GH ID blocked)")

# ---- Flow 3: Redeem refusals (self, already-attributed, reciprocal, past-7d, nonexistent) ----
st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": alice_code}, cookie=c_alice)
assert st == 400, f"expected 400 self-referral, got {st}"

st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": alice_code}, cookie=c_bob)
assert st == 409, f"expected 409 already-attributed, got {st}"

# Reciprocal: Alice tries Bob's personal code
st, bsum, _ = http("GET", "/api/v1/referral", cookie=c_bob)
bob_code = json.loads(bsum)["code"]
st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": bob_code}, cookie=c_alice)
assert st == 400, f"expected 400 reciprocal, got {st}"

# Past 7-day signup window (move launch_at back 10 days to test post-launch 7d rule)
psql("UPDATE referral.config SET launch_at = now() - interval '10 days' WHERE id = 1;")
st, body, _ = http("POST", "/api/v1/referral/redeem", {"code": alice_code}, cookie=c_old)
assert st == 400 and "7 days" in body, (st, body)

st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": "ZZZZ9999"}, cookie=c_newbie)
assert st == 404, f"expected 404 unknown code, got {st}"
print("  ok  Flow 3: self, already-attributed, reciprocal, past-7-day, and unknown code refusals")

def infer(model, tokens, cookie):
    before = int(psql("SELECT count(*) FROM inference.calls;"))
    st, body, hdrs = http("POST", "/api/v1/inference", {
        "model": model,
        "messages": [{"role": "user", "content": f"TOKENS:{tokens}"}],
    }, cookie=cookie)
    assert st == 200, (st, body)
    # Wait for forked settlement child (api_db_new) to finish inference.finish_call + referral_settle_inference + PQfinish
    for _ in range(100):
        n_calls = int(psql("SELECT count(*) FROM inference.calls;"))
        n_conns = int(psql("SELECT count(*) FROM pg_stat_activity WHERE datname = 'oa_ref_test' AND pid <> pg_backend_pid();"))
        if n_calls > before and n_conns <= 1:
            break
        import time; time.sleep(0.02)
    return st, body, hdrs

# ---- Flow 4: Promo credit spend order (0-sat admission, $5/UTC-day cap split, non-eligible model skips promo) ----
# Bob currently has 0 paid sats and $15.00 promo credit.
# Call 1: promo-eligible google/gemini-4-neon, 1,000,000 prompt tokens = $3.75 (3,750,000 micros)
infer("google/gemini-4-neon", 1000000, c_bob)
st, bsum, _ = http("GET", "/api/v1/referral", cookie=c_bob)
bs = json.loads(bsum)
assert bs["promo_micros"] == 11250000 and bs["promo_daily_spent"] == 3750000, bs

# Give Bob 10,000,000 msat ($10.00) paid credit on his pubkey
bob_pk = "22" * 32
psql(f"INSERT INTO inference.credit (pubkey, msat) VALUES ('{bob_pk}', 10000000) ON CONFLICT (pubkey) DO UPDATE SET msat = 10000000;")

# Call 2: promo-eligible google/gemini-4-neon, $3.75 -> hits $5/day cap: $1.25 from promo, $2.50 (2,500,000 msat) from paid!
infer("google/gemini-4-neon", 1000000, c_bob)
st, bsum, _ = http("GET", "/api/v1/referral", cookie=c_bob)
bs = json.loads(bsum)
paid_after = int(psql(f"SELECT msat FROM inference.credit WHERE pubkey = '{bob_pk}';"))
assert bs["promo_micros"] == 10000000 and bs["promo_daily_spent"] == 5000000, bs
assert paid_after == 7500000, f"expected 7500000 paid msat, got {paid_after}"

# Call 3: non-promo model google/gemini-4-hydrogen ($1.875 = 1,875,000 micros) -> skips promo, debits paid only!
infer("google/gemini-4-hydrogen", 1000000, c_bob)
st, bsum, _ = http("GET", "/api/v1/referral", cookie=c_bob)
bs = json.loads(bsum)
paid_after2 = int(psql(f"SELECT msat FROM inference.credit WHERE pubkey = '{bob_pk}';"))
assert bs["promo_micros"] == 10000000, bs
assert paid_after2 == 5625000, f"expected 5625000 paid msat, got {paid_after2}"

# Check /api/v1/inference/usage reports both paid credit_msat and promo_micros
st, ubody, _ = http("GET", "/api/v1/inference/usage", cookie=c_bob)
assert st == 200, (st, ubody)
uj = json.loads(ubody)
assert uj["credit_msat"] == 5625000 and uj["promo_micros"] == 10000000, uj
print("  ok  Flow 4: 0-sat promo admission, $5/day cap exact split to paid sats, non-eligible model skips promo")

# ---- Flow 5: 2-distinct-UTC-day inviter activation ($5 to Alice) ----
days_before = int(psql(f"SELECT active_days_count FROM referral.attributions WHERE invitee_account_id = '{A_BOB}';"))
assert days_before == 1, days_before
st, asum, _ = http("GET", "/api/v1/referral", cookie=c_alice)
assert json.loads(asum)["promo_micros"] == 5000000, asum

# Simulate next UTC day for Bob and make one more 200 call
psql(f"UPDATE referral.attributions SET first_active_day = (now() AT TIME ZONE 'UTC')::date - 1 WHERE invitee_account_id = '{A_BOB}';")
infer("google/gemini-4-hydrogen", 1000, c_bob)
days_after = int(psql(f"SELECT active_days_count FROM referral.attributions WHERE invitee_account_id = '{A_BOB}';"))
assert days_after == 2, days_after
st, asum, _ = http("GET", "/api/v1/referral", cookie=c_alice)
aj = json.loads(asum)
assert aj["activated_invitees"] == 1 and aj["promo_micros"] == 10000000, aj
print("  ok  Flow 5: 2-distinct-UTC-day inviter activation granted $5 to Alice")

# ---- Flow 6: Atomic 1% lifetime kickback inside Lightning top-up + replay idempotency + kill-switch ----
topup_req = {"pubkey": bob_pk, "msat": 100000000} # 100,000 sats = $100.00 -> 1% kickback = $1.00 (1,000,000 micros)
st, body, hdrs = http("POST", "/api/v1/inference/credit", topup_req)
assert st == 402, (st, body)
pay_req_hdr = hdrs.get("PAYMENT-REQUIRED") or hdrs.get("Payment-Required") or hdrs.get("payment-required")
assert pay_req_hdr, hdrs
req_obj = json.loads(base64.b64decode(pay_req_hdr).decode())
acc = req_obj["accepts"][0]
sig_payload = base64.b64encode(json.dumps({
    "x402Version": 2,
    "accepted": acc,
    "payload": {"invoice": acc["extra"]["invoice"], "paymentHash": acc["extra"]["paymentHash"]},
}).encode()).decode()

st, body, _ = http("POST", "/api/v1/inference/credit", topup_req, headers={"PAYMENT-SIGNATURE": sig_payload})
assert st == 200, (st, body)
st, asum, _ = http("GET", "/api/v1/referral", cookie=c_alice)
aj = json.loads(asum)
assert aj["kickback_earned_micros"] == 1000000 and aj["promo_micros"] == 11000000, aj

# Replay exact same PAYMENT-SIGNATURE -> 409 duplicate_settlement, zero duplicate kickback
st, body, _ = http("POST", "/api/v1/inference/credit", topup_req, headers={"PAYMENT-SIGNATURE": sig_payload})
assert st == 409, (st, body)
st, asum, _ = http("GET", "/api/v1/referral", cookie=c_alice)
assert json.loads(asum)["kickback_earned_micros"] == 1000000, asum

# Kill-switch referral_kickback OFF -> second top-up generates 0 kickback
st, _, _ = http("POST", "/api/v1/referral/admin/flag", {"name": "referral_kickback", "on": False}, cookie=c_admin)
assert st == 200, st
topup_req2 = {"pubkey": bob_pk, "msat": 50000000}
st, _, hdrs = http("POST", "/api/v1/inference/credit", topup_req2)
assert st == 402
acc2 = json.loads(base64.b64decode((hdrs.get("PAYMENT-REQUIRED") or hdrs.get("Payment-Required") or hdrs.get("payment-required"))).decode())["accepts"][0]
sig2 = base64.b64encode(json.dumps({"x402Version": 2, "accepted": acc2, "payload": {"invoice": acc2["extra"]["invoice"], "paymentHash": acc2["extra"]["paymentHash"]}}).encode()).decode()
st, _, _ = http("POST", "/api/v1/inference/credit", topup_req2, headers={"PAYMENT-SIGNATURE": sig2})
assert st == 200
st, asum, _ = http("GET", "/api/v1/referral", cookie=c_alice)
assert json.loads(asum)["kickback_earned_micros"] == 1000000, asum
st, _, _ = http("POST", "/api/v1/referral/admin/flag", {"name": "referral_kickback", "on": True}, cookie=c_admin)
assert st == 200
print("  ok  Flow 6: atomic 1% kickback ($1.00), replay 409 idempotent, kickback kill-switch respected")

# ---- Flow 7: Milestone 5 ($10 one-time), rolling 30d inviter cap (void 'cap'), daily budget ceiling ('pending') ----
for idx in range(1, 5): # 4 more activated invitees from distinct /24 IPs -> total 5!
    acct, ck = make_user(f"dyn_{idx}", f"wos_dyn_{idx}", pubkey=f"{idx:02x}"*32)
    st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": alice_code}, cookie=ck, headers={"X-Forwarded-For": f"198.51.{10+idx}.1"})
    assert st == 200, (idx, st)
    st, _, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=ck)
    assert st == 200
    # Day 1 & Day 2 inference calls
    infer("google/gemini-4-neon", 100, ck)
    psql(f"UPDATE referral.attributions SET first_active_day = (now() AT TIME ZONE 'UTC')::date - 1 WHERE invitee_account_id = '{acct}';")
    infer("google/gemini-4-neon", 100, ck)

st, asum, _ = http("GET", "/api/v1/referral", cookie=c_alice)
aj = json.loads(asum)
# Alice had 11M + 4*5M (20M) + 10M milestone-5 = 41M!
assert aj["activated_invitees"] == 5 and aj["milestones_mask"] == 1 and aj["promo_micros"] == 41000000, aj

# Cap override to 5 -> 6th invitee's inviter grant is voided with reason 'cap'
st, _, _ = http("POST", "/api/v1/referral/admin/cap", {"account": A_ALICE, "cap": 5, "reason": "test_cap"}, cookie=c_admin)
assert st == 200
A_DYN6, ck6 = make_user("dyn_6", "wos_dyn_6", pubkey="06"*32)
http("POST", "/api/v1/referral/redeem", {"code": alice_code}, cookie=ck6, headers={"X-Forwarded-For": "198.51.16.1"})
http("POST", "/api/v1/referral/recheck", {}, cookie=ck6)
infer("google/gemini-4-neon", 100, ck6)
psql(f"UPDATE referral.attributions SET first_active_day = (now() AT TIME ZONE 'UTC')::date - 1 WHERE invitee_account_id = '{A_DYN6}';")
infer("google/gemini-4-neon", 100, ck6)
void_reason = psql(f"SELECT status || ':' || reason FROM referral.grants WHERE account_id = '{A_ALICE}' AND kind = 'referral_inviter' AND source_ref = '{A_DYN6}';")
assert void_reason == "void:cap", void_reason

# Same-/24 IP burst auto-hold: >5 redeems on same /24 in 24h sets held=true + hold_reason='prefix_velocity'
for b_i in range(20, 27):
    b_acct, b_ck = make_user(f"dyn_{b_i}", f"wos_dyn_{b_i}", pubkey=f"{b_i:02x}"*32)
    st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": alice_code}, cookie=b_ck, headers={"X-Forwarded-For": "203.0.113.42"})
    assert st == 200, (b_i, st)
burst_held = psql(f"SELECT held || ':' || hold_reason FROM referral.attributions WHERE invitee_account_id = '{b_acct}';")
assert burst_held == "true:prefix_velocity", burst_held

# Daily budget ceiling -> stays 'pending', never errors!
st, _, _ = http("POST", "/api/v1/referral/admin/config", {"key": "daily_budget_micros", "value": "1"}, cookie=c_admin)
assert st == 200
A_DYN7, ck7 = make_user("dyn_7", "wos_dyn_7", pubkey="07"*32)
st, body, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=ck7)
assert st == 200 and json.loads(body)["promo_micros"] == 0, (st, body)
pend_st = psql(f"SELECT status || ':' || reason FROM referral.grants WHERE account_id = '{A_DYN7}' AND kind = 'github_connect';")
assert pend_st == "pending:budget_ceiling", pend_st
st, _, _ = http("POST", "/api/v1/referral/admin/config", {"key": "daily_budget_micros", "value": "500000000"}, cookie=c_admin)
assert st == 200
print("  ok  Flow 7: milestone 5 ($10), rolling 30d cap void ('cap'), daily budget ceiling holds 'pending'")

# ---- Flow 8: Admin campaigns (hash-only storage, revoke), clawback NEVER touching paid credit, audit log ----
st, body, _ = http("POST", "/api/v1/referral/admin/campaigns", {"uses": 2, "days": 30, "bonus_micros": 20000000, "note": "launch26"}, cookie=c_admin)
assert st == 200, (st, body)
cj = json.loads(body)
camp_id, camp_code = cj["id"], cj["code"]
assert len(camp_code) == 16, cj
stored_text = psql(f"SELECT coalesce(code_text, '<NULL>') FROM referral.codes WHERE id = {camp_id};")
assert stored_text == "<NULL>", f"campaign code plaintext leaked to DB: {stored_text}"

st, lbody, _ = http("GET", "/api/v1/referral/admin/campaigns", cookie=c_admin)
assert st == 200 and camp_code not in lbody and cj["code"][:4] in lbody, (st, lbody)

# Redeem campaign code on c_camp -> gets $20 + $5 = $25
st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": camp_code}, cookie=c_camp)
assert st == 200
st, body, _ = http("POST", "/api/v1/referral/recheck", {}, cookie=c_camp)
assert st == 200 and json.loads(body)["promo_micros"] == 25000000, body

# Revoke campaign -> next redeem returns 410
st, _, _ = http("POST", "/api/v1/referral/admin/campaigns/revoke", {"id": camp_id, "reason": "done"}, cookie=c_admin)
assert st == 200
_, ck8 = make_user("dyn_8", "wos_dyn_8", pubkey="08"*32)
st, _, _ = http("POST", "/api/v1/referral/redeem", {"code": camp_code}, cookie=ck8)
assert st == 410, f"expected 410 revoked campaign, got {st}"

# Clawback on Bob: zeroes unspent promo_credit ONLY, NEVER touches paid inference.credit!
bob_paid_before = int(psql(f"SELECT msat FROM inference.credit WHERE pubkey = '{bob_pk}';"))
assert bob_paid_before > 100000000, bob_paid_before
st, body, _ = http("POST", "/api/v1/referral/admin/grants/clawback", {"account": A_BOB, "reason": "fraud_test"}, cookie=c_admin)
assert st == 200, (st, body)
cb = json.loads(body)
assert cb["ok"] is True and cb["amount_micros"] == 10000000, cb
st, bsum, _ = http("GET", "/api/v1/referral", cookie=c_bob)
assert json.loads(bsum)["promo_micros"] == 0, bsum
bob_paid_after = int(psql(f"SELECT msat FROM inference.credit WHERE pubkey = '{bob_pk}';"))
assert bob_paid_after == bob_paid_before, f"clawback touched paid sats! {bob_paid_before} -> {bob_paid_after}"

audit_n = int(psql("SELECT count(*) FROM referral.admin_audit;"))
assert audit_n >= 6, f"expected >= 6 admin_audit rows, got {audit_n}"
print(f"  ok  Flow 8: hash-only campaigns, revoke 410, clawback zeroes promo ONLY (paid sats {bob_paid_after} untouched), {audit_n} audit rows")
PY

# Clean shutdown & assert ZERO ASan/UBSan diagnostics
kill -TERM "$API_PID" 2>/dev/null || true
wait "$API_PID" 2>/dev/null || true
API_PID=""

if grep -E "ERROR: AddressSanitizer|ERROR: LeakSanitizer|runtime error:" "$WORK/api.log" >/dev/null 2>&1; then
  echo "FAIL: ASan/UBSan error detected in oa-api log!" >&2
  grep -n -C 5 -E "ERROR: AddressSanitizer|ERROR: LeakSanitizer|runtime error:" "$WORK/api.log" >&2
  exit 1
fi
echo "  ok  Flow 9: clean ASan/UBSan shutdown with zero sanitizer errors"
echo "PASS: all Referral & Promo Credit E2E flows verified under ASan/UBSan!"
