#!/usr/bin/env bash
# Build the OpenAgents iOS app with its Rust library, openagents-mobile.
#
#   bins/openagents-ios/build.sh sim       build, install, and launch on a simulator
#   bins/openagents-ios/build.sh archive   signed App Store archive; does not upload
#   bins/openagents-ios/build.sh upload    upload the archive to TestFlight
#
# OPENAGENTS_IOS_DEVICE names the simulator (default: booted).
# OPENAGENTS_IOS_BUILD_NUMBER overrides the checked-in build number.
# upload reads ASC_API_KEY_ID, ASC_API_ISSUER_ID, and ASC_API_PRIVATE_KEY_PATH.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
host="$root/bins/openagents-ios/host"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/../target}"
[[ "$CARGO_TARGET_DIR" == /* ]] || CARGO_TARGET_DIR="$root/$CARGO_TARGET_DIR"
output="${OPENAGENTS_IOS_OUTPUT:-$CARGO_TARGET_DIR/openagents-ios}"
device="${OPENAGENTS_IOS_DEVICE:-booted}"
export IPHONEOS_DEPLOYMENT_TARGET=17.0
command="${1:-sim}"
archive="$output/OpenAgents.xcarchive"

case "$command" in
  sim) triple=aarch64-apple-ios-sim; profile=debug; destination='generic/platform=iOS Simulator' ;;
  archive) triple=aarch64-apple-ios; profile=release; destination='generic/platform=iOS' ;;
  upload)
    [[ -d "$archive" ]] || { echo "No archive; run archive first." >&2; exit 1; }
    for setting in ASC_API_KEY_ID ASC_API_ISSUER_ID ASC_API_PRIVATE_KEY_PATH; do
      [[ -n "${!setting:-}" ]] || { echo "$setting is not set." >&2; exit 64; }
    done
    xcodebuild -exportArchive -archivePath "$archive" \
      -exportOptionsPlist "$host/ExportOptions.plist" -exportPath "$output/upload" \
      -authenticationKeyPath "$ASC_API_PRIVATE_KEY_PATH" \
      -authenticationKeyID "$ASC_API_KEY_ID" -authenticationKeyIssuerID "$ASC_API_ISSUER_ID"
    exit ;;
  *) echo "usage: bins/openagents-ios/build.sh sim|archive|upload" >&2; exit 64 ;;
esac

rust=(cargo build --locked -p openagents-mobile --lib --target "$triple")
[[ "$profile" == release ]] && rust+=(--release)
(cd "$root" && "${rust[@]}")
(cd "$host" && xcodegen generate --quiet)
args=(-project "$host/OpenAgents.xcodeproj" -scheme OpenAgents -configuration Release
      -destination "$destination" -derivedDataPath "$output/DerivedData"
      "OPENAGENTS_RUST_LIBRARY_DIR=$CARGO_TARGET_DIR/$triple/$profile")
if [[ -n "${OPENAGENTS_IOS_BUILD_NUMBER:-}" ]]; then
  [[ "$OPENAGENTS_IOS_BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || { echo "Build number must be a positive integer." >&2; exit 64; }
  args+=("CURRENT_PROJECT_VERSION=$OPENAGENTS_IOS_BUILD_NUMBER")
fi

if [[ "$command" == sim ]]; then
  xcodebuild "${args[@]}" CODE_SIGN_IDENTITY=- PROVISIONING_PROFILE_SPECIFIER= build
  app="$output/DerivedData/Build/Products/Release-iphonesimulator/OpenAgents.app"
  xcrun simctl install "$device" "$app"
  xcrun simctl launch "$device" com.openagents.app
else
  git -C "$root" rev-parse HEAD > "$output/archive-source.commit"
  git -C "$root" status --porcelain=v1 > "$output/archive-source.status"
  xcodebuild "${args[@]}" -archivePath "$archive" archive
  echo "Archived $archive. Upload it with: bins/openagents-ios/build.sh upload"
fi
