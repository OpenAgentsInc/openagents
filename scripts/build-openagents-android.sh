#!/usr/bin/env bash
# Build the OpenAgents Android app around its Rust library, openagents-mobile.
# Compiler processes receive toolchain settings, never provider credentials.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${OPENAGENTS_ANDROID_SANITIZED:-}" != 1 ]]; then
  exec env -i \
    HOME="$HOME" PATH="$PATH" TMPDIR="${TMPDIR:-/tmp}" USER="$(id -un)" LOGNAME="$(id -un)" \
    JAVA_HOME="${JAVA_HOME:-}" ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}" \
    ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-}" \
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/../target}" \
    OPENAGENTS_ANDROID_OUTPUT="${OPENAGENTS_ANDROID_OUTPUT:-}" \
    OPENAGENTS_ANDROID_ABI="${OPENAGENTS_ANDROID_ABI:-arm64-v8a}" \
    OPENAGENTS_ANDROID_SERIAL="${OPENAGENTS_ANDROID_SERIAL:-}" \
    OPENAGENTS_ANDROID_PROFILE="${OPENAGENTS_ANDROID_PROFILE:-dev}" \
    OPENAGENTS_ANDROID_VERSION_CODE="${OPENAGENTS_ANDROID_VERSION_CODE:-1}" \
    OPENAGENTS_ANDROID_KEYSTORE="${OPENAGENTS_ANDROID_KEYSTORE:-}" \
    OPENAGENTS_ANDROID_KEY_ALIAS="${OPENAGENTS_ANDROID_KEY_ALIAS:-}" \
    OPENAGENTS_ANDROID_KEYSTORE_PASSWORD="${OPENAGENTS_ANDROID_KEYSTORE_PASSWORD:-}" \
    OPENAGENTS_ANDROID_KEY_PASSWORD="${OPENAGENTS_ANDROID_KEY_PASSWORD:-}" \
    OPENAGENTS_ANDROID_SANITIZED=1 /bin/bash "$root/scripts/build-openagents-android.sh" "$@"
fi

usage() {
  cat >&2 <<'USAGE'
usage: scripts/build-openagents-android.sh rust|apk|package|install|launch|run|bundle|check|abi

rust builds libopenagents_mobile.so; apk packages it as a debug APK; package
does both. install updates the debug app without clearing its data. launch
opens it. run builds, installs, and launches. bundle builds a release Android
App Bundle (.aab) for Google Play, signed when a keystore is set. check runs
Android lint and unit tests.

Set OPENAGENTS_ANDROID_SERIAL for install, launch, and run.
Set OPENAGENTS_ANDROID_ABI to arm64-v8a (default) or x86_64.
bundle builds arm64-v8a with the release Rust profile. It signs with
OPENAGENTS_ANDROID_KEYSTORE, OPENAGENTS_ANDROID_KEY_ALIAS,
OPENAGENTS_ANDROID_KEYSTORE_PASSWORD, and OPENAGENTS_ANDROID_KEY_PASSWORD, and
takes its version code from OPENAGENTS_ANDROID_VERSION_CODE (default 1).
No command creates, resets, or launches an emulator, or uploads an app.
USAGE
}

command="${1:-package}"
case "$command" in rust|apk|package|install|launch|run|bundle|check|abi) ;; *) usage; exit 64 ;; esac
[[ $# -le 1 ]] || { usage; exit 64; }
if [[ "$command" == bundle ]]; then OPENAGENTS_ANDROID_ABI=arm64-v8a; OPENAGENTS_ANDROID_PROFILE=release; fi
case "$OPENAGENTS_ANDROID_ABI" in arm64-v8a|x86_64) ;; *) echo 'Unsupported Android ABI; use arm64-v8a or x86_64.' >&2; exit 64 ;; esac
case "$OPENAGENTS_ANDROID_PROFILE" in dev|release) ;; *) echo 'Android profile must be dev or release.' >&2; exit 64 ;; esac
[[ "$OPENAGENTS_ANDROID_VERSION_CODE" =~ ^[1-9][0-9]{0,8}$ ]] || { echo 'Version code must be a positive integer.' >&2; exit 64; }
if [[ "$command" == abi ]]; then echo "$OPENAGENTS_ANDROID_ABI"; exit; fi

if [[ "$(uname -s)" == Darwin ]]; then
  export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
  export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk@17}"
