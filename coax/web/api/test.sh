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
#                           The build is the local serve mode's (-DOA_LOCAL_SERVE, never the image's): a
#                           request from this machine with no login is the throwaway admin's, so the page needs
#                           no login step (auth.c local_admin). /api/v1/inference and /api/chat/complete ask
#                           Gemini on Vertex with a token kept fresh in a 0600 file: the service account's
#                           gcloud config (OA_SERVE_GCLOUD_CONFIG, else ../.secrets/gcloud-sa-config beside the
#                           main checkout) when there is one, else your own gcloud login. With neither they ask
#                           openagents.com's /api/v1/inference, signed by your `openagents` login (profile
#                           OA_SERVE_PROFILE, default "default"; the CLI is OA_SERVE_CLI, else
#                           ~/.openagents/bin/openagents). OA_SERVE_MODEL=openagents skips Google;
#                           OA_SERVE_INFERENCE_TOKEN, a Google access token, is used as it is, not refreshed.
#                           OA_SERVE_PORT_FILE: the API's port is written there once it answers (serve.sh).
#                           You are your `openagents` login (dev-identity.sh, #13493: OA_SERVE_PROFILE, OPENAGENTS_PROFILE,
#                           then the profiles default and admin, the first with a key): the page, the CLI and Claude's
#                           realm tools on this computer are one account, the same on every run.
#                           OA_SERVE_STATE=DIR keeps the database, its keys and /chat's sessions in DIR across runs
#                           (serve.sh gives ~/.openagents/dev; --fresh none): a restart opens the same database and
#                           applies the new migrations on top. Without it everything is gone when it is stopped.
#                           The account carries your email (OA_SERVE_EMAIL, else git's user.email): the page names
#                           you by it. Log out sticks in that browser; an email login's code shows in this terminal.
# On macOS it takes Apple's clang and Homebrew's postgresql@16, secp256k1 and
# openssl@3; on a Linux build host the system's clang (else gcc, or CC),
# libpq, libsecp256k1, OpenSSL, curl and zlib, and PGDG's postgresql-16
# (coax/remote/host-startup.sh installs them). Each run makes its own
# Postgres cluster in a temporary directory, listening only on a socket
# there, and removes it at exit; it never uses a shared server. It also removes
# it when stopped (Ctrl-C, kill, a closed terminal), and a run that was killed
# outright (kill -9) has its directory and Postgres removed by the next run.
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
  auth_life_checks:8 passkey_checks:8 lnaddr_checks:11 auth_device_checks:5 wallet_checks:6 realm_doc_checks:3 realm_record_checks:3 world_checks:8 worldbuilder_checks:8 script_checks:8 script_cli_checks:60 workers_checks:95 cli-auth:17 anime:13 realms:8 audit:1"
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
  [[ -x "$pgbin/initdb" ]] || pgbin="$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -n 1 || true)"
  # Elsewhere (NixOS, a Nix shell): the Postgres on the PATH.
  [[ -x "$pgbin/initdb" ]] || { on_path="$(command -v initdb 2>/dev/null || true)"; [[ -z "$on_path" ]] || pgbin="$(dirname "$on_path")"; }
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
# The run's directory: oa-api-test.* in the temp folder, with its owner's pid in owner.pid. A run that was
# killed outright (kill -9) can't clean up, so each run first removes the directories of runs that are gone,
# stopping their Postgres first.
tmp_root="${TMPDIR:-/tmp}"; tmp_root="${tmp_root%/}"
for old in "$tmp_root"/oa-api-test.*; do
  [[ -d "$old" && -f "$old/owner.pid" ]] || continue
  old_pid="$(cat "$old/owner.pid" 2>/dev/null || true)"
  [[ "$old_pid" =~ ^[0-9]+$ ]] && ! kill -0 "$old_pid" 2>/dev/null || continue
  "$pgbin/pg_ctl" -D "$old/db" -m immediate stop >/dev/null 2>&1 || true
  rm -rf -- "${old:?}"
done
work="$(mktemp -d "$tmp_root/oa-api-test.XXXXXXXX")"; printf '%s\n' "$$" > "$work/owner.pid"
# OA_SERVE_STATE (--serve only): the database, its keys and /chat's sessions live there and outlast the run.
# One run at a time uses it; a run that was killed outright left its Postgres running, and this one stops it.
state=""; db="$work/db"
if [[ $serve == 1 && -n "${OA_SERVE_STATE:-}" ]]; then
  state="${OA_SERVE_STATE%/}"; mkdir -p "$state"; chmod 700 "$state"; db="$state/db"
  other="$(cat "$state/owner.pid" 2>/dev/null || true)"
  if [[ "$other" =~ ^[0-9]+$ && "$other" != "$$" ]] && kill -0 "$other" 2>/dev/null; then
    echo "test.sh: another dev server (pid $other) has $state open: stop it first, or serve with --fresh (OA_SERVE_FRESH=1) for a throwaway database" >&2
    rm -rf -- "${work:?}"; exit 1
  fi
  "$pgbin/pg_ctl" -D "$db" -m fast stop >/dev/null 2>&1 || true
  printf '%s\n' "$$" > "$state/owner.pid"
