#!/usr/bin/env bash
# The API service's checks: builds api, starts a throwaway Postgres 16 and the
# service on it, and runs the checks in test.py, tests/*.coax and the rest
# against both (migrations, health, ingest, admin, auth, rooms).
#   coax/web/api/test.sh                    # every check
#   coax/remote/remote.sh run -- coax/web/api/test.sh   # the same, on a build host (use this)
#   coax/web/api/test.sh --part I/N         # the I-th of N parts (each its own API and Postgres)
#   coax/web/api/test.sh --serve ORIGIN   for a page under development: the same build, database and
#                           service, left running for the dev server at ORIGIN to send /api/ to
#                           (coax/web/serve.sh APP --listen HOST:PORT --api API_PORT), with no checks run.
#                           It prints the service's port and a link that logs a throwaway admin in.
#                           OA_SERVE_INFERENCE_TOKEN, when set, is a Google access token: /api/v1/inference
#                           and /api/chat/complete then ask Vertex itself, not the checks' fake model.
#                           Everything is gone when it is stopped.
# On macOS it takes Apple's clang and Homebrew's postgresql@16, secp256k1 and
# openssl@3; on a Linux build host the system's clang (else gcc, or CC),
# libpq, libsecp256k1, OpenSSL, curl and zlib, and PGDG's postgresql-16
# (coax/remote/host-startup.sh installs them). Each run makes its own
# Postgres cluster in a temporary directory, listening only on a socket
# there, and removes it at exit; it never uses a shared server.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
serve=0; serve_origin=""
if [[ "${1:-}" == --serve ]]; then serve=1; serve_origin="${2:?test.sh --serve ORIGIN, for example http://127.0.0.1:9471}"; fi
began=$SECONDS
# --part I/N: every part builds the API and starts its own Postgres and server,
# and runs its share of the checks below. A check's share is its time on a
# build host (oa-build-1, 2026-11-11, seconds): longest first, each to the part
# with the least so far, so the parts take about as long as each other.
API_PARTS="checks:20 py:721 room_checks:25 checkpoint2_checks:4 verse_packs:3 auth_checks:4 auth_recover_checks:9 agent_checks:8
  auth_life_checks:8 passkey_checks:8 lnaddr_checks:11 auth_device_checks:5 wallet_checks:6 cli-auth:17 anime:13 realms:8 audit:1"
part=""
if [[ "${1:-}" == --part ]]; then part="${2:-}"; shift 2 || true; fi
if [[ -n "$part" ]]; then
  [[ "$part" =~ ^([1-9][0-9]*)/([1-9][0-9]*)$ ]] && (( BASH_REMATCH[1] <= BASH_REMATCH[2] )) || { echo "test.sh: --part I/N, 1 <= I <= N" >&2; exit 2; }
  mine=" $(printf '%s\n' $API_PARTS | sort -t: -k2,2nr -k1,1 | awk -F: -v i="${BASH_REMATCH[1]}" -v n="${BASH_REMATCH[2]}" '
    { b = 1; for (k = 2; k <= n; k++) if (load[k] < load[b]) b = k; load[b] += $2; if (b == i) printf "%s ", $1 }') "
  echo "api: part $part: $mine" >&2
fi
runs() { [[ -z "$part" || "$mine" == *" $1 "* ]]; }
lap() { echo "api: $1 after $((SECONDS - began)) s" >&2; }
if [[ "$(uname -s)" == Darwin ]]; then
  pgbin=/opt/homebrew/opt/postgresql@16/bin
  cc=/usr/bin/clang
  pq_inc=(-I/opt/homebrew/opt/postgresql@16/include); pq_lib=(-L/opt/homebrew/opt/postgresql@16/lib)
  secp="$(brew --prefix secp256k1 2>/dev/null || true)"
  secp_inc=(${secp:+-I"$secp/include"}); secp_lib=(${secp:+-L"$secp/lib"})
  ssl="$(brew --prefix openssl@3)"
  ssl_inc=(-I"$ssl/include"); ssl_lib=(-L"$ssl/lib")
else
  # PGDG's postgresql-16 (Debian 12 ships 15), else the newest installed.
  pgbin=/usr/lib/postgresql/16/bin
  [[ -x "$pgbin/initdb" ]] || pgbin="$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -n 1)"
  [[ -x "$pgbin/initdb" ]] || { echo "test.sh: no Postgres server (initdb) in /usr/lib/postgresql; coax/remote/host-startup.sh installs postgresql-16" >&2; exit 1; }
  cc="${CC:-}"; [[ -n "$cc" ]] || { command -v clang > /dev/null && cc=clang || cc=gcc; }
  pq_inc=(-I"$(pg_config --includedir 2>/dev/null || echo /usr/include/postgresql)"); pq_lib=()
  secp_inc=(); secp_lib=(); ssl_inc=(); ssl_lib=()
  missing=""
  for h in secp256k1_schnorrsig.h curl/curl.h zlib.h openssl/ssl.h libpq-fe.h; do
    printf '#include <%s>\n' "$h" | "$cc" "${pq_inc[@]}" -E -x c - > /dev/null 2>&1 || missing+=" $h"
  done
  [[ -z "$missing" ]] || { echo "test.sh: no$missing; coax/remote/host-startup.sh installs libsecp256k1-dev libcurl4-openssl-dev zlib1g-dev libssl-dev libpq-dev" >&2; exit 1; }
  # As on macOS, where AddressSanitizer looks for no leaks: the programs keep
  # their contexts until exit (sign.c's secp256k1 context).
  export ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=0}"
