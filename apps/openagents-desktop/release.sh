#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around build.sh, sign.sh, ditto, notarytool, stapler and sign-update-feed.swift; holds no logic)
# Builds, Developer ID signs, notarizes, staples, archives, and signs the update feed for OpenAgents.app (#13785).
# Required env:
#   OA_MAC_SIGN_IDENTITY     Developer ID Application certificate name
#   OA_MAC_PROFILE           Provisioning profile for com.openagents.desktop with Push Notifications
#   OA_NOTARY_PROFILE        Keychain profile from `xcrun notarytool store-credentials` (or OA_APPLE_ID/OA_APPLE_TEAM_ID/OA_APPLE_APP_PASSWORD)
#   OA_UPDATE_SIGN_SEED_HEX  64-hex-char Ed25519 seed for openagents-desktop-2026-v1
# Usage:
#   apps/openagents-desktop/release.sh VERSION DOWNLOAD_URL [OUT_DIR]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
version="${1:?usage: release.sh VERSION DOWNLOAD_URL [OUT_DIR]}"
download_url="${2:?usage: release.sh VERSION DOWNLOAD_URL [OUT_DIR]}"
out_dir="${3:-$here/build/release}"
mkdir -p "$out_dir"

export OA_MAC_NOTARIZE=1
export OA_RELEASE_MINIMAL="${OA_RELEASE_MINIMAL:-1}"
"$here/build.sh"

app="$here/build/OpenAgents.app"
archive="$out_dir/OpenAgents-$version.zip"
rm -f "$archive"
ditto -c -k --keepParent "$app" "$archive"
if [[ "$OA_RELEASE_MINIMAL" == "1" ]]; then
  "$repo/coax/web/tool.sh" "$here/v1-bundle-check.coax" --platform macos --stage "${OA_V1_STAGE:-1}" --archive "$archive" "$app"
fi

build_num="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$app/Contents/Info.plist" 2>/dev/null || echo 1)"
feed="$out_dir/openagents-desktop-update.json"
swift "$here/sign-update-feed.swift" \
  --version "$version" \
  --build "$build_num" \
  --url "$download_url" \
  --archive "$archive" \
  --key-id "${OA_UPDATE_KEY_ID:-openagents-desktop-2026-v1}" \
  --out "$feed"

echo "release archive: $archive"
echo "signed feed:     $feed"
