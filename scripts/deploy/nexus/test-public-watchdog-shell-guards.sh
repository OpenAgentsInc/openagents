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
AUDIT_SCRIPT="${SCRIPT_DIR}/33-audit-public-watchdog.sh"
bash -n "$AUDIT_SCRIPT"
SCRIPT_TEXT="$(cat "$TARGET_SCRIPT")"
AUDIT_SCRIPT_TEXT="$(cat "$AUDIT_SCRIPT")"
CHECK_SCRIPT_TEXT="$(sed -n "/cat >\"\$TMP_CHECK_SCRIPT\" <<'CHECK'/,/^CHECK$/p" "$TARGET_SCRIPT")"

assert_contains 'public_edge_failure()' "$CHECK_SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_PUBLIC_HEALTH_URL' "$SCRIPT_TEXT"
assert_contains 'PUBLIC_HEALTH_URL="${NEXUS_PUBLIC_WATCHDOG_PUBLIC_HEALTH_URL:-https://nexus.openagents.com/healthz}"' "$CHECK_SCRIPT_TEXT"
assert_contains 'startup_public_health_probe' "$CHECK_SCRIPT_TEXT"
assert_contains 'startup_public_stats_probe' "$CHECK_SCRIPT_TEXT"
assert_contains 'public_edge_${startup_public_code}_during_startup_grace' "$CHECK_SCRIPT_TEXT"
assert_contains 'error code: 1033' "$CHECK_SCRIPT_TEXT"
assert_contains 'EVENT_LOG_PATH="${STATE_DIR}/events.jsonl"' "$CHECK_SCRIPT_TEXT"
assert_contains 'vm_reset_required' "$CHECK_SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_ENABLED' "$SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_AFTER_FAILURES' "$SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_EDGE_RECHECK_SECONDS' "$SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_RECOVERY_PROXY_ENABLED' "$SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_RECOVERY_PROXY_SERVICE_NAME' "$SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_RECOVERY_PROXY_ORIGIN_URL' "$SCRIPT_TEXT"
assert_contains 'EDGE_FAILURE_COUNT_PATH="${STATE_DIR}/edge-failure-count"' "$CHECK_SCRIPT_TEXT"
assert_contains 'recover_public_edge_failure()' "$CHECK_SCRIPT_TEXT"
assert_contains 'activate_recovery_proxy_for_public_edge_failure()' "$CHECK_SCRIPT_TEXT"
assert_contains 'restore_normal_origin_after_recovery()' "$CHECK_SCRIPT_TEXT"
assert_contains 'recovery_proxy_public_origin_active()' "$CHECK_SCRIPT_TEXT"
assert_contains 'effective_local_health_url="${RECOVERY_PROXY_ORIGIN_URL%/}/healthz"' "$CHECK_SCRIPT_TEXT"
assert_contains 'restore_normal_origin:${TUNNEL_SERVICE_NAME}' "$CHECK_SCRIPT_TEXT"
assert_contains 'public_edge_still_down_after_tunnel_restart' "$CHECK_SCRIPT_TEXT"
assert_contains 'public_edge_recovered_after_tunnel_restart' "$CHECK_SCRIPT_TEXT"
assert_contains 'systemctl reboot' "$CHECK_SCRIPT_TEXT"
assert_contains 'NEXUS_PUBLIC_WATCHDOG_DRY_RUN' "$CHECK_SCRIPT_TEXT"
assert_contains 'healthy startup_grace' "$CHECK_SCRIPT_TEXT"
assert_contains 'public_health=${startup_public_health_code} public_stats=${startup_public_stats_code}' "$CHECK_SCRIPT_TEXT"
assert_contains 'nexus-public-watchdog.timer' "$AUDIT_SCRIPT_TEXT"
assert_contains 'nexus-http-recovery-proxy.service' "$AUDIT_SCRIPT_TEXT"
assert_contains 'TUNNEL_ORIGIN_URL=' "$AUDIT_SCRIPT_TEXT"
assert_contains 'last-event.json' "$AUDIT_SCRIPT_TEXT"
assert_contains 'edge-failure-count' "$AUDIT_SCRIPT_TEXT"
assert_contains 'local-origin-health' "$AUDIT_SCRIPT_TEXT"
assert_contains 'recovery-proxy-health' "$AUDIT_SCRIPT_TEXT"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