fi
if [[ $serve == 0 ]] && runs checks; then
"$here/mesh-policy.sh" --check-inputs
"$here/caller-policy.sh" --check-inputs
"$here/nip98.sh" --check-inputs
fi
# NIP-98 is Coax (nip98.coax): with a compiler here, its checks and its C cache.
repo_root="$(cd "$here/../../.." && pwd)"
cli_coax="${COAX:-$repo_root/coax/bin/coax}"
if [[ ! -x "$cli_coax" ]]; then
  common="$(git -C "$here" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
  [[ -n "$common" ]] && cli_coax="$(dirname "$common")/coax/bin/coax"
fi
if [[ -x "$cli_coax" && $serve == 0 ]] && runs checks; then
  "$here/nip98.sh" --check || echo 'nip98: the C cache differs from this compiler'"'"'s output (the image builds from source; regenerate with nip98.sh --generate)' >&2
  (cd "$repo_root" && "$cli_coax" test coax/web/api/nip98.coax --run-steps 250000000 | tail -1)
  (cd "$repo_root" && "$cli_coax" test coax/web/api/nip98_checks.coax --steps 4000000000 --run-steps 250000000 | tail -1)
fi
if [[ $serve == 0 ]] && runs checks; then
"$here/test-deploy-command.sh"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_verify_test.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_rollout_test.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_snapshot_test.py"
lap "the policy, NIP-98 and deploy checks"
fi
work="$(mktemp -d)"; trap 'kill ${api_pid:-} ${notice_relay_pid:-} ${runner_pid:-} 2>/dev/null || true; "$pgbin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
"$cli_coax" c "$here/anime-director-policy.coax" --library --header "$work/anime-director-policy.generated.h" > "$work/anime-director-policy.generated.c"
"$cli_coax" c "$here/seat-policy.coax" --library --header "$work/seat-policy.generated.h" > "$work/seat-policy.generated.c"
"$cli_coax" c "$here/progress-policy.coax" --library --header "$work/progress-policy.generated.h" > "$work/progress-policy.generated.c"
"$cli_coax" c "$here/item-policy.coax" --library --header "$work/item-policy.generated.h" > "$work/item-policy.generated.c"
"$cli_coax" c "$here/worldbuilder-policy.coax" --library --header "$work/worldbuilder-policy.generated.h" > "$work/worldbuilder-policy.generated.c"
"$cli_coax" c "$here/npc-personas.coax" --run-steps 250000000 --library --header "$work/npc-personas.generated.h" > "$work/npc-personas.generated.c"
"$cli_coax" c "$here/auth-policy.coax" --library --header "$work/auth-policy.generated.h" > "$work/auth-policy.generated.c"
"$cli_coax" c "$here/auth-seal.coax" --run-steps 250000000 --library --header "$work/auth-seal.generated.h" > "$work/auth-seal.generated.c"
"$cli_coax" c "$here/agent-wire.coax" --run-steps 250000000 --library --header "$work/agent-wire.generated.h" > "$work/agent-wire.generated.c"
"$cli_coax" c "$here/agent-attest.coax" --run-steps 250000000 --library --header "$work/agent-attest.generated.h" > "$work/agent-attest.generated.c"
"$cli_coax" c "$here/auth-recover-policy.coax" --library --header "$work/auth-recover-policy.generated.h" > "$work/auth-recover-policy.generated.c"
"$cli_coax" c "$here/auth-life-policy.coax" --library --header "$work/auth-life-policy.generated.h" > "$work/auth-life-policy.generated.c"
"$cli_coax" c "$here/auth-recover-crypto.coax" --run-steps 250000000 --library --header "$work/auth-recover-crypto.generated.h" > "$work/auth-recover-crypto.generated.c"
"$cli_coax" c "$here/passkey-policy.coax" --library --header "$work/passkey-policy.generated.h" > "$work/passkey-policy.generated.c"
"$cli_coax" c "$here/passkey-verify.coax" --run-steps 250000000 --library --header "$work/passkey-verify.generated.h" > "$work/passkey-verify.generated.c"
"$cli_coax" c "$repo_root/pay/lnaddr-policy.coax" --run-steps 250000000 --library --header "$work/lnaddr-policy.generated.h" > "$work/lnaddr-policy.generated.c"
"$cli_coax" c "$here/auth-device-policy.coax" --library --header "$work/auth-device-policy.generated.h" > "$work/auth-device-policy.generated.c"
"$cli_coax" c "$here/auth-device-crypto.coax" --run-steps 250000000 --library --header "$work/auth-device-crypto.generated.h" > "$work/auth-device-crypto.generated.c"
"$cli_coax" c "$repo_root/pay/wallet-policy.coax" --library --header "$work/wallet-policy.generated.h" > "$work/wallet-policy.generated.c"
"$cli_coax" c "$repo_root/pay/wallet-vault-store.coax" --run-steps 250000000 --library --header "$work/wallet-vault.generated.h" > "$work/wallet-vault.generated.c"
"$cli_coax" c "$here/ticket-sign.coax" --run-steps 250000000 --library --header "$work/ticket-sign.generated.h" > "$work/ticket-sign.generated.c"
# OA_LNADDR_TEST_FETCH: this build's Lightning Address fetches go to the fake services on SPARK_PORT
# (test_makers_tips.py); the image is built without it and fetches only public https hosts.
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -DOA_LNADDR_TEST_FETCH -fsanitize=address,undefined \
  -I"$work" ${pq_inc[@]+"${pq_inc[@]}"} ${secp_inc[@]+"${secp_inc[@]}"} ${ssl_inc[@]+"${ssl_inc[@]}"} \
  -o "$work/api" "$here"/api.c "$here"/analytics.c "$here"/admin.c "$here"/player.c "$here"/rooms.c "$here"/npc.c "$work"/npc-personas.generated.c "$here"/room.c "$here"/private.c "$here"/drops.c "$here"/villa.c "$here"/chat.c "$here"/realms.c "$here"/access.c "$here"/makers.c "$here"/inference.c "$here"/mesh.c "$here"/mesh-policy.generated.c "$here"/caller-policy.generated.c "$here"/nip98.generated.c "$here"/anime.c "$work"/anime-director-policy.generated.c "$here"/seats.c "$work"/seat-policy.generated.c "$work"/progress-policy.generated.c "$work"/item-policy.generated.c "$here"/nostr.c "$here"/email.c "$here"/jobs.c "$here"/relay.c "$here"/verse_packs.c "$here"/worldbuilder.c "$work"/worldbuilder-policy.generated.c "$here"/auth.c "$work"/auth-policy.generated.c "$here"/auth-seal-unit.c "$here"/lnaddr.c "$here"/lnaddr-unit.c "$here"/agents.c "$here"/agent-wire-unit.c "$here"/agent-attest-unit.c "$here"/auth-recover.c "$work"/auth-recover-policy.generated.c "$here"/auth-life.c "$work"/auth-life-policy.generated.c "$here"/auth-recover-crypto-unit.c "$here"/auth-device.c "$work"/auth-device-policy.generated.c "$here"/auth-device-crypto-unit.c "$here"/passkeys.c "$work"/passkey-policy.generated.c "$here"/passkey-verify-unit.c "$here"/wallet.c "$work"/wallet-policy.generated.c "$here"/wallet-vault-unit.c "$here"/ticket-sign-unit.c \
  ${pq_lib[@]+"${pq_lib[@]}"} -lpq ${secp_lib[@]+"${secp_lib[@]}"} -lsecp256k1 ${ssl_lib[@]+"${ssl_lib[@]}"} -lssl -lcrypto -lcurl -lz -lm
