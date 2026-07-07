#!/usr/bin/env bash
# QAM-6 Android emulator lane harness (2026-07-07).
#
# Boots/creates a local Android emulator, installs the Khala Mobile debug APK,
# runs the public-safe Maestro launch/sign-in flows, and captures adb screencaps
# for Android-keyed visual baselines. This is intended for the owned Tailnet Mac
# nightly row, not hosted CI and not EAS.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AVD_NAME="${KHALA_ANDROID_AVD_NAME:-khala_test}"
SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}}"
JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home}"
DEVICE_PROFILE="${KHALA_ANDROID_DEVICE_PROFILE:-pixel_7}"
SYSTEM_IMAGE="${KHALA_ANDROID_SYSTEM_IMAGE:-system-images;android-35;google_apis;arm64-v8a}"
APP_ID="${MAESTRO_APP_ID:-com.openagents.khala.mobile}"
ARTIFACT_DIR="${KHALA_ANDROID_ARTIFACT_DIR:-${HERE}/var/android-emulator-qam-6}"
RECEIPT="${KHALA_ANDROID_EMULATOR_RECEIPT:-${ARTIFACT_DIR}/android-emulator-lane.latest.json}"
APK="${HERE}/android/app/build/outputs/apk/debug/app-debug.apk"

export ANDROID_HOME="$SDK_ROOT"
export ANDROID_SDK_ROOT="$SDK_ROOT"
export JAVA_HOME
export MAESTRO_APP_ID="$APP_ID"
export MAESTRO_CLI_NO_ANALYTICS=1
export PATH="$JAVA_HOME/bin:$SDK_ROOT/cmdline-tools/latest/bin:$SDK_ROOT/emulator:$SDK_ROOT/platform-tools:$HOME/.maestro/bin:$PATH"

mkdir -p "$ARTIFACT_DIR"

write_receipt() {
  local verdict="$1"
  local reason="$2"
  cat > "$RECEIPT" <<EOF
{
  "schema": "openagents.khala_mobile.android_emulator_lane.v1",
  "verdict": "${verdict}",
  "reason": "${reason}",
  "avdName": "${AVD_NAME}",
  "appId": "${APP_ID}",
  "artifactDir": "${ARTIFACT_DIR}",
  "flows": [
    ".maestro/flows/LaunchFallback.yaml",
    ".maestro/flows/LaunchGitHubSignInInteraction.yaml",
    ".maestro/flows/SignedInThreadSmoke.yaml"
  ],
  "captures": {
    "launchFallback": "${ARTIFACT_DIR}/launch-fallback.png",
    "githubSignInInteraction": "${ARTIFACT_DIR}/github-sign-in-interaction.png",
    "signedInThreadSmoke": "${ARTIFACT_DIR}/signed-in-thread-smoke.png"
  }
}
EOF
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    write_receipt "inconclusive" "missing required tool: $1"
    echo "ERROR: missing required tool: $1" >&2
    exit 1
  fi
}

require_tool sdkmanager
require_tool avdmanager
require_tool emulator
require_tool adb
require_tool maestro

if ! avdmanager list avd | grep -q "Name: ${AVD_NAME}$"; then
  yes | sdkmanager --sdk_root="$SDK_ROOT" --licenses >/dev/null
  yes | sdkmanager --sdk_root="$SDK_ROOT" "emulator" "platform-tools" "$SYSTEM_IMAGE" >/dev/null
  echo "no" | avdmanager create avd -n "$AVD_NAME" -k "$SYSTEM_IMAGE" -d "$DEVICE_PROFILE" --force
fi

if ! adb devices | grep -qE "emulator-[0-9]+[[:space:]]+device"; then
  emulator -avd "$AVD_NAME" -no-boot-anim -no-snapshot -netdelay none -netspeed full >"${ARTIFACT_DIR}/emulator.log" 2>&1 &
fi

adb wait-for-device
BOOT_DEADLINE=$((SECONDS + 180))
until [[ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]]; do
  if (( SECONDS > BOOT_DEADLINE )); then
    write_receipt "inconclusive" "emulator boot did not complete within 180 seconds"
    echo "ERROR: emulator boot did not complete within 180 seconds" >&2
    exit 1
  fi
  sleep 2
done

if [[ "${KHALA_ANDROID_SKIP_BUILD:-0}" != "1" ]]; then
  ( cd "$HERE" && bun run prebuild:android && bun run build:android:local )
fi

if [[ ! -f "$APK" ]]; then
  write_receipt "inconclusive" "missing Android debug APK at expected path"
  echo "ERROR: missing Android debug APK at $APK" >&2
  exit 1
fi

adb install -r "$APK"

if [[ "${KHALA_ANDROID_SKIP_METRO:-0}" != "1" ]]; then
  ( cd "$HERE" && bunx expo start --dev-client --host lan --port "${KHALA_ANDROID_METRO_PORT:-8081}" >"${ARTIFACT_DIR}/metro.log" 2>&1 & )
  sleep 8
fi

( cd "$HERE" && maestro test .maestro/flows/LaunchFallback.yaml )
adb exec-out screencap -p > "${ARTIFACT_DIR}/launch-fallback.png"

( cd "$HERE" && maestro test .maestro/flows/LaunchGitHubSignInInteraction.yaml )
adb exec-out screencap -p > "${ARTIFACT_DIR}/github-sign-in-interaction.png"

SIGNED_IN_RESULT="skipped_missing_seeded_thread_env"
if [[ -n "${KHALA_MAESTRO_OWNER_USER_ID:-}" && -n "${KHALA_MAESTRO_TOKEN:-}" && -n "${KHALA_MAESTRO_THREAD_TITLE:-}" ]]; then
  ( cd "$HERE" && maestro test .maestro/flows/SignedInThreadSmoke.yaml )
  adb exec-out screencap -p > "${ARTIFACT_DIR}/signed-in-thread-smoke.png"
  SIGNED_IN_RESULT="passed"
fi

if [[ "$SIGNED_IN_RESULT" == "passed" ]]; then
  write_receipt "passed" "android emulator launch/sign-in/signed-in smoke flows passed"
else
  write_receipt "inconclusive" "launch/sign-in flows passed; signed-in flow skipped because seeded public-safe thread env was absent"
fi

echo "Android emulator lane receipt: $RECEIPT"
