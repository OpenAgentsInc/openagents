#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if [[ "${NEXUS_PUBLIC_WATCHDOG_ENABLED}" != "true" ]]; then
  log "Public watchdog disabled; skipping install"
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
# Managed by scripts/deploy/nexus/16-install-public-watchdog.sh
NEXUS_PUBLIC_WATCHDOG_INTERVAL_SECONDS=${NEXUS_PUBLIC_WATCHDOG_INTERVAL_SECONDS}
NEXUS_PUBLIC_WATCHDOG_STARTUP_GRACE_SECONDS=${NEXUS_PUBLIC_WATCHDOG_STARTUP_GRACE_SECONDS}
NEXUS_PUBLIC_WATCHDOG_MAX_RESTARTS_PER_HOUR=${NEXUS_PUBLIC_WATCHDOG_MAX_RESTARTS_PER_HOUR}
NEXUS_PUBLIC_WATCHDOG_LOCAL_HEALTH_URL=${NEXUS_PUBLIC_WATCHDOG_LOCAL_HEALTH_URL}
NEXUS_PUBLIC_WATCHDOG_PUBLIC_HEALTH_URL=${NEXUS_PUBLIC_WATCHDOG_PUBLIC_HEALTH_URL}
NEXUS_PUBLIC_WATCHDOG_PUBLIC_STATS_URL=${NEXUS_PUBLIC_WATCHDOG_PUBLIC_STATS_URL}
NEXUS_PUBLIC_WATCHDOG_RELAY_SERVICE_NAME=${NEXUS_PUBLIC_WATCHDOG_RELAY_SERVICE_NAME}
NEXUS_PUBLIC_WATCHDOG_TUNNEL_SERVICE_NAME=${NEXUS_PUBLIC_WATCHDOG_TUNNEL_SERVICE_NAME}
NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_ENABLED=${NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_ENABLED}
NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_AFTER_FAILURES=${NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_AFTER_FAILURES}
ENV

cat >"$TMP_CHECK_SCRIPT" <<'CHECK'
#!/usr/bin/env bash
set -euo pipefail

RELAY_SERVICE_NAME="${NEXUS_PUBLIC_WATCHDOG_RELAY_SERVICE_NAME:-nexus-relay}"
TUNNEL_SERVICE_NAME="${NEXUS_PUBLIC_WATCHDOG_TUNNEL_SERVICE_NAME:-nexus-cloudflared}"
LOCAL_HEALTH_URL="${NEXUS_PUBLIC_WATCHDOG_LOCAL_HEALTH_URL:-http://127.0.0.1:8080/healthz}"
PUBLIC_HEALTH_URL="${NEXUS_PUBLIC_WATCHDOG_PUBLIC_HEALTH_URL:-https://nexus.openagents.com/healthz}"
PUBLIC_STATS_URL="${NEXUS_PUBLIC_WATCHDOG_PUBLIC_STATS_URL:-https://nexus.openagents.com/api/stats}"
STARTUP_GRACE_SECONDS="${NEXUS_PUBLIC_WATCHDOG_STARTUP_GRACE_SECONDS:-180}"
MAX_RESTARTS_PER_HOUR="${NEXUS_PUBLIC_WATCHDOG_MAX_RESTARTS_PER_HOUR:-12}"
DRY_RUN="${NEXUS_PUBLIC_WATCHDOG_DRY_RUN:-false}"
EDGE_REBOOT_ENABLED="${NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_ENABLED:-true}"
EDGE_REBOOT_AFTER_FAILURES="${NEXUS_PUBLIC_WATCHDOG_EDGE_REBOOT_AFTER_FAILURES:-2}"
STATE_DIR="${NEXUS_PUBLIC_WATCHDOG_STATE_DIR:-/var/lib/nexus-relay/watchdog/public}"
EVENT_LOG_PATH="${STATE_DIR}/events.jsonl"
LAST_EVENT_PATH="${STATE_DIR}/last-event.json"
EDGE_FAILURE_COUNT_PATH="${STATE_DIR}/edge-failure-count"
NOW_UNIX_S="$(date +%s)"

mkdir -p "$STATE_DIR"

log() {
  local message="$1"
  echo "$message"
  logger -t nexus-public-watchdog "$message"
}

json_escape() {
  python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$1"
}

