#!/usr/bin/env bash
# Run this Mac's Coder host from a development build of this checkout
# (#10118), in place of the host the installed OpenAgents.app registers,
# so the phone reaches current code without packaging the app.
#
#   scripts/desktop/dev-host.sh install [--no-build]
#   scripts/desktop/dev-host.sh status
#   scripts/desktop/dev-host.sh uninstall
#
# install builds `coder` and `microcoder` (release profile) from this
# checkout, copies them to ~/.openagents/dev-host/COMMIT/, and signs them
# with the keychain's Developer ID Application identity under the
# installed host's identifier (com.openagents.desktop.coder) and
# entitlements: the host's keychain items (service com.openagents.desktop)
# are bound to that designated requirement, so the same host keys, and so
# every paired phone, keep working with no prompt. It then
#   - copies the new microcoder over ~/.openagents/bin/microcoder (the
#     auto-start controller), keeping the old one as microcoder.before-dev-host;
#   - stops and disables the app's login agent com.openagents.desktop.host;
#   - installs and starts ~/Library/LaunchAgents/com.openagents.dev.host.plist
#     (backing up any earlier one), which runs
#     `coder host serve --keychain --iroh --control` as the app's does, with
#     the login shell's PATH, and logs to ~/.openagents/dev-host/host.log.
# Install again after pulling to move the host to the new code.
# uninstall stops the development host, removes its plist, puts the old
# microcoder back, and enables the app's login agent again; it starts the
# next time OpenAgents.app opens (or at login).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
label="com.openagents.dev.host"
app_label="com.openagents.desktop.host"
domain="gui/$(id -u)"
plist="$HOME/Library/LaunchAgents/$label.plist"
base="$HOME/.openagents/dev-host"
bin="$HOME/.openagents/bin"
command="${1:-status}"
[ $# -gt 0 ] && shift
build=1
for arg in "$@"; do
  case "$arg" in
    --no-build) build=0 ;;
    *) echo "unknown option $arg" >&2; exit 64 ;;
  esac
done

running_pid() { launchctl print "$domain/$1" 2>/dev/null | sed -n 's/^[[:space:]]*pid = //p' | head -1; }

case "$command" in
  status)
    echo "development host: $(running_pid "$label" || true)"
    [ -L "$base/current" ] && "$base/current/coder" --version
    echo "app's host: $(running_pid "$app_label" || true)"
    launchctl print-disabled "$domain" | grep -F "$app_label" || true ;;
  install)
    [ "$(uname -s)" = Darwin ] || { echo "macOS only" >&2; exit 64; }
    identity="${OA_DEVELOPER_ID_APPLICATION:-$(security find-identity -v -p codesigning |
      sed -n 's/.*[0-9A-F]\{40\} "\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)}"
    [ -n "$identity" ] || { echo "no Developer ID Application identity in the keychain" >&2; exit 1; }
    if [ "$build" = 1 ]; then
      (cd "$root" && cargo build --release -p coder --bin coder -p microcoder --bin microcoder)
    fi
    commit="$(git -C "$root" rev-parse --short=10 HEAD)"
    dir="$base/$commit"
    mkdir -p "$dir"
    cp "$CARGO_TARGET_DIR/release/coder" "$CARGO_TARGET_DIR/release/microcoder" "$dir/"
    for name in coder microcoder; do
      codesign --force --options runtime --timestamp --sign "$identity" \
        --entitlements "$root/bins/openagents-desktop-macos/host.entitlements" \
        --identifier "com.openagents.desktop.$name" "$dir/$name"
    done
    codesign --verify --strict "$dir/coder" "$dir/microcoder"
    ln -sfn "$dir" "$base/current"
    if [ -f "$bin/microcoder" ] && [ ! -f "$bin/microcoder.before-dev-host" ]; then
      cp -p "$bin/microcoder" "$bin/microcoder.before-dev-host"
    fi
    mkdir -p "$bin"
    cp "$dir/microcoder" "$bin/microcoder.new" && mv -f "$bin/microcoder.new" "$bin/microcoder"
    login_path="$("${SHELL:-/bin/zsh}" -l -c 'printf %s "$PATH"' 2>/dev/null || true)"
    [ -n "$login_path" ] || login_path="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    if [ -f "$plist" ]; then
      cp -p "$plist" "$base/$label.plist.bak.$(date +%Y%m%d%H%M%S)"
    fi
    cat > "$plist.new" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- The development Coder host (scripts/desktop/dev-host.sh), in place of
     the app's com.openagents.desktop.host. -->
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>$base/current/coder</string>
    <string>host</string>
    <string>serve</string>
    <string>--keychain</string>
    <string>--iroh</string>
    <string>--control</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>$login_path</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>ThrottleInterval</key>
  <integer>1</integer>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>$base/host.log</string>
  <key>StandardErrorPath</key>
  <string>$base/host.log</string>
</dict>
</plist>
PLIST
    plutil -lint -s "$plist.new"
    mv -f "$plist.new" "$plist"
    # One host at a time: the app's stops and stays off until uninstall.
    launchctl disable "$domain/$app_label" 2>/dev/null || true
    launchctl bootout "$domain/$app_label" 2>/dev/null || true
    launchctl bootout "$domain/$label" 2>/dev/null || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
      [ -z "$(running_pid "$label")" ] && [ -z "$(running_pid "$app_label")" ] && break
      sleep 0.5
    done
    launchctl bootstrap "$domain" "$plist"
    launchctl enable "$domain/$label"
    sleep 3
    pid="$(running_pid "$label")"
    [ -n "$pid" ] || { echo "the development host did not start; see $base/host.log" >&2; exit 1; }
    echo "development host $commit running (pid $pid)" ;;
  uninstall)
    launchctl bootout "$domain/$label" 2>/dev/null || true
    [ -f "$plist" ] && mv -f "$plist" "$base/$label.plist.removed.$(date +%Y%m%d%H%M%S)"
    if [ -f "$bin/microcoder.before-dev-host" ]; then
      mv -f "$bin/microcoder.before-dev-host" "$bin/microcoder"
    fi
    launchctl enable "$domain/$app_label"
    echo "development host removed; the app's host starts when OpenAgents.app opens or at login" ;;
  *) echo "usage: scripts/desktop/dev-host.sh install [--no-build]|status|uninstall" >&2; exit 64 ;;
esac