fi
cleanup() {
  kill ${api_pid:-} ${codes_pid:-} ${notice_relay_pid:-} ${runner_pid:-} ${ports_pid:-} ${token_pid:-} ${script_job_pid:-} 2>/dev/null || true
  if [[ -n "$state" ]]; then
    "$pgbin/pg_ctl" -D "$db" -m fast stop >/dev/null 2>&1 || true
    [[ "$(cat "$state/owner.pid" 2>/dev/null)" != "$$" ]] || rm -f "$state/owner.pid"
  else
    "$pgbin/pg_ctl" -D "$db" -m immediate stop >/dev/null 2>&1 || true
  fi
  rm -rf -- "${work:?}"
}
# A stop (Ctrl-C, kill, a closed terminal) exits, and the exit cleans up.
trap cleanup EXIT; trap 'exit 130' INT; trap 'exit 143' TERM; trap 'exit 129' HUP
# The binary's C files and Coax units are api-sources.sh's, the one list every local build reads (#13380).
source "$here/api-sources.sh"
api_sources "$here" "$work" "$cli_coax"
if [[ $serve == 0 ]] && runs checks; then
# The image keeps its own list: the Dockerfile's C files and deploy.sh's Coax units must be this one's.
diff <(printf '%s\n' "${API_SOURCES[@]##*/}" | sort) <(grep -o '/src/[A-Za-z0-9_.-]*\.c' "$here/Dockerfile" | sed 's|/src/||' | sort) ||
  { echo 'api: the Dockerfile and api-sources.sh name different files' >&2; exit 1; }
for u in $API_COAX $API_COAX_WRAPPED; do
  grep -E '^for m in ' "$here/deploy.sh" | grep -q -- "[ :]${u##*:}[ ;]" || { echo "api: deploy.sh makes no C for ${u##*:} (api-sources.sh)" >&2; exit 1; }
done
fi
# OA_LNADDR_TEST_FETCH: this build's Lightning Address fetches go to the fake services on SPARK_PORT
# (test_makers_tips.py); the image is built without it and fetches only public https hosts.
# --serve: the local serve mode (OA_LOCAL_SERVE: the local admin, the model's token file and fallback); the
# checks' build and the image never have it, and test_zchat.py checks that this build refuses the local admin.
local_serve=(); [[ $serve == 0 ]] || local_serve=(-DOA_LOCAL_SERVE)
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -DOA_LNADDR_TEST_FETCH ${local_serve[@]+"${local_serve[@]}"} -fsanitize=address,undefined \
  -I"$work" ${pq_inc[@]+"${pq_inc[@]}"} ${secp_inc[@]+"${secp_inc[@]}"} ${ssl_inc[@]+"${ssl_inc[@]}"} \
  -o "$work/api" "${API_SOURCES[@]}" \
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
if [[ -n "$state" && -f "$db/PG_VERSION" ]]; then
  [[ "$(cat "$db/PG_VERSION")" == "$("$pgbin/postgres" --version | sed -E 's/.* ([0-9]+)\..*/\1/')" ]] ||
    { echo "test.sh: $db was made by Postgres $(cat "$db/PG_VERSION"), not $pgbin's: move it aside, or serve with --fresh" >&2; exit 1; }
else
  "$pgbin/initdb" -D "$db" -U postgres -A trust >/dev/null
fi
# The run's TCP ports (port_locks.coax, #13331): one block for each service below, free when taken
# and locked for this run until it ends, so neither another run beside it nor an outgoing connection
# (the ranges are under the kernels' ephemeral ports) takes one before its server binds it. WAKE's
# block is 8 wide (test_private_runner.py's runner on WAKE_PORT + 1, test_room.py's on + 7), CHAT's 3
# (test_zchat.py's fake on CHAT_PORT + 1 and runner on + 2).
"$here/../tool.sh" "$here/port_locks.coax" "$work/ports" $$ API_PORT:7000:1 NPC_PORT:7800:1 PIECE_PORT:8600:1 BUILDER_PORT:9400:1 \
  GCS_PORT:10200:1 RELAY_PORT:11000:1 NOTICE_RELAY_PORT:11800:1 WAKE_PORT:12600:8 INFER_PORT:13400:1 SPARK_PORT:14200:1 \
  RESEND_PORT:15000:1 TOKENINFO_PORT:15800:1 CHAT_PORT:16600:3 METAVERSE_GCS_PORT:17400:1 ANIME_PORT:18200:1 SCRIPT_JOB_PORT:19000:1 WORLDBUILDER_PORT:19800:1 \
  > /dev/null 2> "$work/ports.err" & ports_pid=$!
for _ in $(seq 1200); do [[ -s "$work/ports" ]] && break; kill -0 "$ports_pid" 2>/dev/null || break; sleep 0.1; done
[[ -s "$work/ports" ]] || { echo "test.sh: no ports for the fake services" >&2; cat "$work/ports.err" >&2; exit 1; }
while IFS='=' read -r name value; do printf -v "$name" '%s' "$value"; done < "$work/ports"
port=$((56000 + RANDOM % 1000))   # the database's: its socket is in $work, so no other run shares it
api_port=$API_PORT
"$pgbin/pg_ctl" -D "$db" -o "-p $port -k $work -c listen_addresses=''" -l "$work/pg.log" -w start >/dev/null
if [[ -z "$state" || "$("$pgbin/psql" -h "$work" -p $port -U postgres -XAtqc "SELECT 1 FROM pg_roles WHERE rolname = 'oa_analytics'")" != 1 ]]; then
  "$pgbin/psql" -h "$work" -p $port -U postgres -q -c "CREATE ROLE oa_analytics LOGIN" -c "CREATE DATABASE oa_analytics OWNER oa_analytics"
