#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if [[ "${NEXUS_TREASURY_WATCHDOG_ENABLED}" != "true" ]]; then
  log "Treasury watchdog disabled; skipping install"
  exit 0
fi

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

TMP_ENV="$(mktemp)"
TMP_CHECK_SCRIPT="$(mktemp)"
TMP_REMOTE_SCRIPT="$(mktemp)"
trap 'rm -f "$TMP_ENV" "$TMP_CHECK_SCRIPT" "$TMP_REMOTE_SCRIPT"' EXIT

cat >"$TMP_ENV" <<ENV
# Managed by scripts/deploy/nexus/10-install-treasury-watchdog.sh
NEXUS_TREASURY_WATCHDOG_INTERVAL_SECONDS=${NEXUS_TREASURY_WATCHDOG_INTERVAL_SECONDS}
NEXUS_TREASURY_WATCHDOG_MAX_IDLE_SECONDS=${NEXUS_TREASURY_WATCHDOG_MAX_IDLE_SECONDS}
NEXUS_TREASURY_WATCHDOG_MAX_CONFIRM_LAG_SECONDS=${NEXUS_TREASURY_WATCHDOG_MAX_CONFIRM_LAG_SECONDS}
NEXUS_TREASURY_WATCHDOG_MAX_RESTARTS_PER_HOUR=${NEXUS_TREASURY_WATCHDOG_MAX_RESTARTS_PER_HOUR}
NEXUS_TREASURY_WATCHDOG_STARTUP_GRACE_SECONDS=${NEXUS_TREASURY_WATCHDOG_STARTUP_GRACE_SECONDS}
NEXUS_TREASURY_WATCHDOG_LOCAL_STATUS_URL=${NEXUS_TREASURY_WATCHDOG_LOCAL_STATUS_URL}
NEXUS_TREASURY_WATCHDOG_SERVICE_NAME=${NEXUS_TREASURY_WATCHDOG_SERVICE_NAME}
NEXUS_TREASURY_WATCHDOG_RESTART_MODE=${NEXUS_TREASURY_WATCHDOG_RESTART_MODE}
ENV

cat >"$TMP_CHECK_SCRIPT" <<'CHECK'
#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="${NEXUS_TREASURY_WATCHDOG_SERVICE_NAME:-nexus-relay}"
STATUS_URL="${NEXUS_TREASURY_WATCHDOG_LOCAL_STATUS_URL:-http://127.0.0.1:8080/v1/treasury/status}"
MAX_IDLE_SECONDS="${NEXUS_TREASURY_WATCHDOG_MAX_IDLE_SECONDS:-300}"
MAX_CONFIRM_LAG_SECONDS="${NEXUS_TREASURY_WATCHDOG_MAX_CONFIRM_LAG_SECONDS:-300}"
MAX_RESTARTS_PER_HOUR="${NEXUS_TREASURY_WATCHDOG_MAX_RESTARTS_PER_HOUR:-12}"
STARTUP_GRACE_SECONDS="${NEXUS_TREASURY_WATCHDOG_STARTUP_GRACE_SECONDS:-180}"
RESTART_MODE="${NEXUS_TREASURY_WATCHDOG_RESTART_MODE:-service_inactive_only}"
STATE_DIR="/var/lib/nexus-relay/watchdog"
RESTART_LOG="${STATE_DIR}/restart-timestamps.log"
MAX_CONFIRM_LAG_MS=$((MAX_CONFIRM_LAG_SECONDS * 1000))
NOW_UNIX_S="$(date +%s)"
NOW_UNIX_MS="$((NOW_UNIX_S * 1000))"

mkdir -p "$STATE_DIR"
touch "$RESTART_LOG"

log() {
  local message="$1"
  echo "$message"
  logger -t nexus-treasury-watchdog "$message"
}

recent_completed_count() {
  local since_epoch="$1"
  local journal_output
  if ! journal_output="$(journalctl -u "$SERVICE_NAME" --since "@${since_epoch}" --no-pager 2>/dev/null)"; then
    echo 0
    return
  fi
  printf '%s\n' "$journal_output" | grep -Ec 'Inserted payment: Payment \{.*payment_type: Send, status: Completed, amount:' || true
}

