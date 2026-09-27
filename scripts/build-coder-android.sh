#!/usr/bin/env bash
# Build Coder's native Android host around the public Rust mobile library.
# Compiler processes receive toolchain settings, never provider credentials.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${CODER_ANDROID_SANITIZED:-}" != 1 ]]; then
  exec env -i \
    HOME="$HOME" PATH="$PATH" TMPDIR="${TMPDIR:-/tmp}" USER="$(id -un)" LOGNAME="$(id -un)" \
    JAVA_HOME="${JAVA_HOME:-}" ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}" \
    ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-}" \
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/../target}" \
    CODER_ANDROID_OUTPUT="${CODER_ANDROID_OUTPUT:-$root/../target/coder-android}" \
    CODER_ANDROID_ABI="${CODER_ANDROID_ABI:-arm64-v8a}" \
    CODER_ANDROID_SERIAL="${CODER_ANDROID_SERIAL:-}" \
    CODER_ANDROID_PROFILE="${CODER_ANDROID_PROFILE:-dev}" \
    CODER_ANDROID_SANITIZED=1 /bin/bash "$root/scripts/build-coder-android.sh" "$@"
fi

usage() {
  cat >&2 <<'USAGE'
usage: scripts/build-coder-android.sh rust|apk|package|install|launch|run|check|test|abi [--synthetic]

rust builds libcoder_mobile.so; apk packages that library; package does both.
install updates the debug app without clearing its data. launch opens it.
run builds, installs, and launches. check runs Android lint and unit tests.
test builds and runs the isolated synthetic instrumentation suite.

Set CODER_ANDROID_SERIAL explicitly for install, launch, run, or test.
Set CODER_ANDROID_ABI to arm64-v8a (default) or x86_64.
--synthetic launches the isolated test identity/cache instead of saved pairing.
No command creates, resets, or launches an emulator, or publishes an app.
USAGE
}

command="${1:-package}"
case "$command" in rust|apk|package|install|launch|run|check|test|abi) ;; *) usage; exit 64 ;; esac
synthetic=false
if [[ "${2:-}" == --synthetic ]]; then synthetic=true
elif [[ -n "${2:-}" ]]; then usage; exit 64
fi
if [[ $# -gt 2 ]]; then usage; exit 64; fi
case "$CODER_ANDROID_ABI" in arm64-v8a|x86_64) ;; *) echo 'Unsupported Android ABI; use arm64-v8a or x86_64.' >&2; exit 64 ;; esac
case "$CODER_ANDROID_PROFILE" in dev|release) ;; *) echo 'Android profile must be dev or release.' >&2; exit 64 ;; esac
if [[ "$command" == abi ]]; then echo "$CODER_ANDROID_ABI"; exit; fi

if [[ "$(uname -s)" == Darwin ]]; then
  export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
  export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk@17}"
fi
: "${ANDROID_HOME:?Set ANDROID_HOME to an Android SDK with platform 35 and NDK 27.1.12297006.}"
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17.}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.1.12297006}"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=2
[[ "$CARGO_TARGET_DIR" == /* ]] || export CARGO_TARGET_DIR="$root/$CARGO_TARGET_DIR"
[[ "$CODER_ANDROID_OUTPUT" == /* ]] || CODER_ANDROID_OUTPUT="$root/$CODER_ANDROID_OUTPUT"
output="$CODER_ANDROID_OUTPUT"
host="$root/bins/coder-android/host"
native="$output/jniLibs"
apk_path="$output/gradle/app/outputs/apk/debug/app-debug.apk"
mkdir -p "$output"

require_device() {
  [[ -n "$CODER_ANDROID_SERIAL" ]] || { echo 'Set CODER_ANDROID_SERIAL to the intended device from adb devices.' >&2; exit 64; }
  [[ "$CODER_ANDROID_SERIAL" =~ ^[a-zA-Z0-9._:-]+$ ]] || { echo 'Invalid Android device serial.' >&2; exit 64; }
  [[ "$(adb -s "$CODER_ANDROID_SERIAL" get-state)" == device ]] || { echo 'The selected Android device is not ready.' >&2; exit 1; }
  local device_abi
  device_abi="$(adb -s "$CODER_ANDROID_SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
  [[ "$device_abi" == "$CODER_ANDROID_ABI" ]] || { echo "Device ABI is $device_abi; set CODER_ANDROID_ABI to match." >&2; exit 64; }
  export ANDROID_SERIAL="$CODER_ANDROID_SERIAL"
}

gradle_() {
  "$host/gradlew" --no-daemon --console=plain -p "$host" \
    --project-cache-dir "$output/project-cache" \
    -Pandroid.injected.androidTest.leaveApksInstalledAfterRun=true \
    -Pandroid.experimental.testOptions.uninstallIncompatibleApks=false \
    "-PcoderOutputDir=$output/gradle" "-PcoderNativeDir=$native" \
    "-PcoderAbi=$CODER_ANDROID_ABI" "$@"
}

rust() {
  [[ -d "$ANDROID_NDK_HOME" ]] || { echo "Android NDK is missing: $ANDROID_NDK_HOME" >&2; exit 1; }
  command -v cargo-ndk >/dev/null || { echo 'Install cargo-ndk before building Android.' >&2; exit 1; }
  local build=(build --locked -p coder-mobile --lib)
  [[ "$CODER_ANDROID_PROFILE" != release ]] || build+=(--release)
  # NDK r27 needs an explicit alignment for devices with 16 KiB memory pages.
  export RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384'
  (cd "$root" && cargo ndk -t "$CODER_ANDROID_ABI" --platform 26 -o "$native" \
    -- "${build[@]}")
  [[ -f "$native/$CODER_ANDROID_ABI/libcoder_mobile.so" ]] || { echo 'The Rust build did not produce libcoder_mobile.so.' >&2; exit 1; }
}

apk() {
  [[ -f "$native/$CODER_ANDROID_ABI/libcoder_mobile.so" ]] || { echo 'Build the Rust library first with the rust or package command.' >&2; exit 1; }
  gradle_ assembleDebug
  "$ANDROID_HOME/build-tools/35.0.0/zipalign" -c -P 16 4 "$apk_path"
  echo "Built $apk_path"
}

install() {
  require_device
  [[ -f "$apk_path" ]] || { echo 'Build the APK first with the package command.' >&2; exit 1; }
  # An incompatible existing signing key fails here. Never erase app data to
  # work around it; the operator decides how to migrate that installation.
  adb -s "$CODER_ANDROID_SERIAL" install -r "$apk_path"
}

launch() {
  require_device
  adb -s "$CODER_ANDROID_SERIAL" shell am start -S -W \
    -n com.openagents.coder/.MainActivity --ez synthetic "$synthetic"
}

package() { rust; apk; }
run() { require_device; package; install; launch; }
check() { gradle_ lintDebug testDebugUnitTest; }
test() { require_device; package; gradle_ connectedDebugAndroidTest; }
"$command"