fi
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
# And runner-wire.coax's C, the lines and bodies (Protocol C5); a generated parameter it leaves unused
# stays a warning for that file only.
"$cli_coax" c "$here/../agent/runner-wire.coax" --library --header "$work/runner-wire.h" > "$work/runner-wire.c"
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-parameter -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -c -o "$work/runner-wire.o" "$work/runner-wire.c"
# And seat-runner-wire.coax's C, a hosted seat's bodies as the runner reads and writes them (Protocol E11).
"$cli_coax" c "$here/../agent/seat-runner-wire.coax" --library --header "$work/seat-runner-wire.h" > "$work/seat-runner-wire.c"
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-parameter -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -c -o "$work/seat-runner-wire.o" "$work/seat-runner-wire.c"
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$work" ${secp_inc[@]+"${secp_inc[@]}"} ${ssl_inc[@]+"${ssl_inc[@]}"} -o "$work/oa-agent" "$here/../agent/runner.c" "$work/runner-sign.o" "$work/runner-wire.o" "$work/seat-runner-wire.o" \
  ${secp_lib[@]+"${secp_lib[@]}"} -lsecp256k1 ${ssl_lib[@]+"${ssl_lib[@]}"} -lcrypto -lcurl -lm
# The runner's credential sealing with a fixed TEST key and nonce (runner_seal_check.c), for test_zchat.py.
"$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$work" -I"$here/../agent" ${secp_inc[@]+"${secp_inc[@]}"} ${ssl_inc[@]+"${ssl_inc[@]}"} -o "$work/runner-seal-check" "$here/../agent/runner_seal_check.c" "$work/runner-sign.o" "$work/runner-wire.o" "$work/seat-runner-wire.o" \
  ${secp_lib[@]+"${secp_lib[@]}"} -lsecp256k1 ${ssl_lib[@]+"${ssl_lib[@]}"} -lcrypto -lcurl -lm
