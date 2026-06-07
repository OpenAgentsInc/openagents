#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

TMP_REMOTE_SCRIPT="$(mktemp)"
REMOTE_SCRIPT="/tmp/nexus-audit-public-watchdog-$(date -u +%Y%m%d%H%M%S)-$$.sh"
trap 'rm -f "$TMP_REMOTE_SCRIPT"' EXIT

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

STATE_DIR="${NEXUS_PUBLIC_WATCHDOG_STATE_DIR:-/var/lib/nexus-relay/watchdog/public}"
CLOUDFLARED_ENV_PATH="${NEXUS_PUBLIC_WATCHDOG_CLOUDFLARED_ENV_PATH:-/etc/nexus-relay/nexus-cloudflared.env}"
PUBLIC_WATCHDOG_ENV_PATH="/etc/nexus-relay/public-watchdog.env"

section() {
  printf '\n== %s ==\n' "$1"
}

systemd_state() {
  local unit="$1"
  printf '%s enabled: ' "$unit"
  systemctl is-enabled "$unit" 2>/dev/null || true
  printf '%s active: ' "$unit"
  systemctl is-active "$unit" 2>/dev/null || true
}

probe() {
  local label="$1"
  local url="$2"
  local body_path
  body_path="$(mktemp)"
  local code
  code="$(curl -sS --max-time 10 -o "$body_path" -w '%{http_code}' "$url" || true)"
  printf '%s code: %s\n' "$label" "${code:-000}"
  printf '%s body: ' "$label"
  head -c 180 "$body_path" 2>/dev/null | tr '\n' ' ' || true
  printf '\n'
  rm -f "$body_path"
}

section "public watchdog units"
systemd_state nexus-public-watchdog.timer
systemd_state nexus-public-watchdog.service
systemd_state nexus-http-recovery-proxy.service
systemd_state nexus-cloudflared.service
systemd_state nexus-relay.service

section "public watchdog install files"
for path in \
  "$PUBLIC_WATCHDOG_ENV_PATH" \
  /usr/local/bin/nexus-public-watchdog-check \
  /usr/local/bin/nexus-http-recovery-proxy.py \
  /etc/systemd/system/nexus-public-watchdog.service \
  /etc/systemd/system/nexus-public-watchdog.timer \
  /etc/systemd/system/nexus-http-recovery-proxy.service
do
  if [[ -e "$path" ]]; then
    ls -l "$path"
  else
    printf 'missing %s\n' "$path"
  fi
done

section "watchdog environment"
if [[ -f "$PUBLIC_WATCHDOG_ENV_PATH" ]]; then
  sudo grep -E '^NEXUS_PUBLIC_WATCHDOG_' "$PUBLIC_WATCHDOG_ENV_PATH" || true
else
  printf 'missing %s\n' "$PUBLIC_WATCHDOG_ENV_PATH"
fi

section "cloudflared origin"
if [[ -f "$CLOUDFLARED_ENV_PATH" ]]; then
  sudo grep -E '^TUNNEL_ORIGIN_URL=' "$CLOUDFLARED_ENV_PATH" || true
else
  printf 'missing %s\n' "$CLOUDFLARED_ENV_PATH"
fi

section "local probes"
probe local-origin-health http://127.0.0.1:8080/healthz
probe recovery-proxy-health http://127.0.0.1:8081/healthz

section "watchdog receipts"
if [[ -f "${STATE_DIR}/last-event.json" ]]; then
  sudo cat "${STATE_DIR}/last-event.json"
  printf '\n'
else
  printf 'missing %s\n' "${STATE_DIR}/last-event.json"
fi
if [[ -f "${STATE_DIR}/edge-failure-count" ]]; then
  printf 'edge-failure-count: '
  sudo cat "${STATE_DIR}/edge-failure-count"
else
  printf 'missing %s\n' "${STATE_DIR}/edge-failure-count"
fi
if [[ -f "${STATE_DIR}/events.jsonl" ]]; then
  printf '\nlast events:\n'
  sudo tail -n 12 "${STATE_DIR}/events.jsonl"
else
  printf '\nmissing %s\n' "${STATE_DIR}/events.jsonl"
fi

section "recent logs"
sudo journalctl -u nexus-public-watchdog.service -n 40 --no-pager || true
sudo journalctl -u nexus-http-recovery-proxy.service -n 20 --no-pager || true
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:${REMOTE_SCRIPT}"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x ${REMOTE_SCRIPT} && ${REMOTE_SCRIPT}; status=\$?; rm -f ${REMOTE_SCRIPT}; exit \$status"
