#!/usr/bin/env bash
# Firm reason: 5 (thin E2E harness exercising the real openagents CLI --workos RFC 8628 device flow against workos/emulate + oa-api).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
cli_dir="$(cd "$here/.." && pwd)"
repo="$(cd "$cli_dir/../.." && pwd)"
api_dir="$repo/coax/web/api"
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
emu_pid=""; api_pid=""
cleanup() {
  [[ -n "$api_pid" ]] && kill "$api_pid" 2>/dev/null || true
  [[ -n "$emu_pid" ]] && kill "$emu_pid" 2>/dev/null || true
  "$pgbin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

# 1. Build CLI binary
cli_bin="${OA_CLI_BIN:-$cli_dir/build/openagents}"
if [[ ! -x "$cli_bin" ]]; then
  (cd "$repo" && COAX="$coax" "$cli_dir/build.sh" --cli >/dev/null)
fi

# 2. Build oa-api if OA_API_BIN not supplied
api_bin="${OA_API_BIN:-$work/api}"
if [[ ! -x "$api_bin" ]]; then
  source "$api_dir/api-sources.sh"
  api_sources "$api_dir" "$work" "$coax"
  "$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -DOA_LNADDR_TEST_FETCH \
    -I"$work" "${pq_inc[@]}" "${secp_inc[@]}" "${ssl_inc[@]}" -o "$api_bin" "${API_SOURCES[@]}" \
    "${pq_lib[@]}" -lpq "${secp_lib[@]}" -lsecp256k1 "${ssl_lib[@]}" -lssl -lcrypto -lcurl -lz -lpthread -lm ${darwin_fw[@]+"${darwin_fw[@]}"}
fi

# 3. Start throwaway Postgres 16 + workos/emulate + oa-api
pg_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
api_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
emu_port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"

"$pgbin/initdb" -D "$work/db" -U postgres --auth=trust --no-sync >/dev/null 2>&1
"$pgbin/pg_ctl" -D "$work/db" -l "$work/pg.log" -o "-k $work -p $pg_port -h 127.0.0.1 -F" start >/dev/null
"$pgbin/createdb" -h 127.0.0.1 -p "$pg_port" -U postgres oa_analytics
"$pgbin/psql" -h 127.0.0.1 -p "$pg_port" -U postgres -d oa_analytics -c "CREATE ROLE oa_analytics SUPERUSER LOGIN" >/dev/null
pgurl="host=127.0.0.1 port=$pg_port user=oa_analytics dbname=oa_analytics"

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

api_origin="http://127.0.0.1:$api_port"
OA_API_DATABASE="$pgurl" \
OA_API_ORIGIN="$api_origin" \
OA_API_WORKERS=1 \
OA_AUTH_SEAL_KEY="1:0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20" \
OA_AUTH_LOOKUP_PEPPER="1:201f1e1d1c1b1a191817161514131211100f0e0d0c0b0a090807060504030201" \
OA_WORKOS_ENABLED=1 \
OA_WORKOS_BASE_URL="$emu_base" \
OA_WORKOS_API_KEY="sk_test_default" \
OA_WORKOS_CLIENT_ID="client_01E2E_DEFAULT" \
  "$api_bin" --listen "127.0.0.1:$api_port" --migrations "$api_dir/migrations" >"$work/api.log" 2>&1 &
api_pid=$!
for _ in $(seq 1 60); do curl -fsS "$api_origin/api/playtest/health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "$api_origin/api/playtest/health" >/dev/null || { cat "$work/api.log" >&2; exit 1; }

# 4. Run real CLI login --workos -> whoami -> token refresh -> logout in an isolated HOME!
fake_home="$work/home"
mkdir -p "$fake_home"

set +e
login_json="$(HOME="$fake_home" OA_WORKOS_BASE_URL="$emu_base" \
  "$cli_bin" --json --profile wos login --workos --origin "$api_origin" --poll-ms 100 2>"$work/login.err")"
rc=$?
set -e
if [[ $rc -ne 0 ]]; then
  echo "cli login failed (exit $rc): $login_json" >&2
  cat "$work/login.err" >&2
  echo "--- api.log ---" >&2
  tail -30 "$work/api.log" >&2
  echo "--- emu.log ---" >&2
  tail -30 "$work/emu.log" >&2
  exit 1
fi
echo "  cli login: $login_json"

tok_file="$fake_home/.openagents/cli/tokens/wos"
[[ -f "$tok_file" ]] || { echo "missing token file $tok_file" >&2; exit 1; }
mode="$(stat -f '%Lp' "$tok_file" 2>/dev/null || stat -c '%a' "$tok_file")"
[[ "$mode" == "600" ]] || { echo "token file mode is $mode, expected 600" >&2; exit 1; }

whoami_json="$(HOME="$fake_home" "$cli_bin" --json --profile wos whoami)"
echo "  cli whoami: $whoami_json"

# Force expires_at=0 so whoami exercises Option A1 refresh rotation!
python3 - "$tok_file" <<'PY'
import json, sys
p = sys.argv[1]
d = json.load(open(p))
d["expires_at"] = 0
old_rt = d["refresh_token"]
open(p, "w").write(json.dumps(d))
print("  forced refresh from:", old_rt[:12])
PY
whoami2_json="$(HOME="$fake_home" "$cli_bin" --json --profile wos whoami)"
echo "  cli whoami (post-refresh): $whoami2_json"

logout_json="$(HOME="$fake_home" "$cli_bin" --json --profile wos logout)"
echo "  cli logout: $logout_json"
[[ ! -f "$tok_file" ]] || { echo "logout did not remove token file" >&2; exit 1; }

python3 - "$login_json" "$whoami_json" "$whoami2_json" "$logout_json" <<'PY'
import json, sys
l, w1, w2, lo = [json.loads(x) for x in sys.argv[1:]]
assert l["via"] == "workos" and l["account"].startswith("acct_"), l
assert w1["via"] == "workos" and w1["account"] == l["account"], w1
assert w2["via"] == "workos" and w2["account"] == l["account"], w2
assert lo["via"] == "workos" and lo["forgot"] is True, lo
print("==> workos_cli_e2e.sh PASSED (RFC 8628 device flow, JWKS exchange, 0600 storage, refresh rotation, logout)")
PY
