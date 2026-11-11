#!/usr/bin/env bash
# The API service's checks, on this Mac: builds api, starts a throwaway
# Postgres (Homebrew postgresql@16) and the service on it, and runs the
# checks in test.py against both (migrations, health, ingest, admin).
#   coax/web/api/test.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
"$here/mesh-policy.sh" --check-inputs
"$here/caller-policy.sh" --check-inputs
"$here/nip98.sh" --check-inputs
# NIP-98 is Coax (nip98.coax): with a compiler here, its checks and its C cache.
repo_root="$(cd "$here/../../.." && pwd)"
cli_coax="${COAX:-$repo_root/coax/bin/coax}"
if [[ ! -x "$cli_coax" ]]; then
  common="$(git -C "$here" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
  [[ -n "$common" ]] && cli_coax="$(dirname "$common")/coax/bin/coax"
fi
if [[ -x "$cli_coax" ]]; then
  "$here/nip98.sh" --check || echo 'nip98: the C cache differs from this compiler'"'"'s output (the image builds from source; regenerate with nip98.sh --generate)' >&2
  (cd "$repo_root" && "$cli_coax" test coax/web/api/nip98.coax --run-steps 250000000 | tail -1)
  (cd "$repo_root" && "$cli_coax" test coax/web/api/nip98_checks.coax --steps 4000000000 --run-steps 250000000 | tail -1)
fi
"$here/test-deploy-command.sh"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_verify_test.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_rollout_test.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_snapshot_test.py"
pg=/opt/homebrew/opt/postgresql@16
work="$(mktemp -d)"; trap 'kill ${api_pid:-} ${notice_relay_pid:-} 2>/dev/null || true; "$pg/bin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
cc=/usr/bin/clang
secp="$(brew --prefix secp256k1 2>/dev/null || true)"
"$cli_coax" c "$here/anime-director-policy.coax" --library --header "$work/anime-director-policy.generated.h" > "$work/anime-director-policy.generated.c"
"$cli_coax" c "$here/seat-policy.coax" --library --header "$work/seat-policy.generated.h" > "$work/seat-policy.generated.c"
"$cli_coax" c "$here/progress-policy.coax" --library --header "$work/progress-policy.generated.h" > "$work/progress-policy.generated.c"
"$cli_coax" c "$here/worldbuilder-policy.coax" --library --header "$work/worldbuilder-policy.generated.h" > "$work/worldbuilder-policy.generated.c"
"$cli_coax" c "$here/auth-policy.coax" --library --header "$work/auth-policy.generated.h" > "$work/auth-policy.generated.c"
"$cli_coax" c "$here/auth-seal.coax" --run-steps 250000000 --library --header "$work/auth-seal.generated.h" > "$work/auth-seal.generated.c"
"$cli_coax" c "$here/agent-wire.coax" --run-steps 250000000 --library --header "$work/agent-wire.generated.h" > "$work/agent-wire.generated.c"
"$cli_coax" c "$here/agent-attest.coax" --run-steps 250000000 --library --header "$work/agent-attest.generated.h" > "$work/agent-attest.generated.c"
"$cli_coax" c "$here/auth-recover-policy.coax" --library --header "$work/auth-recover-policy.generated.h" > "$work/auth-recover-policy.generated.c"
"$cli_coax" c "$here/auth-recover-crypto.coax" --run-steps 250000000 --library --header "$work/auth-recover-crypto.generated.h" > "$work/auth-recover-crypto.generated.c"
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$work" -I"$pg/include" ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" \
  -o "$work/api" "$here"/api.c "$here"/analytics.c "$here"/admin.c "$here"/player.c "$here"/rooms.c "$here"/npc.c "$here"/room.c "$here"/private.c "$here"/drops.c "$here"/villa.c "$here"/chat.c "$here"/realms.c "$here"/access.c "$here"/makers.c "$here"/inference.c "$here"/mesh.c "$here"/mesh-policy.generated.c "$here"/caller-policy.generated.c "$here"/nip98.generated.c "$here"/anime.c "$work"/anime-director-policy.generated.c "$here"/seats.c "$work"/seat-policy.generated.c "$work"/progress-policy.generated.c "$here"/nostr.c "$here"/email.c "$here"/jobs.c "$here"/relay.c "$here"/verse_packs.c "$here"/worldbuilder.c "$work"/worldbuilder-policy.generated.c "$here"/auth.c "$work"/auth-policy.generated.c "$here"/auth-seal-unit.c "$here"/agents.c "$here"/agent-wire-unit.c "$here"/agent-attest-unit.c "$here"/auth-recover.c "$work"/auth-recover-policy.generated.c "$here"/auth-recover-crypto-unit.c \
  -L"$pg/lib" -lpq ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lssl -lcrypto -lcurl -lz -lm
