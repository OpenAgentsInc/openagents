#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: builds gemini-smoke.coax, starts the stand-in, runs it)
# Gemini Decision 1 through our Coax client.
#   coax/lib/systemone/gemini-smoke.sh          the stand-in (gemini_fake.py): golden bytes, every error kind; no network, a random key
#   coax/lib/systemone/gemini-smoke.sh --live   Google: a mixed request, the mapped ticket, an image (about $0.0001)
# Live, the key comes from GEMINI_API_KEY, or from ~/work/.secrets/openagents-gemini-provider.env
# when that is unset, into this process's environment only. It is never printed.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"
work="$(mktemp -d)"; server=""
trap '[ -n "$server" ] && kill "$server" 2>/dev/null; rm -rf "$work"' EXIT
"$repo/coax/bin/coax" c "$here/gemini-smoke.coax" > "$work/smoke.c"
"$cc" -std=c11 -O1 -g -o "$work/smoke" "$work/smoke.c" "$repo/coax/platform/process-native.c" "$repo/coax/platform/store-file-native.c" -lm
cd "$repo"
if [ "${1:-}" = --live ]; then
  if [ -z "${GEMINI_API_KEY:-}" ] && [ -f "$HOME/work/.secrets/openagents-gemini-provider.env" ]; then
    set -a; . "$HOME/work/.secrets/openagents-gemini-provider.env"; set +a
  fi
  [ -n "${GEMINI_API_KEY:-}" ] || { echo "set GEMINI_API_KEY or write it to ~/work/.secrets/openagents-gemini-provider.env"; exit 1; }
  unset GD_SMOKE_URL
  "$work/smoke" | tee "$work/out"
else
  key="gdfake$(od -An -N12 -tx1 /dev/urandom | tr -d ' \n')"
  GD_FAKE_KEY="$key" GD_FAKE_IMAGE="$repo/apps/openagents-web/realms/everglade/hotbar/ray-of-frost-card-1x.jpg" python3 -I "$here/gemini_fake.py" > "$work/port" &
  server=$!
  for _ in $(seq 50); do [ -s "$work/port" ] && break; sleep 0.1; done
  GEMINI_API_KEY="$key" GD_SMOKE_URL="http://127.0.0.1:$(head -1 "$work/port")/" "$work/smoke" > "$work/out" 2>&1 || true
  cat "$work/out"
  if grep -q "$key" "$work/out"; then echo "smoke: the key appeared in the output"; exit 1; fi
fi
tail -1 "$work/out" | grep -q '^smoke: ok$'
