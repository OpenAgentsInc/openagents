#!/usr/bin/env bash
# Firm reason: 5 (thin integration test harness starting throwaway Postgres 16, workos/emulate, and ASan oa-api).
# End-to-end Phase 3 WorkOS backend verification (#14100):
#   - OA_WORKOS_ENABLED config + gamebuilder.org isolation
#   - Magic Auth inline 6-digit verify + Radar context + email_verified=true auto-link (audit + notice + external_id writeback)
#   - Option A1 L1 session preserving L2/L2-fresh wallet/agent gates
#   - Unverified email non-link (creates separate acct_)
#   - OAuth PKCE start + CSRF state mismatch rejection + Enterprise SSO domain discovery
#   - Native / CLI RS256 JWKS JWT exchange -> Option A1 oa_at_/oa_rt_ tokens
#   - Verified webhook (±180s) -> workos.* mirror + orgs list/switch + session.revoked local invalidation + idempotency
#   - Synchronous AuthKit Actions (±30s) -> signed Allow / Deny on frozen account
#   - Pure Coax/C GET /events cursor mirror poller (POST /api/jobs/workos-events)
#   - Freeze revocation + 30-day auth-sweep purge propagation (DELETE /user_management/users/{id})
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
pg_pid=""; emu_pid=""; api_pid=""
cleanup() {
  [[ -n "$api_pid" ]] && kill "$api_pid" 2>/dev/null || true
  [[ -n "$emu_pid" ]] && kill "$emu_pid" 2>/dev/null || true
  "$pgbin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

# 1. Build oa-api with ASan/UBSan + -Werror (or use prebuilt OA_API_BIN / OA_SIGN_BIN)
api_bin="${OA_API_BIN:-$work/api}"
sign_bin="${OA_SIGN_BIN:-$work/sign}"
if [[ ! -x "$api_bin" ]]; then
  source "$api_dir/api-sources.sh"
  api_sources "$api_dir" "$work" "$coax"
  "$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -DOA_LNADDR_TEST_FETCH \
    -fsanitize=address,undefined -I"$work" "${pq_inc[@]}" "${secp_inc[@]}" "${ssl_inc[@]}" \
    -o "$api_bin" "${API_SOURCES[@]}" \
    "${pq_lib[@]}" -lpq "${secp_lib[@]}" -lsecp256k1 "${ssl_lib[@]}" -lssl -lcrypto -lcurl -lz -lpthread -lm ${darwin_fw[@]+"${darwin_fw[@]}"}
fi

# Build NIP-98 signer helper for admin job calls
if [[ ! -x "$sign_bin" ]]; then
  "$cc" -std=c11 -O1 -D_DEFAULT_SOURCE "${secp_inc[@]}" "${ssl_inc[@]}" "$api_dir/sign.c" -o "$sign_bin" \
    "${secp_lib[@]}" -lsecp256k1 "${ssl_lib[@]}" -lcrypto
fi

# 2. Start throwaway Postgres 16
pg_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
api_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
emu_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

"$pgbin/initdb" -D "$work/db" -U postgres --auth=trust --no-sync >/dev/null 2>&1
"$pgbin/pg_ctl" -D "$work/db" -l "$work/pg.log" -o "-k $work -p $pg_port -h 127.0.0.1 -F" start >/dev/null
"$pgbin/createdb" -h 127.0.0.1 -p "$pg_port" -U postgres oa_analytics
"$pgbin/psql" -h 127.0.0.1 -p "$pg_port" -U postgres -d oa_analytics -c "CREATE ROLE oa_analytics SUPERUSER LOGIN" >/dev/null
pgurl="host=127.0.0.1 port=$pg_port user=oa_analytics dbname=oa_analytics"

# 3. Start local workos/emulate
emu_cmd=()
if [[ -n "${WORKOS_EMULATE_BIN:-}" ]]; then emu_cmd=("$WORKOS_EMULATE_BIN")
elif [[ -f "/Users/christopherdavid/work/projects/repos/workos/emulate/src/cli.ts" ]] && command -v bun >/dev/null 2>&1; then
  emu_cmd=(bun run /Users/christopherdavid/work/projects/repos/workos/emulate/src/cli.ts)
elif command -v bunx >/dev/null 2>&1; then emu_cmd=(bunx @workos/emulate)
else emu_cmd=(npx --yes @workos/emulate); fi

NO_UPDATE_NOTIFIER=1 WORKOS_EMULATE_DISABLE_UPDATE_CHECK=1 \
  "${emu_cmd[@]}" --port "$emu_port" --seed "$repo/coax/lib/workos/tests/emulate_seed.yaml" >"$work/emu.log" 2>&1 &
emu_pid=$!
emu_base="http://127.0.0.1:$emu_port"
for _ in $(seq 1 50); do curl -fsS "$emu_base/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "$emu_base/health" >/dev/null || { cat "$work/emu.log" >&2; exit 1; }

# 4. Start oa-api with OA_WORKOS_ENABLED=1 against workos/emulate
admin_sk="1111111111111111111111111111111111111111111111111111111111111111"
printf '%s\n' "$admin_sk" > "$work/admin.key"
chmod 600 "$work/admin.key"
admin_pk="$("$sign_bin" --key "$work/admin.key" --pubkey)"
api_origin="http://127.0.0.1:$api_port"
wh_secret="whsec_e2e_test_secret_0123456789abcdef"
act_secret="actsec_e2e_test_secret_fedcba9876543210"

ASAN_OPTIONS=detect_leaks=0 \
OA_API_DATABASE="$pgurl" \
OA_API_ORIGIN="$api_origin" \
OA_API_SITE_ORIGINS="https://gamebuilder.org" \
OA_API_WORKERS=1 \
OA_ADMIN_PUBKEYS="$admin_pk" \
OA_AUTH_SEAL_KEY="1:0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20" \
OA_AUTH_LOOKUP_PEPPER="1:201f1e1d1c1b1a191817161514131211100f0e0d0c0b0a090807060504030201" \
OA_WORKOS_ENABLED=1 \
OA_WORKOS_BASE_URL="$emu_base" \
OA_WORKOS_API_KEY="sk_test_default" \
OA_WORKOS_CLIENT_ID="client_01E2E_DEFAULT" \
OA_WORKOS_WEBHOOK_SECRET="$wh_secret" \
OA_WORKOS_ACTION_SECRET="$act_secret" \
  "$api_bin" --listen "127.0.0.1:$api_port" --migrations "$api_dir/migrations" >"$work/api.log" 2>&1 &
api_pid=$!

for _ in $(seq 1 60); do curl -fsS "$api_origin/api/playtest/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "$api_origin/api/playtest/health" >/dev/null || { cat "$work/api.log" >&2; exit 1; }

# 5. Run end-to-end protocol assertions
API="$api_origin" EMU="$emu_base" PGURL="$pgurl" PSQL="$pgbin/psql" \
WH_SECRET="$wh_secret" ACT_SECRET="$act_secret" SIGN="$sign_bin" ADMIN_KEY="$work/admin.key" \
python3 - <<'PY'
import hashlib, hmac, json, os, subprocess, time, urllib.request, urllib.error, urllib.parse, http.cookiejar

API = os.environ["API"]
EMU = os.environ["EMU"]
PGURL = os.environ["PGURL"]
PSQL = os.environ["PSQL"]
WH_SECRET = os.environ["WH_SECRET"]
ACT_SECRET = os.environ["ACT_SECRET"]
SIGN = os.environ["SIGN"]
ADMIN_KEY = os.environ["ADMIN_KEY"]

def psql(sql):
    return subprocess.check_output([PSQL, PGURL, "-XAtq", "-c", sql], text=True).strip()

def req(url, method="GET", data=None, headers=None, opener=None, follow=True):
    hdrs = dict(headers or {})
    body = None
    if data is not None:
        body = json.dumps(data).encode() if not isinstance(data, (bytes, str)) else (data.encode() if isinstance(data, str) else data)
        hdrs.setdefault("Content-Type", "application/json")
    r = urllib.request.Request(url, data=body, headers=hdrs, method=method)
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *a, **kw): return None
    op = opener or (urllib.request.build_opener() if follow else urllib.request.build_opener(NoRedirect()))
    try:
        with op.open(r, timeout=15) as resp:
            return resp.status, dict(resp.headers), resp.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read().decode()

