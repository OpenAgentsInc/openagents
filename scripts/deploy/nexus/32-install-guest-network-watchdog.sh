#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if [[ "${NEXUS_GUEST_NETWORK_WATCHDOG_ENABLED}" != "true" ]]; then
  log "Guest network watchdog disabled; skipping install"
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
# Managed by scripts/deploy/nexus/32-install-guest-network-watchdog.sh
NEXUS_GUEST_NETWORK_WATCHDOG_INTERVAL_SECONDS=${NEXUS_GUEST_NETWORK_WATCHDOG_INTERVAL_SECONDS}
NEXUS_GUEST_NETWORK_WATCHDOG_FAILURE_THRESHOLD=${NEXUS_GUEST_NETWORK_WATCHDOG_FAILURE_THRESHOLD}
NEXUS_GUEST_NETWORK_WATCHDOG_REBOOT_ENABLED=${NEXUS_GUEST_NETWORK_WATCHDOG_REBOOT_ENABLED}
NEXUS_GUEST_NETWORK_WATCHDOG_METADATA_URL=${NEXUS_GUEST_NETWORK_WATCHDOG_METADATA_URL}
NEXUS_GUEST_NETWORK_WATCHDOG_DNS_NAME=${NEXUS_GUEST_NETWORK_WATCHDOG_DNS_NAME}
NEXUS_GUEST_NETWORK_WATCHDOG_EDGE_IP=${NEXUS_GUEST_NETWORK_WATCHDOG_EDGE_IP}
NEXUS_GUEST_NETWORK_WATCHDOG_STATE_DIR=${NEXUS_GUEST_NETWORK_WATCHDOG_STATE_DIR}
ENV

cat >"$TMP_CHECK_SCRIPT" <<'CHECK'
#!/usr/bin/env bash
set -euo pipefail

METADATA_URL="${NEXUS_GUEST_NETWORK_WATCHDOG_METADATA_URL:-http://169.254.169.254/computeMetadata/v1/instance/id}"
DNS_NAME="${NEXUS_GUEST_NETWORK_WATCHDOG_DNS_NAME:-region1.v2.argotunnel.com}"
EDGE_IP="${NEXUS_GUEST_NETWORK_WATCHDOG_EDGE_IP:-198.41.200.113}"
FAILURE_THRESHOLD="${NEXUS_GUEST_NETWORK_WATCHDOG_FAILURE_THRESHOLD:-3}"
REBOOT_ENABLED="${NEXUS_GUEST_NETWORK_WATCHDOG_REBOOT_ENABLED:-true}"
DRY_RUN="${NEXUS_GUEST_NETWORK_WATCHDOG_DRY_RUN:-false}"
STATE_DIR="${NEXUS_GUEST_NETWORK_WATCHDOG_STATE_DIR:-/var/lib/nexus-relay/watchdog/guest-network}"
EVENT_LOG_PATH="${STATE_DIR}/events.jsonl"
LAST_EVENT_PATH="${STATE_DIR}/last-event.json"
FAILURE_COUNT_PATH="${STATE_DIR}/failure-count"
NOW_UNIX_S="$(date +%s)"

mkdir -p "$STATE_DIR"

log() {
  local message="$1"
  echo "$message"
  logger -t nexus-guest-network-watchdog "$message"
}

json_escape() {
  python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$1"
}

emit_event() {
  local status="$1"
  local reason="$2"
  local action="$3"
  local metadata_status="$4"
  local dns_status="$5"
  local route_status="$6"
  local cloudflared_log_status="$7"
  local consecutive_failures="$8"
  local event
  event="{\"recorded_at_unix_s\":${NOW_UNIX_S},\"status\":$(json_escape "$status"),\"reason\":$(json_escape "$reason"),\"action\":$(json_escape "$action"),\"metadata_status\":$(json_escape "$metadata_status"),\"dns_status\":$(json_escape "$dns_status"),\"route_status\":$(json_escape "$route_status"),\"cloudflared_log_status\":$(json_escape "$cloudflared_log_status"),\"consecutive_failures\":$(json_escape "$consecutive_failures")}"
  printf '%s\n' "$event" >>"$EVENT_LOG_PATH"
  printf '%s\n' "$event" >"$LAST_EVENT_PATH"
}

reset_failure_count() {
  printf '0\n' >"$FAILURE_COUNT_PATH"
}

record_failure_count() {
  local prior="0"
  if [[ -f "$FAILURE_COUNT_PATH" ]]; then
    prior="$(tr -cd '0-9' <"$FAILURE_COUNT_PATH" || true)"
  fi
  if [[ -z "$prior" ]]; then
    prior="0"
  fi
  local next=$((prior + 1))
  printf '%s\n' "$next" >"$FAILURE_COUNT_PATH"
  printf '%s\n' "$next"
}

metadata_status="fail"
if curl -fsS --max-time 5 -H 'Metadata-Flavor: Google' "$METADATA_URL" >/dev/null 2>&1; then
  metadata_status="ok"
fi

dns_status="fail"
if python3 - "$DNS_NAME" <<'PY' >/dev/null 2>&1
import socket
import sys
socket.getaddrinfo(sys.argv[1], 443)
PY
then
  dns_status="ok"
