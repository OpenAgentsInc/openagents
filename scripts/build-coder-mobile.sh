#!/usr/bin/env bash
# Build the existing Coder Xcode app with its public Rust application library.
# Provider credentials are excluded from all compiler and signing processes.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${CODER_IOS_SANITIZED:-}" != 1 ]]; then
  exec env -i \
    HOME="$HOME" PATH="$PATH" TMPDIR="${TMPDIR:-/tmp}" USER="$(id -un)" LOGNAME="$(id -un)" \
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/../target}" \
    CODER_IOS_OUTPUT="${CODER_IOS_OUTPUT:-$root/../target/coder-ios}" \
    CODER_IOS_DEV_PROFILE="${CODER_IOS_DEV_PROFILE:-}" \
    CODER_IOS_BUILD_NUMBER="${CODER_IOS_BUILD_NUMBER:-}" \
    CODER_IOS_DEVICE="${CODER_IOS_DEVICE:-booted}" \
    CODER_IOS_RUST_PROFILE="${CODER_IOS_RUST_PROFILE:-debug}" \
    DEVELOPER_DIR="${DEVELOPER_DIR:-$(xcode-select -p)}" \
    CODER_IOS_SANITIZED=1 /bin/bash "$root/scripts/build-coder-mobile.sh" "$@"
fi

