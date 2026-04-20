#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_SCRIPT="${SCRIPT_DIR}/09-recover-treasury-wallet.sh"

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
    printf 'forbidden content found: %s\n' "$needle" >&2
    exit 1
  fi
}

line_number() {
  local needle="$1"
  local line
  line="$(grep -nF "$needle" "$TARGET_SCRIPT" | head -n1 | cut -d: -f1 || true)"
  if [[ -z "$line" ]]; then
    printf 'missing expected content: %s\n' "$needle" >&2
    exit 1
  fi
  printf '%s\n' "$line"
}

assert_before() {
  local first="$1"
  local second="$2"
  local first_line
  local second_line
  first_line="$(line_number "$first")"
  second_line="$(line_number "$second")"
  if (( first_line >= second_line )); then
    printf 'expected "%s" before "%s"\n' "$first" "$second" >&2
    exit 1
  fi
}

SCRIPT_TEXT="$(cat "$TARGET_SCRIPT")"

assert_contains 'REPORT_STDOUT_PATH=' "$SCRIPT_TEXT"
assert_contains '[[ \"\${BASH_SUBSHELL:-0}\" == \"0\" ]] || return 0' "$SCRIPT_TEXT"
assert_contains 'run_nexus_control treasury recovery-report' "$SCRIPT_TEXT"
assert_contains '>\"\$REPORT_STDOUT_PATH\"' "$SCRIPT_TEXT"
assert_contains 'cat \"\$REPORT_STDOUT_PATH\"' "$SCRIPT_TEXT"
assert_contains '\"\$REPORT_STDOUT_PATH\" >/dev/null' "$SCRIPT_TEXT"

assert_not_contains 'REPORT_JSON=\$(' "$SCRIPT_TEXT"
assert_not_contains 'REPORT_JSON="$(' "$SCRIPT_TEXT"
assert_not_contains '<<<\"\$REPORT_JSON\"' "$SCRIPT_TEXT"

assert_before 'sudo docker pull' "trap 'cleanup_relay_service' EXIT"
assert_before "trap 'cleanup_relay_service' EXIT" 'sudo systemctl mask --runtime nexus-relay'
assert_before 'sudo systemctl mask --runtime nexus-relay' 'run_nexus_control treasury recovery-report'

printf 'ok: nexus recovery wrapper avoids cleanup trap subshell capture\n'
