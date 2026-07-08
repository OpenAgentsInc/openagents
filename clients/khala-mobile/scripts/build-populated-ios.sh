#!/usr/bin/env bash
# Owner-authorized POPULATED-happy-path build for the Khala Mobile iOS visual
# tier (QAM-4 follow-up, #8539). It builds a Release iOS-Simulator KhalaCode.app
# that auto-signs-in with a REAL mobile OpenAuth USER session and installs it, so
# the populated flow (SignedInScreensPopulatedVisual) can capture the two screens
# — Credit history + repo picker — with their REAL owner data instead of the
# honest degraded "unavailable" state the seeded agent token produces.
#
# WHY A REAL SESSION IS REQUIRED (not the agent token): the mobile REST routes
# `GET /api/mobile/credits/*` and `GET /api/mobile/repos` require a mobile
# OpenAuth USER bearer session — per apps/openagents.com/INVARIANTS.md the
# `/api/mobile/session` syncToken "is the current OpenAuth mobile access token
# ... not a separate agent/admin credential". The `oa_agent_` token baked by
# scripts/mobile-visual-tier-run.sh (KHALA_MAESTRO_TOKEN) therefore correctly
# 401s on those routes. A real session token (the 400-day OpenAuth mobile access
# token / syncToken from a GitHub sign-in) is what populates them.
#
# HOW TO OBTAIN THE SESSION TOKEN (one-time owner step — needs interactive
# GitHub OAuth, which cannot be done headlessly):
#   1. Install any Khala Code mobile build on the simulator and tap
#      "Sign in with GitHub", complete GitHub login as the AgentFlampy test
#      account, and return via khala://auth.
#   2. That session's syncToken IS the OpenAuth access token. Capture the
#      { ownerUserId, syncToken } pair (e.g. via the app's own
#      `POST /api/mobile/session` echo, or SecureStore) and write it to:
#        ~/work/.secrets/khala-mobile-session.env
#      as:
#        KHALA_MOBILE_SESSION_OWNER_USER_ID=user_...
#        KHALA_MOBILE_SESSION_TOKEN=<the real OpenAuth mobile access token>
#      NEVER commit that file (it is under ~/work/.secrets, gitignored) and
#      never print the token.
#
# SAFETY: `.env.local` is written ONLY for this build and ALWAYS removed on exit
# (trap) — a `.env.local` present during ANY archive would bake a real session
# into a shippable build. NEVER run a TestFlight archive while `.env.local`
# exists.
#
# Usage: bash scripts/build-populated-ios.sh [simulator-udid]
set -euo pipefail

SIM_UDID="${1:-2E5DFC26-DB79-4EE2-BF8E-2EB486A1AFBA}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRET="${HOME}/work/.secrets/khala-mobile-session.env"
ENV_LOCAL="${HERE}/.env.local"
ARCHIVE_DERIVED="/tmp/khala-sim-build"
BUNDLE_ID="com.openagents.khala.mobile"
SIM_NAME="iPhone 17 Pro"
BASE_URL="${OPENAGENTS_BASE_URL:-https://openagents.com}"

if [[ ! -f "$SECRET" ]]; then
  cat >&2 <<MSG
ERROR: missing $SECRET

This build needs a REAL mobile OpenAuth USER session (not the oa_agent_ token).
Obtaining it requires ONE interactive GitHub sign-in on the simulator — see this
script's header for the exact owner step. Once the pair is written to that file,
re-run this script.
MSG
  exit 1
fi
# shellcheck disable=SC1090
set -a; source "$SECRET"; set +a

: "${KHALA_MOBILE_SESSION_OWNER_USER_ID:?set KHALA_MOBILE_SESSION_OWNER_USER_ID in $SECRET}"
: "${KHALA_MOBILE_SESSION_TOKEN:?set KHALA_MOBILE_SESSION_TOKEN in $SECRET}"

# FAIL-CLOSED guard: prove the provided token is a REAL mobile OpenAuth USER
# session before baking anything. An agent token (or any non-user bearer) 401s
# on the owner-scoped mobile credits route, so a 200 with a numeric
# balanceUsdCents is the honest signal that this token will actually populate
# the screens. This refuses to build a "populated" app around a token that would
# only reproduce the degraded state.
echo "==> verifying the provided token is a real mobile OpenAuth session (credits balance 200)"
BAL_STATUS="$(curl -sS -o /tmp/khala-populated-balance.json -w '%{http_code}' \
  -H "authorization: Bearer ${KHALA_MOBILE_SESSION_TOKEN}" \
  "${BASE_URL}/api/mobile/credits/balance")"
if [[ "$BAL_STATUS" != "200" ]]; then
  echo "ERROR: mobile credits balance returned HTTP ${BAL_STATUS}, not 200." >&2
  echo "       The token in $SECRET is NOT a valid mobile OpenAuth user session" >&2
  echo "       (an oa_agent_ token 401s here by design). Re-capture a real GitHub" >&2
  echo "       sign-in session — see this script's header." >&2
  exit 1
fi
if ! grep -q '"balanceUsdCents"' /tmp/khala-populated-balance.json; then
  echo "ERROR: balance response missing balanceUsdCents; refusing to bake." >&2
  exit 1
fi
rm -f /tmp/khala-populated-balance.json
echo "==> token verified as a real mobile session"

cleanup() { rm -f "$ENV_LOCAL"; echo "cleaned up .env.local"; }
trap cleanup EXIT

cat > "$ENV_LOCAL" <<EOF
# TEMPORARY — written by scripts/build-populated-ios.sh, removed on exit.
EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID=${KHALA_MOBILE_SESSION_OWNER_USER_ID}
EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN=${KHALA_MOBILE_SESSION_TOKEN}
EOF

echo "==> building Release simulator app (real-session auto-sign-in baked)"
# Force a fresh JS bundle so .env.local is re-inlined (incremental Xcode builds
# otherwise reuse a cached main.jsbundle).
rm -f "${ARCHIVE_DERIVED}/Build/Products/Release-iphonesimulator/KhalaCode.app/main.jsbundle" 2>/dev/null || true
( cd "${HERE}/ios" && xcodebuild \
    -workspace KhalaCode.xcworkspace -scheme KhalaCode \
    -configuration Release -sdk iphonesimulator \
    -derivedDataPath "$ARCHIVE_DERIVED" \
    -destination "platform=iOS Simulator,name=${SIM_NAME}" \
    CODE_SIGNING_ALLOWED=NO clean build )

APP="${ARCHIVE_DERIVED}/Build/Products/Release-iphonesimulator/KhalaCode.app"

echo "==> verifying real-session owner id baked (sim build sanity only)"
if ! strings "${APP}/main.jsbundle" | grep -q "${KHALA_MOBILE_SESSION_OWNER_USER_ID}"; then
  echo "ERROR: session creds did not bake — .env.local not picked up by the bundle phase." >&2
  exit 1
fi

echo "==> installing on ${SIM_UDID}"
xcrun simctl terminate "$SIM_UDID" "$BUNDLE_ID" 2>/dev/null || true
xcrun simctl uninstall "$SIM_UDID" "$BUNDLE_ID" 2>/dev/null || true
xcrun simctl install "$SIM_UDID" "$APP"
echo "==> POPULATED BUILD INSTALLED OK"
echo "    Next: bash scripts/mobile-visual-tier-run.sh (MOBILE_VISUAL_FLOW=SignedInScreensPopulatedVisual)"
