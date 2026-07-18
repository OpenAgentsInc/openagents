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

# Xcode resolves the app root through its physical path before generating a
# fingerprint. macOS aliases /tmp to /private/tmp, so retaining a logical path
# here can produce a different OTA runtime for the exact same source tree.
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)"
cd "$REPO"
# The only supported publish target is the OpenAgents mobile app. Environment
# overrides remain available for isolated development builds, not retired apps.
MOBILE="${OA_MOBILE_APP_DIR:-apps/openagents-mobile}"
PLATFORM="${OA_MOBILE_PLATFORM:-ios}"
UPDATES_OWNER="${OA_UPDATES_OWNER:-openagents-mobile}"
# Channel doubles as the seed branch on the server (identity mapping). The
# OpenAgents app uses its OWN channel — never the legacy khala "production".
CHANNEL="${OA_UPDATES_CHANNEL:-openagents-production}"

echo "==> computing build runtime fingerprint"
RUNTIME="$(cd "$MOBILE" && pnpm exec expo-updates fingerprint:generate --platform "$PLATFORM" 2>/dev/null \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["hash"])')"
echo "    runtime = $RUNTIME"

EXPECTED_RUNTIME="${OA_MOBILE_EXPECTED_RUNTIME:-}"
if [[ -n "$EXPECTED_RUNTIME" && "$RUNTIME" != "$EXPECTED_RUNTIME" ]]; then
  echo "error: computed runtime $RUNTIME does not match expected native runtime $EXPECTED_RUNTIME" >&2
  exit 1
fi

echo "==> exporting JS bundle + assets"
rm -rf "$REPO/apps/oa-updates/dist"
( cd "$MOBILE" && pnpm exec expo export --platform "$PLATFORM" --output-dir "$REPO/apps/oa-updates/dist" )

echo "==> resolving public app config (embedded as manifest extra.expoClient)"
# expo-constants / expo-linking need Constants.expoConfig on a *downloaded*
# update, not just the embedded one — without this a downloaded update throws
# "runtime not ready" the instant it launches and expo-updates silently rolls
# back to the cached/embedded update.
( cd "$MOBILE" && pnpm exec expo config --type public --json > "$REPO/apps/oa-updates/dist/expo-client.json" 2>/dev/null )

echo "==> deploying to Cloud Run (seed = this export, runtime $RUNTIME, branch $CHANNEL)"
export OA_PUBLIC_URL="${OA_PUBLIC_URL:-https://oa-updates-ezxz4mgdsq-uc.a.run.app}"
export OA_SEED_DIST="/app/dist"
export OA_SEED_RUNTIME="$RUNTIME"
export OA_SEED_PLATFORM="$PLATFORM"
export OA_SEED_BRANCH="$CHANNEL"
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

# A service with explicitly pinned revision traffic does not automatically send
# traffic to a newly created revision. Treat deploy as candidate creation, then
# inspect the candidate's exact bytes before promotion. This prevents a
# successful source deploy from leaving clients on an older launch asset.
EXPECTED_BUNDLE="$(node -e '
  const metadata = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8"))
  const platform = process.argv[2]
  const bundle = metadata?.fileMetadata?.[platform]?.bundle
  if (typeof bundle !== "string" || bundle.length === 0) process.exit(1)
  console.log(bundle.split("/").at(-1))
' "$REPO/apps/oa-updates/dist/metadata.json" "$PLATFORM")"
REVISION="$(gcloud run revisions list \
  --service oa-updates \
  --region us-central1 \
  --sort-by='~metadata.creationTimestamp' \
  --limit 1 \
  --format='value(metadata.name)')"
[[ -n "$REVISION" ]] || { echo "error: could not resolve deployed oa-updates revision" >&2; exit 1; }

CANDIDATE_TAG="mobile-ota-candidate"
gcloud run services update-traffic oa-updates \
  --region us-central1 \
  --set-tags "$CANDIDATE_TAG=$REVISION" >/dev/null
SERVICE_URL="$(gcloud run services describe oa-updates --region us-central1 --format='value(status.url)')"
CANDIDATE_URL="${SERVICE_URL/https:\/\//https:\/\/$CANDIDATE_TAG---}"
MANIFEST_FILE="$(mktemp)"
trap 'rm -f "$MANIFEST_FILE"' EXIT
curl -fsS "$CANDIDATE_URL/openagents-mobile/manifest" \
  -o "$MANIFEST_FILE" \
  -H 'expo-protocol-version: 1' \
  -H "expo-platform: $PLATFORM" \
  -H "expo-runtime-version: $RUNTIME" \
  -H "expo-channel-name: $CHANNEL"
grep -Fq "\"key\":\"$EXPECTED_BUNDLE\"" "$MANIFEST_FILE" || {
  echo "error: candidate $REVISION does not serve exported launch asset $EXPECTED_BUNDLE" >&2
  exit 1
}

gcloud run services update-traffic oa-updates \
  --region us-central1 \
  --to-revisions "$REVISION=100" >/dev/null
curl -fsS "https://updates.openagents.com/openagents-mobile/manifest" \
  -o "$MANIFEST_FILE" \
  -H 'expo-protocol-version: 1' \
  -H "expo-platform: $PLATFORM" \
  -H "expo-runtime-version: $RUNTIME" \
  -H "expo-channel-name: $CHANNEL"
grep -Fq "\"key\":\"$EXPECTED_BUNDLE\"" "$MANIFEST_FILE" || {
  echo "error: production traffic does not serve promoted launch asset $EXPECTED_BUNDLE" >&2
  exit 1
}
echo "==> verified and promoted $REVISION ($EXPECTED_BUNDLE)"

echo "==> published. Verify:"
echo "    curl -H 'expo-protocol-version: 1' -H 'expo-platform: $PLATFORM' -H \"expo-runtime-version: $RUNTIME\" -H \"expo-channel-name: $CHANNEL\" https://updates.openagents.com/$UPDATES_OWNER/manifest"
