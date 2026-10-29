#!/usr/bin/env bash
# The API service's checks, on this Mac: builds api, starts a throwaway
# Postgres (Homebrew postgresql@16) and the service on it, and runs the
# checks in test.py against both (migrations, health, ingest, admin).
#   coax/web/api/test.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
pg=/opt/homebrew/opt/postgresql@16
work="$(mktemp -d)"; trap 'kill ${api_pid:-} 2>/dev/null || true; "$pg/bin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
cc=/usr/bin/clang
secp="$(brew --prefix secp256k1 2>/dev/null || true)"
$cc -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$pg/include" ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" \
  -o "$work/api" "$here"/api.c "$here"/analytics.c "$here"/admin.c "$here"/player.c "$here"/npc.c "$here"/room.c "$here"/private.c "$here"/drops.c "$here"/villa.c "$here"/chat.c "$here"/nostr.c \
  -L"$pg/lib" -lpq ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lcrypto -lcurl
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
export RUN_KEY_FILE="$work/runner.key" OA_AGENT_RUNNER_PUBKEYS="$("$SIGN" --new "$work/runner.key")"
"$SIGN" --new "$OTHER_KEY" >/dev/null
# Character chat asks a fake model (test_npc.py serves it) with a test token.
export NPC_PORT=$((58000 + RANDOM % 1000))
# The Sandbox's set pieces ask their own fake model (test_sandbox_pieces.py serves it).
export PIECE_PORT=$((53000 + RANDOM % 1000))
# Drops go to a fake bucket (test_project_drops.py serves it).
export GCS_PORT=$((55000 + RANDOM % 1000))
# The project agent: only the fixture project "agent-proj" is approved; wakes go to test_private_agent.py's fake runner.
export WAKE_PORT=$((59000 + RANDOM % 1000))
# /chat (test_zchat.py): its own runner on CHAT_PORT, a $1 daily cap.
export CHAT_PORT=$((54000 + RANDOM % 1000))
OA_CHAT_RUNNER_URL="http://127.0.0.1:$CHAT_PORT" OA_CHAT_DAILY_USD=1 \
OA_AGENT_PROJECTS=agent-proj OA_AGENT_WAKE_URL="http://127.0.0.1:$WAKE_PORT/wake" OA_AGENT_WAKE_TOKEN=test-wake \
OA_DROPS_GCS="http://127.0.0.1:$GCS_PORT" OA_DROPS_TOKEN=test-gcs OA_DROPS_BUCKET=drops-test OA_DROPS_PREFIX=test OA_DRAFT_URL="http://127.0.0.1:$GCS_PORT/model" OA_VILLA_URL="http://127.0.0.1:$GCS_PORT/villa-model" \
OA_NPC_URL="http://127.0.0.1:$NPC_PORT/model" OA_NPC_TOKEN=test-token OA_NPC_DAILY_CAP=5 OA_PIECE_URL="http://127.0.0.1:$PIECE_PORT/model" OA_PIECE_DAILY_CAP=40 \
  OA_API_ORIGIN="http://127.0.0.1:$api_port" "$work/api" --listen 127.0.0.1:$api_port --migrations "$here/migrations" 2>"$work/api.log" & api_pid=$!
for _ in $(seq 50); do curl -s "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null && break; sleep 0.1; done
API_LOG="$work/api.log" API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql" python3 -I "$here/test.py" || { cat "$work/api.log"; exit 1; }
if grep -q "ERROR: AddressSanitizer\|runtime error" "$work/api.log"; then cat "$work/api.log"; exit 1; fi
echo "api: checks passed"