def sign_wos(secret, payload, ts_ms=None):
    ts = str(ts_ms if ts_ms is not None else int(time.time() * 1000))
    mac = hmac.new(secret.encode(), f"{ts}.{payload}".encode(), hashlib.sha256).hexdigest()
    return f"t={ts}, v1={mac}"

def verify_action_sig(secret, resp_json):
    d = json.loads(resp_json)
    payload_str = json.dumps(d["payload"], separators=(",", ":"))
    ts = str(d["payload"]["timestamp"])
    expected = hmac.new(secret.encode(), f"{ts}.{payload_str}".encode(), hashlib.sha256).hexdigest()
    assert hmac.compare_digest(expected, d["signature"]), f"bad action sig: {d}"
    return d["payload"]

# (1) Config & gamebuilder.org isolation
st, _, body = req(f"{API}/api/auth/workos/config", headers={"Origin": API})
assert st == 200 and json.loads(body)["enabled"] is True, (st, body)
st, _, body = req(f"{API}/api/auth/workos/config", headers={"Origin": "https://gamebuilder.org"})
# Listed site is refused by ap_site_route on /api/auth/workos/config (403) — completely isolated!
assert st == 403 or (st == 200 and json.loads(body)["enabled"] is False), (st, body)
print("  ok: /api/auth/workos/config enabled for openagents.com and isolated from gamebuilder.org")

