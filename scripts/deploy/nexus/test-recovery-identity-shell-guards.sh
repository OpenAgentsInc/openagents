#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_SCRIPT="${SCRIPT_DIR}/34-provision-recovery-identity.sh"

assert_contains() {
  local needle="$1"
  local haystack="$2"
  if ! grep -Fq "$needle" <<<"$haystack"; then
    printf 'missing expected content: %s\n' "$needle" >&2
    exit 1
  fi
}

assert_not_contains() {
  local needle="$1"
  local haystack="$2"
  if grep -Fq "$needle" <<<"$haystack"; then
    printf 'unexpected content: %s\n' "$needle" >&2
    exit 1
  fi
}

bash -n "$TARGET_SCRIPT"
SCRIPT_TEXT="$(cat "$TARGET_SCRIPT")"

assert_contains 'NEXUS_RECOVERY_IDENTITY_DRY_RUN' "$SCRIPT_TEXT"
assert_contains 'NEXUS_RECOVERY_IMPERSONATOR_MEMBER' "$SCRIPT_TEXT"
assert_contains 'compute.instances.get' "$SCRIPT_TEXT"
assert_contains 'compute.instances.reset' "$SCRIPT_TEXT"
assert_contains 'compute.instances.getSerialPortOutput' "$SCRIPT_TEXT"
assert_contains 'roles/iap.tunnelResourceAccessor' "$SCRIPT_TEXT"
assert_contains 'roles/compute.osAdminLogin' "$SCRIPT_TEXT"
assert_contains 'roles/iam.serviceAccountTokenCreator' "$SCRIPT_TEXT"
assert_contains 'CLOUDSDK_AUTH_IMPERSONATE_SERVICE_ACCOUNT=' "$SCRIPT_TEXT"
assert_not_contains 'iam service-accounts keys create' "$SCRIPT_TEXT"
assert_not_contains 'compute.admin' "$SCRIPT_TEXT"
assert_not_contains 'roles/owner' "$SCRIPT_TEXT"

printf 'ok: recovery identity script keeps Nexus recovery scoped and keyless\n'
