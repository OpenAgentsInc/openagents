#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_SCRIPT="${SCRIPT_DIR}/03-configure-and-start.sh"

assert_contains() {
  local needle="$1"
  local haystack="$2"
  if ! grep -Fq "$needle" <<<"$haystack"; then
    printf 'missing expected content: %s\n' "$needle" >&2
    exit 1
  fi
}

SCRIPT_TEXT="$(cat "$TARGET_SCRIPT")"
ENV_BLOCK="$(sed -n '/cat >"\$TMP_ENV" <<ENV/,/^ENV$/p' "$TARGET_SCRIPT")"

for preserved_var in \
  NEXUS_CONTROL_KERNEL_STATE_PATH \
  NEXUS_CONTROL_TRAINING_TRN_IDENTITY_PATH \
  NEXUS_CONTROL_TRAINING_TRN_RELAY_URLS \
  NEXUS_CONTROL_ADMIN_BEARER_TOKEN \
  NEXUS_CONTROL_TRAINING_GCS_BUCKET_URI \
  NEXUS_CONTROL_TRAINING_GCS_ENDPOINT \
  NEXUS_CONTROL_TRAINING_GCS_SIGNED_URL_TTL_SECONDS \
  NEXUS_CONTROL_TRAINING_GCS_SIGNED_URL_MAX_TTL_SECONDS \
  NEXUS_CONTROL_TRAINING_GCS_SIGNING_CREDENTIALS_PATH
do
  assert_contains "${preserved_var}" "$SCRIPT_TEXT"
  assert_contains "${preserved_var}=\${${preserved_var}}" "$ENV_BLOCK"
done

assert_contains 'Preserving live runtime env ${key}=[redacted]' "$SCRIPT_TEXT"
assert_contains 'Refusing to deploy ${NEXUS_VM} without ${var_name}.' "$SCRIPT_TEXT"
assert_contains 'NEXUS_CONTROL_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED' "$SCRIPT_TEXT"
assert_contains 'NEXUS_CONTROL_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED=${NEXUS_CONTROL_TREASURY_WALLET_REAL_TIME_SYNC_ENABLED}' "$ENV_BLOCK"
assert_contains 'NEXUS_TREASURY_PROVIDER' "$SCRIPT_TEXT"
assert_contains 'NEXUS_TREASURY_PROVIDER=${NEXUS_TREASURY_PROVIDER}' "$ENV_BLOCK"
assert_contains 'NEXUS_LDK_SERVER_URL=${NEXUS_LDK_SERVER_URL}' "$ENV_BLOCK"
assert_contains 'NEXUS_LDK_API_KEY_PATH=${NEXUS_LDK_API_KEY_PATH}' "$ENV_BLOCK"
assert_contains 'NEXUS_LDK_TLS_CERT_PATH=${NEXUS_LDK_TLS_CERT_PATH}' "$ENV_BLOCK"
assert_contains 'resolve_ldk_server_url' "$SCRIPT_TEXT"
assert_contains 'NEXUS_LDK_SERVER_URL:=auto' "$SCRIPT_TEXT"

printf 'ok: nexus configure-and-start preserves runtime env guards\n'