if runs checks; then
# The gate's table (auth.c): an unknown route has no rule, and /docs/limits is the API's own limits table.
[[ "$("$work/api" --rule GET /api/no-such-route || true)" == none && "$("$work/api" --rule POST /api/auth/logout-everywhere)" == L2 ]] || { echo "api --rule: the gate's table is wrong" >&2; exit 1; }
"$work/api" --limits | diff - "$repo_root/apps/openagents-web/docs/limits/index.md" > /dev/null || { echo "apps/openagents-web/docs/limits/index.md is stale: run api --limits > it" >&2; exit 1; }
# The Coax NIP-98 check against the former C check, on the corpus and seeded mutations.
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  ${secp_inc[@]+"${secp_inc[@]}"} ${ssl_inc[@]+"${ssl_inc[@]}"} -o "$work/nip98_diff" \
  "$here/nip98_diff.c" "$here/nip98_reference.c" "$here/nostr.c" "$here/nip98.generated.c" \
  ${secp_lib[@]+"${secp_lib[@]}"} -lsecp256k1 ${ssl_lib[@]+"${ssl_lib[@]}"} -lcrypto
[[ $serve == 1 ]] || "$work/nip98_diff" "$here/nip98_corpus.tsv" 400
fi
lap "the API built (and in the checks' part, its NIP-98 check)"
"$pgbin/initdb" -D "$work/db" -U postgres -A trust >/dev/null
# A free TCP port in [LOW, LOW + 1000) that this run hasn't taken. Agents run these checks side by
# side, and a port another run's server holds would send this run's requests to that server.
taken=" "
free_port() {
  local p
  for _ in $(seq 200); do
    p=$(($1 + RANDOM % 1000))
    [[ "$taken" == *" $p "* ]] && continue
    python3 -I -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', $p)); s.close()" 2>/dev/null || continue
    echo "$p"; return 0
  done
  echo "test.sh: no free port from $1" >&2; return 1
}
port=$((56000 + RANDOM % 1000))   # the database's: its socket is in $work, so no other run shares it
api_port=$(free_port 57000); taken+="$api_port "
"$pgbin/pg_ctl" -D "$work/db" -o "-p $port -k $work -c listen_addresses=''" -l "$work/pg.log" -w start >/dev/null
"$pgbin/psql" -h "$work" -p $port -U postgres -q -c "CREATE ROLE oa_analytics LOGIN" -c "CREATE DATABASE oa_analytics OWNER oa_analytics"
export OA_API_DATABASE="host=$work port=$port user=oa_analytics dbname=oa_analytics"
# The signer (sign.c), and two throwaway keys: an admin and someone else.
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  ${secp_inc[@]+"${secp_inc[@]}"} ${ssl_inc[@]+"${ssl_inc[@]}"} -o "$work/sign" "$here/sign.c" \
  ${secp_lib[@]+"${secp_lib[@]}"} -lsecp256k1 ${ssl_lib[@]+"${ssl_lib[@]}"} -lcrypto
