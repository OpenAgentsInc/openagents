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
# Install again after pulling to move the host to the new code, or let
# the host follow main on its own:
#
#   scripts/desktop/dev-host.sh follow-on [--every MINUTES]
#   scripts/desktop/dev-host.sh follow-off
#
# follow-on keeps a detached checkout of origin/main at
# ~/.openagents/dev-host/checkout (never your working tree) and installs a
# launchd agent, com.openagents.dev.host.follow, that runs `follow` from it
# every 15 minutes (or MINUTES). Each `follow` pass fetches origin/main; when
# it differs from the installed host, it builds coder and microcoder in
# OA_DEV_HOST_TARGET (default ~/work/openagents-target-devhost), and installs
# them only while no task or turn is running, so a restart never cuts work
# short; a busy host gets the build on a later pass. A commit that fails to
# build or to start is skipped until main moves again, and a host that does
# not start is put back on the build it replaced. Passes log to
# ~/.openagents/dev-host/follow.log.
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
every=15
while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) build=0 ;;
    --every) every="${2:?--every needs minutes}"; shift ;;
    *) echo "unknown option $1" >&2; exit 64 ;;
  esac
  shift
done
follow_label="com.openagents.dev.host.follow"
follow_plist="$HOME/Library/LaunchAgents/$follow_label.plist"
checkout="$base/checkout"
follow_log="$base/follow.log"

running_pid() { launchctl print "$domain/$1" 2>/dev/null | sed -n 's/^[[:space:]]*pid = //p' | head -1; }
login_path() {
  local path
  path="$("${SHELL:-/bin/zsh}" -l -c 'printf %s "$PATH"' 2>/dev/null || true)"
  printf %s "${path:-$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin}"
}
note() { printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >> "$follow_log"; }
# Whether the installed host has work in flight: a task that is not
# finished, cancelled, or queued (running, being cancelled, or unknown), or
# a coder turn or delegation that has not finished.
busy() {
  local coder="$base/current/coder"
  [ -x "$coder" ] || return 1
  "$coder" activity >/dev/null 2>&1 && return 0
  "$coder" task list --json 2>/dev/null | python3 -c '
import json, sys
tasks = json.load(sys.stdin)
sys.exit(0 if any(t.get("status") not in ("finished", "cancelled", "queued") for t in tasks) else 1)
'
}

case "$command" in
  status)
    echo "development host: $(running_pid "$label" || true)"
    [ -L "$base/current" ] && "$base/current/coder" --version
    echo "app's host: $(running_pid "$app_label" || true)"
    launchctl print-disabled "$domain" | grep -F "$app_label" || true
    echo "following main: $([ -f "$follow_plist" ] && echo yes || echo no)"
    [ -f "$follow_log" ] && tail -1 "$follow_log"
    true ;;
  follow)
    # One pass, from the follow checkout only: it resets that checkout.
    [ "$root" = "$checkout" ] || { echo "follow runs from $checkout; use follow-on" >&2; exit 64; }
    mkdir "$base/follow.lock" 2>/dev/null || exit 0
    trap 'rmdir "$base/follow.lock"' EXIT
    export CARGO_TARGET_DIR="${OA_DEV_HOST_TARGET:-$HOME/work/openagents-target-devhost}"
    # rustup's cargo first, so the checkout's pinned toolchain builds it
    # rather than another cargo earlier on the login PATH.
    [ -d "$HOME/.cargo/bin" ] && export PATH="$HOME/.cargo/bin:$PATH"
    git -C "$root" fetch -q origin main || { note "fetch failed"; exit 1; }
    target="$(git -C "$root" rev-parse --short=10 origin/main)"
    installed="$(basename "$(readlink "$base/current" 2>/dev/null || echo none)")"
    [ "$target" = "$installed" ] && exit 0
    [ "$(cat "$base/follow.failed" 2>/dev/null)" = "$target" ] && exit 0
    git -C "$root" checkout -q --detach --force origin/main
    note "building $target (installed: $installed)"
    if ! nice -n 10 cargo build -q --release --manifest-path "$root/Cargo.toml" \
        -p coder --bin coder -p microcoder --bin microcoder >> "$follow_log" 2>&1; then
      echo "$target" > "$base/follow.failed"
      note "build of $target failed; staying on $installed"
      exit 1
    fi
    if busy; then
      note "built $target; work is running, so the install waits for a later pass"
      exit 0
    fi
    previous="$(readlink "$base/current" 2>/dev/null || true)"
    if "$root/scripts/desktop/dev-host.sh" install --no-build >> "$follow_log" 2>&1; then
      rm -f "$base/follow.failed"
      note "installed $target"
    else
      echo "$target" > "$base/follow.failed"
      note "$target did not start; putting $installed back"
      if [ -n "$previous" ]; then
        ln -sfn "$previous" "$base/current"
        launchctl kickstart -k "$domain/$label" 2>/dev/null ||
          launchctl bootstrap "$domain" "$plist" 2>/dev/null || true
      fi
      exit 1
    fi ;;
  follow-on)
    [ "$(uname -s)" = Darwin ] || { echo "macOS only" >&2; exit 64; }
    [ -f "$plist" ] || { echo "install the development host first" >&2; exit 64; }
    case "$every" in ''|*[!0-9]*|0) echo "--every takes whole minutes" >&2; exit 64 ;; esac
    if [ ! -d "$checkout/.git" ] && [ ! -f "$checkout/.git" ]; then
      git -C "$root" fetch -q origin main
      git -C "$root" worktree add -q --detach "$checkout" origin/main
    fi
    cat > "$follow_plist.new" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- Moves the development Coder host to origin/main
     (scripts/desktop/dev-host.sh follow). -->
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$follow_label</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>$checkout/scripts/desktop/dev-host.sh</string>
    <string>follow</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>$(login_path)</string>
  </dict>
  <key>StartInterval</key>
  <integer>$((every * 60))</integer>
  <key>RunAtLoad</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
  <key>LowPriorityIO</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$follow_log</string>
  <key>StandardErrorPath</key>
  <string>$follow_log</string>
</dict>
</plist>
PLIST
    plutil -lint -s "$follow_plist.new"
    mv -f "$follow_plist.new" "$follow_plist"
    launchctl bootout "$domain/$follow_label" 2>/dev/null || true
    launchctl bootstrap "$domain" "$follow_plist"
    echo "the development host follows origin/main every $every minutes; see $follow_log" ;;
  follow-off)
    launchctl bootout "$domain/$follow_label" 2>/dev/null || true
    rm -f "$follow_plist"
    echo "the development host no longer follows main; $checkout stays for the next follow-on" ;;
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
    login_path="$(login_path)"
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
    launchctl bootout "$domain/$follow_label" 2>/dev/null || true
    rm -f "$follow_plist"
    echo "development host removed; the app's host starts when OpenAgents.app opens or at login" ;;
  *) echo "usage: scripts/desktop/dev-host.sh install [--no-build]|status|uninstall|follow-on [--every MINUTES]|follow-off" >&2; exit 64 ;;
esac
