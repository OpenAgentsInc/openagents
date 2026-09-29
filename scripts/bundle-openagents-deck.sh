#!/usr/bin/env bash
# Builds the OpenAgents deck and wraps it as a macOS app, `OpenAgents
# Deck.app`, that opens with a double-click.
#
# The app holds the release binary, an Info.plist, and an icon made from the
# OpenAgents iOS app icon. It is signed ad hoc for this machine; it is not
# notarized. Usage:
#
#   scripts/bundle-openagents-deck.sh [OUTPUT_DIR]
#
# OUTPUT_DIR defaults to the Cargo target directory's `release` folder.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
out="${1:-$target/release}"
app="$out/OpenAgents Deck.app"
icon_source="$root/bins/openagents-ios/host/App/Assets.xcassets/AppIcon.appiconset/icon-1024.png"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "the app bundle is macOS only; run \`cargo run -p openagents-deck\` elsewhere" >&2
  exit 1
fi

cargo build --release -p openagents-deck --manifest-path "$root/Cargo.toml"
version="$(cargo metadata --no-deps --format-version 1 --manifest-path "$root/Cargo.toml" |
  sed -n 's/.*"name":"openagents-deck","version":"\([^"]*\)".*/\1/p')"
version="${version:-0.1.0}"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$target/release/openagents-deck" "$app/Contents/MacOS/openagents-deck"

# The icon: every size iconutil wants, scaled from the 1024-pixel source.
iconset="$(mktemp -d)/OpenAgentsDeck.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$icon_source" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$icon_source" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/OpenAgentsDeck.icns"
rm -rf "$(dirname "$iconset")"

cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>OpenAgents Deck</string>
  <key>CFBundleDisplayName</key>
  <string>OpenAgents Deck</string>
  <key>CFBundleIdentifier</key>
  <string>com.openagents.deck</string>
  <key>CFBundleExecutable</key>
  <string>openagents-deck</string>
  <key>CFBundleIconFile</key>
  <string>OpenAgentsDeck</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.productivity</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

codesign --force --sign - "$app" >/dev/null 2>&1
codesign --verify --strict "$app"
echo "built $app"
