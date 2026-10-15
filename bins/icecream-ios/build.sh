#!/usr/bin/env bash
# Build The Ice Cream Man for iPhone and iPad.
#
#   bins/icecream-ios/build.sh game      build the game bundle into host/App/game
#   bins/icecream-ios/build.sh sim       game + build, install, launch on a simulator
#   bins/icecream-ios/build.sh archive   game + signed App Store archive (no upload)
#   bins/icecream-ios/build.sh ipa       export the archive to a signed .ipa (no upload)
#   bins/icecream-ios/build.sh upload    upload the archive to App Store Connect
#   bins/icecream-ios/build.sh test      game + the rotation UI test on ICECREAM_IOS_DEVICE_ID
#                                        (a simulator UDID); ICECREAM_SHOTS=DIR keeps its
#                                        whole-screen PNGs there (ICECREAM_SEED: the bot's run)
#   bins/icecream-ios/build.sh clean     delete DerivedData, archives, and exports
#
# The game comes from scripts/build-icecream-web.sh --with-page (needs the
# wasm32-unknown-unknown target and the matching wasm-bindgen CLI). Set
# ICECREAM_SKIP_GAME=1 to reuse what is already in host/App/game.
# ICECREAM_IOS_DEVICE names the simulator (default: booted).
# ICECREAM_IOS_BUILD_NUMBER overrides the checked-in build number.
# ICECREAM_IOS_OUTPUT is where DerivedData, the archive, and exports go
# (default: $CARGO_TARGET_DIR/icecream-ios).
# upload reads ASC_API_KEY_ID, ASC_API_ISSUER_ID, ASC_API_PRIVATE_KEY_PATH,
# and loads them from ~/work/.secrets/appstoreconnect.env when unset.
# The App Store signing profile ("The Ice Cream Man App Store") comes from
# `python3 bins/icecream-ios/asc.py profile`.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
here="$root/bins/icecream-ios"
host="$here/host"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/../target}"
[[ "$CARGO_TARGET_DIR" == /* ]] || CARGO_TARGET_DIR="$root/$CARGO_TARGET_DIR"
output="${ICECREAM_IOS_OUTPUT:-$CARGO_TARGET_DIR/icecream-ios}"
device="${ICECREAM_IOS_DEVICE:-booted}"
archive="$output/IceCreamMan.xcarchive"
command="${1:-sim}"

game() {
  [[ "${ICECREAM_SKIP_GAME:-}" == 1 && -f "$host/App/game/index.html" ]] && return
  local stage="$output/game"
  rm -rf "$stage"
  "$root/scripts/build-icecream-web.sh" --with-page "$stage"
  rm -rf "$host/App/game"
  mkdir -p "$host/App/game"
  # Everything the page needs, without the .gz copies the site serves.
  (cd "$stage" && find . -type f ! -name '*.gz' -print0 | while IFS= read -r -d '' file; do
    mkdir -p "$host/App/game/$(dirname "$file")"
    cp "$file" "$host/App/game/$file"
  done)
  rm -rf "$stage"
  git -C "$root" rev-parse HEAD > "$host/App/game/SOURCE_COMMIT.txt"
}

asc_env() {
  if [[ -z "${ASC_API_KEY_ID:-}" && -f "$HOME/work/.secrets/appstoreconnect.env" ]]; then
    set -a
    # shellcheck disable=SC1091
    . "$HOME/work/.secrets/appstoreconnect.env"
    set +a
  fi
  for setting in ASC_API_KEY_ID ASC_API_ISSUER_ID ASC_API_PRIVATE_KEY_PATH; do
    [[ -n "${!setting:-}" ]] || { echo "$setting is not set." >&2; exit 64; }
  done
}

args=(-project "$host/IceCreamMan.xcodeproj" -scheme IceCreamMan -configuration Release
      -derivedDataPath "$output/DerivedData")
if [[ -n "${ICECREAM_IOS_BUILD_NUMBER:-}" ]]; then
  [[ "$ICECREAM_IOS_BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || { echo "Build number must be a positive integer." >&2; exit 64; }
  args+=("CURRENT_PROJECT_VERSION=$ICECREAM_IOS_BUILD_NUMBER")
fi

case "$command" in
  game) game ;;
  sim)
    game
    (cd "$host" && xcodegen generate --quiet)
    xcodebuild "${args[@]}" -destination 'generic/platform=iOS Simulator' \
      CODE_SIGN_IDENTITY=- PROVISIONING_PROFILE_SPECIFIER= build
    app="$output/DerivedData/Build/Products/Release-iphonesimulator/IceCreamMan.app"
    xcrun simctl install "$device" "$app"
    xcrun simctl launch "$device" com.openagents.icecreamman ;;
  archive)
    game
    (cd "$host" && xcodegen generate --quiet)
    mkdir -p "$output"
    git -C "$root" rev-parse HEAD > "$output/archive-source.commit"
    rm -rf "$archive"
    xcodebuild "${args[@]}" -destination 'generic/platform=iOS' -archivePath "$archive" archive
    echo "Archived $archive. Upload it with: bins/icecream-ios/build.sh upload" ;;
  test)
    [[ -n "${ICECREAM_IOS_DEVICE_ID:-}" ]] || { echo "ICECREAM_IOS_DEVICE_ID (a simulator UDID) is not set." >&2; exit 64; }
    game
    (cd "$host" && xcodegen generate --quiet)
    if [[ -n "${ICECREAM_SHOTS:-}" ]]; then mkdir -p "$ICECREAM_SHOTS"; fi
    TEST_RUNNER_ICECREAM_SHOTS="${ICECREAM_SHOTS:-}" TEST_RUNNER_ICECREAM_SEED="${ICECREAM_SEED:-}" xcodebuild -project "$host/IceCreamMan.xcodeproj" \
      -scheme IceCreamMan -configuration Debug -derivedDataPath "$output/DerivedData" \
      -destination "id=$ICECREAM_IOS_DEVICE_ID" CODE_SIGN_IDENTITY=- PROVISIONING_PROFILE_SPECIFIER= test ;;
  ipa)
    [[ -d "$archive" ]] || { echo "No archive; run archive first." >&2; exit 1; }
    rm -rf "$output/ipa"
    mkdir -p "$output/ipa"
    cp "$host/ExportOptions.plist" "$output/ipa/ExportOptions.plist"
    plutil -replace destination -string export "$output/ipa/ExportOptions.plist"
    xcodebuild -exportArchive -archivePath "$archive" \
      -exportOptionsPlist "$output/ipa/ExportOptions.plist" -exportPath "$output/ipa"
    ls "$output/ipa"/*.ipa ;;
  upload)
    [[ -d "$archive" ]] || { echo "No archive; run archive first." >&2; exit 1; }
    asc_env
    xcodebuild -exportArchive -archivePath "$archive" \
      -exportOptionsPlist "$host/ExportOptions.plist" -exportPath "$output/upload" \
      -authenticationKeyPath "$ASC_API_PRIVATE_KEY_PATH" \
      -authenticationKeyID "$ASC_API_KEY_ID" -authenticationKeyIssuerID "$ASC_API_ISSUER_ID" ;;
  clean) rm -rf "$output/DerivedData" "$output/upload" "$output/ipa" "$output/game" ;;
  *) echo "usage: bins/icecream-ios/build.sh game|sim|test|archive|ipa|upload|clean" >&2; exit 64 ;;
esac