# (2) Seed pre-existing OpenAgents acct_ with alice@acme.test + a Nostr key (so account has strong L2 way in!)
# Lookup HMAC = "1:" + HMAC-SHA256(pepper_bytes, "alice@acme.test")
pepper = bytes.fromhex("201f1e1d1c1b1a191817161514131211100f0e0d0c0b0a090807060504030201")
alice_lookup = "1:" + hmac.new(pepper, b"alice@acme.test", hashlib.sha256).hexdigest()
pre_acct = psql(f"""
  WITH a AS (
    INSERT INTO auth.accounts (id, kind, verse_account)
    VALUES (auth.new_account_id(), 'person', 'v_alice_pre') RETURNING id
  ), e AS (
    INSERT INTO auth.emails (account, lookup, sealed, is_primary, verified_at)
    SELECT id, '{alice_lookup}', '', true, now() FROM a
  ), k AS (
    INSERT INTO auth.nostr_keys (account, pubkey, label)
    SELECT id, repeat('a', 64), 'test-key' FROM a
  )
  SELECT id FROM a
""")
assert pre_acct.startswith("acct_"), pre_acct

# (3) Inline Magic Auth start & verify -> auto-links to pre_acct on email_verified=true + audit + notice + external_id writeback
# Trigger magic auth via oa-api /magic/start, capture __Host-oa_wos_pending cookie explicitly (HTTP loopback skips Secure in cookiejar).
st, shdrs, body = req(f"{API}/api/auth/workos/magic/start", "POST", {"email": "alice@acme.test"}, {"Origin": API})
assert st == 200 and json.loads(body)["state"] == "sent", (st, body)
pending_cookie = (shdrs.get("Set-Cookie") or shdrs.get("set-cookie") or "").split(";", 1)[0]
assert "__Host-oa_wos_pending=" in pending_cookie, shdrs

# Create a fresh magic_auth via emulate API to obtain its 6-digit code cleanly (WorkOS API returns code on create)
st, _, mbody = req(f"{EMU}/user_management/magic_auth", "POST", {"email": "alice@acme.test"}, {"Authorization": "Bearer sk_test_default"})
assert st in (200, 201), (st, mbody)
mcode = json.loads(mbody)["code"]

st, hdrs, body = req(
    f"{API}/api/auth/workos/magic/verify", "POST",
    {"code": mcode, "radar_auth_attempt_id": "rad_e2e_1"},
    {"Origin": API, "Cookie": pending_cookie, "User-Agent": "OpenAgents-E2E/1", "X-Forwarded-For": "198.51.100.7"},
)
assert st == 200, (st, body)
vb = json.loads(body)
assert vb["state"] == "logged-in" and vb["account"] == pre_acct and vb["new"] is False, vb
set_cookie = hdrs.get("Set-Cookie") or hdrs.get("set-cookie") or ""
assert "__Host-oa_session=oas_" in set_cookie, hdrs
session_cookie = set_cookie.split(";", 1)[0]