trim_restart_log() {
  local cutoff="$1"
  local tmp
  tmp="$(mktemp)"
  awk -v cutoff="$cutoff" '$1 >= cutoff { print $1 }' "$RESTART_LOG" >"$tmp"
  mv "$tmp" "$RESTART_LOG"
}

service_active_enter_unix_s() {
  local active_enter_timestamp
  active_enter_timestamp="$(systemctl show -p ActiveEnterTimestamp --value "$SERVICE_NAME" 2>/dev/null || true)"
  if [[ -z "$active_enter_timestamp" || "$active_enter_timestamp" == "n/a" ]]; then
    echo 0
    return
  fi
  date -d "$active_enter_timestamp" +%s 2>/dev/null || echo 0
}

restart_allowed_for_treasury_faults() {
  [[ "$RESTART_MODE" == "aggressive" ]]
}

restart_service() {
  local reason="$1"
  trim_restart_log "$((NOW_UNIX_S - 3600))"
  local recent_restarts
  recent_restarts="$(wc -l <"$RESTART_LOG" | tr -d ' ')"
  if (( recent_restarts >= MAX_RESTARTS_PER_HOUR )); then
    log "restart_suppressed reason=${reason} recent_restarts=${recent_restarts} max_restarts_per_hour=${MAX_RESTARTS_PER_HOUR}"
    exit 1
  fi
  echo "$NOW_UNIX_S" >>"$RESTART_LOG"
  log "restarting ${SERVICE_NAME} reason=${reason} recent_restarts=$((recent_restarts + 1))"
  systemctl restart "$SERVICE_NAME"
}

handle_treasury_fault() {
  local reason="$1"
  if restart_allowed_for_treasury_faults; then
    restart_service "$reason"
    return
  fi
  log "degraded ${SERVICE_NAME} reason=${reason} action=log_only restart_mode=${RESTART_MODE}"
}

if [[ "$(systemctl is-active "$SERVICE_NAME" 2>/dev/null || true)" != "active" ]]; then
  restart_service "service_inactive"
  exit 0
fi

SERVICE_ACTIVE_ENTER_UNIX_S="$(service_active_enter_unix_s)"
SERVICE_UPTIME_SECONDS=$(( SERVICE_ACTIVE_ENTER_UNIX_S > 0 ? NOW_UNIX_S - SERVICE_ACTIVE_ENTER_UNIX_S : STARTUP_GRACE_SECONDS + 1 ))
if (( SERVICE_UPTIME_SECONDS < STARTUP_GRACE_SECONDS )); then
  log "healthy startup_grace service_uptime_seconds=${SERVICE_UPTIME_SECONDS} startup_grace_seconds=${STARTUP_GRACE_SECONDS}"
  exit 0
fi

RECENT_COMPLETED="$(recent_completed_count "$((NOW_UNIX_S - MAX_IDLE_SECONDS))")"
if (( RECENT_COMPLETED > 0 )); then
  log "healthy recent_completed=${RECENT_COMPLETED} window_seconds=${MAX_IDLE_SECONDS}"
  exit 0
fi

STATUS_JSON="$(curl -fsS --max-time 15 "$STATUS_URL" 2>/dev/null || true)"
if [[ -z "$STATUS_JSON" ]]; then
  handle_treasury_fault "status_unreachable"
  exit 0
fi

wallet_runtime_status="$(jq -r '.wallet_runtime_status // empty' <<<"$STATUS_JSON")"
payout_loop_runtime_status="$(jq -r '.payout_loop_runtime_status // empty' <<<"$STATUS_JSON")"
payout_loop_health="$(jq -r '.payout_loop_health // empty' <<<"$STATUS_JSON")"
degraded_reason="$(jq -r '.degraded_reason // empty' <<<"$STATUS_JSON")"
sellable_pylons_online_now="$(jq -r '.sellable_pylons_online_now // .eligible_online_payout_targets // 0' <<<"$STATUS_JSON")"
last_confirmed_payout_at_unix_ms="$(jq -r '.last_confirmed_payout_at_unix_ms // 0' <<<"$STATUS_JSON")"
last_dispatch_at_unix_ms="$(jq -r '.last_dispatch_at_unix_ms // 0' <<<"$STATUS_JSON")"

