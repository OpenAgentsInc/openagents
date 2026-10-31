#!/usr/bin/env bash
# Only the new mesh route: throwaway DB, keys and HTTP fleet. Does not rerun
# unrelated API suites or touch any live Pylon/API/server.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
"$here/mesh-policy.sh" --check
pg=/opt/homebrew/opt/postgresql@16
work=$(mktemp -d)
trap 'kill ${pid:-} 2>/dev/null || true; "$pg/bin/pg_ctl" -D "$work/db" -m immediate stop >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
cc=${COAX_CC:-/usr/bin/clang}
secp=$(brew --prefix secp256k1)
ssl=$(brew --prefix openssl@3)
flags=(-std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined -I"$pg/include" -I"$secp/include" -I"$ssl/include")
"$cc" "${flags[@]}" -o "$work/api" "$here"/api.c "$here"/analytics.c "$here"/admin.c "$here"/player.c "$here"/npc.c "$here"/room.c "$here"/private.c "$here"/drops.c "$here"/villa.c "$here"/chat.c "$here"/realms.c "$here"/replies.c "$here"/makers.c "$here"/inference.c "$here"/mesh.c "$here"/mesh-policy.generated.c "$here"/nip98.generated.c "$here"/nostr.c "$here"/relay.c -L"$pg/lib" -lpq -L"$secp/lib" -lsecp256k1 -L"$ssl/lib" -lssl -lcrypto -lcurl -lz -lm
"$cc" "${flags[@]}" -o "$work/sign" "$here/sign.c" -L"$secp/lib" -lsecp256k1 -L"$ssl/lib" -lcrypto
"$pg/bin/initdb" -D "$work/db" -U postgres -A trust >/dev/null
port=$((56000 + RANDOM % 1000)); api_port=$((57000 + RANDOM % 1000)); export MESH_PORT=$((58000 + RANDOM % 1000))
"$pg/bin/pg_ctl" -D "$work/db" -o "-p $port -k $work -c listen_addresses=''" -l "$work/db.log" -w start >/dev/null
"$pg/bin/psql" -h "$work" -p "$port" -U postgres -q -c "CREATE DATABASE mesh_test"
export OA_API_DATABASE="host=$work port=$port user=postgres dbname=mesh_test"
export SIGN="$work/sign" MESH_KEY="$work/key"
"$SIGN" --new "$MESH_KEY" >/dev/null
hash=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
export OA_MESH_ROUTES="{\"elephant-1\":{\"url\":\"http://127.0.0.1:$MESH_PORT/v1/systemone\",\"identity\":\"elephant-1@sha256:$hash\",\"provider\":\"$hash\"}}"
export API="http://127.0.0.1:$api_port" PGURL="$OA_API_DATABASE" PSQL="$pg/bin/psql"
OA_API_ORIGIN="$API" "$work/api" --listen "127.0.0.1:$api_port" --migrations "$here/migrations" 2>"$work/api.log" & pid=$!
for _ in $(seq 100); do curl -fsS "$API/api/playtest/health" >/dev/null 2>&1 && break; sleep 0.1; done
python3 -I "$here/mesh_test.py" || { cat "$work/api.log"; exit 1; }
if rg -q 'ERROR: AddressSanitizer|runtime error:' "$work/api.log"; then cat "$work/api.log"; exit 1; fi
echo 'mesh: sanitized bridge checks passed'
