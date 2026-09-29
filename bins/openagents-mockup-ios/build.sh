#!/usr/bin/env bash
# Build OpenAgents Mockup, the screens-only design app (no Rust, no network).
#
#   bins/openagents-mockup-ios/build.sh sim       build, install, and launch on a simulator
#   bins/openagents-mockup-ios/build.sh archive   signed App Store archive; does not upload
#   bins/openagents-mockup-ios/build.sh upload    upload the archive to TestFlight
#   bins/openagents-mockup-ios/build.sh shots     screenshot the key screens into screenshots/
#
# OPENAGENTS_MOCKUP_DEVICE names the simulator (a UDID or "booted"; default: booted).
# OPENAGENTS_MOCKUP_BUILD_NUMBER overrides the checked-in build number (project.yml).
# upload reads ASC_API_KEY_ID, ASC_API_ISSUER_ID, and ASC_API_PRIVATE_KEY_PATH; if
# they are unset it sources ../../../.secrets/appstoreconnect.env when present.
# sim and shots pass any further arguments to the app, e.g. `sim --screen SCR-05.worse`.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
output="${OPENAGENTS_MOCKUP_OUTPUT:-$here/../../../target/openagents-mockup-ios}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
device="${OPENAGENTS_MOCKUP_DEVICE:-booted}"
command="${1:-sim}"
[[ $# -gt 0 ]] && shift
archive="$output/OpenAgentsMockup.xcarchive"
bundle=com.openagents.mockup

generate() { (cd "$here" && xcodegen generate --quiet); }

build_sim() {
  generate
  xcodebuild -project "$here/OpenAgentsMockup.xcodeproj" -scheme OpenAgentsMockup -configuration Debug \
    -destination 'generic/platform=iOS Simulator' -derivedDataPath "$output/DerivedData" \
    CODE_SIGNING_ALLOWED=NO -quiet build
  echo "$output/DerivedData/Build/Products/Debug-iphonesimulator/OpenAgentsMockup.app"
}

case "$command" in
  sim)
    app="$(build_sim | tail -1)"
    xcrun simctl install "$device" "$app"
    xcrun simctl launch "$device" "$bundle" "$@"
    ;;
  shots)
    app="$(build_sim | tail -1)"
    xcrun simctl install "$device" "$app"
    xcrun simctl status_bar "$device" override --time 9:41 --batteryState charged --batteryLevel 100 \
      --cellularBars 4 --wifiBars 3 >/dev/null 2>&1 || true
    mkdir -p "$here/screenshots"
    # name  screen-index-id  seconds-to-wait
    while read -r name screen wait; do
      [[ -z "$name" || "$name" == \#* ]] && continue
      xcrun simctl terminate "$device" "$bundle" >/dev/null 2>&1 || true
      xcrun simctl launch "$device" "$bundle" --screen "$screen" >/dev/null
      sleep "$wait"
      xcrun simctl io "$device" screenshot "$here/screenshots/$name.png" >/dev/null 2>&1
      sips -Z 1311 "$here/screenshots/$name.png" >/dev/null   # half size keeps the repo small
      echo "screenshots/$name.png"
    done < "$here/screenshots.txt"
    ;;
  archive)
    generate
    args=(-project "$here/OpenAgentsMockup.xcodeproj" -scheme OpenAgentsMockup -configuration Release
          -destination 'generic/platform=iOS' -derivedDataPath "$output/DerivedData" -archivePath "$archive")
    if [[ -n "${OPENAGENTS_MOCKUP_BUILD_NUMBER:-}" ]]; then
      [[ "$OPENAGENTS_MOCKUP_BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || { echo "Build number must be a positive integer." >&2; exit 64; }
      args+=("CURRENT_PROJECT_VERSION=$OPENAGENTS_MOCKUP_BUILD_NUMBER")
    fi
    git -C "$here" rev-parse HEAD > "$output/archive-source.commit"
    xcodebuild "${args[@]}" archive
    echo "Archived $archive. Upload it with: bins/openagents-mockup-ios/build.sh upload"
    ;;
  upload)
    [[ -d "$archive" ]] || { echo "No archive; run archive first." >&2; exit 1; }
    secrets="$here/../../../.secrets/appstoreconnect.env"
    if [[ -z "${ASC_API_KEY_ID:-}" && -f "$secrets" ]]; then
      set -a; source "$secrets"; set +a
    fi
    for setting in ASC_API_KEY_ID ASC_API_ISSUER_ID ASC_API_PRIVATE_KEY_PATH; do
      [[ -n "${!setting:-}" ]] || { echo "$setting is not set." >&2; exit 64; }
    done
    xcodebuild -exportArchive -archivePath "$archive" \
      -exportOptionsPlist "$here/ExportOptions.plist" -exportPath "$output/upload" \
      -authenticationKeyPath "$ASC_API_PRIVATE_KEY_PATH" \
      -authenticationKeyID "$ASC_API_KEY_ID" -authenticationKeyIssuerID "$ASC_API_ISSUER_ID"
    ;;
  *) echo "usage: bins/openagents-mockup-ios/build.sh sim|archive|upload|shots" >&2; exit 64 ;;
esac