CHECK_SCRIPT_PATH="${TMP_ROOT}/nexus-public-watchdog-check"
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

cat >"${MOCK_BIN}/systemctl" <<'MOCK'
#!/usr/bin/env bash
case "${1:-}" in
  is-active)
    printf 'active\n'
    ;;
  show)
    printf 'n/a\n'
    ;;
  restart)
    printf 'restart:%s\n' "${2:-}" >>"${WATCHDOG_ACTION_LOG}"
    ;;
  reboot)
    printf 'reboot\n' >>"${WATCHDOG_ACTION_LOG}"
    ;;
esac
exit 0
MOCK
chmod +x "${MOCK_BIN}/systemctl"

cat >"${MOCK_BIN}/curl" <<'MOCK'
#!/usr/bin/env bash
body_path=""
url=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o)
      body_path="$2"
      shift 2
      ;;
    --max-time|-w)
      shift 2
      ;;
    -sS)
      shift
      ;;
    *)
      url="$1"
      shift
      ;;
  esac
done
if [[ "$url" == http://127.0.0.1:8080/* || "$url" == http://127.0.0.1:8081/* ]]; then
  if [[ "${MOCK_LOCAL_FAIL:-false}" == "true" ]]; then
    if [[ -n "$body_path" ]]; then
      printf 'local origin unavailable\n' >"$body_path"
    fi
    printf '530'
    exit 0
  fi
  if [[ -n "$body_path" ]]; then
    printf '{}\n' >"$body_path"
  fi
  printf '200'
  exit 0
fi
if [[ "${MOCK_PUBLIC_OK:-false}" == "true" ]]; then
  if [[ -n "$body_path" ]]; then
    printf '{}\n' >"$body_path"
  fi
  printf '200'
  exit 0
fi
if [[ -n "$body_path" ]]; then
  printf 'error code: 1033\n' >"$body_path"
fi
printf '530'
MOCK
chmod +x "${MOCK_BIN}/curl"

WATCHDOG_ACTION_LOG="${TMP_ROOT}/actions.log"
STATE_DIR="${TMP_ROOT}/state"
mkdir -p "$STATE_DIR"

startup_output="$(
  PATH="${MOCK_BIN}:$PATH" \
  WATCHDOG_ACTION_LOG="$WATCHDOG_ACTION_LOG" \
  NEXUS_PUBLIC_WATCHDOG_STATE_DIR="$STATE_DIR" \
  NEXUS_PUBLIC_WATCHDOG_DRY_RUN=true \
  NEXUS_PUBLIC_WATCHDOG_STARTUP_GRACE_SECONDS=180 \
  "$CHECK_SCRIPT_PATH" || true
)"

assert_contains 'dry_run restarting service=nexus-cloudflared reason=public_edge_530_during_startup_grace' "$startup_output"
assert_contains 'dry_run activate_recovery_proxy service=nexus-http-recovery-proxy origin_url=http://127.0.0.1:8081 reason=public_edge_530_during_startup_grace_after_tunnel_restart' "$startup_output"
assert_contains '"action":"activate_recovery_proxy:nexus-http-recovery-proxy"' "$(cat "${STATE_DIR}/last-event.json")"
assert_contains '"consecutive_edge_failures":"2"' "$(cat "${STATE_DIR}/last-event.json")"
if grep -Fq '"status":"healthy"' "${STATE_DIR}/last-event.json"; then
  printf 'unexpected healthy startup-grace event during Cloudflare 1033\n' >&2
  exit 1
fi

printf '1\n' >"${STATE_DIR}/edge-failure-count"
reboot_output="$(
  PATH="${MOCK_BIN}:$PATH" \
  WATCHDOG_ACTION_LOG="$WATCHDOG_ACTION_LOG" \
  NEXUS_PUBLIC_WATCHDOG_STATE_DIR="$STATE_DIR" \
  NEXUS_PUBLIC_WATCHDOG_DRY_RUN=true \
  NEXUS_PUBLIC_WATCHDOG_RECOVERY_PROXY_ENABLED=false \
  NEXUS_PUBLIC_WATCHDOG_STARTUP_GRACE_SECONDS=180 \
  NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_AFTER_FAILURES=2 \
  "$CHECK_SCRIPT_PATH" || true
)"