export RUNNER_BIN="$work/oa-agent" RUNNER_BIN_SRC="$here/../agent/runner.c" SEAL_CHECK_BIN="$work/runner-seal-check"
export SIGN="$work/sign" ADMIN_KEY="$work/admin.key" OTHER_KEY="$work/other.key"
export OA_ADMIN_PUBKEYS="$("$SIGN" --new "$ADMIN_KEY")"
# The rooms' signed routes (seat tickets, vitals reports, room-control): a local key unless the caller
# set one, so tests/room_checks.coax runs its keyed checks (admission, invites, stats, vitals) too.
export ROOM_KEY="${ROOM_KEY:-local-development-room-ticket-key-not-a-secret}"
export OA_ROOM_TICKET_KEY="${OA_ROOM_TICKET_KEY:-$ROOM_KEY}"
# Every answer names its route function (X-OA-Route; api.c), so routes-goldens.coax can hold each
# route file's bodies to that file's messages. The checks only: production never sets it.
export OA_API_ROUTE_HEADER=1
export API_CAPTURE="${API_CAPTURE:-$work/capture}"; mkdir -p "$API_CAPTURE"
# Shards' own write keys (#13260, room-write.coax): a throwaway master, room-write.coax's RW_TEST_MASTER.
export OA_ROOM_WRITE_MASTER="${OA_ROOM_WRITE_MASTER:-0303030303030303030303030303030303030303030303030303030303030303}"
# Our team (test_team.py): a web player, an old playtest player and an email's hash (team@example.com).
export OA_TEAM="player:p-7ea0000000000001,player:kiki-7ea0000000000002,email_sha256:$(printf '%s' team@example.com | shasum -a 256 | cut -c1-64),player:bad'quote"
export RUN_KEY_FILE="$work/runner.key" OA_AGENT_RUNNER_PUBKEYS="$("$SIGN" --new "$work/runner.key")"
"$SIGN" --new "$OTHER_KEY" >/dev/null
# Character chat asks a fake model (test_npc.py serves it) with a test token.
export NPC_PORT
# The Sandbox's set pieces ask their own fake model (test_sandbox_pieces.py serves it).
export PIECE_PORT
# Build your own realm asks its own fake model (test_realm_builder.py serves it).
export BUILDER_PORT
# WorldBuilder asks its own fake model (tests/worldbuilder_checks.coax serves it).
export WORLDBUILDER_PORT
# Drops go to a fake bucket (test_project_drops.py serves it).
export GCS_PORT
# Publishing a project from a drop (rc-publish): a fake relay (test_project_drops.py) and the fake bucket.
export RELAY_PORT
# Security notices as NIP-17 messages (auth-recover.c): a fake relay of their own, which keeps
# each event in $work/notices.jsonl (tests/auth_recover_checks.coax).
python3 -I "$here/fake_relay.py" "$NOTICE_RELAY_PORT" "$work/notices.jsonl" 2>"$work/notice-relay.log" & notice_relay_pid=$!
# The project agent: only the fixture project "agent-proj" is approved; wakes go to test_private_agent.py's fake runner.
export WAKE_PORT
# /api/v1/inference (test_inference.py): a fake Vertex on INFER_PORT, a key that may use the route, five requests a day.
export INFER_PORT INFER_KEY="$work/infer.key"
export OA_INFERENCE_PUBKEYS="$("$SIGN" --new "$INFER_KEY")"
# /api/v1/inference/credit (test_inference.py): a fake Spark wallet on SPARK_PORT.
export SPARK_PORT
# Email (test_email.py): a fake Resend on RESEND_PORT; staging's allowlist is one address.
# Security notices as push (push.c, #13233): Apple's two services and Google's are the fake Resend too (the auth
# checks' own keeps each request's line and headers), with a push key made here.
export RESEND_PORT
# Scheduled jobs (test_jobs.py): a fake Google tokeninfo on TOKENINFO_PORT, and the scheduler's account.
export TOKENINFO_PORT
# /chat (test_zchat.py): its own runner on CHAT_PORT, a $1 daily cap.
export CHAT_PORT
# Private human packs (tests/verse_packs.coax): a made-up pack, gzipped and stored under a key of its own
# as upload_packs.coax does, served by a fake bucket on METAVERSE_GCS_PORT; three sessions an address an hour.
export METAVERSE_GCS_PORT METAVERSE_PACK_DIR="$work/verse-pack"
mkdir -p "$METAVERSE_PACK_DIR"; head -c 300000 /dev/urandom > "$METAVERSE_PACK_DIR/plain"
gzip -9 -n -c "$METAVERSE_PACK_DIR/plain" > "$METAVERSE_PACK_DIR/plain.gz"
vk="$(openssl rand -hex 32)" viv="$(openssl rand -hex 8)0000000000000000" vsha="$(shasum -a 256 "$METAVERSE_PACK_DIR/plain" | cut -c1-64)"
openssl enc -aes-256-ctr -K "$vk" -iv "$viv" -in "$METAVERSE_PACK_DIR/plain.gz" -out "$METAVERSE_PACK_DIR/object"
export OA_METAVERSE_PACKS="master $(openssl rand -hex 32)
pack test-human $vsha $(wc -c < "$METAVERSE_PACK_DIR/plain" | tr -d ' ') $(wc -c < "$METAVERSE_PACK_DIR/object" | tr -d ' ') packs/$vsha $vk $viv"
export OA_METAVERSE_PACKS_GCS="http://127.0.0.1:$METAVERSE_GCS_PORT" OA_METAVERSE_PACKS_BUCKET=verse-test
export OA_VERSE_PACK_PER_HOUR=3   # by its old name on purpose: the API still reads it (the names before #13405)
# Anime's checks own a separate Coax selector; it has no model credentials.
export ANIME_PORT
# The checks ask a fake model on INFER_PORT, five requests a day. Serving a page with a token, the model is Vertex's.
infer_url="http://127.0.0.1:$INFER_PORT/chat/completions" infer_token=test-token infer_daily=5 api_origin="http://127.0.0.1:$api_port"
# OA_LOCAL_ADMIN names the admin a request from this machine with no login is, in the local serve mode only;
# the checks set it too, so test_zchat.py shows a build without OA_LOCAL_SERVE refuses it all the same.
export OA_LOCAL_ADMIN="$OA_ADMIN_PUBKEYS"
local_model=()
if [[ $serve == 1 ]]; then
  common="$(git -C "$repo_root" rev-parse --path-format=absolute --git-common-dir)"
  cli="${OA_SERVE_CLI:-$HOME/.openagents/bin/openagents}"
  [[ -x "$cli" ]] || cli="$(dirname "$common")/apps/openagents-cli/build/openagents"
  # Who you are here: your `openagents` login's key (dev-identity.sh), the same key a CLI or Claude's realm tools
  # on this computer sign with, so they and the page are one account. With no login, a key of this server's own,
  # kept in OA_SERVE_STATE so you stay one account across restarts.
  me="$(OA_SERVE_CLI="$cli" "$here/../dev-identity.sh" || true)"; me_profile="${me% *}"; me_pub="${me#* }"
  if [[ "$me_pub" =~ ^[0-9a-f]{64}$ ]]; then
    export OA_ADMIN_PUBKEYS="$me_pub" OA_LOCAL_ADMIN="$me_pub"
    me_says="your openagents login (profile $me_profile)"
  elif [[ -n "$state" ]]; then
    [[ -s "$state/admin.key" ]] || "$SIGN" --new "$state/admin.key" >/dev/null
    ADMIN_KEY="$state/admin.key"; me_profile=""
    export OA_ADMIN_PUBKEYS="$("$SIGN" --key "$ADMIN_KEY" --pubkey)"; export OA_LOCAL_ADMIN="$OA_ADMIN_PUBKEYS"
    me_says="this server's own key ($ADMIN_KEY): no openagents login here; run openagents login and restart to be yourself"
  else
    me_profile=""; me_says="a throwaway key: no openagents login here"
  fi
  # The account's email: this computer's owner's (OA_SERVE_EMAIL, else git's user.email; OA_SERVE_EMAIL= none).
  # The local admin's account carries it (auth.c local_owner_email), so the page names you by it and a login
  # by its code here is the same account as the key.
  owner_email="${OA_SERVE_EMAIL-$(git -C "$repo_root" config user.email 2>/dev/null || true)}"
  export OA_LOCAL_EMAIL="$owner_email"
  api_origin="$serve_origin"
  infer_url="https://aiplatform.us.rep.googleapis.com/v1/projects/openagentsgemini/locations/us/endpoints/openapi/chat/completions"
  infer_token="" infer_daily=500
  # The model's Google token, kept fresh in a 0600 file the API reads on every call (never on a command line).
  token_file="$work/google.token"; (umask 077; : > "$token_file")
  sa_config="${OA_SERVE_GCLOUD_CONFIG:-$(dirname "$(dirname "$common")")/.secrets/gcloud-sa-config}"
  gcloud_bin="$(command -v gcloud || true)"
  google_token() {   # a token into the file, or false; an old token is kept until it is 50 minutes old
    local t
    if [[ -n "$gcloud_bin" ]]; then
      if [[ -d "$sa_config" ]]; then t="$(CLOUDSDK_CONFIG="$sa_config" "$gcloud_bin" auth print-access-token 2>/dev/null </dev/null || true)"
      else t="$("$gcloud_bin" auth print-access-token 2>/dev/null </dev/null || true)"; fi
    fi
    if [[ "$t" =~ ^[A-Za-z0-9._-]{20,}$ ]]; then (umask 077; printf '%s\n' "$t" > "$token_file.new") && mv "$token_file.new" "$token_file"; return 0; fi
    [[ -n "$(find "$token_file" -mmin +50 2>/dev/null)" ]] && : > "$token_file"
    return 1
  }
  if [[ -n "${OA_SERVE_INFERENCE_TOKEN:-}" ]]; then
    (umask 077; printf '%s\n' "$OA_SERVE_INFERENCE_TOKEN" > "$token_file")
    model_says="Gemini on Vertex, with the token in OA_SERVE_INFERENCE_TOKEN (not refreshed: it lasts an hour)"
  elif [[ "${OA_SERVE_MODEL:-}" != openagents ]] && google_token; then
    if [[ -d "$sa_config" ]]; then model_says="Gemini on Vertex, with the service account's token ($sa_config), refreshed every 40 minutes"
    else model_says="Gemini on Vertex, with your gcloud login's token, refreshed every 40 minutes"; fi
  fi
  if [[ -z "${OA_SERVE_INFERENCE_TOKEN:-}" && "${OA_SERVE_MODEL:-}" != openagents ]]; then
    # Every minute: a token again when it is 40 minutes old, or when there is none (a gcloud login since).
    ( while sleep 60; do
        [[ -s "$token_file" && -z "$(find "$token_file" -mmin +40 2>/dev/null)" ]] || google_token || true
      done ) & token_pid=$!
  fi
  # With no Google token: openagents.com's inference route, signed by this computer's `openagents` login.
  remote="${OA_SERVE_REMOTE_API:-https://openagents.com}/api/v1/inference"
  profile="${me_profile:-${OA_SERVE_PROFILE:-default}}"
  if [[ -x "$cli" ]]; then local_model=(OA_INFERENCE_SIGNER="$cli" OA_INFERENCE_SIGNER_PROFILE="$profile" OA_INFERENCE_REMOTE_URL="$remote"); fi
  if [[ -z "${model_says:-}" ]]; then
    if [[ -x "$cli" ]] && "$cli" --profile "$profile" nostr show >/dev/null 2>&1; then
      model_says="openagents.com's, on your openagents login (profile $profile): no Google login here"
    else
      model_says="none: no gcloud login and no openagents login here. Run gcloud auth login (or openagents login); the next turn uses it"
    fi
  fi
  local_model+=(OA_INFERENCE_TOKEN_FILE="$token_file")
