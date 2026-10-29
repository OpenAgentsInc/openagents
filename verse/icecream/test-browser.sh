#!/usr/bin/env bash
# Ice Cream Man in the browser replays on the rules: plays the realm in
# headless Chrome (the bot, then real key and mouse events), takes the trace
# the page hands out, and replays it natively step for step (replay.c).
#   verse/icecream/test-browser.sh [BASE_URL]
# BASE_URL defaults to http://127.0.0.1:8724 (coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8724).
set -euo pipefail
cd "$(dirname "$0")/../.."
base="${1:-http://127.0.0.1:8724}"
work=$(mktemp -d "${TMPDIR:-/tmp}/verse-icecream-browser.XXXXXX")
trap 'rm -rf "$work"' EXIT
coax/bin/coax c verse/icecream/rules.coax --library --header "$work/IceCream.h" > "$work/IceCream.c"
"${CC:-/usr/bin/clang}" -std=c11 -O1 -ffp-contract=off -Wno-unused-value -I "$work" "$work/IceCream.c" verse/icecream/replay.c -lm -o "$work/replay"
bun verse/icecream/tools/browser-trace.ts "$base/realms/ice-cream-man#seed=4&bot&trace=600" "$work/bot.trace"
bun verse/icecream/tools/browser-trace.ts "$base/realms/ice-cream-man#seed=6&trace=300" "$work/keys.trace" 1280x800 --keys
bun verse/icecream/tools/browser-trace.ts "$base/realms/ice-cream-man#seed=2&bot&hard&trace=1800" "$work/hard.trace"
"$work/replay" "$work"/*.trace