fi

route_status="fail"
if ip route get "$EDGE_IP" >/dev/null 2>&1; then
  route_status="ok"
fi

cloudflared_log_status="clear"
if journalctl -u nexus-cloudflared --since "5 minutes ago" --no-pager 2>/dev/null \
  | grep -Eq 'network is unreachable|sendmsg: network is unreachable|Failed to refresh DNS'; then
  cloudflared_log_status="network_unreachable"
fi

if [[ "$metadata_status" == "ok" && "$dns_status" == "ok" && "$route_status" == "ok" ]]; then
  reset_failure_count
  emit_event "healthy" "guest_network_ok" "none" "$metadata_status" "$dns_status" "$route_status" "$cloudflared_log_status" "0"
  log "healthy metadata=${metadata_status} dns=${dns_status} route=${route_status} cloudflared_logs=${cloudflared_log_status}"
  exit 0
fi

if [[ "$metadata_status" == "fail" ]] \
  && { [[ "$dns_status" == "fail" ]] || [[ "$route_status" == "fail" ]] || [[ "$cloudflared_log_status" == "network_unreachable" ]]; }; then
  consecutive_failures="$(record_failure_count)"
  reason="guest_network_wedged"
  action="await_next_probe"
  status="degraded"
  if (( consecutive_failures >= FAILURE_THRESHOLD )); then
    status="escalation_required"
    action="vm_reboot"
  fi
  emit_event "$status" "$reason" "$action" "$metadata_status" "$dns_status" "$route_status" "$cloudflared_log_status" "$consecutive_failures"
  log "${status} reason=${reason} metadata=${metadata_status} dns=${dns_status} route=${route_status} cloudflared_logs=${cloudflared_log_status} consecutive_failures=${consecutive_failures} threshold=${FAILURE_THRESHOLD}"
  if (( consecutive_failures >= FAILURE_THRESHOLD )); then
    if [[ "$REBOOT_ENABLED" == "true" ]]; then
      if [[ "$DRY_RUN" == "true" ]]; then
        log "dry_run reboot reason=${reason}"
      else
        log "reboot reason=${reason}"
        systemctl reboot
      fi
    fi
    exit 1
  fi
  exit 1
fi

reset_failure_count
emit_event "degraded" "partial_network_probe_failure" "none" "$metadata_status" "$dns_status" "$route_status" "$cloudflared_log_status" "0"
log "degraded partial_network_probe_failure metadata=${metadata_status} dns=${dns_status} route=${route_status} cloudflared_logs=${cloudflared_log_status}"
exit 0
CHECK

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

ENV_SOURCE_PATH="$1"
CHECK_SOURCE_PATH="$2"
WATCHDOG_INTERVAL_SECONDS="$3"

sudo mkdir -p /etc/nexus-relay
sudo mv "$ENV_SOURCE_PATH" /etc/nexus-relay/guest-network-watchdog.env
sudo chmod 640 /etc/nexus-relay/guest-network-watchdog.env
sudo chown root:root /etc/nexus-relay/guest-network-watchdog.env

sudo mv "$CHECK_SOURCE_PATH" /usr/local/bin/nexus-guest-network-watchdog-check
sudo chmod 755 /usr/local/bin/nexus-guest-network-watchdog-check
sudo chown root:root /usr/local/bin/nexus-guest-network-watchdog-check

sudo tee /etc/systemd/system/nexus-guest-network-watchdog.service >/dev/null <<'UNIT'
[Unit]
Description=OpenAgents Nexus guest network watchdog
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
EnvironmentFile=-/etc/nexus-relay/guest-network-watchdog.env
ExecStart=/usr/local/bin/nexus-guest-network-watchdog-check
UNIT

sudo tee /etc/systemd/system/nexus-guest-network-watchdog.timer >/dev/null <<UNIT
[Unit]
Description=Run OpenAgents Nexus guest network watchdog periodically

[Timer]
OnBootSec=90s
OnUnitActiveSec=${WATCHDOG_INTERVAL_SECONDS}s
AccuracySec=15s
Persistent=true
Unit=nexus-guest-network-watchdog.service

[Install]
WantedBy=timers.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable --now nexus-guest-network-watchdog.timer
sudo systemctl reset-failed nexus-guest-network-watchdog.service || true
sudo systemctl --no-pager --full status nexus-guest-network-watchdog.service | sed -n '1,40p' || true
sudo systemctl --no-pager --full status nexus-guest-network-watchdog.timer | sed -n '1,40p' || true
REMOTE

chmod +x "$TMP_CHECK_SCRIPT" "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_ENV" "${NEXUS_VM}:/tmp/nexus-guest-network-watchdog.env"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_CHECK_SCRIPT" "${NEXUS_VM}:/tmp/nexus-guest-network-watchdog-check"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-install-guest-network-watchdog.sh"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/nexus-install-guest-network-watchdog.sh && /tmp/nexus-install-guest-network-watchdog.sh '/tmp/nexus-guest-network-watchdog.env' '/tmp/nexus-guest-network-watchdog-check' '${NEXUS_GUEST_NETWORK_WATCHDOG_INTERVAL_SECONDS}'"

log "Guest network watchdog installed on ${NEXUS_VM}"
