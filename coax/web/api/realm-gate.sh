#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the API and the CLI, starts a throwaway PostgreSQL and the API on it, and runs the Coax checks)
# Aethyrforge's made-realm gate (GitHub #12629), on Linux (NixOS too) or a Mac: the API built from
# this checkout on a fresh PostgreSQL with every migration, then
#   tests/realm_engine_pin.coax with realm_engine_pin.sql, realm_shapes.sql and realm_edit_log.sql
#   tests/realm_walkthrough.coax                       edit, publish, reload, fork, refusals
#   tests/realm_cli_api.coax (PINNED=1)                the built CLI against this API; publishes pin
#                                                      tests/engine-template.html (OA_ENGINE_TEMPLATE)
#   coax/web/forge-access.coax (with --page)           keyboard, screen reader, lifecycle, guest and
#                                                      controller in muted headless Chrome; needs
#                                                      apps/openagents-web/build/myrealm.wasm
#   coax/web/api/realm-gate.sh [--page [--require-hardware]]
# --require-hardware is handed to forge-access: the page must get a GPU's WebGPU adapter.
# REALM_GATE_PAGE_ONLY=1 skips the database checks (the page alone, after they passed).
# COAX names the compiler (default coax/bin/coax). Nothing is kept: the database, the API and the
# site server stop when it ends.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
[[ -x "$coax" ]] || { echo "realm-gate: no compiler at $coax (set COAX)" >&2; exit 2; }
if [[ "$(uname -s)" == Linux && -e /etc/NIXOS && -z "${IN_NIX_SHELL:-}" ]]; then
  exec nix-shell -p postgresql postgresql.lib postgresql.dev secp256k1 openssl.dev curl.dev zlib.dev pkg-config --run "$(printf '%q ' "$0" "$@")"
fi
if [[ "$(uname -s)" == Darwin ]]; then
  pg=/opt/homebrew/opt/postgresql@16/bin; cc=(/usr/bin/clang)
  flags=(-I/opt/homebrew/opt/postgresql@16/include -I"$(brew --prefix secp256k1)/include" -I"$(brew --prefix openssl@3)/include")
  libs=(-L/opt/homebrew/opt/postgresql@16/lib -L"$(brew --prefix secp256k1)/lib" -L"$(brew --prefix openssl@3)/lib" -lpq -lsecp256k1 -lssl -lcrypto -lcurl -lz -lm)
else
  pg="$(dirname "$(command -v initdb)")"; cc=("${CC:-gcc}")
  read -r -a flags <<< "$(pkg-config --cflags libpq libsecp256k1 openssl libcurl zlib)"
  read -r -a libs <<< "$(pkg-config --libs libpq libsecp256k1 openssl libcurl zlib) -lm"
fi
work="$(mktemp -d /tmp/rg.XXXXXX)"
trap 'kill ${api_pid:-} ${site_pid:-} 2>/dev/null || true; "$pg/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
a="$here"
"$coax" c "$a/anime-director-policy.coax" --library --header "$work/anime-director-policy.generated.h" > "$work/anime-director-policy.generated.c"
"$coax" c "$a/seat-policy.coax" --library --header "$work/seat-policy.generated.h" > "$work/seat-policy.generated.c"
"${cc[@]}" -std=c11 -O1 -w -D_DEFAULT_SOURCE -I"$work" "${flags[@]}" -o "$work/api" \
  "$a"/api.c "$a"/analytics.c "$a"/admin.c "$a"/player.c "$a"/npc.c "$a"/room.c "$a"/private.c "$a"/drops.c "$a"/villa.c "$a"/chat.c "$a"/realms.c \
  "$a"/access.c "$a"/replies.c "$a"/makers.c "$a"/inference.c "$a"/mesh.c "$a"/mesh-policy.generated.c "$a"/caller-policy.generated.c "$a"/nip98.generated.c \
  "$a"/anime.c "$work"/anime-director-policy.generated.c "$a"/seats.c "$work"/seat-policy.generated.c "$a"/nostr.c "$a"/email.c "$a"/jobs.c "$a"/relay.c "${libs[@]}"
cli="$(COAX="$coax" "$repo/apps/openagents-cli/build.sh" | tail -1)"
"$pg/initdb" -D "$work/db" -U postgres -A trust >/dev/null
port=$((56000 + RANDOM % 1000)); api_port=$((57000 + RANDOM % 1000))
"$pg/pg_ctl" -D "$work/db" -o "-p $port -k $work -c listen_addresses=''" -l "$work/pg.log" -w start >/dev/null
"$pg/psql" -h "$work" -p $port -U postgres -q -c "CREATE ROLE oa_analytics LOGIN" -c "CREATE DATABASE oa_analytics OWNER oa_analytics"
db="host=$work port=$port user=oa_analytics dbname=oa_analytics"
OA_API_DATABASE="$db" OA_API_ORIGIN="http://127.0.0.1:$api_port" OA_ENGINE_TEMPLATE="$here/tests/engine-template.html" \
  "$work/api" --listen 127.0.0.1:$api_port --migrations "$here/migrations" 2>"$work/api.log" & api_pid=$!
for _ in $(seq 100); do curl -s "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null && break; sleep 0.2; done
cd "$repo"
export PGURL="$db" PSQL="$pg/psql" COAX="$coax"
if [[ -z "${REALM_GATE_PAGE_ONLY:-}" ]]; then
  for f in realm_engine_pin realm_shapes realm_edit_log; do
    PIN_SQL="coax/web/api/tests/$f.sql" coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax | tail -1
  done
  mkdir -p "$work/walk"
  WORK="$work/walk" coax/web/tool.sh coax/web/api/tests/realm_walkthrough.coax | tail -1
  CLI="$cli" API="http://127.0.0.1:$api_port" PINNED=1 coax/web/tool.sh coax/web/api/tests/realm_cli_api.coax | tail -1
fi
if [[ "${1:-}" == --page ]]; then
  [[ -s apps/openagents-web/build/myrealm.wasm ]] || { echo "realm-gate: build apps/openagents-web/build/myrealm.wasm first" >&2; exit 2; }
  site_port=$((58000 + RANDOM % 1000))
  coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:$site_port >"$work/site.log" 2>&1 & site_pid=$!
  for _ in $(seq 50); do curl -s "http://127.0.0.1:$site_port/worlds/built" >/dev/null && break; sleep 0.2; done
  curl -sf "http://127.0.0.1:$site_port/worlds/built" >/dev/null || { echo "realm-gate: the site server didn't answer:" >&2; cat "$work/site.log" >&2; exit 1; }
  SITE="http://127.0.0.1:$site_port" API="http://127.0.0.1:$api_port" coax/web/tool.sh forge-access "${@:2}"
fi
echo "realm-gate: passed"