# The project agent's runner (coax/web/agent/runner.c), run by test_private_runner.py with a stand-in engine.
# It links runner-sign.coax's C, compiled here as deploy.sh compiles it. The generated
# signer sets a value it never reads (schnorr_sign.coax's ss_write result), so that
# warning stays a warning for that file only.
"$cli_coax" c "$here/../agent/runner-sign.coax" --library --run-steps 250000000 --header "$work/runner-sign.h" > "$work/runner-sign.c"
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -c -o "$work/runner-sign.o" "$work/runner-sign.c"
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$work" ${secp_inc[@]+"${secp_inc[@]}"} ${ssl_inc[@]+"${ssl_inc[@]}"} -o "$work/oa-agent" "$here/../agent/runner.c" "$work/runner-sign.o" \
  ${secp_lib[@]+"${secp_lib[@]}"} -lsecp256k1 ${ssl_lib[@]+"${ssl_lib[@]}"} -lcrypto -lcurl -lm
export RUNNER_BIN="$work/oa-agent" RUNNER_BIN_SRC="$here/../agent/runner.c"
export SIGN="$work/sign" ADMIN_KEY="$work/admin.key" OTHER_KEY="$work/other.key"
export OA_ADMIN_PUBKEYS="$("$SIGN" --new "$ADMIN_KEY")"
# The rooms' signed routes (seat tickets, vitals reports, room-control): a local key unless the caller
# set one, so tests/room_checks.coax runs its keyed checks (admission, invites, stats, vitals) too.
export ROOM_KEY="${ROOM_KEY:-local-development-room-ticket-key-not-a-secret}"
export OA_ROOM_TICKET_KEY="${OA_ROOM_TICKET_KEY:-$ROOM_KEY}"
# Shards' own write keys (#13260, room-write.coax): a throwaway master, room-write.coax's RW_TEST_MASTER.
export OA_ROOM_WRITE_MASTER="${OA_ROOM_WRITE_MASTER:-0303030303030303030303030303030303030303030303030303030303030303}"
# Our team (test_team.py): a web player, an old playtest player and an email's hash (team@example.com).
export OA_TEAM="player:p-7ea0000000000001,player:kiki-7ea0000000000002,email_sha256:$(printf '%s' team@example.com | shasum -a 256 | cut -c1-64),player:bad'quote"
export RUN_KEY_FILE="$work/runner.key" OA_AGENT_RUNNER_PUBKEYS="$("$SIGN" --new "$work/runner.key")"
"$SIGN" --new "$OTHER_KEY" >/dev/null
# Character chat asks a fake model (test_npc.py serves it) with a test token.
NPC_PORT=$(free_port 58000); taken+="$NPC_PORT "; export NPC_PORT
# The Sandbox's set pieces ask their own fake model (test_sandbox_pieces.py serves it).
PIECE_PORT=$(free_port 53000); taken+="$PIECE_PORT "; export PIECE_PORT
# Build your own realm asks its own fake model (test_realm_builder.py serves it).
BUILDER_PORT=$(free_port 51000); taken+="$BUILDER_PORT "; export BUILDER_PORT
# Drops go to a fake bucket (test_project_drops.py serves it).
GCS_PORT=$(free_port 55000); taken+="$GCS_PORT "; export GCS_PORT
# Publishing a project from a drop (rc-publish): a fake relay (test_project_drops.py) and the fake bucket.
RELAY_PORT=$(free_port 52000); taken+="$RELAY_PORT "; export RELAY_PORT
# Security notices as NIP-17 messages (auth-recover.c): a fake relay of their own, which keeps
# each event in $work/notices.jsonl (tests/auth_recover_checks.coax).
NOTICE_RELAY_PORT=$(free_port 52500); taken+="$NOTICE_RELAY_PORT "
python3 -I "$here/fake_relay.py" "$NOTICE_RELAY_PORT" "$work/notices.jsonl" 2>"$work/notice-relay.log" & notice_relay_pid=$!
# The project agent: only the fixture project "agent-proj" is approved; wakes go to test_private_agent.py's fake runner.
WAKE_PORT=$(free_port 59000); taken+="$WAKE_PORT "; export WAKE_PORT
# /api/v1/inference (test_inference.py): a fake Vertex on INFER_PORT, a key that may use the route, five requests a day.
INFER_PORT=$(free_port 50000); taken+="$INFER_PORT "; export INFER_PORT INFER_KEY="$work/infer.key"
export OA_INFERENCE_PUBKEYS="$("$SIGN" --new "$INFER_KEY")"
# /api/v1/inference/credit (test_inference.py): a fake Spark wallet on SPARK_PORT.
SPARK_PORT=$(free_port 49000); taken+="$SPARK_PORT "; export SPARK_PORT
# Email (test_email.py): a fake Resend on RESEND_PORT; staging's allowlist is one address.
RESEND_PORT=$(free_port 48000); taken+="$RESEND_PORT "; export RESEND_PORT
# Scheduled jobs (test_jobs.py): a fake Google tokeninfo on TOKENINFO_PORT, and the scheduler's account.
TOKENINFO_PORT=$(free_port 47000); taken+="$TOKENINFO_PORT "; export TOKENINFO_PORT
# /chat (test_zchat.py): its own runner on CHAT_PORT, a $1 daily cap.
CHAT_PORT=$(free_port 54000); taken+="$CHAT_PORT "; export CHAT_PORT
# Private human packs (tests/verse_packs.coax): a made-up pack, gzipped and stored under a key of its own
# as upload_packs.coax does, served by a fake bucket on VERSE_GCS_PORT; three sessions an address an hour.
VERSE_GCS_PORT=$(free_port 45000); taken+="$VERSE_GCS_PORT "; export VERSE_GCS_PORT VERSE_PACK_DIR="$work/verse-pack"
mkdir -p "$VERSE_PACK_DIR"; head -c 300000 /dev/urandom > "$VERSE_PACK_DIR/plain"
gzip -9 -n -c "$VERSE_PACK_DIR/plain" > "$VERSE_PACK_DIR/plain.gz"
vk="$(openssl rand -hex 32)" viv="$(openssl rand -hex 8)0000000000000000" vsha="$(shasum -a 256 "$VERSE_PACK_DIR/plain" | cut -c1-64)"
openssl enc -aes-256-ctr -K "$vk" -iv "$viv" -in "$VERSE_PACK_DIR/plain.gz" -out "$VERSE_PACK_DIR/object"
export OA_VERSE_PACKS="master $(openssl rand -hex 32)
pack test-human $vsha $(wc -c < "$VERSE_PACK_DIR/plain" | tr -d ' ') $(wc -c < "$VERSE_PACK_DIR/object" | tr -d ' ') packs/$vsha $vk $viv"
export OA_VERSE_PACKS_GCS="http://127.0.0.1:$VERSE_GCS_PORT" OA_VERSE_PACKS_BUCKET=verse-test OA_VERSE_PACK_PER_HOUR=3
# Anime's checks own a separate Coax selector; it has no model credentials.
ANIME_PORT=$(free_port 46000); taken+="$ANIME_PORT "; export ANIME_PORT
# The checks ask a fake model on INFER_PORT, five requests a day. Serving a page with a token, the model is Vertex's.
infer_url="http://127.0.0.1:$INFER_PORT/chat/completions" infer_token=test-token infer_daily=5 api_origin="http://127.0.0.1:$api_port"
if [[ $serve == 1 ]]; then
  api_origin="$serve_origin"
  if [[ -n "${OA_SERVE_INFERENCE_TOKEN:-}" ]]; then
    infer_url="https://aiplatform.us.rep.googleapis.com/v1/projects/openagentsgemini/locations/us/endpoints/openapi/chat/completions"
    infer_token="$OA_SERVE_INFERENCE_TOKEN" infer_daily=500
  fi