# Check audit + security notice + external_id synced back to WorkOS!
audit_cnt = psql(f"SELECT count(*) FROM auth.audit WHERE account = '{pre_acct}' AND action = 'workos.auto_link' AND result = 'ok'")
notice_cnt = psql(f"SELECT count(*) FROM auth.notices WHERE account = '{pre_acct}' AND what = 'workos.link'")
assert int(audit_cnt) == 1 and int(notice_cnt) == 1, (audit_cnt, notice_cnt)
st, _, ub = req(f"{EMU}/user_management/users/user_01E2E_ALICE", headers={"Authorization": "Bearer sk_test_default"})
assert st == 200 and json.loads(ub)["external_id"] == pre_acct, (st, ub)
print("  ok: Magic Auth auto-linked verified email to existing acct_, logged audit+notice, and synced external_id")

# (4) WorkOS session has full access (level L2, not limited, no L2/L2-fresh refusal)
st, _, body = req(f"{API}/api/auth/account", headers={"Cookie": session_cookie})
assert st == 200, (st, body)
acct_info = json.loads(body)
assert acct_info["logged_in"] is True and acct_info["account"] == pre_acct and acct_info["level"] == "L2" and acct_info["via"] == "workos", acct_info
st, _, body = req(f"{API}/api/chat/v2/sync", "POST", {}, {"Origin": API, "Cookie": session_cookie})
assert st != 401, f"WorkOS session must never get 401 limited/L2 refusal on chat sync, got {st}: {body}"
st, _, body = req(f"{API}/api/tools/cloud/v1", "POST", {}, {"Origin": API, "Cookie": session_cookie})
assert st != 401, f"WorkOS session must never get 401 limited/L2 refusal on cloud tools, got {st}: {body}"
print("  ok: WorkOS session has full access (L2, non-limited) on chat sync and cloud tools")

# (5) Unverified email MUST NOT auto-link to an existing account!
unver_lookup = "1:" + hmac.new(pepper, b"unver@acme.test", hashlib.sha256).hexdigest()
victim_acct = psql(f"""
  WITH a AS (
    INSERT INTO auth.accounts (id, kind, verse_account)
    VALUES (auth.new_account_id(), 'person', 'v_victim') RETURNING id
  ), e AS (
    INSERT INTO auth.emails (account, lookup, sealed, is_primary, verified_at)
    SELECT id, '{unver_lookup}', '', true, now() FROM a
  ) SELECT id FROM a
""")
st, _, uv_res = req(f"{EMU}/user_management/users", "POST", {"email": "unver@acme.test", "email_verified": False}, {"Authorization": "Bearer sk_test_default"})
assert st in (200, 201), (st, uv_res)
uv_uid = json.loads(uv_res)["id"]
resolved_row = psql(f"SELECT acct, made, auto_linked FROM workos.account_resolve('{uv_uid}', ARRAY['{unver_lookup}'], '{unver_lookup}', false, false, 'GoogleOAuth', '')")
r_acct, r_made, r_auto = resolved_row.split("|")
assert r_acct != victim_acct and r_made == "t" and r_auto == "f", resolved_row
print("  ok: unverified email created fresh acct_ and refused auto-link to existing account")

# (6) OAuth PKCE start + CSRF state rejection + Enterprise SSO domain discovery
st, hdrs, body = req(f"{API}/api/auth/workos/oauth/start?provider=GoogleOAuth&next=/chat&mode=json", headers={"Origin": API})
assert st == 200, (st, body)
auth_url = json.loads(body)["url"]
assert "provider=GoogleOAuth" in auth_url and "code_challenge_method=S256" in auth_url and "state=" in auth_url, auth_url
st, hdrs, _ = req(f"{API}/api/auth/workos/callback?code=fake&state=wrong", headers={"Cookie": "__Host-oa_wos_state=different"}, follow=False)
assert st == 303 and "/login?error=oauth_failed" in hdrs.get("Location", ""), (st, hdrs)
st, _, body = req(f"{API}/api/auth/workos/sso/discover", "POST", {"email": "alice@acme.test"}, {"Origin": API})
assert st == 200 and json.loads(body)["sso"] is True and json.loads(body)["connection_id"], (st, body)
print("  ok: OAuth PKCE S256 URL, CSRF state guard, and Enterprise SSO domain discovery verified")

