#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq

ensure_gcloud_context

DEPLOY_IMAGE="${DEPLOY_IMAGE:-${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:latest}"
UPSTREAM_CONFIG_SOURCE="${ROOT_DIR}/apps/nexus-relay/deploy/upstream-config.toml"

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi
[[ -f "$UPSTREAM_CONFIG_SOURCE" ]] || die "Missing upstream config template: ${UPSTREAM_CONFIG_SOURCE}"

TMP_ENV="$(mktemp)"
TMP_UPSTREAM_CONFIG="$(mktemp)"
TMP_REMOTE_SCRIPT="$(mktemp)"
trap 'rm -f "$TMP_ENV" "$TMP_UPSTREAM_CONFIG" "$TMP_REMOTE_SCRIPT"' EXIT

cp "$UPSTREAM_CONFIG_SOURCE" "$TMP_UPSTREAM_CONFIG"

cat >"$TMP_ENV" <<ENV
# Managed by scripts/deploy/nexus/03-configure-and-start.sh
RUST_LOG=info
NEXUS_RELAY_LISTEN_ADDR=${NEXUS_LISTEN_ADDR}
NEXUS_RELAY_UPSTREAM_LISTEN_ADDR=${NEXUS_UPSTREAM_LISTEN_ADDR}
NEXUS_RELAY_DATA_DIR=${NEXUS_DATA_DIR}
NEXUS_RELAY_PUBLIC_WS_URL=${NEXUS_PUBLIC_WS_URL}
NEXUS_RELAY_UPSTREAM_CONFIG_FILE=/etc/nexus-relay/upstream-config.toml
NEXUS_CONTROL_HOSTED_NEXUS_RELAY_URL=${NEXUS_PUBLIC_WS_URL}
NEXUS_CONTROL_RECEIPT_LOG_PATH=${NEXUS_RECEIPT_LOG_PATH}
ENV

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

DEPLOY_IMAGE="$1"
ENV_SOURCE_PATH="$2"
UPSTREAM_CONFIG_SOURCE_PATH="$3"
NEXUS_DATA_DIR="$4"
NEXUS_DATA_DISK_DEVICE_NAME="$5"

sudo apt-get update -y
sudo apt-get install -y ca-certificates curl jq docker.io sqlite3

sudo systemctl enable docker
sudo systemctl start docker

DATA_DISK_PATH="/dev/disk/by-id/google-${NEXUS_DATA_DISK_DEVICE_NAME}"
if [[ ! -b "$DATA_DISK_PATH" ]]; then
  echo "Could not locate Nexus data disk by-id path: ${DATA_DISK_PATH}" >&2
  exit 1
fi

if ! sudo blkid "$DATA_DISK_PATH" >/dev/null 2>&1; then
  sudo mkfs.ext4 -F "$DATA_DISK_PATH"
fi

sudo mkdir -p "$NEXUS_DATA_DIR"
if ! grep -q "${DATA_DISK_PATH} ${NEXUS_DATA_DIR} ext4" /etc/fstab; then
  echo "${DATA_DISK_PATH} ${NEXUS_DATA_DIR} ext4 defaults,nofail 0 2" | sudo tee -a /etc/fstab >/dev/null
fi
sudo mount -a

sudo mkdir -p /etc/nexus-relay
sudo mv "$ENV_SOURCE_PATH" /etc/nexus-relay/nexus-relay.env
sudo mv "$UPSTREAM_CONFIG_SOURCE_PATH" /etc/nexus-relay/upstream-config.toml
sudo chmod 640 /etc/nexus-relay/nexus-relay.env
sudo chmod 644 /etc/nexus-relay/upstream-config.toml
sudo chown root:root /etc/nexus-relay/nexus-relay.env
sudo chown root:root /etc/nexus-relay/upstream-config.toml

sudo mkdir -p "$NEXUS_DATA_DIR"
sudo chown -R 60000:60000 "$NEXUS_DATA_DIR"

AR_HOST="$(echo "$DEPLOY_IMAGE" | cut -d'/' -f1)"
ACCESS_TOKEN="$(curl -fsS -H 'Metadata-Flavor: Google' \
  'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' | jq -r '.access_token')"

echo "$ACCESS_TOKEN" | sudo docker login -u oauth2accesstoken --password-stdin "https://${AR_HOST}"
sudo docker pull "$DEPLOY_IMAGE"

sudo tee /etc/systemd/system/nexus-relay.service >/dev/null <<UNIT
[Unit]
Description=OpenAgents Nexus durable relay + authority host
After=network-online.target docker.service
Wants=network-online.target
Requires=docker.service

[Service]
Type=simple
Restart=always
RestartSec=10
ExecStartPre=-/usr/bin/docker rm -f nexus-relay
ExecStartPre=/usr/bin/docker pull ${DEPLOY_IMAGE}
ExecStart=/usr/bin/docker run --rm --name nexus-relay --network host \
  --env-file /etc/nexus-relay/nexus-relay.env \
  -v /etc/nexus-relay:/etc/nexus-relay:ro \
  -v ${NEXUS_DATA_DIR}:${NEXUS_DATA_DIR} \
  ${DEPLOY_IMAGE}
ExecStop=/usr/bin/docker stop nexus-relay
TimeoutStopSec=45

[Install]
WantedBy=multi-user.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable nexus-relay
sudo systemctl restart nexus-relay

sudo systemctl --no-pager --full status nexus-relay | sed -n '1,40p'
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_ENV" "${NEXUS_VM}:/tmp/nexus-relay.env"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_UPSTREAM_CONFIG" "${NEXUS_VM}:/tmp/upstream-config.toml"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${NEXUS_VM}:/tmp/nexus-bootstrap.sh"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/nexus-bootstrap.sh && /tmp/nexus-bootstrap.sh '$DEPLOY_IMAGE' '/tmp/nexus-relay.env' '/tmp/upstream-config.toml' '$NEXUS_DATA_DIR' '$NEXUS_DATA_DISK_DEVICE_NAME'"

log "Nexus deployment refreshed on ${NEXUS_VM} using image ${DEPLOY_IMAGE}"
