#!/usr/bin/env bash
# Submit an already-uploaded TestFlight build to App Store review through
# Apple-native App Store Connect tooling. This is intentionally owner-gated:
# TestFlight upload can be automated, but public store release is a product/
# pricing/review decision and must be explicit.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
APP="$REPO/openagents/clients/mobile/AutopilotRemoteControl"
[ -d "$APP" ] || APP="$REPO/clients/mobile/AutopilotRemoteControl"
ENV_FILE="${OA_ASC_ENV:-$REPO/.secrets/appstoreconnect.env}"
[ -f "$ENV_FILE" ] || ENV_FILE="$REPO/openagents/.secrets/appstoreconnect.env"

if [[ "${OA_STORE_RELEASE_APPROVED:-}" != "true" ]]; then
  echo "Refusing to submit App Store review without OA_STORE_RELEASE_APPROVED=true" >&2
  exit 1
fi

: "${OA_STORE_VERSION:?set OA_STORE_VERSION, for example 1.0.0}"
: "${OA_STORE_BUILD_NUMBER:?set OA_STORE_BUILD_NUMBER from the processed TestFlight build}"

set -a
. "$ENV_FILE"
set +a

: "${ASC_API_KEY_ID:?missing ASC_API_KEY_ID in $ENV_FILE}"
: "${ASC_API_ISSUER_ID:?missing ASC_API_ISSUER_ID in $ENV_FILE}"
: "${ASC_API_PRIVATE_KEY_PATH:?missing ASC_API_PRIVATE_KEY_PATH in $ENV_FILE}"

API_KEY_JSON="${OA_ASC_API_KEY_JSON:-}"
if [[ -z "$API_KEY_JSON" ]]; then
  API_KEY_JSON="$(mktemp -t oa-asc-api-key.XXXXXX.json)"
  trap 'rm -f "$API_KEY_JSON"' EXIT
  cat > "$API_KEY_JSON" <<JSON
{
  "key_id": "$ASC_API_KEY_ID",
  "issuer_id": "$ASC_API_ISSUER_ID",
  "key_filepath": "$ASC_API_PRIVATE_KEY_PATH",
  "duration": 1200,
  "in_house": false
}
JSON
fi

cd "$APP"

echo "==> submitting build $OA_STORE_BUILD_NUMBER for App Store review"
fastlane deliver \
  --api_key_path "$API_KEY_JSON" \
  --app_identifier "com.openagents.autopilot-mobile" \
  --app_version "$OA_STORE_VERSION" \
  --build_number "$OA_STORE_BUILD_NUMBER" \
  --skip_binary_upload true \
  --skip_metadata true \
  --skip_screenshots true \
  --submit_for_review true \
  --automatic_release "${OA_STORE_AUTOMATIC_RELEASE:-false}" \
  --force

echo "==> submitted. App Store approval and release timing remain Apple/owner controlled."
