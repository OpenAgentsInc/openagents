#!/usr/bin/env bash
# Build and install the Rust cleanup tool. --schedule adds a read-only scan timer.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
dest=$HOME/.openagents/bin/openagents-disk-cleanup
if [ "${1:-}" = --uninstall ]; then
  if [ "$(uname -s)" = Linux ]; then
    systemctl --user disable --now openagents-disk-cleanup.timer 2>/dev/null || true
    rm -f "$HOME/.config/systemd/user/openagents-disk-cleanup.service" "$HOME/.config/systemd/user/openagents-disk-cleanup.timer"
    systemctl --user daemon-reload
  else
    launchctl bootout "gui/$(id -u)/com.openagents.disk-cleanup-scan" 2>/dev/null || true
    rm -f "$HOME/Library/LaunchAgents/com.openagents.disk-cleanup-scan.plist"
  fi
  rm -f "$dest" "$HOME/.openagents/bin/disk-janitor"
  exit 0
fi
case ${1:-} in ''|--schedule) ;; *) echo 'usage: install-disk-janitor.sh [--schedule | --uninstall]' >&2; exit 2 ;; esac
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  if [ "$(uname -s)" = Darwin ]; then
    export CARGO_TARGET_DIR=$HOME/work/openagents-target-1
  else
    export CARGO_TARGET_DIR=$HOME/work/agent-targets/openagents
  fi
fi
(
  cd "$repo"
  . ./scripts/dev/cargo-build-env.sh
  cargo build -p background --bin openagents-disk-cleanup
)
mkdir -p "$(dirname "$dest")"
install -m 0755 "$CARGO_TARGET_DIR/debug/openagents-disk-cleanup" "$dest.new.$$"
mv -f "$dest.new.$$" "$dest"
install -m 0755 "$repo/scripts/dev/disk-janitor.sh" "$HOME/.openagents/bin/disk-janitor.new.$$"
mv -f "$HOME/.openagents/bin/disk-janitor.new.$$" "$HOME/.openagents/bin/disk-janitor"
echo "Installed $dest. Run it with scan; applying requires a saved plan."
[ "${1:-}" = --schedule ] || exit 0
if [ "$(uname -s)" = Linux ]; then
  units=$HOME/.config/systemd/user
  mkdir -p "$units"
  cat > "$units/openagents-disk-cleanup.service" <<EOF
[Unit]
Description=OpenAgents disk cleanup preview
[Service]
Type=oneshot
ExecStart="$dest" scan --json
Nice=10
EOF
  cat > "$units/openagents-disk-cleanup.timer" <<'EOF'
[Unit]
Description=Check old OpenAgents builds and worktrees
[Timer]
OnBootSec=5m
OnUnitActiveSec=10m
[Install]
WantedBy=timers.target
EOF
  systemctl --user daemon-reload
  systemctl --user enable --now openagents-disk-cleanup.timer
else
  plist=$HOME/Library/LaunchAgents/com.openagents.disk-cleanup-scan.plist
  mkdir -p "$(dirname "$plist")" "$HOME/.openagents/logs"
  cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.openagents.disk-cleanup-scan</string>
<key>ProgramArguments</key><array><string>$dest</string><string>scan</string><string>--json</string></array>
<key>StartInterval</key><integer>600</integer>
<key>StandardOutPath</key><string>$HOME/.openagents/logs/disk-cleanup-scan.log</string>
<key>StandardErrorPath</key><string>$HOME/.openagents/logs/disk-cleanup-scan.stderr</string>
</dict></plist>
EOF
  launchctl bootout "gui/$(id -u)/com.openagents.disk-cleanup-scan" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$plist"
fi
echo 'Scheduled read-only scans every ten minutes. Nothing is deleted by the timer.'
