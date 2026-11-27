#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: builds Release OpenAgentsMac.app + coder-host + oa-worker + agyworker, runs check.sh, and installs to /Applications/OpenAgentsMac.app by default; holds no logic).
# Builds and installs the SwiftUI macOS companion (/Applications/OpenAgentsMac.app by default, or /Applications/OpenAgents.app with --replace-desktop),
# plus ~/.openagents/bin/coder-host, ~/.openagents/bin/oa-worker, and ~/.openagents/bin/agyworker (GitHub #13993, #14060).
set -euo pipefail
cd "$(dirname "$0")/../.."
repo="$(pwd -P)"

coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then
  echo "install.sh: coax binary not found at $coax" >&2
  exit 1
fi

compiler_hash="$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"

echo "==> Building coder-host (~/.openagents/bin/coder-host)"
mkdir -p "$HOME/.openagents/bin"
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/host.coax" build-only </dev/null >/dev/null 2>&1 || true
host_built="$repo/coax/web/build/tools/host-$compiler_hash"
test -x "$host_built"
cp -f "$host_built" "$HOME/.openagents/bin/coder-host.tmp"
chmod 755 "$HOME/.openagents/bin/coder-host.tmp"
mv -f "$HOME/.openagents/bin/coder-host.tmp" "$HOME/.openagents/bin/coder-host"

echo "==> Building oa-worker (~/.openagents/bin/oa-worker)"
COAX_TOOL_BUILD_ONLY=1 COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/oa-worker.coax" </dev/null >/dev/null
oa_built="$repo/coax/web/build/tools/oa-worker-$compiler_hash"
test -x "$oa_built"
cp -f "$oa_built" "$HOME/.openagents/bin/oa-worker.tmp"
chmod 755 "$HOME/.openagents/bin/oa-worker.tmp"
mv -f "$HOME/.openagents/bin/oa-worker.tmp" "$HOME/.openagents/bin/oa-worker"

echo "==> Building agyworker (~/.openagents/bin/agyworker)"
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/agyworker.coax" </dev/null >/dev/null
agy_built="$repo/coax/web/build/tools/agyworker-$compiler_hash"
test -x "$agy_built"
cp -f "$agy_built" "$HOME/.openagents/bin/agyworker.tmp"
chmod 755 "$HOME/.openagents/bin/agyworker.tmp"
mv -f "$HOME/.openagents/bin/agyworker.tmp" "$HOME/.openagents/bin/agyworker"

echo "==> Building OpenAgents.app (Release)"
cd "$repo/apps/openagents-macos"
./generate.sh
xcodegen generate
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -configuration Release -sdk macosx \
  -derivedDataPath build CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- build

app="$repo/apps/openagents-macos/build/Build/Products/Release/OpenAgents.app"
mkdir -p "$app/Contents/Resources"
cp -f "$HOME/.openagents/bin/coder-host" "$app/Contents/Resources/coder-host"
cp -f "$HOME/.openagents/bin/oa-worker" "$app/Contents/Resources/oa-worker"
cp -f "$HOME/.openagents/bin/agyworker" "$app/Contents/Resources/agyworker"
chmod 755 "$app/Contents/Resources/coder-host" "$app/Contents/Resources/oa-worker" "$app/Contents/Resources/agyworker"
codesign --force --deep --sign - "$app"

./check.sh "$app"

# Install to /Applications/OpenAgentsMac.app by default so this SwiftUI companion never clobbers
# OpenAgents Desktop (/Applications/OpenAgents.app) or kills its com.openagents.desktop-update LaunchAgent.
# Pass --replace-desktop only if you explicitly want to replace /Applications/OpenAgents.app.
replace_desktop=0
for arg in "$@"; do
  case "$arg" in
    --replace-desktop) replace_desktop=1 ;;
    *) echo "install.sh: unknown argument $arg (usage: install.sh [--replace-desktop])" >&2; exit 2 ;;
  esac
done

if [[ "$replace_desktop" == 1 ]]; then
  dest="/Applications/OpenAgents.app"
else
  dest="/Applications/OpenAgentsMac.app"
fi
echo "==> Installing $dest"
# Gracefully quit running target app unless this script is running inside it.
running_pids="$(pgrep -f "^$dest/Contents/MacOS/OpenAgents$" || true)"
if [[ -n "$running_pids" ]]; then
  in_app=0
  p=$$
  while [[ -n "$p" && "$p" -gt 1 ]]; do
    if [[ " $running_pids " == *" $p "* ]]; then in_app=1; break; fi
    p="$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')"
  done
  if [[ "$in_app" == 0 ]]; then
    osascript -e 'tell application id "com.openagents.desktop" to quit' 2>/dev/null || true
    for _ in $(seq 1 20); do
      pgrep -f "^$dest/Contents/MacOS/OpenAgents$" >/dev/null 2>&1 || break
      sleep 0.2
    done
    pkill -TERM -f "^$dest/Contents/MacOS/OpenAgents$" 2>/dev/null || true
  fi
fi

stage="$dest.new.$$"
old="$dest.old.$$"
rm -rf "$stage" "$old"
ditto "$app" "$stage"
if [[ -e "$dest" ]]; then
  mv "$dest" "$old"
fi
mv "$stage" "$dest"
rm -rf "$old"
touch "$dest"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$dest" 2>/dev/null || true
echo "Installed $dest, $HOME/.openagents/bin/coder-host, $HOME/.openagents/bin/oa-worker, and $HOME/.openagents/bin/agyworker"
