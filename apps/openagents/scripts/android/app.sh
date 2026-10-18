#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd -P)"
ANDROID_DIR="$ROOT/apps/android"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/work/openagents-target-1-app}"
OPENAGENTS_ANDROID_WORK="${OPENAGENTS_ANDROID_WORK:-$HOME/.openagents/android-build}"
GRADLE_USER_HOME="${GRADLE_USER_HOME:-$OPENAGENTS_ANDROID_WORK/gradle}"
export CARGO_TARGET_DIR OPENAGENTS_ANDROID_WORK GRADLE_USER_HOME

if [[ $# -lt 1 ]]; then
  echo "usage: $0 core|compile|apk|install|run|bundle|check" >&2
  exit 2
fi
command="$1"
shift

clean_env() {
  env -i \
    HOME="$HOME" \
    PATH="$PATH" \
    TMPDIR="${TMPDIR:-/tmp}" \
    JAVA_HOME="${JAVA_HOME:-}" \
    ANDROID_HOME="${ANDROID_HOME:-}" \
    ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-}" \
    ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-}" \
    NIX_SSL_CERT_FILE="${NIX_SSL_CERT_FILE:-}" \
    SSL_CERT_FILE="${SSL_CERT_FILE:-}" \
    ANDROID_USER_HOME="${ANDROID_USER_HOME:-$OPENAGENTS_ANDROID_WORK/android-user}" \
    ANDROID_EMULATOR_HOME="${ANDROID_EMULATOR_HOME:-$OPENAGENTS_ANDROID_WORK/emulator}" \
    GRADLE_USER_HOME="$GRADLE_USER_HOME" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    CARGO_INCREMENTAL=0 \
    OPENAGENTS_KEEP_TARGET_DIR=1 \
    RUSTUP_TOOLCHAIN=1.98.1 \
    OPENAGENTS_ANDROID_WORK="$OPENAGENTS_ANDROID_WORK" \
    "$@"
}

gradle() {
  clean_env gradle --no-daemon --max-workers=2 --project-dir "$ANDROID_DIR" "$@"
}

case "$command" in
  core)
    clean_env bash "$ROOT/scripts/android/build-core.sh"
    ;;
  compile)
    gradle :app:compileDebugKotlin :app:compileReleaseKotlin :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest "$@"
    ;;
  apk)
    gradle :app:assembleDebug "$@"
    ;;
  install)
    apk="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
    [[ -f "$apk" ]] || { echo "Build the debug APK first with apk." >&2; exit 1; }
    clean_env adb install -r "$apk"
    ;;
  run)
    clean_env adb shell monkey -p com.openagents.app.debug 1
    ;;
  bundle)
    gradle :app:bundleRelease "$@"
    ;;
  check)
    gradle :app:compileDebugAndroidTestKotlin :app:testDebugUnitTest :app:assembleDebug :app:assembleRelease "$@"
    clean_env bash "$ROOT/scripts/android/check-release-apk.sh" \
      "$ANDROID_DIR/app/build/outputs/apk/release/app-release-unsigned.apk"
    ;;
  *)
    echo "unknown Android command: $command" >&2
    exit 2
    ;;
esac