if [[ "$wallet_runtime_status" == "error" ]]; then
  handle_treasury_fault "wallet_runtime_error"
  exit 0
fi

if [[ "$payout_loop_runtime_status" == "error" || "$payout_loop_runtime_status" == "degraded" ]]; then
  handle_treasury_fault "payout_loop_runtime_${payout_loop_runtime_status}"
  exit 0
fi

if (( sellable_pylons_online_now == 0 )); then
  log "healthy idle_with_no_sellable_targets payout_loop_health=${payout_loop_health:-unknown}"
  exit 0
fi

confirm_lag_ms=$(( last_confirmed_payout_at_unix_ms > 0 ? NOW_UNIX_MS - last_confirmed_payout_at_unix_ms : MAX_CONFIRM_LAG_MS + 1 ))
dispatch_lag_ms=$(( last_dispatch_at_unix_ms > 0 ? NOW_UNIX_MS - last_dispatch_at_unix_ms : MAX_CONFIRM_LAG_MS + 1 ))

if (( dispatch_lag_ms > MAX_CONFIRM_LAG_MS && confirm_lag_ms > MAX_CONFIRM_LAG_MS )); then
  handle_treasury_fault "payouts_idle sellable=${sellable_pylons_online_now} confirm_lag_ms=${confirm_lag_ms} dispatch_lag_ms=${dispatch_lag_ms} degraded_reason=${degraded_reason:-none}"
  exit 0
fi

log "healthy idle_without_restart sellable=${sellable_pylons_online_now} confirm_lag_ms=${confirm_lag_ms} dispatch_lag_ms=${dispatch_lag_ms} degraded_reason=${degraded_reason:-none}"
CHECK

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

ENV_SOURCE_PATH="$1"
CHECK_SOURCE_PATH="$2"
WATCHDOG_INTERVAL_SECONDS="$3"

sudo mkdir -p /etc/nexus-relay
sudo mv "$ENV_SOURCE_PATH" /etc/nexus-relay/treasury-watchdog.env
sudo chmod 640 /etc/nexus-relay/treasury-watchdog.env
sudo chown root:root /etc/nexus-relay/treasury-watchdog.env

sudo mv "$CHECK_SOURCE_PATH" /usr/local/bin/nexus-treasury-watchdog-check
sudo chmod 755 /usr/local/bin/nexus-treasury-watchdog-check
sudo chown root:root /usr/local/bin/nexus-treasury-watchdog-check

sudo tee /etc/systemd/system/nexus-treasury-watchdog.service >/dev/null <<'UNIT'
[Unit]
Description=OpenAgents Nexus treasury continuity watchdog
After=nexus-relay.service
Wants=nexus-relay.service

[Service]
Type=oneshot
EnvironmentFile=-/etc/nexus-relay/treasury-watchdog.env
ExecStart=/usr/local/bin/nexus-treasury-watchdog-check
UNIT

sudo tee /etc/systemd/system/nexus-treasury-watchdog.timer >/dev/null <<UNIT
[Unit]
Description=Run OpenAgents Nexus treasury continuity watchdog periodically

[Timer]
OnBootSec=2min
OnUnitActiveSec=${WATCHDOG_INTERVAL_SECONDS}s
AccuracySec=30s
Persistent=true
Unit=nexus-treasury-watchdog.service

[Install]
WantedBy=timers.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable --now nexus-treasury-watchdog.timer
sudo systemctl reset-failed nexus-treasury-watchdog.service || true
sudo systemctl --no-pager --full status nexus-treasury-watchdog.service | sed -n '1,40p' || true
sudo systemctl --no-pager --full status nexus-treasury-watchdog.timer | sed -n '1,40p' || true
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_ENV" "${NEXUS_VM}:/tmp/nexus-treasury-watchdog.env"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_CHECK_SCRIPT" "${NEXUS_VM}:/tmp/nexus-treasury-watchdog-check"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-install-treasury-watchdog.sh"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/nexus-install-treasury-watchdog.sh && /tmp/nexus-install-treasury-watchdog.sh '/tmp/nexus-treasury-watchdog.env' '/tmp/nexus-treasury-watchdog-check' '${NEXUS_TREASURY_WATCHDOG_INTERVAL_SECONDS}'"

log "Treasury watchdog installed on ${NEXUS_VM}"
