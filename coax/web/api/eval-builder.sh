#!/usr/bin/env bash
# The realm builder's eval (byo-never-drop): twenty prompts go to the real planner (Gemini on
# Vertex, as production asks it) through a throwaway api and Postgres on this Mac, and a second
# model reads each prompt and its board and says, for every thing asked for, whether the board
# shows it, lists it as still wanted, or neither. eval_builder.py scores it; nothing matches words.
#   coax/web/api/eval-builder.sh [--only N] [--out FILE]
# Needs a key for a service account that may call Vertex: OA_EVAL_SA_KEY, else
# ~/work/.secrets/vertex-sa-inference.json. Its token stays in this script's environment.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
pg=/opt/homebrew/opt/postgresql@16
work="$(mktemp -d)"; trap 'kill ${api_pid:-} 2>/dev/null || true; "$pg/bin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
sa="${OA_EVAL_SA_KEY:-$HOME/work/.secrets/vertex-sa-inference.json}"
mkdir -p "$work/gcloud"
CLOUDSDK_CONFIG="$work/gcloud" gcloud auth activate-service-account --key-file="$sa" >/dev/null 2>&1
OA_NPC_TOKEN="$(CLOUDSDK_CONFIG="$work/gcloud" gcloud auth print-access-token 2>/dev/null)"
export OA_NPC_TOKEN
cc=/usr/bin/clang
secp="$(brew --prefix secp256k1 2>/dev/null || true)"
$cc -std=c11 -O1 -w -D_DEFAULT_SOURCE \
  -I"$pg/include" ${secp:+-I"$secp/include"} -I"$(brew --prefix openssl@3)/include" \
  -o "$work/api" "$here"/api.c "$here"/analytics.c "$here"/admin.c "$here"/player.c "$here"/npc.c "$here"/room.c "$here"/private.c "$here"/drops.c "$here"/villa.c "$here"/chat.c "$here"/realms.c "$here"/replies.c "$here"/makers.c "$here"/inference.c "$here"/mesh.c "$here"/mesh-policy.generated.c "$here"/nip98.generated.c "$here"/nostr.c "$here"/relay.c \
  -L"$pg/lib" -lpq ${secp:+-L"$secp/lib"} -lsecp256k1 -L"$(brew --prefix openssl@3)/lib" -lssl -lcrypto -lcurl -lz -lm
"$pg/bin/initdb" -D "$work/db" -U postgres -A trust >/dev/null
port=$((56000 + RANDOM % 1000)); api_port=$((57000 + RANDOM % 1000))
"$pg/bin/pg_ctl" -D "$work/db" -o "-p $port -k $work -c listen_addresses=''" -l "$work/pg.log" -w start >/dev/null
"$pg/bin/psql" -h "$work" -p $port -U postgres -q -c "CREATE ROLE oa_analytics LOGIN" -c "CREATE DATABASE oa_analytics OWNER oa_analytics"
export OA_API_DATABASE="host=$work port=$port user=oa_analytics dbname=oa_analytics"
OA_BUILDER_TIMEOUT_MS=20000 OA_BUILDER_PER_DAY=100 OA_API_ORIGIN="http://127.0.0.1:$api_port" \
  "$work/api" --listen 127.0.0.1:$api_port --migrations "$here/migrations" 2>"$work/api.log" & api_pid=$!
for _ in $(seq 100); do curl -s "http://127.0.0.1:$api_port/api/playtest/health" >/dev/null && break; sleep 0.1; done
API="http://127.0.0.1:$api_port" python3 -I "$here/eval_builder.py" "$@" || { tail -20 "$work/api.log"; exit 1; }
