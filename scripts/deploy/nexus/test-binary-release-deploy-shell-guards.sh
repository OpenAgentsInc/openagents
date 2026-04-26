#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMMON_SCRIPT="${SCRIPT_DIR}/common.sh"
UPLOAD_SCRIPT="${SCRIPT_DIR}/13-upload-binary-release.sh"
ACTIVATE_SCRIPT="${SCRIPT_DIR}/14-activate-binary-release.sh"

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
    printf 'unexpected content found: %s\n' "$needle" >&2
    exit 1
  fi
}

bash -n "$COMMON_SCRIPT" "$UPLOAD_SCRIPT" "$ACTIVATE_SCRIPT"

COMMON_TEXT="$(cat "$COMMON_SCRIPT")"
UPLOAD_TEXT="$(cat "$UPLOAD_SCRIPT")"
ACTIVATE_TEXT="$(cat "$ACTIVATE_SCRIPT")"

assert_contains 'run_with_timeout()' "$COMMON_TEXT"
assert_contains 'subprocess.TimeoutExpired' "$COMMON_TEXT"
assert_contains 'verify_nexus_public_edge_healthy()' "$COMMON_TEXT"
assert_contains 'error code: 1033' "$COMMON_TEXT"
assert_contains 'NEXUS_BINARY_RELEASE_UPLOAD_TIMEOUT_SECONDS' "$COMMON_TEXT"
assert_contains 'NEXUS_BINARY_RELEASE_ACTIVATION_TIMEOUT_SECONDS' "$COMMON_TEXT"

assert_contains 'verify_nexus_public_edge_healthy "pre-binary-upload"' "$UPLOAD_TEXT"
assert_contains 'verify_nexus_public_edge_healthy "pre-target-upload"' "$UPLOAD_TEXT"
assert_contains 'verify_nexus_public_edge_healthy "post-target-upload"' "$UPLOAD_TEXT"
assert_contains 'verify_nexus_public_edge_healthy "post-binary-upload"' "$UPLOAD_TEXT"
assert_contains 'cleanup_remote_release_archive' "$UPLOAD_TEXT"
assert_contains 'upload failed or timed out before remote install; activation was not attempted' "$UPLOAD_TEXT"
assert_contains 'run_with_timeout "$NEXUS_BINARY_RELEASE_UPLOAD_TIMEOUT_SECONDS" "upload-target-release-archive"' "$UPLOAD_TEXT"
assert_not_contains 'systemctl restart nexus-relay' "$UPLOAD_TEXT"

assert_contains 'verify_nexus_public_edge_healthy "pre-binary-activation"' "$ACTIVATE_TEXT"
assert_contains 'verify_nexus_public_edge_healthy "post-binary-activation"' "$ACTIVATE_TEXT"
assert_contains 'run_with_timeout "$NEXUS_BINARY_RELEASE_ACTIVATION_TIMEOUT_SECONDS" "activate-target-release"' "$ACTIVATE_TEXT"
assert_contains 'systemctl restart nexus-relay' "$ACTIVATE_TEXT"

printf 'ok: binary release upload/activation scripts have timeout, cleanup, and public-edge guards\n'
