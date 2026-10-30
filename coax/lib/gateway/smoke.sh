#!/usr/bin/env bash
# The gateway transport against a local stand-in provider (smoke_server.py):
# deltas must arrive before the response ends, the key must reach the
# server's header only (never curl's arguments, the output or a refusal),
# a 401 must refuse once, and an unreachable server must refuse once. No
# network and no real key: the key is a random sentinel.
#
#   coax/lib/gateway/smoke.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"; coax="${COAX:-$repo/coax/bin/coax}"
work="$(mktemp -d)"; server=""
trap '[ -n "$server" ] && kill "$server" 2>/dev/null; rm -rf "$work"' EXIT
"$coax" c "$here/smoke.coax" > "$work/smoke.c"
"$cc" -std=c11 -O1 -g -o "$work/smoke" "$work/smoke.c" "$repo/coax/platform/process-native.c" -lm
key="gwsmoke$(od -An -N12 -tx1 /dev/urandom | tr -d ' \n')"
GW_SMOKE_EXPECT="$key" python3 -I "$here/smoke_server.py" > "$work/port" &
server=$!
for _ in $(seq 50); do [ -s "$work/port" ] && break; sleep 0.1; done
port="$(head -1 "$work/port")"
GW_SMOKE_KEY="$key" GW_SMOKE_URL="http://127.0.0.1:$port/" "$work/smoke" > "$work/out" 2>&1 || true
cat "$work/out"
if grep -q "$key" "$work/out"; then echo "smoke: the key appeared in the output"; exit 1; fi
tail -1 "$work/out" | grep -q '^smoke: ok$'
