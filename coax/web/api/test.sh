#!/usr/bin/env bash
# The API service's checks, on this Mac: builds api, starts a throwaway
# Postgres (Homebrew postgresql@16) and the service on it, and runs the
# checks in test.py against both (migrations, health, ingest, admin).
#   coax/web/api/test.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
"$here/mesh-policy.sh" --check-inputs
"$here/nip98.sh" --check-inputs
# NIP-98 is Coax (nip98.coax): with a compiler here, its checks and its C cache.
repo_root="$(cd "$here/../../.." && pwd)"
if [[ -x "$repo_root/coax/bin/coax" ]]; then
  "$here/nip98.sh" --check || echo 'nip98: the C cache differs from this compiler'"'"'s output (the image builds from source; regenerate with nip98.sh --generate)' >&2
  (cd "$repo_root" && coax/bin/coax test coax/web/api/nip98.coax --run-steps 250000000 | tail -1)
  (cd "$repo_root" && coax/bin/coax test coax/web/api/nip98_checks.coax --steps 4000000000 --run-steps 250000000 | tail -1)
fi
"$here/test-deploy-command.sh"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_verify_test.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_rollout_test.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$here/deploy_snapshot_test.py"
pg=/opt/homebrew/opt/postgresql@16
work="$(mktemp -d)"; trap 'kill ${api_pid:-} 2>/dev/null || true; "$pg/bin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
cc=/usr/bin/clang
secp="$(brew --prefix secp256k1 2>/dev/null || true)"
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$pg/include" ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" \
  -o "$work/api" "$here"/api.c "$here"/analytics.c "$here"/admin.c "$here"/player.c "$here"/npc.c "$here"/room.c "$here"/private.c "$here"/drops.c "$here"/villa.c "$here"/chat.c "$here"/realms.c "$here"/replies.c "$here"/makers.c "$here"/inference.c "$here"/mesh.c "$here"/mesh-policy.generated.c "$here"/nip98.generated.c "$here"/nostr.c "$here"/relay.c \
  -L"$pg/lib" -lpq ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lssl -lcrypto -lcurl -lz -lm
# The Coax NIP-98 check against the former C check, on the corpus and seeded mutations.
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" -o "$work/nip98_diff" \
  "$here/nip98_diff.c" "$here/nip98_reference.c" "$here/nostr.c" "$here/nip98.generated.c" \
  ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto
"$work/nip98_diff" "$here/nip98_corpus.tsv" 400
"$pg/bin/initdb" -D "$work/db" -U postgres -A trust >/dev/null
port=$((56000 + RANDOM % 1000)); api_port=$((57000 + RANDOM % 1000))
"$pg/bin/pg_ctl" -D "$work/db" -o "-p $port -k $work -c listen_addresses=''" -l "$work/pg.log" -w start >/dev/null
"$pg/bin/psql" -h "$work" -p $port -U postgres -q -c "CREATE ROLE oa_analytics LOGIN" -c "CREATE DATABASE oa_analytics OWNER oa_analytics"
export OA_API_DATABASE="host=$work port=$port user=oa_analytics dbname=oa_analytics"
# The signer (sign.c), and two throwaway keys: an admin and someone else.
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" -o "$work/sign" "$here/sign.c" \
  ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto
# The project agent's runner (coax/web/agent/runner.c), run by test_private_runner.py with a stand-in engine.
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" -o "$work/oa-agent" "$here/../agent/runner.c" \
  ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto -lcurl