fi
OA_CHAT_RUNNER_URL="http://127.0.0.1:$CHAT_PORT" OA_CHAT_DAILY_USD=1 \
OA_AGENT_PROJECTS=agent-proj OA_AGENT_WAKE_URL="http://127.0.0.1:$WAKE_PORT/wake" OA_AGENT_WAKE_TOKEN=test-wake \
OA_PROJECT_KEK="$(openssl rand -hex 32)" OA_PUBLISH_RELAY="ws://127.0.0.1:$RELAY_PORT" OA_SITE_BUCKET=site-test \
OA_DROPS_GCS="http://127.0.0.1:$GCS_PORT" OA_DROPS_TOKEN=test-gcs OA_DROPS_BUCKET=drops-test OA_DROPS_PREFIX=test OA_DRAFT_URL="http://127.0.0.1:$GCS_PORT/model" OA_VILLA_URL="http://127.0.0.1:$GCS_PORT/villa-model" \
OA_INFERENCE_URL="$infer_url" OA_INFERENCE_TOKEN="$infer_token" OA_INFERENCE_DAILY_REQUESTS="$infer_daily" \
OA_SPARK_URL="http://127.0.0.1:$SPARK_PORT" OA_SPARK_TOKEN=test-spark OA_LNADDR_TEST_PORT="$SPARK_PORT" \
OA_RESEND_URL="http://127.0.0.1:$RESEND_PORT/emails" OA_RESEND_API_KEY=test-resend OA_EMAIL_ALLOW=device-a@example.com,device-b@example.com,life-a@example.com,life-c@example.com,life-d@example.com,life-f@example.com,life-h@example.com,owner@example.com,ada@example.com,frozen@example.com,app@example.com,rec@example.com,solo@example.com,pk@example.com,newcomer@example.com,wallet@example.com OA_EMAIL_BACKOFF_MS=10 \
OA_AUTH_SEAL_KEY="1:$(openssl rand -hex 32),2:$(openssl rand -hex 32)" OA_AUTH_LOOKUP_PEPPER="1:$(openssl rand -hex 32)" \
OA_AUTH_NOTICE_NSEC="$(openssl rand -hex 32)" OA_AUTH_NOTICE_RELAY="ws://127.0.0.1:$NOTICE_RELAY_PORT" OA_LNADDR_PROFILE_RELAYS="ws://127.0.0.1:$NOTICE_RELAY_PORT" OA_AUTH_NOTICE_SYNC=1 OA_AUTH_SCRYPT_LOG_N=10 \
OA_TOKENINFO_URL="http://127.0.0.1:$TOKENINFO_PORT/tokeninfo" OA_JOBS_CALLER=jobs@example.iam.gserviceaccount.com OA_DEVICE_TTL_MS=4000 \
OA_NPC_URL="http://127.0.0.1:$NPC_PORT/model" OA_NPC_TOKEN=test-token OA_NPC_DAILY_CAP=5 OA_NPC_KEY_10M=3 OA_NPC_ADDR_10M=5 OA_NPC_RUNNING=3 OA_NPC_RUNNING_KEY=1 OA_PIECE_URL="http://127.0.0.1:$PIECE_PORT/model" OA_PIECE_DAILY_CAP=40 OA_EDIT_URL="http://127.0.0.1:$PIECE_PORT/edit-model" OA_EDIT_TIMEOUT_MS=1500 OA_BUILDER_URL="http://127.0.0.1:$BUILDER_PORT/model" OA_BUILDER_TIMEOUT_MS=1200 OA_BUILDER_PER_DAY=6 \
OA_ANIME_URL="http://127.0.0.1:$ANIME_PORT/model" OA_ANIME_TIMEOUT_MS=500 \
  OA_BUILD_SHA=1111111111111111111111111111111111111111 OA_RELEASE_ID=123e4567-e89b-42d3-a456-426614174000 K_SERVICE=oa-api-staging K_REVISION=oa-api-staging-00002-new \
  OA_ROOM_CHECKPOINTS=1 OA_API_ORIGIN="$api_origin" "$work/api" --listen 127.0.0.1:$api_port --migrations "$here/migrations" 2>"$work/api.log" & api_pid=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null && break; sleep 0.1; done