fi
# The keys the database's rows are sealed with: new each run, or with OA_SERVE_STATE made once and kept there
# (0600), so a restart opens what the last run sealed (emails, projects, /chat's saved credentials).
keys_new() {
  printf 'k_project=%s\nk_seal=1:%s,2:%s\nk_pepper=1:%s\nk_notice=%s\nk_chat=%s\n' "$(openssl rand -hex 32)" "$(openssl rand -hex 32)" \
    "$(openssl rand -hex 32)" "$(openssl rand -hex 32)" "$(openssl rand -hex 32)" "$(openssl rand -hex 32)"
}
keys_file="$work/keys"
[[ -z "$state" ]] || keys_file="$state/keys"
[[ -s "$keys_file" ]] || (umask 077; keys_new > "$keys_file.new" && mv "$keys_file.new" "$keys_file")
while IFS='=' read -r name value; do [[ "$name" =~ ^k_[a-z]+$ && "$value" =~ ^[0-9a-f:,]+$ ]] && printf -v "$name" '%s' "$value"; done < "$keys_file"
[[ -n "${k_project:-}" && -n "${k_seal:-}" && -n "${k_pepper:-}" && -n "${k_notice:-}" && -n "${k_chat:-}" ]] || { echo "test.sh: $keys_file is not a keys file" >&2; exit 1; }
# The realm scripts' compile job (#13467, tests/script_checks.coax and script_cli_checks.coax): the local tool through its wrapper, where this
# machine has the Wasm toolchain and the job's own check passes its first case; else the API has no job and the
# checks say the compile route's did not run. SCRIPT_JOB_READY tells the checks which.
script_job=""
# Serving (coax/web/serve.sh): the job is the one the caller names, so the editor's Scripts pane can build
# (OA_SCRIPT_JOB=$PWD/coax/web/script-job.sh coax/web/serve.sh ...); with none named the API compiles nothing.
if [[ $serve == 1 && -n "${OA_SCRIPT_JOB:-}" && -x "${OA_SCRIPT_JOB}" ]]; then script_job="$OA_SCRIPT_JOB"; fi
if [[ $serve == 0 && -x "$cli_coax" ]] && { runs script_checks || runs script_cli_checks; } &&
   [[ -z "${API_TESTS_ONLY:-}" || ",$API_TESTS_ONLY," == *",script_checks.coax,"* || ",$API_TESTS_ONLY," == *",script_cli_checks.coax,"* ]]; then
  if (cd "$repo_root" && COAX="$cli_coax" coax/web/script-job.sh --warm) > /dev/null 2>&1 &&
     (cd "$repo_root" && COAX="$cli_coax" coax/web/script-job.sh metaverse-engine/script/examples/pacer.coax pacer "$work/script-warm") > /dev/null 2>&1; then
    script_job="$repo_root/coax/web/script-job.sh"
  else echo "api: no compile job here (no Wasm toolchain): the compile route's checks do not run" >&2; fi
