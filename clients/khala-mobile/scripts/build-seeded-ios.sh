#!/usr/bin/env bash
# Owner-authorized seeded-account Release simulator build for the Khala Mobile
# unattended straight-line E2E (issue #8543).
#
# Builds a Release iOS-Simulator app that AUTO-SIGNS-IN as the seeded
# public-safe AgentFlampy account (the owner-approved launch seed recorded on
# #8543: GitHub user AgentFlampy with fork AgentFlampy/openagents), reading
# credentials from `~/work/.secrets/khala-maestro.env` — the SAME env file the
# SignedInThreadSmoke / SignedInThreadReply / straight-line runners source, so
# the baked identity and the runner identity can never drift apart (that drift
# is exactly what broke the 2026-07-08 smoke run: the installed build was baked
# from the OLDER khala-mobile-emulator-test.env identity while the runner
# asserted the AgentFlampy seeded thread).
#
# HOW THE AUTH BYPASS WORKS: identical to scripts/emulator-test-run.sh — the
# app's `devEnvCredentials` path (src/config/khala-sync-demo.ts +
# src/auth/khala-auth-context.tsx) seeds a signed-in session directly from
# EXPO_PUBLIC_KHALA_SYNC_DEMO_* inlined at Metro bundle time from `.env.local`.
# DEV-ONLY by construction: a normal Release/TestFlight archive has no
# `.env.local` and always shows the real GitHub sign-in screen.
#
# SAFETY: `.env.local` is written ONLY for the duration of this build and is
# ALWAYS removed on exit (trap). NEVER run a TestFlight archive while
# `.env.local` exists. NEVER commit `~/work/.secrets/khala-maestro.env`.
#
# Prereqs: a booted simulator, `expo prebuild --platform ios` + `pod install`
# already run (ios/ is gitignored CNG output), and the seeded secret file.
#
# Usage: bash scripts/build-seeded-ios.sh [simulator-name]   (default: iPhone 17 Pro)
set -euo pipefail

SIM_NAME="${1:-iPhone 17 Pro}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRET="${HOME}/work/.secrets/khala-maestro.env"
ENV_LOCAL="${HERE}/.env.local"
ARCHIVE_DERIVED="${KHALA_IOS_DERIVED_DIR:-/tmp/khala-sim-build-seeded}"
BUNDLE_ID="com.openagents.khala.mobile"

if [[ ! -f "$SECRET" ]]; then
  echo "ERROR: missing $SECRET (seeded AgentFlampy Maestro creds)." >&2
  exit 1
fi
# shellcheck disable=SC1090
source "$SECRET"

cleanup() {
  rm -f "$ENV_LOCAL"
  echo "cleaned up .env.local"
}
trap cleanup EXIT

cat > "$ENV_LOCAL" <<EOF
# TEMPORARY — written by scripts/build-seeded-ios.sh, removed on exit.
EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID=${KHALA_MAESTRO_OWNER_USER_ID}
EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN=${KHALA_MAESTRO_TOKEN}
EOF

echo "==> building Release simulator app (seeded AgentFlampy auto-sign-in baked from .env.local)"
rm -f "${ARCHIVE_DERIVED}/Build/Products/Release-iphonesimulator/KhalaCode.app/main.jsbundle" 2>/dev/null || true
( cd "${HERE}/ios" && xcodebuild \
    -workspace KhalaCode.xcworkspace -scheme KhalaCode \
    -configuration Release -sdk iphonesimulator \
    -derivedDataPath "$ARCHIVE_DERIVED" \
    -destination "platform=iOS Simulator,name=${SIM_NAME}" \
    CODE_SIGNING_ALLOWED=NO build )

APP="${ARCHIVE_DERIVED}/Build/Products/Release-iphonesimulator/KhalaCode.app"

echo "==> verifying seeded creds ARE baked (sim build only)"
# Retry once after a short settle: on 2026-07-09 the very first strings|grep
# immediately after xcodebuild returned a false negative on a bundle that was
# in fact correctly baked (verified seconds later by the identical command).
if ! strings "${APP}/main.jsbundle" | grep -qF "${KHALA_MAESTRO_OWNER_USER_ID}"; then
  sleep 3
  if ! strings "${APP}/main.jsbundle" | grep -qF "${KHALA_MAESTRO_OWNER_USER_ID}"; then
    echo "ERROR: seeded creds did not bake — .env.local not picked up by the bundle phase." >&2
    exit 1
  fi
fi

echo "==> installing on '${SIM_NAME}'"
xcrun simctl terminate booted "$BUNDLE_ID" 2>/dev/null || true
xcrun simctl uninstall booted "$BUNDLE_ID" 2>/dev/null || true
xcrun simctl install booted "$APP"
echo "==> installed. Run the E2E with scripts/straight-line-e2e-run.sh."
