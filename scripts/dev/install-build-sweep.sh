#!/usr/bin/env bash
# Installs the build sweep (coder_lease::agent_builds::sweep, #12023) as a
# launchd agent on this Mac.
#
# It builds openagents-build-sweep in the shared root target, copies it to
# ~/.openagents/bin/openagents-build-sweep (so it keeps running whatever
# checkout is on disk), and loads com.openagents.build-sweep, which runs it
# every 5 minutes with no agent session. Each run keeps 100 GiB free: it caps
# every shared Cargo target at 120 GiB by removing its least recently used
# artifacts, removes whole targets no build holds when free space is under
# the floor, and flags or removes target copies beyond
# ~/work/openagents-target-1 and ~/work/openagents-target-1-app.
# docs/coder/runtime/leases.md ("The build sweep") has the rules.
#
# Usage: install-build-sweep.sh             build, install, and run once now
#        install-build-sweep.sh --dry-run   build and print what a run would
#                                           remove; install nothing
#        install-build-sweep.sh --uninstall stop and remove the agent
# Pause without uninstalling: touch ~/.openagents/build-sweep.off
# Log: ~/.openagents/logs/build-sweep.log (last run: build-sweep.last.json)
set -euo pipefail

LABEL=com.openagents.build-sweep
PLIST=$HOME/Library/LaunchAgents/$LABEL.plist
BIN=$HOME/.openagents/bin/openagents-build-sweep
DOMAIN=gui/$(id -u)

if [ "${1:-}" = "--uninstall" ]; then
  launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
  rm -f "$PLIST" "$BIN"
  echo "build sweep removed"
  exit 0
fi

repo=$(cd "$(dirname "$0")/../.." && pwd)
target=${CARGO_TARGET_DIR:-$HOME/work/openagents-target-1}
(cd "$repo" && OPENAGENTS_KEEP_TARGET_DIR=1 CARGO_TARGET_DIR=$target \
  cargo build -q -p coder-lease --bin openagents-build-sweep)
built=$target/debug/openagents-build-sweep

if [ "${1:-}" = "--dry-run" ]; then
  exec "$built" --dry-run
fi

mkdir -p "$(dirname "$BIN")" "$HOME/.openagents/logs" "$(dirname "$PLIST")"
install -m 0755 "$built" "$BIN"

cat >"$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key>
  <array><string>$BIN</string></array>
  <key>StartInterval</key><integer>300</integer>
  <key>RunAtLoad</key><true/>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
  <key>ProcessType</key><string>Background</string>
  <key>EnvironmentVariables</key>
  <dict><key>PATH</key><string>/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
  <key>StandardOutPath</key><string>$HOME/.openagents/logs/build-sweep.launchd.log</string>
  <key>StandardErrorPath</key><string>$HOME/.openagents/logs/build-sweep.launchd.log</string>
</dict>
</plist>
EOF

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
launchctl bootstrap "$DOMAIN" "$PLIST"
echo "build sweep installed: $BIN, every 5 minutes ($PLIST)"
echo "log: ~/.openagents/logs/build-sweep.log"
echo "pause: touch ~/.openagents/build-sweep.off; remove: $0 --uninstall"
