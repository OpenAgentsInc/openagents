#!/usr/bin/env bash
# Build the OpenAgents iOS app with its Rust library, openagents-mobile.
#
#   bins/openagents-ios/build.sh sim       build, install, and launch on a simulator
#   bins/openagents-ios/build.sh archive   signed App Store archive; does not upload
#   bins/openagents-ios/build.sh validate  export the archive and validate it with App Store
#                                          Connect, without uploading (a dry run of upload)
#   bins/openagents-ios/build.sh upload    upload the archive to TestFlight
#   bins/openagents-ios/build.sh bench     transcript benchmark build on a device
#
# OPENAGENTS_IOS_DEVICE names the simulator (default: booted).
# OPENAGENTS_IOS_BUILD_NUMBER overrides the checked-in build number.
# OPENAGENTS_MOBILE_PREVIEW=on shows the features still in development (the
# Verse, the Gym, Trainer, Playtest, Tailnet); unset, every build hides them
# (docs/mobile/1.0-audit.md). OPENAGENTS_PLAYTEST_LOGGING=on|off overrides
# playtest logging, which otherwise follows OPENAGENTS_MOBILE_PREVIEW: off in
# archives and normal simulator builds. The Rust library reads both when it
# compiles; see docs/game/playtesting.md.
# validate and upload read ASC_API_KEY_ID, ASC_API_ISSUER_ID, and
# ASC_API_PRIVATE_KEY_PATH. scripts/release/testflight.sh runs the whole
# release (build number, archive, validate or upload, processing).
# Push wakes are off by default. OPENAGENTS_IOS_PUSH=development|production
# signs with the push entitlement, and OPENAGENTS_PUSH_RELAY_URL,
# OPENAGENTS_PUSH_GATEWAY_URL, and OPENAGENTS_PUSH_APP_PROFILE turn them on.
# bench builds optimized Rust and a development-signed app with the
# transcript fixture and benchmarks compiled in (RUST_NATIVE_BENCH), under its
# own bundle ID (com.openagents.app.bench) so it never replaces the installed
# app, and installs it on OPENAGENTS_IOS_DEVICE_ID (from `xcrun devicectl list
# devices`). Launch it on the unlocked phone with the fixture arguments, for
# example:
#   xcrun devicectl device process launch --device ID --console \
#     com.openagents.app.bench --rust-native-fixture \
#     --rust-native-fixture-rows 3000 --rust-native-transcript-bench
# Remove it afterward with `xcrun devicectl device uninstall app --device ID
# com.openagents.app.bench`.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
host="$root/bins/openagents-ios/host"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/../target}"
[[ "$CARGO_TARGET_DIR" == /* ]] || CARGO_TARGET_DIR="$root/$CARGO_TARGET_DIR"
output="${OPENAGENTS_IOS_OUTPUT:-$CARGO_TARGET_DIR/openagents-ios}"
device="${OPENAGENTS_IOS_DEVICE:-booted}"
export IPHONEOS_DEPLOYMENT_TARGET=17.0
case "${OPENAGENTS_PLAYTEST_LOGGING:-}" in on|off) export OPENAGENTS_PLAYTEST_LOGGING ;; "") unset OPENAGENTS_PLAYTEST_LOGGING ;; *) echo 'OPENAGENTS_PLAYTEST_LOGGING must be on, off, or unset.' >&2; exit 64 ;; esac
case "${OPENAGENTS_MOBILE_PREVIEW:-}" in on) export OPENAGENTS_MOBILE_PREVIEW ;; ""|off) unset OPENAGENTS_MOBILE_PREVIEW ;; *) echo 'OPENAGENTS_MOBILE_PREVIEW must be on, off, or unset.' >&2; exit 64 ;; esac
command="${1:-sim}"
archive="$output/OpenAgents.xcarchive"

case "$command" in
  sim) triple=aarch64-apple-ios-sim; profile=debug; destination='generic/platform=iOS Simulator' ;;
  bench)
    [[ -n "${OPENAGENTS_IOS_DEVICE_ID:-}" ]] || { echo "OPENAGENTS_IOS_DEVICE_ID is not set." >&2; exit 64; }
    triple=aarch64-apple-ios; profile=release; destination='generic/platform=iOS' ;;
  archive) triple=aarch64-apple-ios; profile=release; destination='generic/platform=iOS' ;;
  upload|validate)
    [[ -d "$archive" ]] || { echo "No archive; run archive first." >&2; exit 1; }
    for setting in ASC_API_KEY_ID ASC_API_ISSUER_ID ASC_API_PRIVATE_KEY_PATH; do
      [[ -n "${!setting:-}" ]] || { echo "$setting is not set." >&2; exit 64; }
    done
    if [[ "$command" == validate ]]; then
      # The same signed export as upload, kept on disk, then App Store
      # Connect's validation of it; nothing is uploaded.
      rm -rf "$output/validate"
      mkdir -p "$output/validate"
      cp "$host/ExportOptions.plist" "$output/validate/ExportOptions.plist"
      plutil -replace destination -string export "$output/validate/ExportOptions.plist"
      xcodebuild -exportArchive -archivePath "$archive" \
        -exportOptionsPlist "$output/validate/ExportOptions.plist" -exportPath "$output/validate"
      ipa="$(find "$output/validate" -maxdepth 1 -name '*.ipa' | head -1)"
      [[ -n "$ipa" ]] || { echo "The export made no .ipa." >&2; exit 1; }
      xcrun altool --validate-app -f "$ipa" -t ios --api-key "$ASC_API_KEY_ID" \
        --api-issuer "$ASC_API_ISSUER_ID" --p8-file-path "$ASC_API_PRIVATE_KEY_PATH"
      exit
    fi
    xcodebuild -exportArchive -archivePath "$archive" \
      -exportOptionsPlist "$host/ExportOptions.plist" -exportPath "$output/upload" \
      -authenticationKeyPath "$ASC_API_PRIVATE_KEY_PATH" \
      -authenticationKeyID "$ASC_API_KEY_ID" -authenticationKeyIssuerID "$ASC_API_ISSUER_ID"
    exit ;;
  *) echo "usage: bins/openagents-ios/build.sh sim|archive|validate|upload|bench" >&2; exit 64 ;;
esac

# openagents-mobile is its own Cargo workspace (see its Cargo.toml).
rust=(cargo build --locked --manifest-path crates/openagents-mobile/Cargo.toml --lib --target "$triple")
[[ "$profile" == release ]] && rust+=(--release)
(cd "$root" && "${rust[@]}")
(cd "$host" && xcodegen generate --quiet)
args=(-project "$host/OpenAgents.xcodeproj" -scheme OpenAgents -configuration Release
      -destination "$destination" -derivedDataPath "$output/DerivedData"
      "OPENAGENTS_RUST_LIBRARY_DIR=$CARGO_TARGET_DIR/$triple/$profile")
args+=("OPENAGENTS_PUSH_RELAY_URL=${OPENAGENTS_PUSH_RELAY_URL:-}"
      "OPENAGENTS_PUSH_GATEWAY_URL=${OPENAGENTS_PUSH_GATEWAY_URL:-}"
      "OPENAGENTS_PUSH_APP_PROFILE=${OPENAGENTS_PUSH_APP_PROFILE:-}")
case "${OPENAGENTS_IOS_PUSH:-}" in
  "") ;;
  development|production)
    if [[ "$command" == archive && "$OPENAGENTS_IOS_PUSH" != production ]]; then
      echo "An archive for TestFlight needs OPENAGENTS_IOS_PUSH=production." >&2; exit 64
    fi
    args+=(CODE_SIGN_ENTITLEMENTS=Push/OpenAgents-Push.entitlements "OPENAGENTS_APS_ENVIRONMENT=$OPENAGENTS_IOS_PUSH") ;;
  *) echo "OPENAGENTS_IOS_PUSH must be development, production, or empty." >&2; exit 64 ;;
esac
if [[ -n "${OPENAGENTS_IOS_BUILD_NUMBER:-}" ]]; then
  [[ "$OPENAGENTS_IOS_BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || { echo "Build number must be a positive integer." >&2; exit 64; }
  args+=("CURRENT_PROJECT_VERSION=$OPENAGENTS_IOS_BUILD_NUMBER")
fi

if [[ "$command" == bench ]]; then
  # The bench bundle ID has no multicast approval, so it signs without it.
  xcodebuild "${args[@]}" PRODUCT_BUNDLE_IDENTIFIER=com.openagents.app.bench CODE_SIGN_STYLE=Automatic \
    CODE_SIGN_ENTITLEMENTS= \
    "CODE_SIGN_IDENTITY=Apple Development" PROVISIONING_PROFILE_SPECIFIER= \
    'SWIFT_ACTIVE_COMPILATION_CONDITIONS=$(inherited) RUST_NATIVE_BENCH' -allowProvisioningUpdates build
  xcrun devicectl device install app --device "$OPENAGENTS_IOS_DEVICE_ID" \
    "$output/DerivedData/Build/Products/Release-iphoneos/OpenAgents.app"
elif [[ "$command" == sim ]]; then
  xcodebuild "${args[@]}" CODE_SIGN_IDENTITY=- PROVISIONING_PROFILE_SPECIFIER= build
  app="$output/DerivedData/Build/Products/Release-iphonesimulator/OpenAgents.app"
  xcrun simctl install "$device" "$app"
  xcrun simctl launch "$device" com.openagents.app
else
  mkdir -p "$output"
  git -C "$root" rev-parse HEAD > "$output/archive-source.commit"
  git -C "$root" status --porcelain=v1 > "$output/archive-source.status"
  xcodebuild "${args[@]}" -archivePath "$archive" archive
  echo "Archived $archive. Upload it with: bins/openagents-ios/build.sh upload"
fi
