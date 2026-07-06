#!/usr/bin/env bash
# Publish an OTA to our own OpenAgents Updates server (updates.openagents.com) —
# fully off Expo's CDN. Computes the mobile build's exact runtime fingerprint,
# exports the JS bundle + assets, bakes them as the Cloud Run seed for that
# runtime + the `production` branch, and deploys. The installed off-Expo build
# (which embeds updates.url -> our server) then pulls this on next launch.
#
# Usage:
#   bash apps/oa-updates/scripts/publish-ota.sh
#   OA_MOBILE_PLATFORM=android bash apps/oa-updates/scripts/publish-ota.sh
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$REPO"
MOBILE="${OA_MOBILE_APP_DIR:-clients/khala-mobile}"
PLATFORM="${OA_MOBILE_PLATFORM:-ios}"
UPDATES_OWNER="${OA_UPDATES_OWNER:-khala-mobile}"

echo "==> computing build runtime fingerprint"
RUNTIME="$(cd "$MOBILE" && bunx expo-updates fingerprint:generate --platform "$PLATFORM" 2>/dev/null \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["hash"])')"
echo "    runtime = $RUNTIME"

echo "==> exporting JS bundle + assets"
rm -rf "$REPO/apps/oa-updates/dist"
( cd "$MOBILE" && bunx expo export --platform "$PLATFORM" --output-dir "$REPO/apps/oa-updates/dist" )

echo "==> resolving public app config (embedded as manifest extra.expoClient)"
# expo-constants / expo-linking need Constants.expoConfig on a *downloaded*
# update, not just the embedded one — without this a downloaded update throws
# "runtime not ready" the instant it launches and expo-updates silently rolls
# back to the cached/embedded update.
( cd "$MOBILE" && bunx expo config --type public --json > "$REPO/apps/oa-updates/dist/expo-client.json" 2>/dev/null )

echo "==> deploying to Cloud Run (seed = this export, runtime $RUNTIME, branch production)"
export OA_PUBLIC_URL="${OA_PUBLIC_URL:-https://oa-updates-ezxz4mgdsq-uc.a.run.app}"
export OA_SEED_DIST="/app/dist"
export OA_SEED_RUNTIME="$RUNTIME"
export OA_SEED_PLATFORM="$PLATFORM"
export OA_SEED_EXPO_CLIENT_PATH="/app/dist/expo-client.json"
# #4949 code signing: the server signs every manifest with our private key
# (keyid "main", rsa-v1_5-sha256) so the client's embedded
# codeSigningCertificate verifies it. Since #8530 (CFG-14) the key reaches
# Cloud Run from GCP Secret Manager (secret `oa-updates-codesign-key`,
# mounted as OA_SIGNING_KEY by deploy-cloudrun.sh via --set-secrets) — it is
# no longer read from the local .pem here or passed as inline env. The local
# backup stays at .secrets/oa-updates-codesign-private.pem; to rotate, add a
# new secret version (newline-stripped to match the historical inline value):
#   printf '%s' "$(cat .secrets/oa-updates-codesign-private.pem)" \
#     | gcloud secrets versions add oa-updates-codesign-key --data-file=-
echo "    code signing: enabled (keyid main, key via Secret Manager)"
bash "$REPO/apps/oa-updates/scripts/deploy-cloudrun.sh"

echo "==> published. Verify:"
echo "    curl -H 'expo-protocol-version: 1' -H 'expo-platform: $PLATFORM' -H \"expo-runtime-version: $RUNTIME\" -H 'expo-channel-name: production' https://updates.openagents.com/$UPDATES_OWNER/manifest"