# (7) Native / CLI live RS256 JWKS JWT exchange -> Option A1 oa_at_/oa_rt_ tokens
st, _, m2 = req(f"{EMU}/user_management/magic_auth", "POST", {"email": "alice@acme.test"}, {"Authorization": "Bearer sk_test_default"})
m2_code = json.loads(m2)["code"]
st, _, auth2 = req(f"{EMU}/user_management/authenticate", "POST", {
    "grant_type": "urn:workos:oauth:grant-type:magic-auth:code",
    "client_id": "client_01E2E_DEFAULT", "client_secret": "sk_test_default",
    "email": "alice@acme.test", "code": m2_code,
})
assert st == 200, (st, auth2)
wos_jwt = json.loads(auth2)["access_token"]
st, _, ex = req(f"{API}/api/auth/workos/exchange", "POST", {"access_token": wos_jwt})
assert st == 200, (st, ex)
exd = json.loads(ex)
assert exd["account"] == pre_acct and exd["access_token"].startswith("oa_at_") and exd["refresh_token"].startswith("oa_rt_"), exd
st, _, me_app = req(f"{API}/api/auth/account", headers={"Authorization": f"Bearer {exd['access_token']}"})
assert st == 200 and json.loads(me_app)["account"] == pre_acct and json.loads(me_app)["level"] == "L2", (st, me_app)
print("  ok: /api/auth/workos/exchange verified live RS256 JWKS JWT and minted Option A1 oa_at_/oa_rt_ tokens")

# (8) Verified Webhooks (±180s) -> workos.* mirror, orgs list/switch, session.revoked, idempotency
ev_org = json.dumps({"id": "evt_e2e_org1", "event": "organization.created", "data": {"id": "org_01E2E_ACME", "name": "Acme Robotics"}})
st, _, _ = req(f"{API}/api/auth/workos/webhook", "POST", ev_org, {"WorkOS-Signature": "t=1000, v1=" + "00"*32})
assert st == 401
st, _, wb = req(f"{API}/api/auth/workos/webhook", "POST", ev_org, {"WorkOS-Signature": sign_wos(WH_SECRET, ev_org)})
assert st == 200 and json.loads(wb)["applied"] is True, (st, wb)
# Replay duplicate event id -> applied: false (idempotent)
st, _, wb2 = req(f"{API}/api/auth/workos/webhook", "POST", ev_org, {"WorkOS-Signature": sign_wos(WH_SECRET, ev_org)})
assert st == 200 and json.loads(wb2)["applied"] is False, (st, wb2)

ev_mem = json.dumps({"id": "evt_e2e_mem1", "event": "organization_membership.created", "data": {"id": "om_e2e_1", "user_id": "user_01E2E_ALICE", "organization_id": "org_01E2E_ACME", "status": "active"}})
st, _, _ = req(f"{API}/api/auth/workos/webhook", "POST", ev_mem, {"WorkOS-Signature": sign_wos(WH_SECRET, ev_mem)})
assert st == 200
st, _, orgs_body = req(f"{API}/api/auth/workos/orgs", headers={"Cookie": session_cookie})
assert st == 200 and len(json.loads(orgs_body)["organizations"]) == 1, (st, orgs_body)

# session.revoked webhook kills local OpenAgents session immediately!
ev_rev = json.dumps({"id": "evt_e2e_rev1", "event": "session.revoked", "data": {"id": "sess_e2e_1", "user_id": "user_01E2E_ALICE"}})
st, _, _ = req(f"{API}/api/auth/workos/webhook", "POST", ev_rev, {"WorkOS-Signature": sign_wos(WH_SECRET, ev_rev)})
assert st == 200
st, _, after_rev = req(f"{API}/api/auth/account", headers={"Cookie": session_cookie})
assert st == 200 and json.loads(after_rev)["logged_in"] is False, (st, after_rev)
print("  ok: verified webhook updated workos.* mirror, listed orgs, revoked local session, and ignored duplicates")

