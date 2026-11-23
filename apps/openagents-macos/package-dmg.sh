#!/usr/bin/env bash
# Firm reason: 5 (thin release packaging wrapper around coax build, xcodebuild, codesign, notarytool, and hdiutil; holds no application logic).
# Packages a self-contained OpenAgents.dmg bundling OpenAgents.app + coder-host + agyworker (GitHub #13993).
# Optional signing & notarization:
#   MACOS_CODESIGN_IDENTITY="Developer ID Application: ..." NOTARY_PROFILE="openagents-notary" ./package-dmg.sh [OUT_DIR]
set -euo pipefail
cd "$(dirname "$0")/../.."
repo="$(pwd -P)"

out_dir="${1:-$repo/apps/openagents-macos/dist}"
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd -P)"

coax="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax" ]]; then
  echo "package-dmg.sh: coax binary not found at $coax" >&2
  exit 1
fi

stage="$(mktemp -d /tmp/oa-macos-dmg-XXXXXX)"
trap 'rm -rf "$stage"' EXIT

echo "==> [1/4] Building bundled coder-host and agyworker"
compiler_hash="$(shasum -a 256 "$coax" | cut -d ' ' -f 1)"
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/host.coax" build-only </dev/null >/dev/null 2>&1 || true
COAX="$coax" "$repo/coax/web/tool.sh" "$repo/coder/tui/agyworker.coax" </dev/null >/dev/null
cp -f "$repo/coax/web/build/tools/host-$compiler_hash" "$stage/coder-host"
cp -f "$repo/coax/web/build/tools/agyworker-$compiler_hash" "$stage/agyworker"
chmod 755 "$stage/coder-host" "$stage/agyworker"

echo "==> [2/4] Building OpenAgents.app (Release)"
cd "$repo/apps/openagents-macos"
./generate.sh
xcodegen generate
sign_id="${MACOS_CODESIGN_IDENTITY:--}"
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -configuration Release -sdk macosx \
  -derivedDataPath "$stage/derived" \
  CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY="$sign_id" build

app="$stage/OpenAgents.app"
ditto "$stage/derived/Build/Products/Release/OpenAgents.app" "$app"
mkdir -p "$app/Contents/Resources"
cp -f "$stage/coder-host" "$app/Contents/Resources/coder-host"
cp -f "$stage/agyworker" "$app/Contents/Resources/agyworker"
chmod 755 "$app/Contents/Resources/coder-host" "$app/Contents/Resources/agyworker"

echo "==> [3/4] Signing & verifying clean-sheet invariants"
if [[ "$sign_id" != "-" ]]; then
  codesign --force --options runtime --timestamp --sign "$sign_id" "$app/Contents/Resources/coder-host"
  codesign --force --options runtime --timestamp --sign "$sign_id" "$app/Contents/Resources/agyworker"
  codesign --force --options runtime --timestamp --deep --sign "$sign_id" "$app"
else
  codesign --force --deep --sign - "$app"
fi
./check.sh "$app"

echo "==> [4/4] Creating OpenAgents.dmg"
dmg_root="$stage/dmg-root"
mkdir -p "$dmg_root"
ditto "$app" "$dmg_root/OpenAgents.app"
ln -s /Applications "$dmg_root/Applications"

dmg_path="$out_dir/OpenAgents.dmg"
rm -f "$dmg_path"
hdiutil create -volname "OpenAgents" -srcfolder "$dmg_root" -ov -format UDZO "$dmg_path" >/dev/null

if [[ "$sign_id" != "-" ]]; then
  codesign --force --timestamp --sign "$sign_id" "$dmg_path"
fi
if [[ -n "${NOTARY_PROFILE:-}" && "$sign_id" != "-" ]]; then
  xcrun notarytool submit "$dmg_path" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$dmg_path"
fi

echo "PACKAGED: $dmg_path ($(du -h "$dmg_path" | awk '{print $1}'))"
