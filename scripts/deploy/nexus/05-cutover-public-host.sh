#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd cloudflared
require_cmd curl
require_cmd gcloud
require_cmd jq

ensure_gcloud_context

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

TUNNEL_NAME="${NEXUS_CLOUDFLARE_TUNNEL_NAME}"
TUNNEL_IMAGE="${NEXUS_CLOUDFLARE_TUNNEL_IMAGE}"
ORIGIN_URL="${NEXUS_CLOUDFLARE_TUNNEL_ORIGIN_URL}"

tunnel_json="$(cloudflared tunnel list --output json | jq -c --arg name "$TUNNEL_NAME" 'map(select(.name == $name)) | .[0]')"
if [[ "$tunnel_json" == "null" || -z "$tunnel_json" ]]; then
  log "Creating Cloudflare tunnel '${TUNNEL_NAME}'"
  tunnel_json="$(cloudflared tunnel create --output json "$TUNNEL_NAME")"
else
  log "Reusing existing Cloudflare tunnel '${TUNNEL_NAME}'"
fi

TUNNEL_ID="$(jq -r '.id' <<<"$tunnel_json")"
[[ -n "$TUNNEL_ID" && "$TUNNEL_ID" != "null" ]] || die "Failed to determine tunnel id for ${TUNNEL_NAME}"

log "Routing ${NEXUS_PUBLIC_HOST} to tunnel ${TUNNEL_NAME} (${TUNNEL_ID})"
cloudflared tunnel route dns --overwrite-dns "$TUNNEL_NAME" "$NEXUS_PUBLIC_HOST" >/dev/null

TUNNEL_TOKEN="$(cloudflared tunnel token "$TUNNEL_NAME" | tr -d '\r\n')"
[[ -n "$TUNNEL_TOKEN" ]] || die "Failed to fetch Cloudflare tunnel token for ${TUNNEL_NAME}"

TMP_ENV="$(mktemp)"
TMP_REMOTE_SCRIPT="$(mktemp)"
trap 'rm -f "$TMP_ENV" "$TMP_REMOTE_SCRIPT"' EXIT

cat >"$TMP_ENV" <<ENV
# Managed by scripts/deploy/nexus/05-cutover-public-host.sh
TUNNEL_TOKEN=${TUNNEL_TOKEN}
TUNNEL_IMAGE=${TUNNEL_IMAGE}
TUNNEL_ORIGIN_URL=${ORIGIN_URL}
ENV

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

ENV_SOURCE_PATH="$1"

sudo mkdir -p /etc/nexus-relay
sudo mv "$ENV_SOURCE_PATH" /etc/nexus-relay/nexus-cloudflared.env
sudo chmod 600 /etc/nexus-relay/nexus-cloudflared.env
sudo chown root:root /etc/nexus-relay/nexus-cloudflared.env

sudo tee /etc/systemd/system/nexus-cloudflared.service >/dev/null <<'UNIT'
[Unit]
Description=Cloudflare tunnel for OpenAgents Nexus
After=network-online.target docker.service nexus-relay.service
Wants=network-online.target
Requires=docker.service nexus-relay.service

[Service]
Type=simple
Restart=always
RestartSec=10
EnvironmentFile=/etc/nexus-relay/nexus-cloudflared.env
ExecStartPre=-/usr/bin/docker rm -f nexus-cloudflared
ExecStart=/bin/sh -c '/usr/bin/docker run --rm --name nexus-cloudflared --network host "${TUNNEL_IMAGE}" tunnel --no-autoupdate run --token "${TUNNEL_TOKEN}" --url "${TUNNEL_ORIGIN_URL}"'
ExecStop=/usr/bin/docker stop nexus-cloudflared
TimeoutStopSec=45

[Install]
WantedBy=multi-user.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable nexus-cloudflared
sudo systemctl restart nexus-cloudflared

sudo systemctl --no-pager --full status nexus-cloudflared | sed -n '1,40p'
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_ENV" "${NEXUS_VM}:/tmp/nexus-cloudflared.env"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-cloudflared-bootstrap.sh"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/nexus-cloudflared-bootstrap.sh && /tmp/nexus-cloudflared-bootstrap.sh '/tmp/nexus-cloudflared.env'"

log "Cloudflare tunnel '${TUNNEL_NAME}' now fronts ${NEXUS_VM} for ${NEXUS_PUBLIC_HOST}"
