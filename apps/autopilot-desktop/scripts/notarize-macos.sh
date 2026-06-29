#!/usr/bin/env bash
# Sign and notarize an Autopilot Desktop macOS .app with Apple Developer ID.
#
# Required:
#   OA_DEVELOPER_ID_APPLICATION="Developer ID Application: OpenAgents, Inc. (...)"
#   OA_DESKTOP_APP_PATH=/path/to/Autopilot\ Desktop.app
#
# Notary auth, either:
#   OA_NOTARY_KEYCHAIN_PROFILE=<notarytool keychain profile>
# or App Store Connect API key env:
#   ASC_API_KEY_ID / ASC_API_ISSUER_ID / ASC_API_PRIVATE_KEY_PATH
#
# Optional:
#   OA_ASC_ENV=/path/to/appstoreconnect.env
#   OA_DESKTOP_ARCHIVE=/tmp/autopilot-desktop-notary.zip
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ENV_FILE="${OA_ASC_ENV:-$REPO/.secrets/appstoreconnect.env}"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  . "$ENV_FILE"
  set +a
fi

APP_PATH="${OA_DESKTOP_APP_PATH:-}"
IDENTITY="${OA_DEVELOPER_ID_APPLICATION:-${APPLE_DEVELOPER_ID_APPLICATION:-}}"
ARCHIVE="${OA_DESKTOP_ARCHIVE:-/tmp/autopilot-desktop-notary.zip}"

if [[ -z "$APP_PATH" || ! -d "$APP_PATH" ]]; then
  echo "OA_DESKTOP_APP_PATH must point to the built Autopilot .app" >&2
  exit 1
fi

if [[ -z "$IDENTITY" ]]; then
  echo "Set OA_DEVELOPER_ID_APPLICATION to the Developer ID Application signing identity" >&2
  exit 1
fi

# Pre-notarization gate: a signed/notarized recut for
# autopilot.local_apple_fm_tool_chat.v1 must actually bundle the Apple FM bridge
# helper at the Pylon-discovery path, else local sessions can never start. Set
# OA_SKIP_APPLE_FM_BRIDGE_CHECK=1 to ship an intentionally Apple-FM-less build.
if [[ "${OA_SKIP_APPLE_FM_BRIDGE_CHECK:-0}" != "1" ]]; then
  echo "==> verifying packaged Apple FM bridge helper"
  bun "$(dirname "${BASH_SOURCE[0]}")/verify-packaged-apple-fm-bridge.ts"
fi

echo "==> code signing $APP_PATH"
codesign --force --deep --options runtime --timestamp --sign "$IDENTITY" "$APP_PATH"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

echo "==> packaging for notarytool"
rm -f "$ARCHIVE"
ditto -c -k --keepParent "$APP_PATH" "$ARCHIVE"

echo "==> submitting to Apple notarization"
if [[ -n "${OA_NOTARY_KEYCHAIN_PROFILE:-}" ]]; then
  xcrun notarytool submit "$ARCHIVE" \
    --keychain-profile "$OA_NOTARY_KEYCHAIN_PROFILE" \
    --wait
else
  : "${ASC_API_KEY_ID:?set ASC_API_KEY_ID or OA_NOTARY_KEYCHAIN_PROFILE}"
  : "${ASC_API_ISSUER_ID:?set ASC_API_ISSUER_ID or OA_NOTARY_KEYCHAIN_PROFILE}"
  : "${ASC_API_PRIVATE_KEY_PATH:?set ASC_API_PRIVATE_KEY_PATH or OA_NOTARY_KEYCHAIN_PROFILE}"
  xcrun notarytool submit "$ARCHIVE" \
    --key "$ASC_API_PRIVATE_KEY_PATH" \
    --key-id "$ASC_API_KEY_ID" \
    --issuer "$ASC_API_ISSUER_ID" \
    --wait
fi

echo "==> stapling notarization ticket"
xcrun stapler staple "$APP_PATH"
spctl -a -vvv -t exec "$APP_PATH"

echo "==> notarized: $APP_PATH"
echo "    archive: $ARCHIVE"
