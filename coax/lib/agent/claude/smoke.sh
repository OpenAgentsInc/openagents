#!/usr/bin/env bash
# Build and run the Claude Agent SDK smoke against the installed claude CLI.
# It runs on the Claude Code login (ANTHROPIC_API_KEY is removed from the
# child), never opens a browser, saves no session and loads no settings.
#
#   coax/lib/agent/claude/smoke.sh                  # the kill check, then the live queries
#   SMOKE_SKIP_LIVE=1 coax/lib/agent/claude/smoke.sh  # the kill check only
#   SMOKE_MODEL=sonnet coax/lib/agent/claude/smoke.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../../.." && pwd)"
cc="${COAX_CC:-/usr/bin/clang}"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$repo/coax/bin/coax" c "$here/smoke.coax" > "$work/smoke.c"
"$cc" -std=c11 -O1 -g -o "$work/smoke" "$work/smoke.c" "$repo/coax/platform/process-native.c" -lm
cat > "$work/fake-claude" <<'FAKE'
#!/bin/sh
sleep 300 &
echo '{"type":"keep_alive"}'
wait
FAKE
chmod +x "$work/fake-claude"
cat > "$work/slow-claude" <<'SLOW'
#!/bin/sh
sleep 0.3
echo '{"type":"keep_alive"}'
sleep 300
SLOW
chmod +x "$work/slow-claude"
mkdir "$work/deny"
export BROWSER=/usr/bin/true NO_OPEN_BROWSER=1 SMOKE_FAKE_CLI="$work/fake-claude" SMOKE_SLOW_CLI="$work/slow-claude" SMOKE_DIR="$work/deny"
"$work/smoke" | tee "$work/out"
if [ -e "$work/deny/smoke-denied.txt" ]; then echo "deny: Bash ran after all"; exit 1; fi
tail -1 "$work/out" | grep -q '^smoke: ok$'