fi
export SCRIPT_JOB_READY="${script_job:+1}"
# The compile service (coax/web/script-job/serve.c, Cloud Run oa-script-job) in front of that job, which the API
# asks through OA_SCRIPT_JOB_URL as production does (#13467): the compile checks go through it. The command path
# runs the same job runner (script-job-run.c) inside the service.
script_job_url=""
if [[ $serve == 0 && -n "$script_job" ]]; then
  "$cc" -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined -I"$here" \
    -o "$work/oa-script-job" "$here/../script-job/serve.c" "$here/script-job-run.c"
  (cd "$repo_root" && OA_SCRIPT_JOB="$script_job" COAX="$cli_coax" exec "$work/oa-script-job" --listen "127.0.0.1:$SCRIPT_JOB_PORT") 2> "$work/script-job.log" & script_job_pid=$!
  for _ in $(seq 100); do curl -s -m 1 "http://127.0.0.1:$SCRIPT_JOB_PORT/healthz" > /dev/null && { script_job_url="http://127.0.0.1:$SCRIPT_JOB_PORT"; break; }; sleep 0.1; done
  [[ -n "$script_job_url" ]] || { echo "test.sh: the compile service didn't start" >&2; cat "$work/script-job.log" >&2; exit 1; }
fi
OA_SCRIPT_JOB="$script_job" OA_SCRIPT_JOB_URL="$script_job_url" OA_SCRIPT_JOB_TOKEN=test-script-job COAX="$cli_coax" \
OA_CHAT_RUNNER_URL="http://127.0.0.1:$CHAT_PORT" OA_CHAT_DAILY_USD=1 \
OA_AGENT_PROJECTS=agent-proj OA_AGENT_WAKE_URL="http://127.0.0.1:$WAKE_PORT/wake" OA_AGENT_WAKE_TOKEN=test-wake \
OA_PROJECT_KEK="$k_project" OA_PUBLISH_RELAY="ws://127.0.0.1:$RELAY_PORT" OA_SITE_BUCKET=site-test \
OA_DROPS_GCS="http://127.0.0.1:$GCS_PORT" OA_DROPS_TOKEN=test-gcs OA_DROPS_BUCKET=drops-test OA_DROPS_PREFIX=test OA_DRAFT_URL="http://127.0.0.1:$GCS_PORT/model" OA_VILLA_URL="http://127.0.0.1:$GCS_PORT/villa-model" \
OA_INFERENCE_URL="$infer_url" OA_INFERENCE_TOKEN="$infer_token" OA_INFERENCE_DAILY_REQUESTS="$infer_daily" \
OA_SPARK_URL="http://127.0.0.1:$SPARK_PORT" OA_SPARK_TOKEN=test-spark OA_LNADDR_TEST_PORT="$SPARK_PORT" \
OA_RESEND_URL="http://127.0.0.1:$RESEND_PORT/emails" OA_RESEND_API_KEY=test-resend OA_EMAIL_ALLOW=device-a@example.com,device-b@example.com,life-a@example.com,life-c@example.com,life-d@example.com,life-f@example.com,life-h@example.com,owner@example.com,ada@example.com,frozen@example.com,app@example.com,rec@example.com,push-a@example.com,push-b@example.com,solo@example.com,pk@example.com,newcomer@example.com,wallet@example.com OA_EMAIL_BACKOFF_MS=10 \
OA_AUTH_SEAL_KEY="$k_seal" OA_AUTH_LOOKUP_PEPPER="$k_pepper" \
OA_APNS_KEY="$(openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 2>/dev/null)" OA_APNS_KEY_ID=TESTKEY123 OA_APNS_URL="http://127.0.0.1:$RESEND_PORT/apns" OA_APNS_SANDBOX_URL="http://127.0.0.1:$RESEND_PORT/apns-sandbox" \
OA_FCM_PROJECT=test-project OA_FCM_URL="http://127.0.0.1:$RESEND_PORT/fcm" OA_FCM_TOKEN=test-fcm \
OA_AUTH_NOTICE_NSEC="$k_notice" OA_AUTH_NOTICE_RELAY="ws://127.0.0.1:$NOTICE_RELAY_PORT" OA_LNADDR_PROFILE_RELAYS="ws://127.0.0.1:$NOTICE_RELAY_PORT" OA_AUTH_NOTICE_SYNC=1 OA_AUTH_SCRYPT_LOG_N=10 \
OA_TOKENINFO_URL="http://127.0.0.1:$TOKENINFO_PORT/tokeninfo" OA_JOBS_CALLER=jobs@example.iam.gserviceaccount.com OA_ALERT_TO=owner@example.com OA_DEVICE_TTL_MS=4000 \
OA_NPC_URL="http://127.0.0.1:$NPC_PORT/model" OA_NPC_TOKEN=test-token OA_NPC_DAILY_CAP=5 OA_NPC_KEY_10M=3 OA_NPC_ADDR_10M=5 OA_NPC_RUNNING=3 OA_NPC_RUNNING_KEY=1 OA_PIECE_URL="http://127.0.0.1:$PIECE_PORT/model" OA_PIECE_DAILY_CAP=40 OA_EDIT_URL="http://127.0.0.1:$PIECE_PORT/edit-model" OA_EDIT_TIMEOUT_MS=1500 OA_BUILDER_URL="http://127.0.0.1:$BUILDER_PORT/model" OA_BUILDER_TIMEOUT_MS=1200 OA_BUILDER_PER_DAY=6 \
OA_ANIME_URL="http://127.0.0.1:$ANIME_PORT/model" OA_ANIME_TIMEOUT_MS=500 OA_WORLDBUILDER_URL="http://127.0.0.1:$WORLDBUILDER_PORT/model" \
  OA_BUILD_SHA=1111111111111111111111111111111111111111 OA_RELEASE_ID=123e4567-e89b-42d3-a456-426614174000 K_SERVICE=oa-api-staging K_REVISION=oa-api-staging-00002-new \
  OA_ROOM_CHECKPOINTS=1 OA_API_ORIGIN="$api_origin" env ${local_model[@]+"${local_model[@]}"} "$work/api" --listen 127.0.0.1:$api_port --migrations "$here/migrations" 2>"$work/api.log" & api_pid=$!
