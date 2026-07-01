#!/usr/bin/env bash
#
# launch.sh — start (or restart) the durable codex-supervisor in the background.
#
# Runs codex-supervisor.sh under `nohup` + `caffeinate -i` so it survives the
# launching shell/agent ending and keeps the Mac awake. Idempotent: refuses to
# start a second supervisor if one is already alive.
#
# Usage:
#   OPENAGENTS_AGENT_TOKEN=... ./launch.sh            # start
#   ./launch.sh status                                # show pid + tail log
#   ./launch.sh stop                                  # stop the supervisor
#   ./launch.sh wedge-check                           # liveness verdict (#6646)
#   OPENAGENTS_AGENT_TOKEN=... ./launch.sh wedge-watch  # restart IF wedged (#6646)
#
# Run this from a CLEAN worktree at current origin/main (deps installed via
# `bun install --frozen-lockfile`). See the Khala -> Pylon -> Codex runbook.
#
# Wedge auto-restart (#6646): the supervisor's #1 token-burn failure mode is a
# WEDGE — alive + heartbeating but no longer dispatching (an external call hung
# the dispatch loop). `wedge-check` runs the tested liveness check in
# `apps/pylon/src/blueprint-gates/fleet-liveness.ts` against the supervisor pid +
# `$SUP_STATE_DIR/last_dispatch_time`, `$HOME/.pylon/account-quota/*.json`, and
# `$SUP_STATE_DIR/heartbeat_payload.json`; `wedge-watch` additionally force-kills
# and restarts the supervisor when that check reports `wedged`. Run `wedge-watch` on
# an interval from an external watcher (cron / standing loop) with
# OPENAGENTS_AGENT_TOKEN in the environment.
#
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
SUP_STATE_DIR="${SUP_STATE_DIR:-$HOME/.codex-supervisor}"
SUP_LOG="${SUP_LOG:-$SUP_STATE_DIR/supervisor.log}"
PIDFILE="$SUP_STATE_DIR/supervisor.pid"
FLEET_LIVENESS_TS="$REPO_ROOT/apps/pylon/src/blueprint-gates/fleet-liveness.ts"
mkdir -p "$SUP_STATE_DIR"

# bun must be on PATH for the liveness CLI and the detached supervisor.
export PATH="$HOME/.bun/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"

alive() { [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE" 2>/dev/null)" 2>/dev/null; }

# Run the tested liveness check; returns its exit code (0 healthy, 3 wedged,
# 4 unknown) and prints the JSON verdict.
wedge_check() {
  SUP_STATE_DIR="$SUP_STATE_DIR" bun "$FLEET_LIVENESS_TS" --check
}

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
  wedge-check)
    # One-shot liveness verdict; exit code mirrors the check (0/3/4).
    wedge_check
    exit $?
    ;;
  wedge-watch|restart-if-wedged)
    # Force-restart the supervisor ONLY when the liveness check says wedged.
    wedge_check
    code=$?
    if [ "$code" -eq 3 ]; then
      echo "WEDGE DETECTED (#6646): supervisor alive but last_dispatch_time stale > threshold -> force-restarting"
      if [ -f "$PIDFILE" ]; then
        PID="$(cat "$PIDFILE" 2>/dev/null)"
        if [ -n "$PID" ]; then
          kill -TERM "$PID" 2>/dev/null
          for _ in 1 2 3 4 5; do kill -0 "$PID" 2>/dev/null || break; sleep 1; done
          kill -KILL "$PID" 2>/dev/null
          echo "killed wedged supervisor pid=$PID"
        fi
      fi
      rm -f "$PIDFILE"
      # Re-enter via the normal start path (requires OPENAGENTS_AGENT_TOKEN).
      exec bash "$0" start
    fi
    echo "supervisor not wedged (liveness exit=$code); no restart"
    exit 0
    ;;
  start) ;;
  *) echo "usage: launch.sh [start|status|stop|wedge-check|wedge-watch]" >&2; exit 2 ;;
esac

if alive; then
  echo "supervisor already running pid=$(cat "$PIDFILE"); not starting a second one" >&2
  exit 0
fi

if [ -z "${OPENAGENTS_AGENT_TOKEN:-}" ]; then
  echo "FATAL: OPENAGENTS_AGENT_TOKEN must be set in the environment before launch" >&2
  exit 1
fi

echo "launching codex-supervisor (log: $SUP_LOG)"
nohup caffeinate -i bash "$SCRIPT_DIR/codex-supervisor.sh" >> "$SUP_LOG" 2>&1 &
disown || true

sleep 2
if alive; then
  echo "supervisor STARTED pid=$(cat "$PIDFILE")"
else
  echo "supervisor failed to start; see $SUP_LOG" >&2
  tail -n 20 "$SUP_LOG" 2>/dev/null
  exit 1
fi
