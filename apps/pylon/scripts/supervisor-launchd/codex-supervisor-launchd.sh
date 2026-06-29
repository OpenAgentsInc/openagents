#!/usr/bin/env bash
#
# codex-supervisor-launchd.sh (#6408)
#
# launchd entrypoint for the Codex supervisor. Runs the supervisor IN THE
# FOREGROUND (exec) so launchd's KeepAlive restarts it if it ever dies — closing
# the "supervisor crashed and nobody noticed" hole. It sources the owner-linked
# agent token from a LOCAL SECRET (never embedded in the plist or printed),
# resolves the live pylon ref (refs drift), and execs the real supervisor.
#
# Installed via install.sh; driven by com.openagents.codex-supervisor.plist.
set -uo pipefail

REPO_ROOT="${SUP_REPO_ROOT:-__REPO_ROOT__}"

# Owner-linked (Artanis) agent token — the supervisor FIRES khala requests, so
# it must be owner-linked (never the fable token). Read from .secrets; the file
# exports OPENAGENTS_AGENT_TOKEN. Tokens are never echoed.
SUP_SECRET_ENV="${SUP_SECRET_ENV:-$HOME/work/.secrets/openagents-artanis-agent.env}"
if [ -f "$SUP_SECRET_ENV" ]; then
  set -a; . "$SUP_SECRET_ENV"; set +a
fi

# Codex supervisor uses the standing-pylon home by default (registered owner
# home with the linked Codex accounts).
export PYLON_HOME="${PYLON_HOME:-$HOME/.pylon-fable}"
export PYLON_DISABLE_DAEMON_ROUTING="${PYLON_DISABLE_DAEMON_ROUTING:-1}"
export PYLON_OPENAGENTS_BASE_URL="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"

# Resolve the LIVE pylon ref unless one is pinned (the supervisor's hardcoded
# default is stale). Best-effort; the supervisor still runs if this fails.
if [ -z "${SUP_PYLON_REF:-}" ]; then
  live_ref=$(cd "$REPO_ROOT" && bun apps/pylon/src/index.ts provider go-online --json 2>/dev/null \
    | sed -n 's/.*"pylonRef" *: *"\([^"]*\)".*/\1/p' | head -1)
  [ -n "$live_ref" ] && export SUP_PYLON_REF="$live_ref"
fi

export SUP_MAX_SLOTS="${SUP_MAX_SLOTS:-8}"
export SUP_PER_ACCOUNT="${SUP_PER_ACCOUNT:-2}"

cd "$REPO_ROOT" || exit 1
exec bash "$REPO_ROOT/apps/pylon/scripts/codex-supervisor/codex-supervisor.sh"
