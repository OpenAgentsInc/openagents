#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: builds Release OpenAgentsMac.app + coder-host + agyworker, runs check.sh, and installs locally; holds no logic).
# Builds and installs clean-sheet OpenAgents macOS (/Applications/OpenAgentsMac.app),
# ~/.openagents/bin/coder-host, and ~/.openagents/bin/agyworker (GitHub #13993).
# Never overwrites legacy /Applications/OpenAgents.app.
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

echo "==> Building agyworker (~/.openagents/bin/agyworker)"
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/agyworker.coax" </dev/null >/dev/null
agy_built="$repo/coax/web/build/tools/agyworker-$compiler_hash"
test -x "$agy_built"
cp -f "$agy_built" "$HOME/.openagents/bin/agyworker.tmp"
chmod 755 "$HOME/.openagents/bin/agyworker.tmp"
mv -f "$HOME/.openagents/bin/agyworker.tmp" "$HOME/.openagents/bin/agyworker"

echo "==> Building OpenAgentsMac.app (Release)"
cd "$repo/apps/openagents-macos"
./generate.sh
xcodegen generate
xcodebuild -project OpenAgentsMac.xcodeproj -scheme OpenAgentsMac -configuration Release -sdk macosx \
  -derivedDataPath build CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- build

app="$repo/apps/openagents-macos/build/Build/Products/Release/OpenAgentsMac.app"
mkdir -p "$app/Contents/Resources"
cp -f "$HOME/.openagents/bin/coder-host" "$app/Contents/Resources/coder-host"
cp -f "$HOME/.openagents/bin/agyworker" "$app/Contents/Resources/agyworker"
chmod 755 "$app/Contents/Resources/coder-host" "$app/Contents/Resources/agyworker"
codesign --force --deep --sign - "$app"

./check.sh "$app"

dest="/Applications/OpenAgentsMac.app"
echo "==> Installing $dest"
rm -rf "$dest"
ditto "$app" "$dest"
echo "Installed $dest, $HOME/.openagents/bin/coder-host, and $HOME/.openagents/bin/agyworker"