# The API listens once its migrations are in, and on a busy host they take longer than any fixed wait
# (5 s was not enough, #13376): wait for its answer for as long as its process lives. Ten minutes with
# the process there and no answer is a hung API, and the run says so.
api_up=0
for _ in $(seq 6000); do
  curl -s -m 2 "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null && { api_up=1; break; }
  kill -0 "$api_pid" 2>/dev/null || break
  sleep 0.1
done
# The answer came from this run's server, not from another run's that took the port first.
(( api_up )) && kill -0 "$api_pid" 2>/dev/null || { echo "test.sh: the API didn't start" >&2; cat "$work/api.log"; exit 1; }
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
    # The mirror is made by hand, not with git clone --shared: git ignores --shared when the checkout is
    # shallow and copies every object instead, 8 GB a run.
    mirror=""
    if [[ -z "${OA_SERVE_NO_MIRROR:-}" ]]; then
      mirror="$work/mirrors/openagents.git"; mkdir -p "$work/mirrors"
      git init --quiet --bare "$mirror"
      printf '%s\n' "$common/objects" > "$mirror/objects/info/alternates"
      [[ ! -f "$common/shallow" ]] || cp "$common/shallow" "$mirror/shallow"
      git --git-dir="$mirror" remote add origin "$(dirname "$common")"
      git --git-dir="$mirror" fetch --quiet origin '+refs/heads/*:refs/heads/*' '+refs/tags/*:refs/tags/*'
      git --git-dir="$mirror" symbolic-ref HEAD refs/heads/main
    fi
    env -i HOME="$HOME" PATH="$(dirname "$claude_bin"):/opt/homebrew/bin:/usr/bin:/bin" \
      OA_API_ORIGIN="$api_origin" OA_RUNNER_KEY="$(cat "$RUN_KEY_FILE")" OA_CHAT_SEAL_KEY="$k_chat" \
      OA_CHAT_ENGINE="$work/oa-chat-turn" OA_CLAUDE="$claude_bin" OA_CHAT_REPO="${OA_SERVE_CHAT_REPO:-file://$(dirname "$common")}" \
      OA_CHAT_SESSIONS="${state:-$work}/sessions" OA_AGENT_WORK="$work/agent" OA_GIT="$(command -v git)" OA_REPO_MIRROR="$mirror" \
      "$RUNNER_BIN" --listen "127.0.0.1:$CHAT_PORT" 2>"$work/runner.log" & runner_pid=$!
    echo "serve: /chat's runner is on 127.0.0.1:$CHAT_PORT with $claude_bin (its log: $work/runner.log)${mirror:+, reading the mirror $mirror}"
  else
    echo "serve: no claude on PATH, so /chat's Claude turns have no runner"
  fi
  # The local admin: a request from this machine with no login is the admin's (auth.c local_admin).
  admin_now="$(curl -s -o /dev/null -w '%{http_code}' -m 5 -H "Host: 127.0.0.1:$api_port" "http://127.0.0.1:$api_port/api/chat/account" || true)"
  [[ "$admin_now" == 200 ]] || { echo "test.sh: the local admin isn't working (/api/chat/account answered $admin_now)" >&2; cat "$work/api.log" >&2; exit 1; }
  if [[ -n "$me_profile" ]]; then login="$("$cli" --profile "$me_profile" nostr auth "$api_origin/api/admin/login" 2>/dev/null || true)"; login="${login#Nostr }"
  else login="$("$SIGN" --key "$ADMIN_KEY" GET "$api_origin/api/admin/login")"; fi
  echo "serve: the API is on 127.0.0.1:$api_port (its log: $work/api.log)"
  [[ -n "${OA_SERVE_PORT_FILE:-}" ]] || echo "serve: run the dev server with --api $api_port"
  echo "serve: on this computer you are its admin with no login step: the key $OA_LOCAL_ADMIN, $me_says"
  if [[ -n "$state" ]]; then echo "serve: the database is $db, kept across restarts (serve.sh --fresh: a throwaway one)"
  else echo "serve: the database is a throwaway one, gone when this stops"; fi
  echo "serve: the model: $model_says"
  [[ -z "$owner_email" ]] || echo "serve: your account carries $owner_email; after Log out, log in with it: the code shows here (this server sends no mail)"
  # Email login codes: the API writes them to its log instead of sending mail; this terminal shows them.
  tail -n 0 -F "$work/api.log" 2>/dev/null \
    > >(while IFS= read -r l; do [[ "$l" != *"dev server login code"* ]] || printf 'serve: %s\n' "${l#*auth: }"; done) & codes_pid=$!
  echo "serve: from another computer, an admin login good for 60 seconds: $api_origin/api/admin/login?auth=$(python3 -I -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$login")&next=chat"
  [[ -z "${OA_SERVE_PORT_FILE:-}" ]] || printf '%s\n' "$api_port" > "$OA_SERVE_PORT_FILE"
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
  # the vitals fault check and the owner and travel checks. API_TESTS_ONLY runs them when it names them (room_checks.coax);
  # API_ROOM_SLICES="parties deploy" runs only those of its slices.
  # API_BIN: the region check starts a second API on a promoted replica of this database.
  coax_failed=0
  # tests/script_cli_checks.coax drives the built CLI (#13467): built here when that check runs.
  realm_cli="$repo_root/apps/openagents-cli/build/openagents"
  for f in room_checks checkpoint2_checks verse_packs auth_checks auth_recover_checks agent_checks auth_life_checks passkey_checks lnaddr_checks auth_device_checks wallet_checks realm_doc_checks realm_record_checks world_checks worldbuilder_checks script_checks script_cli_checks workers_checks; do
    [[ -z "${API_TESTS_ONLY:-}" || ",$API_TESTS_ONLY," == *",$f.coax,"* ]] || continue
    runs "$f" || continue
    if [[ $f == script_cli_checks ]]; then
      (cd "$repo_root" && COAX="$cli_coax" apps/openagents-cli/build.sh --cli > "$work/cli-build.log" 2>&1) || { cat "$work/cli-build.log"; coax_failed=1; continue; }
    fi
    (cd "$repo_root" && API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pgbin/psql" COAX="$cli_coax" API_BIN="$work/api" RESEND_PORT="$RESEND_PORT" CLI="$realm_cli" \
      coax/web/tool.sh "coax/web/api/tests/$f.coax" $([[ $f == room_checks ]] && echo "${API_ROOM_SLICES:-}")) || coax_failed=1
    lap "tests/$f.coax"
  done
  # Every answer of this run, held to its route file's messages (routes-goldens.coax; Protocol, #13250).
  (cd "$repo_root" && COAX="$cli_coax" coax/web/tool.sh coax/web/api/routes-goldens.coax --read-all "$API_CAPTURE") || coax_failed=1
  lap "routes-goldens.coax"
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
  # Every anime answer of this run, held to the two directors' codecs (anime-director-profile.coax; Protocol, #13250).
  (cd "$repo_root" && COAX="$cli_coax" coax/web/tool.sh coax/web/api/anime-director-profile.coax --read-all "$API_CAPTURE") || { echo "the anime answers are not the codecs'" >&2; exit 1; }
  lap "anime_direct"
  fi
  # A made realm's engine pins, shapes and edit log, and the realm editor's whole route checked end to end (#12629).
  # (The built CLI against a local API is tests/realm_cli_api.coax; its publish needs an API with no
  # bucket or with OA_ENGINE_TEMPLATE, which this run's fake bucket isn't, so it runs on its own.)
  if runs realms; then
  mkdir -p "$work/walk"
  for f in realm_engine_pin realm_shapes realm_edit_log realm_places; do
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
# The moderation log is append-only too (0170_moderation.sql, #13302): the API's role appends and reads, and
# no role changes, deletes or truncates a row: not one given UPDATE and DELETE, and not the table's owner.
asu -U postgres -c "SET ROLE oa_api_check; INSERT INTO verse.mod_log (at_ms, by_who, verb, whom, who, minutes, until_ms, reason, source, outcome) VALUES (1, 'site', 'kick', 'p-check', 'p-check', 0, 0, 'a check', 'api', 'ok')" > /dev/null || { echo "moderation: the API's role can't append to the log" >&2; exit 1; }
if asu -U postgres -c "SET ROLE oa_api_check; UPDATE verse.mod_log SET reason = 'rewritten'" 2>/dev/null; then echo "moderation: the API's role changed a log row" >&2; exit 1; fi
if asu -U postgres -c "SET ROLE oa_api_check; DELETE FROM verse.mod_log" 2>/dev/null; then echo "moderation: the API's role deleted a log row" >&2; exit 1; fi
asu -U oa_analytics -c "GRANT USAGE ON SCHEMA verse TO oa_api_loose; GRANT SELECT, UPDATE, DELETE ON verse.mod_log TO oa_api_loose"
if asu -U postgres -c "SET ROLE oa_api_loose; UPDATE verse.mod_log SET reason = 'rewritten'" 2>/dev/null; then echo "moderation: the log's trigger let an update through" >&2; exit 1; fi
if asu -U postgres -c "SET ROLE oa_api_loose; DELETE FROM verse.mod_log" 2>/dev/null; then echo "moderation: the log's trigger let a delete through" >&2; exit 1; fi
if asu -U oa_analytics -c "UPDATE verse.mod_log SET reason = 'rewritten'" 2>/dev/null; then echo "moderation: the log's owner changed a row" >&2; exit 1; fi
if asu -U oa_analytics -c "DELETE FROM verse.mod_log" 2>/dev/null; then echo "moderation: the log's owner deleted a row" >&2; exit 1; fi
if asu -U oa_analytics -c "TRUNCATE verse.mod_log" 2>/dev/null; then echo "moderation: the log's owner truncated it" >&2; exit 1; fi
[[ "$(asu -U oa_analytics -c "SELECT count(*) FROM verse.mod_log WHERE reason = 'a check'")" == 1 ]] || { echo "moderation: the log's check row is gone" >&2; exit 1; }
echo "  ok    the moderation log is append-only for the API's role, its owner and anyone else"
# roles.coax's catalog check (db/checks.sql) knows both logs: the role without UPDATE and DELETE on them is right.
roles_said="$(asu -U oa_analytics -v runtime=oa_api_check -f "$here/db/checks.sql")" || { echo "roles: db/checks.sql does not run" >&2; exit 1; }
if grep -E "append-only log|can't (UPDATE|DELETE) (auth\.audit|verse\.mod_log)" <<<"$roles_said"; then echo "roles: db/checks.sql is wrong about the append-only logs" >&2; exit 1; fi
echo "  ok    the roles' catalog check takes the append-only logs as they are"
fi
if grep -q "ERROR: AddressSanitizer\|runtime error" "$work/api.log"; then cat "$work/api.log"; exit 1; fi
echo "api: checks passed in $((SECONDS - began)) s"
