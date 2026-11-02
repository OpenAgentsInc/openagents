#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: names the working directory, then runs the Coax tool)
# Coder in the terminal: builds coder/tui/coder.coax when stale and runs it.
# The claude and codex engines use the installed claude and codex CLIs on
# their own logins; microcoder (coder/tui/mcworker.coax, a process of its
# own) reads its gateway keys from ~/work/.secrets into curl's environment
# only. No credential is exported from here. Shift-Tab
# switches agent.
#   coder/tui/coder.sh [--engine claude|codex|microcoder|echo] [--model M] [--ask] [--cwd DIR]
#   coder/tui/coder.sh run "TASK" [--json] | ls | status ID | log ID   (sessions in ~/.openagents/coder/sessions)
#   coder/tui/coder.sh host [serve|list|allow|deny|forget|key|call]   (coder/tui/host.coax, a tool of its own)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"
export PWD="$(pwd)" CODER_REPO="$repo"
# The host loads the secp256k1 tables, which take its build about 40 s; Coder's own build doesn't pay that.
[[ "${1:-}" != host ]] || exec "$repo/coax/web/tool.sh" "$here/host.coax" "$@"
exec "$repo/coax/web/tool.sh" "$here/coder.coax" "$@"
