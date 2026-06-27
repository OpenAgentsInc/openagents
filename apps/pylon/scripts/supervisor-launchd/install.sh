#!/usr/bin/env bash
#
# install.sh (#6408) — install/uninstall the launchd KeepAlive jobs for the
# Codex + Claude supervisors so they auto-restart if they die (closing the
# "supervisor crashed and nobody noticed" hole). Mirrors the standing-pylon job
# `com.openagents.pylon.fable`.
#
# Usage:
#   bash apps/pylon/scripts/supervisor-launchd/install.sh install [codex|claude|both]
#   bash apps/pylon/scripts/supervisor-launchd/install.sh uninstall [codex|claude|both]
#   bash apps/pylon/scripts/supervisor-launchd/install.sh status
#
# It substitutes the absolute repo root + home into the plist, copies it to
# ~/Library/LaunchAgents, and bootstraps it into the per-user GUI domain.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
LAUNCH_AGENTS="$HOME/Library/LaunchAgents"
ACTION="${1:-status}"
WHICH="${2:-both}"

uid="$(id -u)"

plist_label() { echo "com.openagents.$1-supervisor"; }

render_and_install() {
  local kind="$1"
  local label; label="$(plist_label "$kind")"
  local src="$SCRIPT_DIR/$label.plist"
  local dst="$LAUNCH_AGENTS/$label.plist"
  mkdir -p "$LAUNCH_AGENTS" "$HOME/.$kind-supervisor"
  sed -e "s|__REPO_ROOT__|$REPO_ROOT|g" -e "s|__HOME__|$HOME|g" "$src" > "$dst"
  chmod +x "$SCRIPT_DIR/$kind-supervisor-launchd.sh"
  # Idempotent re-bootstrap.
  launchctl bootout "gui/$uid/$label" 2>/dev/null || true
  launchctl bootstrap "gui/$uid" "$dst"
  launchctl enable "gui/$uid/$label" 2>/dev/null || true
  echo "installed: $label -> $dst"
}

uninstall_one() {
  local kind="$1"
  local label; label="$(plist_label "$kind")"
  launchctl bootout "gui/$uid/$label" 2>/dev/null || true
  rm -f "$LAUNCH_AGENTS/$label.plist"
  echo "uninstalled: $label"
}

targets() {
  case "$WHICH" in
    codex) echo codex ;;
    claude) echo claude ;;
    both|"") echo codex claude ;;
    *) echo "unknown target: $WHICH" >&2; exit 1 ;;
  esac
}

case "$ACTION" in
  install)
    for k in $(targets); do render_and_install "$k"; done
    echo "done. tail logs: tail -f ~/.codex-supervisor/supervisor.log ~/.claude-supervisor/supervisor.log"
    ;;
  uninstall)
    for k in $(targets); do uninstall_one "$k"; done
    ;;
  status)
    launchctl list | grep -E 'com\.openagents\.(codex|claude)-supervisor' || echo "no supervisor jobs loaded"
    ;;
  *)
    echo "usage: install.sh [install|uninstall|status] [codex|claude|both]" >&2
    exit 1
    ;;
esac
