#!/usr/bin/env bash
# Publish an OTA to our own OpenAgents Updates server (updates.openagents.com) —
# fully off Expo's CDN. Computes the mobile build's exact runtime fingerprint,
# exports the JS bundle + assets, bakes them as the Cloud Run seed for that
# runtime + the `production` branch, and deploys. The installed off-Expo build
# (which embeds updates.url -> our server) then pulls this on next launch.
#
# Usage: bash apps/oa-updates/scripts/publish-ota.sh
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO"
MOBILE="clients/khala-ios/AutopilotRemoteControl"

echo "==> computing build runtime fingerprint"
RUNTIME="$(cd "$MOBILE" && bunx expo-updates fingerprint:generate --platform ios 2>/dev/null \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["hash"])')"
echo "    runtime = $RUNTIME"

echo "==> exporting JS bundle + assets"
rm -rf "$REPO/apps/oa-updates/dist"
( cd "$MOBILE" && bunx expo export --platform ios --output-dir "$REPO/apps/oa-updates/dist" )

echo "==> deploying to Cloud Run (seed = this export, runtime $RUNTIME, branch production)"
export OA_PUBLIC_URL="${OA_PUBLIC_URL:-https://oa-updates-ezxz4mgdsq-uc.a.run.app}"
export OA_SEED_DIST="/app/dist"
export OA_SEED_RUNTIME="$RUNTIME"
export OA_SEED_PLATFORM="ios"
# #4949 code signing: sign every manifest with our private key (keyid "main",
# rsa-v1_5-sha256) so the client's embedded codeSigningCertificate verifies it.
SIGN_KEY="$REPO/.secrets/oa-updates-codesign-private.pem"
if [[ -z "${OA_SIGNING_KEY:-}" && -f "$SIGN_KEY" ]]; then
  export OA_SIGNING_KEY="$(cat "$SIGN_KEY")"
  echo "    code signing: enabled (keyid main)"
fi
bash "$REPO/apps/oa-updates/scripts/deploy-cloudrun.sh"

echo "==> published. Verify:"
echo "    curl -H 'expo-protocol-version: 1' -H 'expo-platform: ios' -H \"expo-runtime-version: $RUNTIME\" -H 'expo-channel-name: production' https://updates.openagents.com/autopilot/manifest"
