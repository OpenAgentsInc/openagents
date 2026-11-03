#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: compile the Coax smoke and run it)
# Build and run the Cursor client smoke against the installed Cursor CLI,
# on its own login (CURSOR_API_KEY and CURSOR_AUTH_TOKEN never reach the
# child), never opening a browser, in a scratch folder.
#   coax/lib/agent/cursor/smoke.sh
#   SMOKE_CURSOR=/path/to/agent coax/lib/agent/cursor/smoke.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
cursor="${SMOKE_CURSOR:-}"
if [ -z "$cursor" ]; then
  sea="$(ls -d "$HOME"/.local/share/cursor-agent/versions/*/cursor-agent-sea 2>/dev/null | sort | tail -1 || true)"
  if [ -n "$sea" ] && [ -x "$sea" ]; then cursor="$sea"; fi
fi
if [ -z "$cursor" ]; then cursor="$(command -v cursor-agent || true)"; fi
[ -n "$cursor" ] && [ -x "$cursor" ] || { echo "cursor agent not found"; exit 1; }
"$repo/coax/bin/coax" c "$here/smoke.coax" > "$work/smoke.c"
"$cc" -std=c11 -O1 -g -o "$work/smoke" "$work/smoke.c" "$repo/coax/platform/process-native.c" -lm
mkdir "$work/dir"; echo "hello" > "$work/dir/hello.txt"
export BROWSER=/usr/bin/true NO_OPEN_BROWSER=1 SMOKE_CURSOR="$cursor" SMOKE_DIR="$work/dir"
"$work/smoke" | tee "$work/out"
if [ -e "$work/dir/smoke-denied.txt" ]; then echo "deny: the command ran after all"; exit 1; fi
tail -1 "$work/out" | grep -q '^smoke: ok$'