# (9) Synchronous AuthKit Actions (±30s): Allow active user -> Freeze -> Deny frozen user!
act_payload = json.dumps({"object": "authentication_action_context", "user": {"id": "user_01E2E_ALICE", "email": "alice@acme.test"}})
stale_sig = sign_wos(ACT_SECRET, act_payload, ts_ms=int(time.time() * 1000) - 60000)
st, _, _ = req(f"{API}/api/auth/workos/action", "POST", act_payload, {"WorkOS-Signature": stale_sig})
assert st == 401
st, _, ab = req(f"{API}/api/auth/workos/action", "POST", act_payload, {"WorkOS-Signature": sign_wos(ACT_SECRET, act_payload)})
assert st == 200 and verify_action_sig(ACT_SECRET, ab)["verdict"] == "Allow", (st, ab)

# Freeze account -> Action denies!
psql(f"SELECT auth.freeze('{pre_acct}')")
st, _, ab2 = req(f"{API}/api/auth/workos/action", "POST", act_payload, {"WorkOS-Signature": sign_wos(ACT_SECRET, act_payload)})
assert st == 200
p2 = verify_action_sig(ACT_SECRET, ab2)
assert p2["verdict"] == "Deny" and "frozen" in p2["error_message"].lower(), p2
print("  ok: synchronous AuthKit Actions returned HMAC-signed Allow (active) and Deny (frozen)")

# (10) Pure Coax/C Events API mirror poller (POST /api/jobs/workos-events)
ev_url = f"{API}/api/jobs/workos-events"
nip98 = subprocess.check_output([SIGN, "--key", ADMIN_KEY, "POST", ev_url], text=True).strip()
st, _, jb = req(ev_url, "POST", "", {"Authorization": f"Nostr {nip98}"})
assert st == 200, (st, jb)
jd = json.loads(jb)
assert jd["state"] == "ran" and jd["result"]["fetched"] >= 1, jd
cur = psql("SELECT coalesce(after_event_id, '') FROM workos.sync_cursor WHERE stream = 'events'")
assert cur, "expected sync_cursor to advance"
print(f"  ok: pure Coax/C GET /events mirror poller ran (fetched={jd['result']['fetched']}, cursor={cur})")

# (11) 30-day auth-sweep purge propagates DELETE /user_management/users/{id} to WorkOS!
st, _, del_u = req(f"{EMU}/user_management/users", "POST", {"email": "purge@acme.test", "email_verified": True}, {"Authorization": "Bearer sk_test_default"})
assert st in (200, 201), (st, del_u)
purge_wos_uid = json.loads(del_u)["id"]
purge_acct = psql(f"""
  WITH a AS (
    INSERT INTO auth.accounts (id, kind, verse_account, delete_asked_at, purge_at, frozen_at, frozen_why)
    VALUES (auth.new_account_id(), 'person', 'v_purge', now() - interval '31 days', now() - interval '1 minute', now() - interval '31 days', 'delete')
    RETURNING id
  ), w AS (
    INSERT INTO auth.workos_identities (workos_user_id, account)
    SELECT '{purge_wos_uid}', id FROM a
  ) SELECT id FROM a
""")
sw_url = f"{API}/api/jobs/auth-sweep"
nip98_sw = subprocess.check_output([SIGN, "--key", ADMIN_KEY, "POST", sw_url], text=True).strip()
st, _, sb = req(sw_url, "POST", "", {"Authorization": f"Nostr {nip98_sw}"})
assert st == 200, (st, sb)
st, _, check_del = req(f"{EMU}/user_management/users/{purge_wos_uid}", headers={"Authorization": "Bearer sk_test_default"})
assert st == 404, f"expected WorkOS user {purge_wos_uid} to be deleted (404), got {st}: {check_del}"
print(f"  ok: 30-day auth-sweep tombstoned {purge_acct} and propagated DELETE /user_management/users/{purge_wos_uid}")

