#!/usr/bin/env bash
# Build the iOS app LOCALLY (our infra, no Expo/EAS cloud) and submit to
# TestFlight via Apple (xcrun altool). This is the rebuild path of the
# self-driving ship loop (CL-39) and the manual TESTFLIGHT.md runbook.
#
# Prereqs (this Mac): Xcode, CocoaPods, fastlane; iOS distribution cert +
# provisioning profile in the login keychain; App Store Connect API key in
# workspace .secrets/appstoreconnect.env (ASC_API_KEY_ID / ASC_API_ISSUER_ID /
# ASC_API_PRIVATE_KEY_PATH). EAS is NOT used.
set -euo pipefail

MODE="build-and-upload"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --build-only)
      MODE="build-only"
      shift
      ;;
    --upload-only)
      MODE="upload-only"
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
APP="$REPO/openagents/clients/mobile/AutopilotRemoteControl"
[ -d "$APP" ] || APP="$REPO/clients/mobile/AutopilotRemoteControl"
IPA="${OA_BUILD_IPA_OUT:-/tmp/oa-autopilot-local.ipa}"
ENV_FILE="${OA_ASC_ENV:-$REPO/.secrets/appstoreconnect.env}"
[ -f "$ENV_FILE" ] || ENV_FILE="$REPO/openagents/.secrets/appstoreconnect.env"

cd "$APP"

if [[ "$MODE" != "upload-only" ]]; then
  echo "==> building signed .ipa locally (off Expo cloud)"
  # Local compile of the production profile. Produces a signed App Store .ipa
  # on this Mac (the heavy native build lives here, not in any cloud).
  npx expo prebuild --platform ios --clean
  ( cd ios && pod install )
  fastlane gym --scheme AutopilotRemoteControl --export_method app-store \
    --output_directory "$(dirname "$IPA")" --output_name "$(basename "$IPA")"
fi

if [[ "$MODE" != "build-only" ]]; then
  echo "==> uploading to TestFlight via Apple altool (no eas submit)"
  set -a; . "$ENV_FILE"; set +a
  xcrun altool --upload-app -f "$IPA" -t ios \
    --apiKey "$ASC_API_KEY_ID" --apiIssuer "$ASC_API_ISSUER_ID"
fi

echo "==> done."
if [[ "$MODE" != "build-only" ]]; then
  echo "    New build will appear in TestFlight after Apple processing (~10-15m)."
fi
echo "    Pull any crash with: bun scripts/testflight-crashes.mjs (see CRASH_LOGS.md)"
