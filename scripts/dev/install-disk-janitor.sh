#!/usr/bin/env bash
# Installs disk-janitor.sh as a launchd agent on this Mac.
#
# It copies the script to ~/.openagents/bin/disk-janitor (so it keeps running
# whatever checkout is on disk) and loads com.openagents.disk-janitor, which
# starts it every 10 minutes; the script itself does a full pass hourly, or at
# once when free space is under JANITOR_LOW_GB (150 GB by default).
# `openagents lease build` also starts it when a build is short of disk.
#
# Usage: install-disk-janitor.sh            install or update, and run once now
#        install-disk-janitor.sh --uninstall stop and remove the agent
# Pause without uninstalling: touch ~/.openagents/disk-janitor.off
set -euo pipefail

LABEL=com.openagents.disk-janitor
PLIST=$HOME/Library/LaunchAgents/$LABEL.plist
BIN=$HOME/.openagents/bin/disk-janitor
DOMAIN=gui/$(id -u)

if [ "${1:-}" = "--uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST" "$BIN"
  echo "disk-janitor removed"
  exit 0
fi

src=$(cd "$(dirname "$0")" && pwd)/disk-janitor.sh
mkdir -p "$(dirname "$BIN")" "$HOME/.openagents/logs" "$(dirname "$PLIST")"
install -m 0755 "$src" "$BIN"

cat >"$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array><string>/bin/bash</string><string>$BIN</string></array>
  <key>StartInterval</key><integer>600</integer>
  <key>RunAtLoad</key><true/>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
  <key>ProcessType</key><string>Background</string>
  <key>EnvironmentVariables</key>
  <dict><key>PATH</key><string>$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
  <key>StandardOutPath</key><string>$HOME/.openagents/logs/disk-janitor.launchd.log</string>
  <key>StandardErrorPath</key><string>$HOME/.openagents/logs/disk-janitor.launchd.log</string>
</dict>
</plist>
EOF

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "disk-janitor installed: $BIN, every 10 minutes ($PLIST)"
echo "log: ~/.openagents/logs/disk-janitor.log; keep list: ~/.openagents/disk-janitor.keep"
echo "pause: touch ~/.openagents/disk-janitor.off; remove: $0 --uninstall"