emit_event() {
  local status="$1"
  local reason="$2"
  local action="$3"
  local local_code="${4:-unknown}"
  local public_code="${5:-unknown}"
  local relay_uptime="${6:-unknown}"
  local tunnel_uptime="${7:-unknown}"
  local edge_failure_count="${8:-0}"
  local event
  event="{\"recorded_at_unix_s\":${NOW_UNIX_S},\"status\":$(json_escape "$status"),\"reason\":$(json_escape "$reason"),\"action\":$(json_escape "$action"),\"local_health_code\":$(json_escape "$local_code"),\"public_edge_code\":$(json_escape "$public_code"),\"relay_uptime_seconds\":$(json_escape "$relay_uptime"),\"tunnel_uptime_seconds\":$(json_escape "$tunnel_uptime"),\"consecutive_edge_failures\":$(json_escape "$edge_failure_count")}"
  printf '%s\n' "$event" >>"$EVENT_LOG_PATH"
  printf '%s\n' "$event" >"$LAST_EVENT_PATH"
}

reset_edge_failure_count() {
  printf '0\n' >"$EDGE_FAILURE_COUNT_PATH"
}

record_edge_failure_count() {
  local prior="0"
  if [[ -f "$EDGE_FAILURE_COUNT_PATH" ]]; then
    prior="$(tr -cd '0-9' <"$EDGE_FAILURE_COUNT_PATH" || true)"
  fi
  if [[ -z "$prior" ]]; then
    prior="0"
  fi
  local next=$((prior + 1))
  printf '%s\n' "$next" >"$EDGE_FAILURE_COUNT_PATH"
  printf '%s\n' "$next"
}

trim_restart_log() {
  local log_path="$1"
  local cutoff="$2"
  local tmp
  tmp="$(mktemp)"
  awk -v cutoff="$cutoff" '$1 >= cutoff { print $1 }' "$log_path" >"$tmp"
  mv "$tmp" "$log_path"
}

restart_service_with_limit() {
  local service_name="$1"
  local reason="$2"
  local local_code="${3:-unknown}"
  local public_code="${4:-unknown}"
  local relay_uptime="${5:-unknown}"
  local tunnel_uptime="${6:-unknown}"
  local edge_failure_count="${7:-0}"
  local log_path="${STATE_DIR}/${service_name}.restart.log"

  touch "$log_path"
  trim_restart_log "$log_path" "$((NOW_UNIX_S - 3600))"
  local recent_restarts
  recent_restarts="$(wc -l <"$log_path" | tr -d ' ')"
  if (( recent_restarts >= MAX_RESTARTS_PER_HOUR )); then
    emit_event "escalation_required" "$reason" "vm_reset_required" "$local_code" "$public_code" "$relay_uptime" "$tunnel_uptime" "$edge_failure_count"
    log "restart_suppressed service=${service_name} reason=${reason} recent_restarts=${recent_restarts} max_restarts_per_hour=${MAX_RESTARTS_PER_HOUR}"
    exit 1
  fi

  echo "$NOW_UNIX_S" >>"$log_path"
  emit_event "recovering" "$reason" "restart:${service_name}" "$local_code" "$public_code" "$relay_uptime" "$tunnel_uptime" "$edge_failure_count"
  if [[ "$DRY_RUN" == "true" ]]; then
    log "dry_run restarting service=${service_name} reason=${reason} recent_restarts=$((recent_restarts + 1))"
    return
  fi
  log "restarting service=${service_name} reason=${reason} recent_restarts=$((recent_restarts + 1))"
  systemctl restart "$service_name"
}

reboot_vm_for_public_edge_failure() {
  local reason="$1"
  local local_code="$2"
  local public_code="$3"
  local relay_uptime="$4"
  local tunnel_uptime="$5"
  local edge_failure_count="$6"

  emit_event "escalation_required" "$reason" "vm_reset" "$local_code" "$public_code" "$relay_uptime" "$tunnel_uptime" "$edge_failure_count"
  if [[ "$DRY_RUN" == "true" ]]; then
    log "dry_run vm_reset reason=${reason} consecutive_edge_failures=${edge_failure_count} threshold=${EDGE_REBOOT_AFTER_FAILURES}"
    return
  fi
  log "vm_reset reason=${reason} consecutive_edge_failures=${edge_failure_count} threshold=${EDGE_REBOOT_AFTER_FAILURES}"
  systemctl reboot
}

recover_public_edge_failure() {
  local reason="$1"
  local local_code="$2"
  local public_code="$3"
  local relay_uptime="$4"
  local tunnel_uptime="$5"
  local edge_failure_count
  edge_failure_count="$(record_edge_failure_count)"

  if [[ "$EDGE_REBOOT_ENABLED" == "true" ]] && (( edge_failure_count >= EDGE_REBOOT_AFTER_FAILURES )); then
    reboot_vm_for_public_edge_failure "$reason" "$local_code" "$public_code" "$relay_uptime" "$tunnel_uptime" "$edge_failure_count"
    exit 1
  fi

  restart_service_with_limit "$TUNNEL_SERVICE_NAME" "$reason" "$local_code" "$public_code" "$relay_uptime" "$tunnel_uptime" "$edge_failure_count"
}

