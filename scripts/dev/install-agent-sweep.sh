#!/usr/bin/env bash
# Installs the nightly agent sweep (#11433) as a launchd agent on this Mac.
#
# Every night at 04:00 it runs `agent-run.py sweep --kill`: launchd-parented
# claude, coder-new and bench processes and orphaned Claude Code tool shells
# with no terminal that are older than
# OA_SWEEP_HOURS (default 12), the leftovers of runs whose session is gone,
# and this host's run-labelled VMs whose run is gone. Interactive sessions on
# a terminal are never touched. See what it would stop first:
#   scripts/dev/stop-agent-burn.sh --sweep
#
# Usage: install-agent-sweep.sh              install or update
#        install-agent-sweep.sh --uninstall  stop and remove the agent
set -euo pipefail

LABEL=com.openagents.agent-sweep
PLIST=$HOME/Library/LaunchAgents/$LABEL.plist
BIN=$HOME/.openagents/bin/agent-run.py
DOMAIN=gui/$(id -u)
HOURS=${OA_SWEEP_HOURS:-12}

if [ "${1:-}" = "--uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST" "$BIN"
  echo "agent-sweep removed"
  exit 0
fi

src=$(cd "$(dirname "$0")" && pwd)/agent-run.py
mkdir -p "$(dirname "$BIN")" "$HOME/.openagents/logs" "$(dirname "$PLIST")"
install -m 0755 "$src" "$BIN"

cat >"$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array><string>/usr/bin/python3</string><string>$BIN</string><string>sweep</string><string>--hours</string><string>$HOURS</string><string>--kill</string></array>
  <key>StartCalendarInterval</key>
  <dict><key>Hour</key><integer>4</integer><key>Minute</key><integer>0</integer></dict>
  <key>ProcessType</key><string>Background</string>
  <key>EnvironmentVariables</key>
  <dict><key>PATH</key><string>$HOME/google-cloud-sdk/bin:/opt/homebrew/bin:/opt/homebrew/share/google-cloud-sdk/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
  <key>StandardOutPath</key><string>$HOME/.openagents/logs/agent-sweep.log</string>
  <key>StandardErrorPath</key><string>$HOME/.openagents/logs/agent-sweep.log</string>
</dict>
</plist>
PL

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "agent-sweep installed: nightly 04:00, processes older than ${HOURS}h ($PLIST)"
echo "log: ~/.openagents/logs/agent-sweep.log; remove: $0 --uninstall"