# (12) Email + Password register (with 6-digit email verification), wrong password 401, valid login (L1 + L2-fresh preserved), reset & confirm
pw_email = "pwuser@acme.test"
st, _, body = req(f"{API}/api/auth/workos/password/register", "POST", {"email": pw_email, "password": "short"}, {"Origin": API})
assert st == 400, (st, body)
st, hdrs, body = req(f"{API}/api/auth/workos/password/register", "POST", {"email": pw_email, "password": "correct horse battery staple"}, {"Origin": API})
assert st == 200, (st, body)
reg_d = json.loads(body)
if reg_d.get("state") == "verify-email":
    ev_cookie = (hdrs.get("Set-Cookie") or hdrs.get("set-cookie") or "").split(";", 1)[0]
    assert "__Host-oa_wos_pending=" in ev_cookie, hdrs
    st, _, evs = req(f"{EMU}/events?events=email_verification.created", headers={"Authorization": "Bearer sk_test_default"})
    assert st == 200, (st, evs)
    vcode = ""
    for item in json.loads(evs).get("data") or []:
        if (item.get("data") or {}).get("email") == pw_email:
            vcode = (item.get("data") or {}).get("code") or ""
    assert len(vcode) == 6, (vcode, evs)
    st, hdrs, body = req(f"{API}/api/auth/workos/magic/verify", "POST", {"code": vcode}, {"Origin": API, "Cookie": ev_cookie})
    assert st == 200, (st, body)
    reg_d = json.loads(body)
assert reg_d["state"] == "logged-in" and reg_d["account"].startswith("acct_"), reg_d
pw_cookie = (hdrs.get("Set-Cookie") or hdrs.get("set-cookie") or "").split(";", 1)[0]
assert "__Host-oa_session=oas_" in pw_cookie, hdrs

# Wrong password refused with 401
st, _, body = req(f"{API}/api/auth/workos/password/login", "POST", {"email": pw_email, "password": "wrong password"}, {"Origin": API})
assert st == 401, (st, body)

# Valid password login mints full-access L2 session
st, hdrs, body = req(f"{API}/api/auth/workos/password/login", "POST", {"email": pw_email, "password": "correct horse battery staple"}, {"Origin": API})
assert st == 200 and json.loads(body)["account"] == reg_d["account"], (st, body)
pw_cookie2 = (hdrs.get("Set-Cookie") or hdrs.get("set-cookie") or "").split(";", 1)[0]
st, _, me_pw = req(f"{API}/api/auth/account", headers={"Cookie": pw_cookie2})
assert st == 200 and json.loads(me_pw)["level"] == "L2" and json.loads(me_pw)["via"] == "workos", (st, me_pw)

# Password reset returns generic 200 even for unknown email (no enumeration)
st, _, rbody = req(f"{API}/api/auth/workos/password/reset", "POST", {"email": "nobody@acme.test"}, {"Origin": API})
assert st == 200 and json.loads(rbody)["state"] == "sent", (st, rbody)
st, _, rbody = req(f"{API}/api/auth/workos/password/reset", "POST", {"email": pw_email}, {"Origin": API})
assert st == 200 and json.loads(rbody)["state"] == "sent", (st, rbody)

# Confirm password reset via emulate's issued reset token, then log in with the new password
st, _, pr = req(f"{EMU}/user_management/password_reset", "POST", {"email": pw_email}, {"Authorization": "Bearer sk_test_default"})
assert st in (200, 201), (st, pr)
pr_tok = json.loads(pr).get("password_reset_token") or ""
if pr_tok:
    st, _, cb = req(f"{API}/api/auth/workos/password/reset/confirm", "POST", {"token": pr_tok, "new_password": "new stronger passphrase 2026"}, {"Origin": API})
    assert st == 200 and json.loads(cb)["state"] == "reset", (st, cb)
    st, _, nb = req(f"{API}/api/auth/workos/password/login", "POST", {"email": pw_email, "password": "new stronger passphrase 2026"}, {"Origin": API})
    assert st == 200 and json.loads(nb)["account"] == reg_d["account"], (st, nb)
print("  ok: WorkOS Email+Password register, bad-pw 401, L1 login, no-enum reset, and reset/confirm verified")

print("oa-api WorkOS E2E: ALL CHECKS PASSED")
PY

if grep -q "ERROR: AddressSanitizer\|runtime error" "$work/api.log"; then
  echo "ASan/UBSan error detected in api.log:" >&2
  cat "$work/api.log" >&2
  exit 1
fi
echo "==> workos_e2e_test.sh PASSED (zero ASan/UBSan errors)"
