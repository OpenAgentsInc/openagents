#!/usr/bin/env bash
# Owner-authorized emulator functional-test harness (2026-07-06).
#
# Builds a Release iOS-Simulator build that AUTO-SIGNS-IN as a dedicated
# TEST agent (no GitHub login), installs it on the booted simulator, and
# launches it — so an agent can drive real app functionality (threads, sync,
# settings, onboarding) programmatically to surface bugs before users hit
# them.
#
# HOW THE AUTH BYPASS WORKS: the app's `devEnvCredentials` path
# (src/config/khala-sync-demo.ts + src/auth/khala-auth-context.tsx) seeds a
# signed-in session directly from EXPO_PUBLIC_KHALA_SYNC_DEMO_* when both are
# present. Expo inlines those at Metro bundle time from `.env.local`. This is
# DEV-ONLY by construction: a normal Release/TestFlight build has no
# `.env.local`, so it never auto-signs-in and always shows the real GitHub
# sign-in screen. It is never surfaced to users.
#
# SAFETY: `.env.local` is written ONLY for the duration of this build and is
# ALWAYS removed on exit (trap), because a `.env.local` present during ANY
# archive would bake the test token into a shippable build. NEVER run a
# TestFlight archive while `.env.local` exists. This script guarantees it
# doesn't linger.
#
# Prereqs: a booted simulator (`xcrun simctl boot <name>`), CocoaPods
# installed (`cd ios && pod install`), and the secret file
# `~/work/.secrets/khala-mobile-emulator-test.env` (KHALA_MOBILE_TEST_TOKEN,
# KHALA_MOBILE_TEST_OWNER_USER_ID). Regenerate that identity by registering a
# throwaway agent: `POST https://openagents.com/api/agents/register`.
#
# Usage: bash scripts/emulator-test-run.sh [simulator-name]   (default: iPhone 17)
set -euo pipefail

SIM_NAME="${1:-iPhone 17}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRET="${HOME}/work/.secrets/khala-mobile-emulator-test.env"
ENV_LOCAL="${HERE}/.env.local"
ARCHIVE_DERIVED="/tmp/khala-sim-build"
BUNDLE_ID="com.openagents.khala.mobile"

if [[ ! -f "$SECRET" ]]; then
  echo "ERROR: missing $SECRET (the test agent token). See header for how to mint one." >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$SECRET"

cleanup() {
  # ALWAYS remove .env.local so a later archive can never bake test creds.
  rm -f "$ENV_LOCAL"
  echo "cleaned up .env.local"
}
trap cleanup EXIT

cat > "$ENV_LOCAL" <<EOF
# TEMPORARY — written by scripts/emulator-test-run.sh, removed on exit.
EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID=${KHALA_MOBILE_TEST_OWNER_USER_ID}
EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN=${KHALA_MOBILE_TEST_TOKEN}
EOF

echo "==> building Release simulator app (test auto-sign-in baked from .env.local)"
# --reset-cache forces a fresh JS bundle so .env.local + code changes actually
# land (incremental Xcode builds otherwise reuse a cached main.jsbundle).
rm -f "${ARCHIVE_DERIVED}/Build/Products/Release-iphonesimulator/KhalaCode.app/main.jsbundle" 2>/dev/null || true
( cd "${HERE}/ios" && xcodebuild \
    -workspace KhalaCode.xcworkspace -scheme KhalaCode \
    -configuration Release -sdk iphonesimulator \
    -derivedDataPath "$ARCHIVE_DERIVED" \
    -destination "platform=iOS Simulator,name=${SIM_NAME}" \
    CODE_SIGNING_ALLOWED=NO clean build )

APP="${ARCHIVE_DERIVED}/Build/Products/Release-iphonesimulator/KhalaCode.app"

echo "==> verifying test creds ARE baked (sanity for the sim build only)"
if ! strings "${APP}/main.jsbundle" | grep -q "${KHALA_MOBILE_TEST_OWNER_USER_ID}"; then
  echo "ERROR: test creds did not bake — .env.local not picked up by the bundle phase." >&2
  exit 1
fi

echo "==> installing + launching on '${SIM_NAME}'"
xcrun simctl terminate booted "$BUNDLE_ID" 2>/dev/null || true
xcrun simctl uninstall booted "$BUNDLE_ID" 2>/dev/null || true
xcrun simctl install booted "$APP"
xcrun simctl launch booted "$BUNDLE_ID"
echo "==> launched. Drive it with Maestro (.maestro/flows) or simctl screenshots."
