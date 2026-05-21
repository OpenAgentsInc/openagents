#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_SCRIPT="${SCRIPT_DIR}/32-install-guest-network-watchdog.sh"

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

assert_contains 'NEXUS_GUEST_NETWORK_WATCHDOG_METADATA_URL' "$SCRIPT_TEXT"
assert_contains 'NEXUS_GUEST_NETWORK_WATCHDOG_FAILURE_THRESHOLD' "$SCRIPT_TEXT"
assert_contains 'guest_network_wedged' "$CHECK_SCRIPT_TEXT"
assert_contains 'systemctl reboot' "$CHECK_SCRIPT_TEXT"
assert_contains 'Metadata-Flavor: Google' "$CHECK_SCRIPT_TEXT"
assert_contains 'journalctl -u nexus-cloudflared' "$CHECK_SCRIPT_TEXT"
assert_contains 'network is unreachable' "$CHECK_SCRIPT_TEXT"
assert_contains 'nexus-guest-network-watchdog.timer' "$SCRIPT_TEXT"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

CHECK_SCRIPT_PATH="${TMP_ROOT}/nexus-guest-network-watchdog-check"
sed -n "/cat >\"\$TMP_CHECK_SCRIPT\" <<'CHECK'/,/^CHECK$/p" "$TARGET_SCRIPT" \
  | sed '1d;$d' >"$CHECK_SCRIPT_PATH"
chmod +x "$CHECK_SCRIPT_PATH"

MOCK_BIN="${TMP_ROOT}/bin"
mkdir -p "$MOCK_BIN"

cat >"${MOCK_BIN}/date" <<'MOCK'
#!/usr/bin/env bash
if [[ "${1:-}" == "+%s" ]]; then
  printf '1000\n'
  exit 0
fi
printf 'n/a\n'
MOCK
chmod +x "${MOCK_BIN}/date"

cat >"${MOCK_BIN}/logger" <<'MOCK'
#!/usr/bin/env bash
exit 0
MOCK
chmod +x "${MOCK_BIN}/logger"

cat >"${MOCK_BIN}/curl" <<'MOCK'
#!/usr/bin/env bash
exit "${MOCK_CURL_EXIT:-0}"
MOCK
chmod +x "${MOCK_BIN}/curl"

cat >"${MOCK_BIN}/python3" <<'MOCK'
#!/usr/bin/env bash
if [[ "${1:-}" == "-c" ]]; then
  /usr/bin/python3 "$@"
  exit $?
fi
exit "${MOCK_PYTHON_EXIT:-0}"
MOCK
chmod +x "${MOCK_BIN}/python3"

cat >"${MOCK_BIN}/ip" <<'MOCK'
#!/usr/bin/env bash
exit "${MOCK_IP_EXIT:-0}"
MOCK
chmod +x "${MOCK_BIN}/ip"

cat >"${MOCK_BIN}/journalctl" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "${MOCK_JOURNAL_OUTPUT:-}"
MOCK
chmod +x "${MOCK_BIN}/journalctl"

cat >"${MOCK_BIN}/systemctl" <<'MOCK'
#!/usr/bin/env bash
if [[ "${1:-}" == "reboot" ]]; then
  printf 'reboot\n' >>"${WATCHDOG_ACTION_LOG}"
fi
exit 0
MOCK
chmod +x "${MOCK_BIN}/systemctl"

WATCHDOG_ACTION_LOG="${TMP_ROOT}/actions.log"
STATE_DIR="${TMP_ROOT}/state"
mkdir -p "$STATE_DIR"

for _ in 1 2 3; do
  output="$(
    PATH="${MOCK_BIN}:$PATH" \
    WATCHDOG_ACTION_LOG="$WATCHDOG_ACTION_LOG" \
    NEXUS_GUEST_NETWORK_WATCHDOG_STATE_DIR="$STATE_DIR" \
    NEXUS_GUEST_NETWORK_WATCHDOG_DRY_RUN=true \
    NEXUS_GUEST_NETWORK_WATCHDOG_FAILURE_THRESHOLD=3 \
    MOCK_CURL_EXIT=1 \
    MOCK_PYTHON_EXIT=1 \
    MOCK_IP_EXIT=1 \
    MOCK_JOURNAL_OUTPUT='sendmsg: network is unreachable' \
    "$CHECK_SCRIPT_PATH" || true
  )"
done

assert_contains 'dry_run reboot reason=guest_network_wedged' "$output"
assert_contains '"action":"vm_reboot"' "$(cat "${STATE_DIR}/last-event.json")"
assert_contains '"consecutive_failures":"3"' "$(cat "${STATE_DIR}/last-event.json")"

healthy_output="$(
  PATH="${MOCK_BIN}:$PATH" \
  WATCHDOG_ACTION_LOG="$WATCHDOG_ACTION_LOG" \
  NEXUS_GUEST_NETWORK_WATCHDOG_STATE_DIR="$STATE_DIR" \
  MOCK_CURL_EXIT=0 \
  MOCK_PYTHON_EXIT=0 \
  MOCK_IP_EXIT=0 \
  MOCK_JOURNAL_OUTPUT='' \
  "$CHECK_SCRIPT_PATH"
)"

assert_contains 'healthy metadata=ok dns=ok route=ok' "$healthy_output"
assert_contains '"status":"healthy"' "$(cat "${STATE_DIR}/last-event.json")"

printf 'ok: guest network watchdog reboots only after repeated metadata + network failure\n'
