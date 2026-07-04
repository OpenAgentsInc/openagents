#!/usr/bin/env bash
# Owner-run Khala Code Desktop macOS release lane. It intentionally stops before
# outward-facing upload or GitHub release creation unless the owner opts in.
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO="$(cd "$APP_DIR/../.." && pwd)"

CHANNEL=""
VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --channel)
      CHANNEL="${2:-}"
      shift 2
      ;;
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    *)
      echo "Usage: bash scripts/release-macos.sh --channel rc --version 0.1.0-rc.1" >&2
      exit 64
      ;;
  esac
done

if [[ "$CHANNEL" != "rc" && "$CHANNEL" != "stable" ]]; then
  echo "channel must be rc or stable" >&2
  exit 64
fi

if [[ -z "$VERSION" ]]; then
  echo "version is required" >&2
  exit 64
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Khala Code Desktop signing/notarization must run on macOS" >&2
  exit 1
fi

for tool in bun codesign hdiutil xcrun spctl; do
  command -v "$tool" >/dev/null || {
    echo "missing required tool: $tool" >&2
    exit 1
  }
done

DIST_DIR="$APP_DIR/artifacts/release/$VERSION"
DMG_PATH="$DIST_DIR/Khala-Code-$VERSION.dmg"
ARCHIVE_PATH="$DIST_DIR/Khala-Code-$VERSION.app-notary.zip"
PLAN_PATH="$DIST_DIR/release-plan.json"
NOTES_PATH="$DIST_DIR/github-release-notes.md"
TAG="khala-code-desktop-v$VERSION"

mkdir -p "$DIST_DIR"

echo "==> validating Khala Code release plan"
bun "$APP_DIR/scripts/release-plan.ts" \
  --version "$VERSION" \
  --channel "$CHANNEL" \
  --artifact "$DMG_PATH" > "$PLAN_PATH"

echo "==> building unsigned Electrobun app"
bun run --cwd "$APP_DIR" "build:$CHANNEL"

APP_PATH="${KHALA_CODE_RELEASE_APP_PATH:-}"
if [[ -z "$APP_PATH" ]]; then
  APP_PATH="$(find "$APP_DIR/build" -maxdepth 2 -type d -name "Khala Code.app" | sort | tail -n 1)"
fi

if [[ -z "$APP_PATH" || ! -d "$APP_PATH" ]]; then
  echo "built Khala Code.app was not found under $APP_DIR/build" >&2
  exit 1
fi

echo "==> signing, notarizing, and stapling app"
OA_DESKTOP_APP_PATH="$APP_PATH" \
OA_DESKTOP_ARCHIVE="$ARCHIVE_PATH" \
OA_SKIP_APPLE_FM_BRIDGE_CHECK=1 \
bash "$REPO/apps/autopilot-desktop/scripts/notarize-macos.sh"

ENV_FILE="${OA_ASC_ENV:-$REPO/.secrets/appstoreconnect.env}"
if [[ -f "$ENV_FILE" ]]; then
  set -a
  . "$ENV_FILE"
  set +a
fi

IDENTITY="${OA_DEVELOPER_ID_APPLICATION:-${APPLE_DEVELOPER_ID_APPLICATION:-}}"
if [[ -z "$IDENTITY" ]]; then
  echo "Set OA_DEVELOPER_ID_APPLICATION to the Developer ID Application identity" >&2
  exit 1
fi

echo "==> re-creating DMG from stapled app"
rm -f "$DMG_PATH"
hdiutil create \
  -volname "Khala Code" \
  -srcfolder "$APP_PATH" \
  -ov \
  -format UDZO \
  "$DMG_PATH"

echo "==> signing DMG"
codesign --force --timestamp --sign "$IDENTITY" "$DMG_PATH"
codesign --verify --verbose=2 "$DMG_PATH"

echo "==> notarizing DMG"
if [[ -n "${OA_NOTARY_KEYCHAIN_PROFILE:-}" ]]; then
  xcrun notarytool submit "$DMG_PATH" \
    --keychain-profile "$OA_NOTARY_KEYCHAIN_PROFILE" \
    --wait
else
  : "${ASC_API_KEY_ID:?set ASC_API_KEY_ID or OA_NOTARY_KEYCHAIN_PROFILE}"
  : "${ASC_API_ISSUER_ID:?set ASC_API_ISSUER_ID or OA_NOTARY_KEYCHAIN_PROFILE}"
  : "${ASC_API_PRIVATE_KEY_PATH:?set ASC_API_PRIVATE_KEY_PATH or OA_NOTARY_KEYCHAIN_PROFILE}"
  xcrun notarytool submit "$DMG_PATH" \
    --key "$ASC_API_PRIVATE_KEY_PATH" \
    --key-id "$ASC_API_KEY_ID" \
    --issuer "$ASC_API_ISSUER_ID" \
    --wait
fi

echo "==> stapling DMG"
xcrun stapler staple "$DMG_PATH"
spctl -a -vvv -t install "$DMG_PATH"

echo "==> staging product-specific updates feed"
bun "$REPO/apps/oa-updates/scripts/publish-desktop-release.ts" \
  --product khala-code-desktop \
  --channel "$CHANNEL" \
  --version "$VERSION" \
  --artifact "$DMG_PATH" \
  --out "$DIST_DIR/desktop-dist"

cat > "$NOTES_PATH" <<NOTES
Khala Code Desktop $VERSION

- Signed, notarized, and stapled macOS DMG.
- Updates feed product: khala-code-desktop.
- Channel: $CHANNEL.
- Clean-Mac first-run smoke receipt must be attached before closing RL-1.
NOTES

if [[ "${KHALA_CODE_RELEASE_UPLOAD:-0}" == "1" ]]; then
  gcloud storage cp --recursive "$DIST_DIR/desktop-dist/" \
    "gs://openagentsgemini-oa-updates/desktop/khala-code-desktop/$CHANNEL/"
else
  echo "==> upload skipped; set KHALA_CODE_RELEASE_UPLOAD=1 after owner approval"
fi

if [[ "${KHALA_CODE_RELEASE_CREATE_GITHUB:-0}" == "1" ]]; then
  if [[ "$CHANNEL" == "rc" ]]; then
    gh release create "$TAG" "$DMG_PATH" \
      --repo OpenAgentsInc/openagents \
      --title "Khala Code Desktop $VERSION" \
      --notes-file "$NOTES_PATH" \
      --prerelease \
      --latest=false
  else
    gh release create "$TAG" "$DMG_PATH" \
      --repo OpenAgentsInc/openagents \
      --title "Khala Code Desktop $VERSION" \
      --notes-file "$NOTES_PATH"
  fi
else
  echo "==> GitHub release skipped; set KHALA_CODE_RELEASE_CREATE_GITHUB=1 after owner approval"
fi

echo "==> Khala Code Desktop release artifact staged at $DMG_PATH"