# The gate's table (auth.c): an unknown route has no rule, and /docs/limits is the API's own limits table.
[[ "$("$work/api" --rule GET /api/no-such-route || true)" == none && "$("$work/api" --rule POST /api/auth/logout-everywhere)" == L2 ]] || { echo "api --rule: the gate's table is wrong" >&2; exit 1; }
"$work/api" --limits | diff - "$repo_root/apps/openagents-web/docs/limits/index.md" > /dev/null || { echo "apps/openagents-web/docs/limits/index.md is stale: run api --limits > it" >&2; exit 1; }
# The Coax NIP-98 check against the former C check, on the corpus and seeded mutations.
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" -o "$work/nip98_diff" \
  "$here/nip98_diff.c" "$here/nip98_reference.c" "$here/nostr.c" "$here/nip98.generated.c" \
  ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto
"$work/nip98_diff" "$here/nip98_corpus.tsv" 400
"$pg/bin/initdb" -D "$work/db" -U postgres -A trust >/dev/null
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
"$pg/bin/pg_ctl" -D "$work/db" -o "-p $port -k $work -c listen_addresses=''" -l "$work/pg.log" -w start >/dev/null
"$pg/bin/psql" -h "$work" -p $port -U postgres -q -c "CREATE ROLE oa_analytics LOGIN" -c "CREATE DATABASE oa_analytics OWNER oa_analytics"
export OA_API_DATABASE="host=$work port=$port user=oa_analytics dbname=oa_analytics"
# The signer (sign.c), and two throwaway keys: an admin and someone else.
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" -o "$work/sign" "$here/sign.c" \
  ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto
# The project agent's runner (coax/web/agent/runner.c), run by test_private_runner.py with a stand-in engine.
# It links runner-sign.coax's C, compiled here as deploy.sh compiles it. The generated
# signer sets a value it never reads (schnorr_sign.coax's ss_write result), so that
# warning stays a warning for that file only.
"$cli_coax" c "$here/../agent/runner-sign.coax" --library --run-steps 250000000 --header "$work/runner-sign.h" > "$work/runner-sign.c"
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -Wno-error=unused-but-set-variable -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -c -o "$work/runner-sign.o" "$work/runner-sign.c"
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$work" ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" -o "$work/oa-agent" "$here/../agent/runner.c" "$work/runner-sign.o" \
  ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto -lcurl -lm
