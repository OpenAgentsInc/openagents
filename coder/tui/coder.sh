#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: names the working directory, then runs the Coax tool)
# Coder in the terminal: builds coder/tui/coder.coax when stale and runs it.
# The claude and codex engines use the installed claude and codex CLIs on
# their own logins; they need no credentials from here. Shift-Tab switches.
#   coder/tui/coder.sh [--engine claude|codex|echo] [--model M] [--ask] [--cwd DIR]
#   coder/tui/coder.sh --print "TASK"
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
export PWD="$(pwd)"
exec "$repo/coax/web/tool.sh" "$here/coder.coax" "$@"
