#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_SCRIPT="${SCRIPT_DIR}/16-install-public-watchdog.sh"

assert_contains() {
  local needle="$1"
  local haystack="$2"
  if ! grep -Fq "$needle" <<<"$haystack"; then
    printf 'missing expected content: %s\n' "$needle" >&2
    exit 1
  fi
}

bash -n "$TARGET_SCRIPT"
SCRIPT_TEXT="$(cat "$TARGET_SCRIPT")"
CHECK_SCRIPT_TEXT="$(sed -n "/cat >\"\$TMP_CHECK_SCRIPT\" <<'CHECK'/,/^CHECK$/p" "$TARGET_SCRIPT")"

assert_contains 'public_edge_failure()' "$CHECK_SCRIPT_TEXT"
assert_contains 'startup_public_probe' "$CHECK_SCRIPT_TEXT"
assert_contains 'public_edge_${startup_public_code}_during_startup_grace' "$CHECK_SCRIPT_TEXT"
assert_contains 'error code: 1033' "$CHECK_SCRIPT_TEXT"
assert_contains 'EVENT_LOG_PATH="${STATE_DIR}/events.jsonl"' "$CHECK_SCRIPT_TEXT"
assert_contains 'vm_reset_required' "$CHECK_SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_DRY_RUN' "$CHECK_SCRIPT_TEXT"
assert_contains 'healthy startup_grace' "$CHECK_SCRIPT_TEXT"
assert_contains 'public_stats=${startup_public_code}' "$CHECK_SCRIPT_TEXT"

printf 'ok: public watchdog treats Cloudflare 1033 during startup grace as recovery, not healthy\n'