fi
: "${ANDROID_HOME:?Set ANDROID_HOME to an Android SDK with platform 35 and NDK 27.1.12297006.}"
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17.}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.1.12297006}"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
[[ "$CARGO_TARGET_DIR" == /* ]] || export CARGO_TARGET_DIR="$root/$CARGO_TARGET_DIR"
output="${OPENAGENTS_ANDROID_OUTPUT:-$CARGO_TARGET_DIR/openagents-android}"
[[ "$output" == /* ]] || output="$root/$output"
host="$root/bins/openagents-android/host"
native="$output/jniLibs"
apk_path="$output/gradle/app/outputs/apk/debug/app-debug.apk"
bundle_path="$output/gradle/app/outputs/bundle/release/app-release.aab"
case "$OPENAGENTS_ANDROID_ABI" in arm64-v8a) triple=aarch64-linux-android ;; x86_64) triple=x86_64-linux-android ;; esac
profile_dir=debug; [[ "$OPENAGENTS_ANDROID_PROFILE" == release ]] && profile_dir=release
mkdir -p "$output"

require_device() {
  [[ -n "$OPENAGENTS_ANDROID_SERIAL" ]] || { echo 'Set OPENAGENTS_ANDROID_SERIAL to the intended device from adb devices.' >&2; exit 64; }
  [[ "$OPENAGENTS_ANDROID_SERIAL" =~ ^[a-zA-Z0-9._:-]+$ ]] || { echo 'Invalid Android device serial.' >&2; exit 64; }
  [[ "$(adb -s "$OPENAGENTS_ANDROID_SERIAL" get-state)" == device ]] || { echo 'The selected Android device is not ready.' >&2; exit 1; }
  local device_abi
  device_abi="$(adb -s "$OPENAGENTS_ANDROID_SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
  [[ "$device_abi" == "$OPENAGENTS_ANDROID_ABI" ]] || { echo "Device ABI is $device_abi; set OPENAGENTS_ANDROID_ABI to match." >&2; exit 64; }
  export ANDROID_SERIAL="$OPENAGENTS_ANDROID_SERIAL"
}

gradle_() {
  local signing=()
  if [[ -n "$OPENAGENTS_ANDROID_KEYSTORE" ]]; then
    [[ -f "$OPENAGENTS_ANDROID_KEYSTORE" ]] || { echo 'The release keystore does not exist.' >&2; exit 1; }
    signing+=("-PopenagentsReleaseStoreFile=$OPENAGENTS_ANDROID_KEYSTORE"
      "-PopenagentsReleaseKeyAlias=$OPENAGENTS_ANDROID_KEY_ALIAS"
      "-PopenagentsReleaseStorePassword=$OPENAGENTS_ANDROID_KEYSTORE_PASSWORD"
      "-PopenagentsReleaseKeyPassword=$OPENAGENTS_ANDROID_KEY_PASSWORD")
  fi
  "$host/gradlew" --no-daemon --console=plain -p "$host" \
    --project-cache-dir "$output/project-cache" \
    "-PopenagentsOutputDir=$output/gradle" "-PopenagentsNativeDir=$native" \
    "-PopenagentsAbi=$OPENAGENTS_ANDROID_ABI" \
    "-PopenagentsVersionCode=$OPENAGENTS_ANDROID_VERSION_CODE" ${signing[@]+"${signing[@]}"} "$@"
}

rust() {
  [[ -d "$ANDROID_NDK_HOME" ]] || { echo "Android NDK is missing: $ANDROID_NDK_HOME" >&2; exit 1; }
  command -v cargo-ndk >/dev/null || { echo 'Install cargo-ndk before building Android.' >&2; exit 1; }
  # openagents-mobile is its own Cargo workspace (see its Cargo.toml).
  local build=(build --locked --manifest-path crates/openagents-mobile/Cargo.toml --lib)
  [[ "$OPENAGENTS_ANDROID_PROFILE" != release ]] || build+=(--release)
  # NDK r27 needs an explicit alignment for devices with 16 KiB memory pages.
  export RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384'
  (cd "$root" && cargo ndk -t "$OPENAGENTS_ANDROID_ABI" --platform 26 -- "${build[@]}")
  # Package only this app's library; the target directory also holds other
  # crates' Android libraries, such as Coder's.
  local library="$CARGO_TARGET_DIR/$triple/$profile_dir/libopenagents_mobile.so"
  [[ -f "$library" ]] || { echo 'The Rust build did not produce libopenagents_mobile.so.' >&2; exit 1; }
  rm -rf "$native"; mkdir -p "$native/$OPENAGENTS_ANDROID_ABI"
  cp "$library" "$native/$OPENAGENTS_ANDROID_ABI/"
}

apk() {
  [[ -f "$native/$OPENAGENTS_ANDROID_ABI/libopenagents_mobile.so" ]] || { echo 'Build the Rust library first with the rust or package command.' >&2; exit 1; }
  gradle_ assembleDebug
  "$ANDROID_HOME/build-tools/35.0.0/zipalign" -c -P 16 4 "$apk_path"
  echo "Built $apk_path"
}

bundle() {
  rust
  gradle_ bundleRelease
  echo "Built $bundle_path"
  [[ -n "$OPENAGENTS_ANDROID_KEYSTORE" ]] || echo 'The bundle is unsigned; set the keystore variables to sign it for Google Play.'
}

install() {
  require_device
  [[ -f "$apk_path" ]] || { echo 'Build the APK first with the package command.' >&2; exit 1; }
  # An incompatible signing key fails here. Never erase app data to work
  # around it; the operator decides how to migrate that installation.
  adb -s "$OPENAGENTS_ANDROID_SERIAL" install -r "$apk_path"
}

launch() {
  require_device
  adb -s "$OPENAGENTS_ANDROID_SERIAL" shell am start -S -W -n com.openagents.app/.MainActivity
}

package() { rust; apk; }
run() { require_device; package; install; launch; }
check() { gradle_ lintDebug testDebugUnitTest; }
"$command"
