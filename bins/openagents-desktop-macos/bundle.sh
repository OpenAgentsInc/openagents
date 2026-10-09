#!/usr/bin/env bash
# Builds the OpenAgents desktop app for this Mac and assembles `OpenAgents.app`.
#
# The bundle holds the window (`Contents/MacOS/OpenAgents`), the Coder host
# and the binaries a task runs (`coder`, `microcoder`), the `openagents`
# command a phone's command card runs, the login agent the
# window registers on first launch
# (`Contents/Library/LaunchAgents/com.openagents.desktop.host.plist`), and
# an icon made from the OpenAgents iOS app icon. It is built for this Mac's
# architecture only, signed ad hoc, and not notarized: a quick local build.
# The universal, Developer ID signed, notarized .dmg comes from
# `scripts/desktop/package-macos.sh`, which reads the same files from this
# folder (docs/desktop/release.md).
#
# Usage:
#
#   bins/openagents-desktop-macos/bundle.sh [OUTPUT_DIR]
#
# OUTPUT_DIR defaults to the Cargo target directory's `release` folder.
# Set SKIP_CODER=1 to leave out the Coder binaries (a window-only bundle).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
out="${1:-$target/release}"
app="$out/OpenAgents.app"
icon_source="$root/bins/openagents-ios/host/App/Assets.xcassets/AppIcon.appiconset/icon-1024.png"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "OpenAgents.app is macOS only; run \`cargo run -p openagents-desktop\` elsewhere" >&2
  exit 1
fi

packages=(-p openagents-desktop)
if [[ "${SKIP_CODER:-0}" != "1" ]]; then
  packages+=(-p coder -p microcoder -p openagents-cli)
fi
cargo build --release "${packages[@]}" --manifest-path "$root/Cargo.toml"
# The version is the phone app's marketing version (MARKETING_VERSION in
# bins/openagents-ios/host/project.yml); the crate's version must match it.
version="$(sed -n 's/^ *MARKETING_VERSION: *\([0-9][0-9.]*\) *$/\1/p' "$root/bins/openagents-ios/host/project.yml" | head -1)"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Could not read MARKETING_VERSION from bins/openagents-ios/host/project.yml.' >&2; exit 1; }
crate_version="$(cargo metadata --no-deps --format-version 1 --manifest-path "$root/Cargo.toml" |
  sed -n 's/.*"name":"openagents-desktop","version":"\([^"]*\)".*/\1/p')"
[[ "$crate_version" == "$version" ]] || { echo "openagents-desktop is $crate_version but the phone app is $version; change crates/openagents-desktop/Cargo.toml to match" >&2; exit 1; }
build="$(git -C "$root" rev-list --count HEAD 2>/dev/null || echo 1)"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$app/Contents/Library/LaunchAgents"
cp "$target/release/openagents-desktop" "$app/Contents/MacOS/OpenAgents"
if [[ "${SKIP_CODER:-0}" != "1" ]]; then
  cp "$target/release/coder" "$app/Contents/MacOS/coder"
  cp "$target/release/microcoder" "$app/Contents/MacOS/microcoder"
  # In Helpers, never MacOS: on a case-insensitive volume openagents is
  # the app's own OpenAgents.
  mkdir -p "$app/Contents/Helpers"
  cp "$target/release/openagents" "$app/Contents/Helpers/openagents"
fi
cp "$here/com.openagents.desktop.host.plist" "$app/Contents/Library/LaunchAgents/"
cp "$here/Info.plist" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $build" "$app/Contents/Info.plist"

# The icon: every size iconutil wants, scaled from the 1024-pixel source.
work="$(mktemp -d)"
iconset="$work/OpenAgents.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$icon_source" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$icon_source" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/OpenAgents.icns"
rm -rf "$work"

plutil -lint "$app/Contents/Info.plist" \
  "$app/Contents/Library/LaunchAgents/com.openagents.desktop.host.plist" >/dev/null

# Sign the helpers first, then the bundle, ad hoc.
for binary in "$app/Contents/MacOS/"*; do
  [[ "$(basename "$binary")" == "OpenAgents" ]] && continue
  codesign --force --sign - --entitlements "$here/host.entitlements" \
    --identifier "com.openagents.desktop.$(basename "$binary")" "$binary" >/dev/null 2>&1
done
codesign --force --sign - --entitlements "$here/OpenAgents.entitlements" "$app" >/dev/null 2>&1
codesign --verify --strict "$app"
echo "built $app"
