#!/usr/bin/env bash
# Firm reason: 5 (OS service manager glue: installs macOS launchd LaunchAgent or Linux systemd --user unit for headless coder-host --tailnet-listen).
set -euo pipefail

BIN="${OPENAGENTS_HOST_BIN:-$HOME/.openagents/bin/coder-host}"
PORT="${OPENAGENTS_TAILNET_PORT:-47109}"
LOG_DIR="${HOME}/.openagents/logs"
mkdir -p "$LOG_DIR"

if [[ ! -x "$BIN" ]]; then
  echo "coder-host binary not found or not executable at: $BIN" >&2
  exit 1
fi

OS="$(uname -s)"
if [[ "$OS" == "Darwin" ]]; then
  PLIST_DIR="$HOME/Library/LaunchAgents"
  PLIST="$PLIST_DIR/com.openagents.coder-host.plist"
  mkdir -p "$PLIST_DIR"
  cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.openagents.coder-host</string>
  <key>ProgramArguments</key>
  <array>
    <string>${BIN}</string>
    <string>serve</string>
    <string>--no-relay</string>
    <string>--tailnet-listen</string>
    <string>--tailnet-port</string>
    <string>${PORT}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>${HOME}/.openagents/bin:${HOME}/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>
  </dict>
  <key>StandardOutPath</key>
  <string>${LOG_DIR}/coder-host.out.log</string>
  <key>StandardErrorPath</key>
  <string>${LOG_DIR}/coder-host.err.log</string>
</dict>
</plist>
EOF
  if [[ "${OPENAGENTS_SKIP_LOAD:-0}" != "1" ]]; then
    launchctl bootout "gui/$(id -u)" "$PLIST" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$(id -u)" "$PLIST"
  fi
  echo "Installed macOS LaunchAgent: $PLIST (port $PORT)"
elif [[ "$OS" == "Linux" ]]; then
  UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
  UNIT="$UNIT_DIR/openagents-coder-host.service"
  mkdir -p "$UNIT_DIR"
  cat > "$UNIT" <<EOF
[Unit]
Description=OpenAgents Headless Antigravity Host (Tailscale Direct)
After=network-online.target tailscaled.service

[Service]
Type=simple
ExecStart=${BIN} serve --no-relay --tailnet-listen --tailnet-port ${PORT}
Restart=always
RestartSec=3
Environment=PATH=${HOME}/.openagents/bin:${HOME}/.local/bin:/usr/local/bin:/usr/bin:/bin
StandardOutput=append:${LOG_DIR}/coder-host.out.log
StandardError=append:${LOG_DIR}/coder-host.err.log

[Install]
WantedBy=default.target
EOF
  if [[ "${OPENAGENTS_SKIP_LOAD:-0}" != "1" ]] && command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload
    systemctl --user enable --now openagents-coder-host.service
  fi
  echo "Installed Linux systemd --user unit: $UNIT (port $PORT)"
else
  echo "Unsupported OS for service installation: $OS" >&2
  exit 1
fi
