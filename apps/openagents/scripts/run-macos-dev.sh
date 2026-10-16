#!/usr/bin/env bash
# Build and run an isolated OpenAgents desktop development bundle ("OpenAgents Dev.app").
# OpenAgents.app, Zeron.app and Zeron Dev.app may remain open: this bundle has its
# own LaunchServices/TCC identity, data directory, and engine IPC port.
# Set ZERON_DEV_BUILD_ONLY=1 to prepare the signed bundle without launching it.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v cargo >/dev/null 2>&1 || PATH="$HOME/.cargo/bin:$PATH"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
DEV_ROOT="$ROOT/target/macos-dev"
APP="$DEV_ROOT/OpenAgents Dev.app"
CONTENTS="$APP/Contents"
# The dev bundle keeps the checkout-local data the fork has used since it was
# created (it holds the owner's dev sessions); never a real install's data.
DATA_DIR="${ZERON_DEV_DATA_DIR:-$DEV_ROOT/data}"
IPC_PORT="${ZERON_DEV_IPC_PORT:-49777}"
BUNDLE_ID="${ZERON_DEV_BUNDLE_ID:-com.openagents.desktop.dev}"

if pgrep -f -x "$CONTENTS/MacOS/zeron" >/dev/null 2>&1; then
  echo "OpenAgents Dev is already running. Quit it before rebuilding the signed bundle." >&2
  exit 1
fi

cd "$ROOT"
cargo build -p zeron

mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources" "$DATA_DIR"
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
install -m 755 "$TARGET_DIR/debug/zeron" "$CONTENTS/MacOS/zeron"
sed "s/__VERSION__/$VERSION/g" "$ROOT/dist/macos/Info-dev.plist" >"$CONTENTS/Info.plist"
plutil -replace CFBundleIdentifier -string "$BUNDLE_ID" "$CONTENTS/Info.plist"
plutil -replace LSEnvironment.ZERON_DATA_DIR -string "$DATA_DIR" "$CONTENTS/Info.plist"
plutil -replace LSEnvironment.ZERON_IPC_PORT -string "$IPC_PORT" "$CONTENTS/Info.plist"
if [[ -n "${CODEX_EXECUTABLE:-}" ]]; then
  plutil -insert LSEnvironment.CODEX_EXECUTABLE -string "$CODEX_EXECUTABLE" "$CONTENTS/Info.plist"
fi

mkdir -p "$CONTENTS/Resources/licenses"
cp "$ROOT/crates/voice/NOTICE.md" "$CONTENTS/Resources/licenses"/parakeet-v3.txt

# Coder Desktop icon, generated from dist/coder/icon-1024.png (the OpenAgents
# app icon); see FORK.md.
install -m 644 "$ROOT/dist/coder/coder.icns" "$CONTENTS/Resources/coder.icns"

# A real Apple Development identity gives TCC a stable signing requirement
# across rebuilds. Set ZERON_DEV_CODESIGN_IDENTITY explicitly when more than
# one identity is installed; otherwise fall back to an ad-hoc signature.
IDENTITY="${ZERON_DEV_CODESIGN_IDENTITY:-}"
if [[ -z "$IDENTITY" ]]; then
  IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(Apple Development:[^"]*\)".*/\1/p' | head -1)"
fi
if [[ -n "$IDENTITY" ]]; then
  codesign --entitlements "$ROOT/dist/macos/Zeron.entitlements" --force --sign "$IDENTITY" --identifier "$BUNDLE_ID" "$APP"
else
  codesign --entitlements "$ROOT/dist/macos/Zeron.entitlements" --force --sign - --identifier "$BUNDLE_ID" "$APP"
  echo "warning: no Apple Development signing identity found; macOS may ask for permissions again after a rebuild" >&2
fi

if [[ "${ZERON_DEV_BUILD_ONLY:-0}" == "1" ]]; then
  echo "built: $APP" >&2
  exit 0
fi

echo "running OpenAgents Dev (bundle $BUNDLE_ID, data $DATA_DIR, IPC $IPC_PORT)" >&2
# LaunchServices must own the process. Launching Contents/MacOS/zeron directly
# makes TCC attribute Screen Recording to the terminal (Warp, Terminal, etc.).
# -W keeps the script attached until the app exits. Runtime logs remain in the
# isolated data directory (`target/macos-dev/data/logs/zeron-headed.log`).
OPEN_ENV=(
  --env "ZERON_AUTO_UPDATE=0"
  --env "ZERON_DATA_DIR=$DATA_DIR"
  --env "ZERON_IPC_PORT=$IPC_PORT"
)
if [[ -n "${CODEX_EXECUTABLE:-}" ]]; then
  OPEN_ENV+=(--env "CODEX_EXECUTABLE=$CODEX_EXECUTABLE")
fi
if [[ -n "${ZERON_OPEN_ROUTE:-}" ]]; then
  OPEN_ENV+=(--env "ZERON_OPEN_ROUTE=$ZERON_OPEN_ROUTE")
fi
exec open -W "${OPEN_ENV[@]}" "$APP" --args "$@"