# The answer came from this run's server, not from another run's that took the port first.
kill -0 "$api_pid" 2>/dev/null || { echo "test.sh: the API didn't start" >&2; cat "$work/api.log"; exit 1; }
# The migrations (#13301, migrations/README.md): one number each, none edited once landed, and
# schema.sql the schema the API's own runner just made (dumped before any check writes to it).
if [[ $serve == 0 && -x "$cli_coax" ]] && runs checks; then
  (cd "$repo_root" && COAX="$cli_coax" coax/web/tool.sh coax/web/api/migration-check.coax) || exit 1
  (cd "$repo_root" && COAX="$cli_coax" coax/web/tool.sh coax/web/api/migration-check.coax --schema --pgbin "$pgbin" --host "$work" --port "$port") || exit 1
  lap "the migration checks"
fi
if [[ $serve == 1 ]]; then
  # /chat's Claude turns: the real runner and its real engine (coax/web/agent/chat.coax, the Coax Claude
  # Agent SDK) on CHAT_PORT, with the Claude CLI on this machine, when there is one. A turn runs on the
  # credential the admin saved (the page's Settings); this machine's own Claude login is never used: the
  # runner gives the CLI a fresh configuration directory and only that credential. The copy of the code
  # the agent reads is this repository's main, from disk.
  claude_bin="$(command -v claude || true)"
  if [[ -n "$claude_bin" ]]; then
    "$cli_coax" c "$here/../agent/chat.coax" > "$work/chat-engine.c"
    "$cc" -std=c11 -O1 -g -D_DEFAULT_SOURCE -I"$repo_root/coax/platform" -o "$work/oa-chat-turn" "$work/chat-engine.c" \
      "$repo_root/coax/platform/process-native.c" "$here/../agent/engine-io.c" "$here/../agent/confine-native.c" -lm
    common="$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)"
    # An indexed repo for the agent to read (docs/repos/2026-11-11-indexed-repos.md, #13326): a bare mirror of
    # this repository that borrows the checkout's objects, so it is made at once and takes no disk. With it a
    # turn makes no checkout. OA_SERVE_NO_MIRROR=1 leaves it out, and the agent reads a checkout as before.
    mirror=""
    if [[ -z "${OA_SERVE_NO_MIRROR:-}" ]]; then
      mirror="$work/mirrors/openagents.git"; mkdir -p "$work/mirrors"
      git clone --quiet --bare --shared "$(dirname "$common")" "$mirror"
    fi
    env -i HOME="$HOME" PATH="$(dirname "$claude_bin"):/opt/homebrew/bin:/usr/bin:/bin" \
      OA_API_ORIGIN="$api_origin" OA_RUNNER_KEY="$(cat "$RUN_KEY_FILE")" OA_CHAT_SEAL_KEY="$(openssl rand -hex 32)" \
      OA_CHAT_ENGINE="$work/oa-chat-turn" OA_CLAUDE="$claude_bin" OA_CHAT_REPO="${OA_SERVE_CHAT_REPO:-file://$(dirname "$common")}" \
      OA_CHAT_SESSIONS="$work/sessions" OA_AGENT_WORK="$work/agent" OA_GIT="$(command -v git)" OA_REPO_MIRROR="$mirror" \
      "$RUNNER_BIN" --listen "127.0.0.1:$CHAT_PORT" 2>"$work/runner.log" & runner_pid=$!
    echo "serve: /chat's runner is on 127.0.0.1:$CHAT_PORT with $claude_bin (its log: $work/runner.log)${mirror:+, reading the mirror $mirror}"
  else
    echo "serve: no claude on PATH, so /chat's Claude turns have no runner"
  fi
  login="$("$SIGN" --key "$ADMIN_KEY" GET "$api_origin/api/admin/login")"
  echo "serve: the API is on 127.0.0.1:$api_port (its log: $work/api.log)"
  echo "serve: run the dev server with --api $api_port"
  echo "serve: admin login, good for 60 seconds: $api_origin/api/admin/login?auth=$(python3 -I -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$login")&next=chat"
  echo "serve: a new link later: auth=\$($SIGN --key $ADMIN_KEY GET $api_origin/api/admin/login), then open $api_origin/api/admin/login?auth=<that, URL-encoded>&next=chat"
  wait "$api_pid"
  exit 0
