#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: builds Release OpenAgentsMac.app + coder-host, runs check.sh, and installs locally; holds no logic).
# Builds and installs clean-sheet OpenAgents macOS (/Applications/OpenAgentsMac.app) and
# ~/.openagents/bin/coder-host (GitHub #13993). Never overwrites legacy /Applications/OpenAgents.app.
set -euo pipefail
cd "$(dirname "$0")/../.."
repo="$(pwd -P)"

coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then
  echo "install.sh: coax binary not found at $coax" >&2
  exit 1
fi

echo "==> Building coder-host (~/.openagents/bin/coder-host)"
mkdir -p "$HOME/.openagents/bin"
"$coax" build coder/tui/host.coax -o "$HOME/.openagents/bin/coder-host.tmp"
chmod 755 "$HOME/.openagents/bin/coder-host.tmp"
mv -f "$HOME/.openagents/bin/coder-host.tmp" "$HOME/.openagents/bin/coder-host"

echo "==> Building OpenAgentsMac.app (Release)"
cd "$repo/apps/openagents-macos"
./generate.sh
xcodegen generate
xcodebuild -project OpenAgentsMac.xcodeproj -scheme OpenAgentsMac -configuration Release -sdk macosx \
  -derivedDataPath build CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- build

app="$repo/apps/openagents-macos/build/Build/Products/Release/OpenAgentsMac.app"
./check.sh "$app"

dest="/Applications/OpenAgentsMac.app"
echo "==> Installing $dest"
rm -rf "$dest"
ditto "$app" "$dest"
echo "Installed $dest and $HOME/.openagents/bin/coder-host"