command="${1:-sim-build}"
host="$root/bins/coder-ios/host"
output="$CODER_IOS_OUTPUT"
# Cargo resolves a relative target directory from the checkout; Xcode resolves
# library paths from its project directory. Give both the same absolute path.
[[ "$CARGO_TARGET_DIR" == /* ]] || export CARGO_TARGET_DIR="$root/$CARGO_TARGET_DIR"
[[ "$output" == /* ]] || output="$root/$output"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=2
export IPHONEOS_DEPLOYMENT_TARGET=17.0

usage() {
  echo "usage: scripts/build-coder-mobile.sh project|sim-build|sim|sim-test|device|archive|export [--synthetic]" >&2
  echo "sim installs and launches the existing com.openagents.coder app on CODER_IOS_DEVICE (default: booted)." >&2
  echo "CODER_IOS_RUST_PROFILE=release also checks optimized Rust in the simulator; device/archive always use release." >&2
}

verify_app() {
  python3 "$root/scripts/verify-coder-ios-bundle.py" "$1" | tee "$2"
}

case "$command" in project|sim-build|sim|sim-test|device|archive|export) ;; *) usage; exit 64 ;; esac
synthetic=false
if [[ "${2:-}" == --synthetic ]]; then synthetic=true
elif [[ -n "${2:-}" ]]; then usage; exit 64
fi
if [[ $# -gt 2 ]]; then usage; exit 64; fi

mkdir -p "$output"
if [[ "$command" == archive ]]; then
  # The release operator commits source before archiving. Keep its identity and
  # any remaining workspace changes beside the archive, without storing a diff.
  git -C "$root" rev-parse HEAD > "$output/archive-source.commit"
  git -C "$root" diff --binary HEAD | shasum -a 256 > "$output/archive-source.diff.sha256"
  git -C "$root" status --porcelain=v1 --untracked-files=all > "$output/archive-source.status"
  shasum -a 256 "$root/Cargo.lock" > "$output/archive-cargo-lock.sha256"
  rustc --version > "$output/archive-rust-version.txt"
  xcodebuild -version > "$output/archive-xcode-version.txt"
fi
if [[ "$command" == export ]]; then
  [[ -d "$output/Coder.xcarchive" ]] || { echo "No archive; build one first." >&2; exit 1; }
  verify_app "$output/Coder.xcarchive/Products/Applications/Coder.app" "$output/archive-bundle-verification.json"
  # Preserve the existing upload configuration. A local export changes only
  # destination in a temporary copy and does not publish the archive.
  options="$output/LocalExportOptions.plist"
  cp "$host/ExportOptions.plist" "$options"
  /usr/libexec/PlistBuddy -c 'Set :destination export' "$options"
  xcodebuild -exportArchive -archivePath "$output/Coder.xcarchive" \
    -exportOptionsPlist "$options" -exportPath "$output/export"
  extracted="$(mktemp -d "${TMPDIR:-/tmp}/coder-ios-export.XXXXXX")"
  trap 'rm -rf "$extracted"' EXIT
  /usr/bin/ditto -xk "$output/export/Coder.ipa" "$extracted"
  verify_app "$extracted/Payload/Coder.app" "$output/export-bundle-verification.json"
  exit
fi
project() { (cd "$host" && xcodegen generate); }
if [[ "$command" == project ]]; then project; exit; fi

case "$command" in
  sim|sim-build|sim-test) triple=aarch64-apple-ios-sim; destination='generic/platform=iOS Simulator' ;;
  *) triple=aarch64-apple-ios; destination='generic/platform=iOS' ;;
esac
if [[ "$command" == sim-test ]]; then
  device_id="$(xcrun simctl getenv "$CODER_IOS_DEVICE" SIMULATOR_UDID)"
  destination="platform=iOS Simulator,id=$device_id"
fi
profile="${CODER_IOS_RUST_PROFILE:-debug}"
case "$profile" in debug|release) ;; *) echo "CODER_IOS_RUST_PROFILE must be debug or release." >&2; exit 64 ;; esac
rust_command=(cargo build --locked -p coder-mobile --lib --target "$triple")
if [[ "$command" == device || "$command" == archive ]]; then
  profile=release
fi
if [[ "$profile" == release ]]; then rust_command+=(--release); fi
(cd "$root" && "${rust_command[@]}")
project
library="$CARGO_TARGET_DIR/$triple/$profile"
args=(-project "$host/Coder.xcodeproj" -scheme Coder -configuration Release
      -destination "$destination" -derivedDataPath "$output/DerivedData"
      "CODER_RUST_LIBRARY_DIR=$library")
if [[ -n "$CODER_IOS_BUILD_NUMBER" ]]; then
  [[ "$CODER_IOS_BUILD_NUMBER" =~ ^[0-9]+$ ]] || { echo "Build number must be a positive integer." >&2; exit 64; }
  [[ "$CODER_IOS_BUILD_NUMBER" != 0 ]] || { echo "Build number must be positive." >&2; exit 64; }
  args+=("CURRENT_PROJECT_VERSION=$CODER_IOS_BUILD_NUMBER")
fi

case "$command" in
  sim-test)
    results="$output/TestResults-$(date -u +%Y%m%dT%H%M%SZ)-$$.xcresult"
    xcodebuild "${args[@]}" \
      CODE_SIGN_IDENTITY=- PROVISIONING_PROFILE_SPECIFIER= \
      -resultBundlePath "$results" -parallel-testing-enabled NO test
    verify_app "$output/DerivedData/Build/Products/Release-iphonesimulator/Coder.app" "$output/simulator-bundle-verification.json"
    echo "Native app test results: $results"
    ;;
  sim|sim-build)
    xcodebuild "${args[@]}" CODE_SIGN_IDENTITY=- PROVISIONING_PROFILE_SPECIFIER= build
    app="$output/DerivedData/Build/Products/Release-iphonesimulator/Coder.app"
    verify_app "$app" "$output/simulator-bundle-verification.json"
    echo "Built $app"
    if [[ "$command" == sim ]]; then
      xcrun simctl install "$CODER_IOS_DEVICE" "$app"
      launch=(xcrun simctl launch "$CODER_IOS_DEVICE" com.openagents.coder)
      if [[ "$synthetic" == true ]]; then launch+=(--synthetic); fi
      "${launch[@]}"
    fi
    ;;
  device)
    if [[ -n "$CODER_IOS_DEV_PROFILE" ]]; then
      signing=(CODE_SIGN_STYLE=Manual 'CODE_SIGN_IDENTITY=Apple Development'
               "PROVISIONING_PROFILE_SPECIFIER=$CODER_IOS_DEV_PROFILE")
    else
      signing=(CODE_SIGN_STYLE=Automatic 'CODE_SIGN_IDENTITY=Apple Development'
               PROVISIONING_PROFILE_SPECIFIER= -allowProvisioningUpdates)
    fi
    xcodebuild "${args[@]}" "${signing[@]}" build
    verify_app "$output/DerivedData/Build/Products/Release-iphoneos/Coder.app" "$output/device-bundle-verification.json"
    echo "Built $output/DerivedData/Build/Products/Release-iphoneos/Coder.app"
    ;;
  archive)
    xcodebuild "${args[@]}" -archivePath "$output/Coder.xcarchive" archive
    verify_app "$output/Coder.xcarchive/Products/Applications/Coder.app" "$output/archive-bundle-verification.json"
    shasum -a 256 "$output/Coder.xcarchive/Products/Applications/Coder.app/Coder" \
      > "$output/archive-executable.sha256"
    echo "Archived $output/Coder.xcarchive. This command does not upload or distribute it."
    ;;
esac