export RUNNER_BIN="$work/oa-agent" RUNNER_BIN_SRC="$here/../agent/runner.c"
export SIGN="$work/sign" ADMIN_KEY="$work/admin.key" OTHER_KEY="$work/other.key"
export OA_ADMIN_PUBKEYS="$("$SIGN" --new "$ADMIN_KEY")"
# Our team (test_team.py): a web player, an old playtest player and an email's hash (team@example.com).
export OA_TEAM="player:p-7ea0000000000001,player:kiki-7ea0000000000002,email_sha256:$(printf '%s' team@example.com | shasum -a 256 | cut -c1-64),player:bad'quote"
export RUN_KEY_FILE="$work/runner.key" OA_AGENT_RUNNER_PUBKEYS="$("$SIGN" --new "$work/runner.key")"
"$SIGN" --new "$OTHER_KEY" >/dev/null
# Character chat asks a fake model (test_npc.py serves it) with a test token.
export NPC_PORT=$((58000 + RANDOM % 1000))
# The Sandbox's set pieces ask their own fake model (test_sandbox_pieces.py serves it).
export PIECE_PORT=$((53000 + RANDOM % 1000))
# Build your own realm asks its own fake model (test_realm_builder.py serves it).
export BUILDER_PORT=$((51000 + RANDOM % 1000))
# Drops go to a fake bucket (test_project_drops.py serves it).
export GCS_PORT=$((55000 + RANDOM % 1000))
# Publishing a project from a drop (rc-publish): a fake relay (test_project_drops.py) and the fake bucket.
export RELAY_PORT=$((52000 + RANDOM % 1000))
# The project agent: only the fixture project "agent-proj" is approved; wakes go to test_private_agent.py's fake runner.
export WAKE_PORT=$((59000 + RANDOM % 1000))
# /api/v1/inference (test_inference.py): a fake Vertex on INFER_PORT, a key that may use the route, five requests a day.
export INFER_PORT=$((50000 + RANDOM % 1000)) INFER_KEY="$work/infer.key"
export OA_INFERENCE_PUBKEYS="$("$SIGN" --new "$INFER_KEY")"
# /api/v1/inference/credit (test_inference.py): a fake Spark wallet on SPARK_PORT.
export SPARK_PORT=$((49000 + RANDOM % 1000))
# /chat (test_zchat.py): its own runner on CHAT_PORT, a $1 daily cap.
export CHAT_PORT=$((54000 + RANDOM % 1000))
OA_CHAT_RUNNER_URL="http://127.0.0.1:$CHAT_PORT" OA_CHAT_DAILY_USD=1 \
OA_AGENT_PROJECTS=agent-proj OA_AGENT_WAKE_URL="http://127.0.0.1:$WAKE_PORT/wake" OA_AGENT_WAKE_TOKEN=test-wake \
OA_PROJECT_KEK="$(openssl rand -hex 32)" OA_PUBLISH_RELAY="ws://127.0.0.1:$RELAY_PORT" OA_SITE_BUCKET=site-test \
OA_DROPS_GCS="http://127.0.0.1:$GCS_PORT" OA_DROPS_TOKEN=test-gcs OA_DROPS_BUCKET=drops-test OA_DROPS_PREFIX=test OA_DRAFT_URL="http://127.0.0.1:$GCS_PORT/model" OA_VILLA_URL="http://127.0.0.1:$GCS_PORT/villa-model" \
OA_INFERENCE_URL="http://127.0.0.1:$INFER_PORT/chat/completions" OA_INFERENCE_TOKEN=test-token OA_INFERENCE_DAILY_REQUESTS=5 \
OA_SPARK_URL="http://127.0.0.1:$SPARK_PORT" OA_SPARK_TOKEN=test-spark \
OA_NPC_URL="http://127.0.0.1:$NPC_PORT/model" OA_NPC_TOKEN=test-token OA_NPC_DAILY_CAP=5 OA_PIECE_URL="http://127.0.0.1:$PIECE_PORT/model" OA_PIECE_DAILY_CAP=40 OA_EDIT_URL="http://127.0.0.1:$PIECE_PORT/edit-model" OA_EDIT_TIMEOUT_MS=1500 OA_BUILDER_URL="http://127.0.0.1:$BUILDER_PORT/model" OA_BUILDER_TIMEOUT_MS=1200 OA_BUILDER_PER_DAY=6 \
  OA_BUILD_SHA=1111111111111111111111111111111111111111 OA_RELEASE_ID=123e4567-e89b-42d3-a456-426614174000 K_SERVICE=oa-api-staging K_REVISION=oa-api-staging-00002-new \
  OA_API_ORIGIN="http://127.0.0.1:$api_port" "$work/api" --listen 127.0.0.1:$api_port --migrations "$here/migrations" 2>"$work/api.log" & api_pid=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null && break; sleep 0.1; done
API_LOG="$work/api.log" API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql" python3 -I "$here/test.py" || { cat "$work/api.log"; exit 1; }
if grep -q "ERROR: AddressSanitizer\|runtime error" "$work/api.log"; then cat "$work/api.log"; exit 1; fi
echo "api: checks passed"
