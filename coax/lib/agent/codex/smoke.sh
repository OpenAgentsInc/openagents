#!/usr/bin/env bash
# Build and run the Codex client smoke against the installed codex CLI, on
# its own ChatGPT login (OPENAI_API_KEY and CODEX_API_KEY never reach the
# child), never opening a browser, in a scratch folder.
#   coax/lib/agent/codex/smoke.sh
#   SMOKE_CODEX=/path/to/codex coax/lib/agent/codex/smoke.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
codex="${SMOKE_CODEX:-$(command -v codex || ls -d "$HOME"/.nvm/versions/node/*/bin/codex 2>/dev/null | tail -1)}"
[ -x "$codex" ] || { echo "codex not found"; exit 1; }
"$repo/coax/bin/coax" c "$here/smoke.coax" > "$work/smoke.c"
"$cc" -std=c11 -O1 -g -o "$work/smoke" "$work/smoke.c" "$repo/coax/platform/process-native.c" -lm
mkdir "$work/dir"; echo "hello" > "$work/dir/hello.txt"
export BROWSER=/usr/bin/true NO_OPEN_BROWSER=1 SMOKE_CODEX="$codex" SMOKE_DIR="$work/dir"
export PATH="$(dirname "$codex"):$PATH"
"$work/smoke" | tee "$work/out"
if [ -e "$work/dir/smoke-denied.txt" ]; then echo "deny: the command ran after all"; exit 1; fi
tail -1 "$work/out" | grep -q '^smoke: ok$'
