#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: names the working directory, then runs the Coax tool)
# Coder in the terminal: builds coder/tui/coder.coax when stale and runs it.
# The claude engine uses the installed claude CLI on its own login; it needs
# no credentials from here.
#   coder/tui/coder.sh [--engine claude|echo] [--model M] [--cwd DIR]
#   coder/tui/coder.sh --print "TASK"
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
export PWD="$(pwd)"
exec "$repo/coax/web/tool.sh" "$here/coder.coax" "$@"