assert_contains 'dry_run vm_reset reason=public_edge_530_during_startup_grace' "$reboot_output"
assert_contains '"action":"vm_reset"' "$(cat "${STATE_DIR}/last-event.json")"
assert_contains '"consecutive_edge_failures":"2"' "$(cat "${STATE_DIR}/last-event.json")"

LOCAL_FAIL_STATE_DIR="${TMP_ROOT}/local-fail-state"
mkdir -p "$LOCAL_FAIL_STATE_DIR"
printf '1\n' >"${LOCAL_FAIL_STATE_DIR}/edge-failure-count"
local_fail_output="$(
  PATH="${MOCK_BIN}:$PATH" \
  WATCHDOG_ACTION_LOG="$WATCHDOG_ACTION_LOG" \
  NEXUS_PUBLIC_WATCHDOG_STATE_DIR="$LOCAL_FAIL_STATE_DIR" \
  NEXUS_PUBLIC_WATCHDOG_DRY_RUN=true \
  NEXUS_PUBLIC_WATCHDOG_STARTUP_GRACE_SECONDS=180 \
  NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_AFTER_FAILURES=2 \
  MOCK_LOCAL_FAIL=true \
  "$CHECK_SCRIPT_PATH" || true
)"

assert_contains 'dry_run vm_reset reason=public_edge_530_during_startup_grace' "$local_fail_output"
assert_contains '"action":"vm_reset"' "$(cat "${LOCAL_FAIL_STATE_DIR}/last-event.json")"
if grep -Fq '"action":"activate_recovery_proxy:nexus-http-recovery-proxy"' "${LOCAL_FAIL_STATE_DIR}/last-event.json"; then
  printf 'unexpected recovery-proxy activation for unhealthy local origin\n' >&2
  exit 1
fi

RESTORE_STATE_DIR="${TMP_ROOT}/restore-state"
RESTORE_ENV_PATH="${TMP_ROOT}/cloudflared.env"
mkdir -p "$RESTORE_STATE_DIR"
printf 'TUNNEL_ORIGIN_URL=http://127.0.0.1:8081\n' >"$RESTORE_ENV_PATH"
restore_output="$(
  PATH="${MOCK_BIN}:$PATH" \
  WATCHDOG_ACTION_LOG="$WATCHDOG_ACTION_LOG" \
  NEXUS_PUBLIC_WATCHDOG_STATE_DIR="$RESTORE_STATE_DIR" \
  NEXUS_PUBLIC_WATCHDOG_CLOUDFLARED_ENV_PATH="$RESTORE_ENV_PATH" \
  NEXUS_PUBLIC_WATCHDOG_DRY_RUN=true \
  NEXUS_PUBLIC_WATCHDOG_STARTUP_GRACE_SECONDS=0 \
  MOCK_PUBLIC_OK=true \
  "$CHECK_SCRIPT_PATH" || true
)"

assert_contains 'dry_run restore_normal_origin service=nexus-cloudflared origin_url=http://127.0.0.1:8080 reason=recovery_proxy_exit' "$restore_output"
assert_contains '"action":"restore_normal_origin:nexus-cloudflared"' "$(cat "${RESTORE_STATE_DIR}/last-event.json")"

printf 'ok: public watchdog treats Cloudflare 1033 as recoverable, reboots dead origins, and exits recovery proxy\n'
