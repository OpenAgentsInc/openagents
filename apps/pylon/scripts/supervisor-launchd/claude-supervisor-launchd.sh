#!/usr/bin/env bash
#
# claude-supervisor-launchd.sh (#6408)
#
# launchd entrypoint for the Claude supervisor. Runs the supervisor IN THE
# FOREGROUND (exec) so launchd's KeepAlive restarts it if it ever dies. Sources
# the owner-linked agent token from a LOCAL SECRET (never embedded in the plist
# or printed), resolves the live pylon ref, and execs the real supervisor.
#
# Installed via install.sh; driven by com.openagents.claude-supervisor.plist.
set -uo pipefail

REPO_ROOT="${SUP_REPO_ROOT:-__REPO_ROOT__}"

# launchd's GUI-domain agents get a minimal PATH (no bun, no Homebrew) — add
# the common install locations so `bun` resolves without needing an absolute
# path baked in (verified needed against the sibling runtime-supervisor job:
# `exec: bun: not found` before this was added).
export PATH="$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

SUP_SECRET_ENV="${SUP_SECRET_ENV:-$HOME/work/.secrets/openagents-artanis-agent.env}"
if [ -f "$SUP_SECRET_ENV" ]; then
  set -a; . "$SUP_SECRET_ENV"; set +a
fi

export PYLON_HOME="${PYLON_HOME:-$HOME/.pylon-fable}"
export PYLON_DISABLE_DAEMON_ROUTING="${PYLON_DISABLE_DAEMON_ROUTING:-1}"
export PYLON_OPENAGENTS_BASE_URL="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"
# Distinct state dir so the Claude supervisor never collides with the Codex one.
export SUP_STATE_DIR="${SUP_STATE_DIR:-$HOME/.claude-supervisor}"

if [ -z "${SUP_PYLON_REF:-}" ]; then
  live_ref=$(cd "$REPO_ROOT" && bun apps/pylon/src/index.ts provider go-online --json 2>/dev/null \
    | sed -n 's/.*"pylonRef" *: *"\([^"]*\)".*/\1/p' | head -1)
  [ -n "$live_ref" ] && export SUP_PYLON_REF="$live_ref"
fi

export SUP_MAX_SLOTS="${SUP_MAX_SLOTS:-8}"
export SUP_PER_ACCOUNT="${SUP_PER_ACCOUNT:-2}"

cd "$REPO_ROOT" || exit 1
exec bash "$REPO_ROOT/apps/pylon/scripts/claude-supervisor/claude-supervisor.sh"