fi
lap "the database, the signer, the runner and the API up"
if runs py; then
API_TEST_TIMES=1 API_LOG="$work/api.log" API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pgbin/psql" python3 -I "$here/test.py" || { cat "$work/api.log"; exit 1; }
lap "test.py"
fi
# The CLI as a caller (NIP-98 callers, realms/mine, visibility, admin keys, device login), in Coax: tests/cli_auth.coax.
# The CLI-caller checks are a Coax program: they need a compiler. A deploy's
# fresh worktree has none of its own, so take the main checkout's.
if [[ -x "$cli_coax" ]]; then
  # The rooms' checks and the private packs' checks (tests/room_checks.coax, tests/verse_packs.coax).
  # The rooms' signed routes run with the key above; with ROOM_BIN (coax/web/room/build.sh) as well,
  # the vitals fault check and the owner and travel checks. API_TESTS_ONLY runs them when it names them (room_checks.coax).
  # API_BIN: the region check starts a second API on a promoted replica of this database.
  coax_failed=0
  for f in room_checks checkpoint2_checks verse_packs auth_checks auth_recover_checks agent_checks auth_life_checks passkey_checks lnaddr_checks auth_device_checks wallet_checks; do
    [[ -z "${API_TESTS_ONLY:-}" || ",$API_TESTS_ONLY," == *",$f.coax,"* ]] || continue
    runs "$f" || continue
    (cd "$repo_root" && API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pgbin/psql" COAX="$cli_coax" API_BIN="$work/api" RESEND_PORT="$RESEND_PORT" \
      coax/web/tool.sh "coax/web/api/tests/$f.coax") || coax_failed=1
    lap "tests/$f.coax"
  done
  [[ $coax_failed == 0 ]] || { cat "$work/api.log"; exit 1; }
  # A bounded character-recovery slice, including optional real native faults.
  # An unfiltered invocation still runs every check below.
  if [[ "${API_TESTS_ONLY:-}" == checkpoint2_checks.coax ]]; then
    echo "api: checkpoint2 checks passed"
    exit 0
  fi
  if runs cli-auth; then
  ADMIN_SECRET_HEX="$(tr -d '\n' < "$ADMIN_KEY")" COAX="$cli_coax" "$here/cli-auth-test.sh" "http://127.0.0.1:$api_port" || { cat "$work/api.log"; exit 1; }
  lap "cli-auth-test.sh"
  fi
  # The shell only compiles platform entry points. Coax owns the selector,
  # all requests/assertions, bounded subprocesses and fixture cleanup.
  if runs anime; then
  p="$repo_root/coax/platform"
  "$cli_coax" c "$repo_root/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
  for unit in anime_direct anime_direct_selector; do
    "$cli_coax" c "$here/tests/$unit.coax" --library --run-steps 250000000 > "$work/$unit.c"
    if [[ "$(uname -s)" == Darwin ]]; then
      "$cc" -std=c11 -O2 -fblocks -D_DARWIN_C_SOURCE -I"$work" -I"$p" -o "$work/$unit" "$work/$unit.c" \
        "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/socket-native.c" "$p/tls-native.c" \
        "$p/entropy-native.c" "$p/entropy-foreign.c" "$work/entropy.c" -framework Network -framework Security -framework CoreFoundation -lm
    else
      # As coax/web/tool.sh links a tool on Linux: OpenSSL for tls-native.c from tls-env.sh.
      # shellcheck source=/dev/null
      source "$p/tls-env.sh"
      "$cc" -std=c11 -O2 -D_DEFAULT_SOURCE -I"$work" -I"$p" -o "$work/$unit" "$work/$unit.c" \
        "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/socket-native.c" "$p/tls-native.c" \
        "$p/entropy-native.c" "$p/entropy-foreign.c" "$work/entropy.c" $CXTLS_CFLAGS $CXTLS_LIBS -lpthread -lm
    fi
  done
  API="http://127.0.0.1:$api_port" ORIGIN="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pgbin/psql" \
    ANIME_ONLY= ANIME_ISOLATED_DB=oa_analytics ANIME_SELECTOR_BIN="$work/anime_direct_selector" "$work/anime_direct" || { cat "$work/api.log"; exit 1; }
  lap "anime_direct"
  fi
  # A made realm's engine pins, shapes and edit log, and Aethyrforge's whole route checked end to end (#12629).
  # (The built CLI against a local API is tests/realm_cli_api.coax; its publish needs an API with no
  # bucket or with OA_ENGINE_TEMPLATE, which this run's fake bucket isn't, so it runs on its own.)
  if runs realms; then
  mkdir -p "$work/walk"
  for f in realm_engine_pin realm_shapes realm_edit_log; do
    (cd "$repo_root" && PGURL="$OA_API_DATABASE" PSQL="$pgbin/psql" PIN_SQL="coax/web/api/tests/$f.sql" COAX="$cli_coax" coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax) || exit 1
  done
  (cd "$repo_root" && PGURL="$OA_API_DATABASE" PSQL="$pgbin/psql" WORK="$work/walk" COAX="$cli_coax" coax/web/tool.sh coax/web/api/tests/realm_walkthrough.coax) || exit 1
  lap "the realm checks"
  fi
else
  echo 'cli-auth: skipped, no Coax compiler (build coax/bin/coax, or set COAX)' >&2
fi
# The audit log is append-only (0144_auth_audit.sql, #13202): with db/grants.sql's grants (its guard
# step aside), the API's role inserts and reads but can't change or delete a row, and a role
# given UPDATE anyway is stopped by the table's trigger.
if runs audit; then
asu() { "$pgbin/psql" -h "$work" -p "$port" -d oa_analytics -XAtq -v ON_ERROR_STOP=1 "$@"; }
asu -U postgres -c "CREATE ROLE oa_api_check; CREATE ROLE oa_api_loose; GRANT oa_api_check, oa_api_loose TO postgres"
sed '/oa_guard.attach/,/\\gexec/d' "$here/db/grants.sql" | asu -U oa_analytics -v runtime=oa_api_check -v readers= -f - > /dev/null
asu -U postgres -c "SET ROLE oa_api_check; SELECT auth.log(NULL, NULL, 'check.append', 'ok', '{}')" > /dev/null || { echo "audit: the API's role can't append" >&2; exit 1; }
if asu -U postgres -c "SET ROLE oa_api_check; UPDATE auth.audit SET result = 'failed'" 2>/dev/null; then echo "audit: the API's role changed a row" >&2; exit 1; fi
if asu -U postgres -c "SET ROLE oa_api_check; DELETE FROM auth.audit" 2>/dev/null; then echo "audit: the API's role deleted a row" >&2; exit 1; fi
asu -U oa_analytics -c "GRANT USAGE ON SCHEMA auth TO oa_api_loose; GRANT SELECT, UPDATE, DELETE ON auth.audit TO oa_api_loose"
if asu -U postgres -c "SET ROLE oa_api_loose; UPDATE auth.audit SET result = 'failed'" 2>/dev/null; then echo "audit: the trigger let an update through" >&2; exit 1; fi
echo "  ok    the audit log is append-only for the API's role, and its trigger stops anyone else"
fi
if grep -q "ERROR: AddressSanitizer\|runtime error" "$work/api.log"; then cat "$work/api.log"; exit 1; fi
echo "api: checks passed in $((SECONDS - began)) s"