service_uptime_seconds() {
  local service_name="$1"
  local timestamp
  timestamp="$(systemctl show -p ActiveEnterTimestamp --value "$service_name" 2>/dev/null || true)"
  if [[ -z "$timestamp" || "$timestamp" == "n/a" ]]; then
    echo 0
    return
  fi
  local entered
  entered="$(date -d "$timestamp" +%s 2>/dev/null || echo 0)"
  if (( entered <= 0 )); then
    echo 0
    return
  fi
  echo $((NOW_UNIX_S - entered))
}

http_probe() {
  local url="$1"
  local body_path
  body_path="$(mktemp)"
  local http_code
  http_code="$(curl -sS --max-time 20 -o "$body_path" -w '%{http_code}' "$url" || true)"
  local body
  body="$(cat "$body_path" 2>/dev/null || true)"
  rm -f "$body_path"
  printf '%s\n%s\n' "${http_code:-000}" "$body"
}

public_edge_failure() {
  local public_code="$1"
  local public_body="${2:-}"
  [[ "$public_code" == "530" || "$public_code" == "000" || "$public_body" == *"error code: 1033"* || "$public_body" == *"Error 1033"* ]]
}

public_code_for_event() {
  local public_health_code="$1"
  local public_stats_code="$2"
  if [[ "$public_health_code" != "200" ]]; then
    printf '%s\n' "$public_health_code"
  else
    printf '%s\n' "$public_stats_code"
  fi
}

probe_http_code() {
  sed -n '1p'
}

probe_http_body() {
  sed '1d'
}

if [[ "$(systemctl is-active "$RELAY_SERVICE_NAME" 2>/dev/null || true)" != "active" ]]; then
  restart_service_with_limit "$RELAY_SERVICE_NAME" "relay_inactive"
  exit 0
fi

if [[ "$(systemctl is-active "$TUNNEL_SERVICE_NAME" 2>/dev/null || true)" != "active" ]]; then
  restart_service_with_limit "$TUNNEL_SERVICE_NAME" "tunnel_inactive"
  exit 0
fi

relay_uptime_seconds="$(service_uptime_seconds "$RELAY_SERVICE_NAME")"
tunnel_uptime_seconds="$(service_uptime_seconds "$TUNNEL_SERVICE_NAME")"
if (( relay_uptime_seconds < STARTUP_GRACE_SECONDS || tunnel_uptime_seconds < STARTUP_GRACE_SECONDS )); then
  startup_public_health_probe="$(http_probe "$PUBLIC_HEALTH_URL")"
  startup_public_health_code="$(printf '%s\n' "$startup_public_health_probe" | probe_http_code)"
  startup_public_health_body="$(printf '%s\n' "$startup_public_health_probe" | probe_http_body)"
  startup_public_stats_probe="$(http_probe "$PUBLIC_STATS_URL")"
  startup_public_stats_code="$(printf '%s\n' "$startup_public_stats_probe" | probe_http_code)"
  startup_public_stats_body="$(printf '%s\n' "$startup_public_stats_probe" | probe_http_body)"
  startup_public_code="$(public_code_for_event "$startup_public_health_code" "$startup_public_stats_code")"
  if public_edge_failure "$startup_public_health_code" "$startup_public_health_body" || public_edge_failure "$startup_public_stats_code" "$startup_public_stats_body"; then
    recover_public_edge_failure "public_edge_${startup_public_code}_during_startup_grace" "skipped" "$startup_public_code" "$relay_uptime_seconds" "$tunnel_uptime_seconds"
    exit 0
  fi
  reset_edge_failure_count
  emit_event "healthy" "startup_grace" "none" "skipped" "$startup_public_code" "$relay_uptime_seconds" "$tunnel_uptime_seconds"
  log "healthy startup_grace relay_uptime_seconds=${relay_uptime_seconds} tunnel_uptime_seconds=${tunnel_uptime_seconds} startup_grace_seconds=${STARTUP_GRACE_SECONDS} public_health=${startup_public_health_code} public_stats=${startup_public_stats_code}"
  exit 0
fi

