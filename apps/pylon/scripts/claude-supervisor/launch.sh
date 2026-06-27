#!/usr/bin/env bash
#
# launch.sh — start (or restart) the durable claude-supervisor in the background.
#
# Runs claude-supervisor.sh under `nohup` + `caffeinate -i` so it survives the
# launching shell/agent ending and keeps the Mac awake. Idempotent: refuses to
# start a second supervisor if one is already alive.
#
# Usage:
#   OPENAGENTS_AGENT_TOKEN=... ./launch.sh            # start
#   ./launch.sh status                                # show pid + tail log
#   ./launch.sh stop                                  # stop the supervisor
#
# Run this from a CLEAN worktree at current origin/main (deps installed via
# `bun install --frozen-lockfile`). See the Khala -> Pylon -> Claude runbook.
#
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SUP_STATE_DIR="${SUP_STATE_DIR:-$HOME/.claude-supervisor}"
SUP_LOG="${SUP_LOG:-$SUP_STATE_DIR/supervisor.log}"
PIDFILE="$SUP_STATE_DIR/supervisor.pid"
mkdir -p "$SUP_STATE_DIR"

alive() { [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE" 2>/dev/null)" 2>/dev/null; }

case "${1:-start}" in
  status)
    if alive; then
      echo "supervisor RUNNING pid=$(cat "$PIDFILE")"
    else
      echo "supervisor NOT running"
    fi
    echo "--- last 30 log lines ($SUP_LOG) ---"
    tail -n 30 "$SUP_LOG" 2>/dev/null
    exit 0
    ;;
  stop)
    if alive; then
      PID="$(cat "$PIDFILE")"
      kill "$PID" 2>/dev/null
      echo "sent TERM to supervisor pid=$PID"
    else
      echo "supervisor not running"
    fi
    exit 0
    ;;
  start) ;;
  *) echo "usage: launch.sh [start|status|stop]" >&2; exit 2 ;;
esac

if alive; then
  echo "supervisor already running pid=$(cat "$PIDFILE"); not starting a second one" >&2
  exit 0
fi

if [ -z "${OPENAGENTS_AGENT_TOKEN:-}" ]; then
  echo "FATAL: OPENAGENTS_AGENT_TOKEN must be set in the environment before launch" >&2
  exit 1
fi

# Ensure bun is on PATH for the detached process.
export PATH="$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"

echo "launching claude-supervisor (log: $SUP_LOG)"
nohup caffeinate -i bash "$SCRIPT_DIR/claude-supervisor.sh" >> "$SUP_LOG" 2>&1 &
disown || true

sleep 2
if alive; then
  echo "supervisor STARTED pid=$(cat "$PIDFILE")"
else
  echo "supervisor failed to start; see $SUP_LOG" >&2
  tail -n 20 "$SUP_LOG" 2>/dev/null
  exit 1
fi