export RUNNER_BIN="$work/oa-agent" RUNNER_BIN_SRC="$here/../agent/runner.c"
export SIGN="$work/sign" ADMIN_KEY="$work/admin.key" OTHER_KEY="$work/other.key"
export OA_ADMIN_PUBKEYS="$("$SIGN" --new "$ADMIN_KEY")"
# The rooms' signed routes (seat tickets, vitals reports, room-control): a local key unless the caller
# set one, so tests/room_checks.coax runs its keyed checks (admission, invites, stats, vitals) too.
export ROOM_KEY="${ROOM_KEY:-local-development-room-ticket-key-not-a-secret}"
export OA_ROOM_TICKET_KEY="${OA_ROOM_TICKET_KEY:-$ROOM_KEY}"
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
OA_CHAT_RUNNER_URL="http://127.0.0.1:$CHAT_PORT" OA_CHAT_DAILY_USD=1 \
OA_AGENT_PROJECTS=agent-proj OA_AGENT_WAKE_URL="http://127.0.0.1:$WAKE_PORT/wake" OA_AGENT_WAKE_TOKEN=test-wake \
OA_PROJECT_KEK="$(openssl rand -hex 32)" OA_PUBLISH_RELAY="ws://127.0.0.1:$RELAY_PORT" OA_SITE_BUCKET=site-test \
OA_DROPS_GCS="http://127.0.0.1:$GCS_PORT" OA_DROPS_TOKEN=test-gcs OA_DROPS_BUCKET=drops-test OA_DROPS_PREFIX=test OA_DRAFT_URL="http://127.0.0.1:$GCS_PORT/model" OA_VILLA_URL="http://127.0.0.1:$GCS_PORT/villa-model" \
OA_INFERENCE_URL="http://127.0.0.1:$INFER_PORT/chat/completions" OA_INFERENCE_TOKEN=test-token OA_INFERENCE_DAILY_REQUESTS=5 \
OA_SPARK_URL="http://127.0.0.1:$SPARK_PORT" OA_SPARK_TOKEN=test-spark \
OA_WORKOS_CLIENT_ID=client_test \
OA_RESEND_URL="http://127.0.0.1:$RESEND_PORT/emails" OA_RESEND_API_KEY=test-resend OA_EMAIL_ALLOW=owner@example.com,ada@example.com,frozen@example.com,app@example.com,rec@example.com,solo@example.com OA_EMAIL_BACKOFF_MS=10 \
OA_AUTH_SEAL_KEY="1:$(openssl rand -hex 32),2:$(openssl rand -hex 32)" OA_AUTH_LOOKUP_PEPPER="1:$(openssl rand -hex 32)" \
OA_AUTH_NOTICE_NSEC="$(openssl rand -hex 32)" OA_AUTH_NOTICE_RELAY="ws://127.0.0.1:$NOTICE_RELAY_PORT" OA_AUTH_NOTICE_SYNC=1 OA_AUTH_SCRYPT_LOG_N=10 \
OA_TOKENINFO_URL="http://127.0.0.1:$TOKENINFO_PORT/tokeninfo" OA_JOBS_CALLER=jobs@example.iam.gserviceaccount.com OA_DEVICE_TTL_MS=4000 \
OA_NPC_URL="http://127.0.0.1:$NPC_PORT/model" OA_NPC_TOKEN=test-token OA_NPC_DAILY_CAP=5 OA_PIECE_URL="http://127.0.0.1:$PIECE_PORT/model" OA_PIECE_DAILY_CAP=40 OA_EDIT_URL="http://127.0.0.1:$PIECE_PORT/edit-model" OA_EDIT_TIMEOUT_MS=1500 OA_BUILDER_URL="http://127.0.0.1:$BUILDER_PORT/model" OA_BUILDER_TIMEOUT_MS=1200 OA_BUILDER_PER_DAY=6 \
OA_ANIME_URL="http://127.0.0.1:$ANIME_PORT/model" OA_ANIME_TIMEOUT_MS=500 \
  OA_BUILD_SHA=1111111111111111111111111111111111111111 OA_RELEASE_ID=123e4567-e89b-42d3-a456-426614174000 K_SERVICE=oa-api-staging K_REVISION=oa-api-staging-00002-new \
  OA_ROOM_CHECKPOINTS=1 OA_API_ORIGIN="http://127.0.0.1:$api_port" "$work/api" --listen 127.0.0.1:$api_port --migrations "$here/migrations" 2>"$work/api.log" & api_pid=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null && break; sleep 0.1; done