local_probe="$(http_probe "$LOCAL_HEALTH_URL")"
local_code="$(printf '%s\n' "$local_probe" | probe_http_code)"
if [[ "$local_code" != "200" ]]; then
  reset_edge_failure_count
  emit_event "recovering" "local_health_${local_code}" "restart:${RELAY_SERVICE_NAME}" "$local_code" "skipped" "$relay_uptime_seconds" "$tunnel_uptime_seconds"
  restart_service_with_limit "$RELAY_SERVICE_NAME" "local_health_${local_code}" "$local_code" "skipped" "$relay_uptime_seconds" "$tunnel_uptime_seconds"
  exit 0
fi

public_health_probe="$(http_probe "$PUBLIC_HEALTH_URL")"
public_health_code="$(printf '%s\n' "$public_health_probe" | probe_http_code)"
public_health_body="$(printf '%s\n' "$public_health_probe" | probe_http_body)"
public_probe="$(http_probe "$PUBLIC_STATS_URL")"
public_stats_code="$(printf '%s\n' "$public_probe" | probe_http_code)"
public_stats_body="$(printf '%s\n' "$public_probe" | probe_http_body)"
public_code="$(public_code_for_event "$public_health_code" "$public_stats_code")"

if [[ "$public_health_code" == "200" && "$public_stats_code" == "200" ]]; then
  reset_edge_failure_count
  emit_event "healthy" "public_edge_ok" "none" "$local_code" "$public_code" "$relay_uptime_seconds" "$tunnel_uptime_seconds"
  log "healthy local_health=${local_code} public_health=${public_health_code} public_stats=${public_stats_code}"
  exit 0
fi

if public_edge_failure "$public_health_code" "$public_health_body" || public_edge_failure "$public_stats_code" "$public_stats_body"; then
  recover_public_edge_failure "public_edge_${public_code}" "$local_code" "$public_code" "$relay_uptime_seconds" "$tunnel_uptime_seconds"
  exit 0
fi

reset_edge_failure_count
emit_event "degraded" "public_stats_${public_code}" "none" "$local_code" "$public_code" "$relay_uptime_seconds" "$tunnel_uptime_seconds"
log "degraded local_health=${local_code} public_health=${public_health_code} public_stats=${public_stats_code}"
exit 0
CHECK

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

ENV_SOURCE_PATH="$1"
CHECK_SOURCE_PATH="$2"
WATCHDOG_INTERVAL_SECONDS="$3"

sudo mkdir -p /etc/nexus-relay
sudo mv "$ENV_SOURCE_PATH" /etc/nexus-relay/public-watchdog.env
sudo chmod 640 /etc/nexus-relay/public-watchdog.env
sudo chown root:root /etc/nexus-relay/public-watchdog.env

sudo mv "$CHECK_SOURCE_PATH" /usr/local/bin/nexus-public-watchdog-check
sudo chmod 755 /usr/local/bin/nexus-public-watchdog-check
sudo chown root:root /usr/local/bin/nexus-public-watchdog-check

sudo tee /etc/systemd/system/nexus-public-watchdog.service >/dev/null <<'UNIT'
[Unit]
Description=OpenAgents Nexus public reachability watchdog
After=nexus-relay.service nexus-cloudflared.service
Wants=nexus-relay.service nexus-cloudflared.service

[Service]
Type=oneshot
EnvironmentFile=-/etc/nexus-relay/public-watchdog.env
ExecStart=/usr/local/bin/nexus-public-watchdog-check
UNIT

sudo tee /etc/systemd/system/nexus-public-watchdog.timer >/dev/null <<UNIT
[Unit]
Description=Run OpenAgents Nexus public reachability watchdog periodically

[Timer]
OnBootSec=2min
OnUnitActiveSec=${WATCHDOG_INTERVAL_SECONDS}s
AccuracySec=15s
Persistent=true
Unit=nexus-public-watchdog.service

[Install]
WantedBy=timers.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable --now nexus-public-watchdog.timer
sudo systemctl reset-failed nexus-public-watchdog.service || true
sudo systemctl --no-pager --full status nexus-public-watchdog.service | sed -n '1,40p' || true
sudo systemctl --no-pager --full status nexus-public-watchdog.timer | sed -n '1,40p' || true
REMOTE

chmod +x "$TMP_CHECK_SCRIPT" "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_ENV" "${NEXUS_VM}:/tmp/nexus-public-watchdog.env"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_CHECK_SCRIPT" "${NEXUS_VM}:/tmp/nexus-public-watchdog-check"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-install-public-watchdog.sh"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/nexus-install-public-watchdog.sh && /tmp/nexus-install-public-watchdog.sh '/tmp/nexus-public-watchdog.env' '/tmp/nexus-public-watchdog-check' '${NEXUS_PUBLIC_WATCHDOG_INTERVAL_SECONDS}'"

log "Public watchdog installed on ${NEXUS_VM}"