# The answer came from this run's server, not from another run's that took the port first.
kill -0 "$api_pid" 2>/dev/null || { echo "test.sh: the API didn't start" >&2; cat "$work/api.log"; exit 1; }
API_LOG="$work/api.log" API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql" python3 -I "$here/test.py" || { cat "$work/api.log"; exit 1; }
# The CLI as a caller (NIP-98 callers, realms/mine, visibility, admin keys, device login), in Coax: tests/cli_auth.coax.
# The CLI-caller checks are a Coax program: they need a compiler. A deploy's
# fresh worktree has none of its own, so take the main checkout's.
if [[ -x "$cli_coax" ]]; then
  # The rooms' checks and the private packs' checks (tests/room_checks.coax, tests/verse_packs.coax).
  # The rooms' signed routes run with the key above; with ROOM_BIN (coax/web/room/build.sh) as well,
  # the vitals fault check and the owner and travel checks. API_TESTS_ONLY runs them when it names them (room_checks.coax).
  # API_BIN: the region check starts a second API on a promoted replica of this database.
  coax_failed=0
  for f in room_checks checkpoint2_checks verse_packs auth_checks auth_recover_checks agent_checks; do
    [[ -z "${API_TESTS_ONLY:-}" || ",$API_TESTS_ONLY," == *",$f.coax,"* ]] || continue
    (cd "$repo_root" && API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql" COAX="$cli_coax" API_BIN="$work/api" RESEND_PORT="$RESEND_PORT" \
      coax/web/tool.sh "coax/web/api/tests/$f.coax") || coax_failed=1
  done
  [[ $coax_failed == 0 ]] || { cat "$work/api.log"; exit 1; }
  # A bounded character-recovery slice, including optional real native faults.
  # An unfiltered invocation still runs every check below.
  if [[ "${API_TESTS_ONLY:-}" == checkpoint2_checks.coax ]]; then
    echo "api: checkpoint2 checks passed"
    exit 0
  fi
  ADMIN_SECRET_HEX="$(tr -d '\n' < "$ADMIN_KEY")" COAX="$cli_coax" "$here/cli-auth-test.sh" "http://127.0.0.1:$api_port" || { cat "$work/api.log"; exit 1; }
  # The shell only compiles platform entry points. Coax owns the selector,
  # all requests/assertions, bounded subprocesses and fixture cleanup.
  p="$repo_root/coax/platform"
  "$cli_coax" c "$repo_root/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
  for unit in anime_direct anime_direct_selector; do
    "$cli_coax" c "$here/tests/$unit.coax" --library --run-steps 250000000 > "$work/$unit.c"
    "$cc" -std=c11 -O2 -fblocks -D_DARWIN_C_SOURCE -I"$work" -I"$p" -o "$work/$unit" "$work/$unit.c" \
      "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/socket-native.c" "$p/tls-native.c" \
      "$p/entropy-native.c" "$p/entropy-foreign.c" "$work/entropy.c" -framework Network -framework Security -framework CoreFoundation -lm
  done
  API="http://127.0.0.1:$api_port" ORIGIN="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql" \
    ANIME_ONLY= ANIME_ISOLATED_DB=oa_analytics ANIME_SELECTOR_BIN="$work/anime_direct_selector" "$work/anime_direct" || { cat "$work/api.log"; exit 1; }
  # A made realm's engine pins, shapes and edit log, and Aethyrforge's whole route checked end to end (#12629).
  # (The built CLI against a local API is tests/realm_cli_api.coax; its publish needs an API with no
  # bucket or with OA_ENGINE_TEMPLATE, which this run's fake bucket isn't, so it runs on its own.)
  mkdir -p "$work/walk"
  for f in realm_engine_pin realm_shapes realm_edit_log; do
    (cd "$repo_root" && PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql" PIN_SQL="coax/web/api/tests/$f.sql" COAX="$cli_coax" coax/web/tool.sh coax/web/api/tests/realm_engine_pin.coax) || exit 1
  done
  (cd "$repo_root" && PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql" WORK="$work/walk" COAX="$cli_coax" coax/web/tool.sh coax/web/api/tests/realm_walkthrough.coax) || exit 1
else
  echo 'cli-auth: skipped, no Coax compiler (build coax/bin/coax, or set COAX)' >&2
fi
# The audit log is append-only (0144_auth_audit.sql, #13202): with db/grants.sql's grants (its guard
# step aside), the API's role inserts and reads but can't change or delete a row, and a role
# given UPDATE anyway is stopped by the table's trigger.
asu() { "$pg/bin/psql" -h "$work" -p "$port" -d oa_analytics -XAtq -v ON_ERROR_STOP=1 "$@"; }
asu -U postgres -c "CREATE ROLE oa_api_check; CREATE ROLE oa_api_loose; GRANT oa_api_check, oa_api_loose TO postgres"
sed '/oa_guard.attach/,/\\gexec/d' "$here/db/grants.sql" | asu -U oa_analytics -v runtime=oa_api_check -v readers= -f - > /dev/null
asu -U postgres -c "SET ROLE oa_api_check; SELECT auth.log(NULL, NULL, 'check.append', 'ok', '{}')" > /dev/null || { echo "audit: the API's role can't append" >&2; exit 1; }
if asu -U postgres -c "SET ROLE oa_api_check; UPDATE auth.audit SET result = 'failed'" 2>/dev/null; then echo "audit: the API's role changed a row" >&2; exit 1; fi
if asu -U postgres -c "SET ROLE oa_api_check; DELETE FROM auth.audit" 2>/dev/null; then echo "audit: the API's role deleted a row" >&2; exit 1; fi
asu -U oa_analytics -c "GRANT USAGE ON SCHEMA auth TO oa_api_loose; GRANT SELECT, UPDATE, DELETE ON auth.audit TO oa_api_loose"
if asu -U postgres -c "SET ROLE oa_api_loose; UPDATE auth.audit SET result = 'failed'" 2>/dev/null; then echo "audit: the trigger let an update through" >&2; exit 1; fi
echo "  ok    the audit log is append-only for the API's role, and its trigger stops anyone else"
if grep -q "ERROR: AddressSanitizer\|runtime error" "$work/api.log"; then cat "$work/api.log"; exit 1; fi
echo "api: checks passed"
