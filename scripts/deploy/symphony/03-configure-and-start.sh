#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq

ensure_gcloud_context
fetch_bitcoind_rpc_creds

DEPLOY_IMAGE="${DEPLOY_IMAGE:-${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${SYMPHONY_ARTIFACT_REPO}/${SYMPHONY_IMAGE_NAME}:latest}"

if ! instance_exists "$SYMPHONY_VM"; then
  die "VM does not exist: ${SYMPHONY_VM}. Run 02-provision-baseline.sh first."
fi

TMP_CONFIG="$(mktemp)"
TMP_REMOTE_SCRIPT="$(mktemp)"
trap 'rm -f "$TMP_CONFIG" "$TMP_REMOTE_SCRIPT"' EXIT

cat >"$TMP_CONFIG" <<CONFIG
# Managed by scripts/deploy/symphony/03-configure-and-start.sh
db_path = "/var/lib/symphony/db"

[storage]
rocksdb_memory_budget = 8.0

[sync.node]
p2p_address = "${BITCOIND_P2P_HOST}:${BITCOIND_P2P_PORT}"
rpc_address = "http://${BITCOIND_RPC_HOST}:${BITCOIND_RPC_PORT}"
rpc_user = "${BITCOIND_RPC_USER}"
rpc_pass = "${BITCOIND_RPC_PASS}"

[sync]
network = "mainnet"
mempool = true
max_rollback = 64
utxo_cache_size = 16.0

[sync.indexers]
transaction_indexers = [
  { type = "TxCountByAddress" },
  { type = "UtxosByAddress" },
  { type = "Runes", start_height = 840000, index_activity = true }
]

[server]
address = "${SYMPHONY_SERVER_BIND}"
CONFIG

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

DEPLOY_IMAGE="$1"
CONFIG_SOURCE_PATH="$2"
CONFIG_DEST_PATH="/etc/symphony/mainnet.toml"

sudo apt-get update -y
sudo apt-get install -y ca-certificates curl jq docker.io

sudo systemctl enable docker
sudo systemctl start docker

DATA_DISK_PATH="/dev/disk/by-id/google-symphony-data"
if [[ ! -b "$DATA_DISK_PATH" ]]; then
  DATA_DISK_PATH="/dev/disk/by-id/google-symphony-data-mainnet"
fi
if [[ ! -b "$DATA_DISK_PATH" ]]; then
  echo "Could not locate Symphony data disk by-id path" >&2
  exit 1
fi

if ! sudo blkid "$DATA_DISK_PATH" >/dev/null 2>&1; then
  sudo mkfs.ext4 -F "$DATA_DISK_PATH"
fi

sudo mkdir -p /var/lib/symphony
if ! grep -q "${DATA_DISK_PATH} /var/lib/symphony ext4" /etc/fstab; then
  echo "${DATA_DISK_PATH} /var/lib/symphony ext4 defaults,nofail 0 2" | sudo tee -a /etc/fstab >/dev/null
fi
sudo mount -a
sudo mkdir -p /var/lib/symphony/db

sudo mkdir -p /etc/symphony
sudo mv "$CONFIG_SOURCE_PATH" "$CONFIG_DEST_PATH"
sudo chown 60000:60000 "$CONFIG_DEST_PATH"
sudo chmod 640 "$CONFIG_DEST_PATH"

# Container runs as uid/gid 60000
sudo chown -R 60000:60000 /var/lib/symphony

AR_HOST="$(echo "$DEPLOY_IMAGE" | cut -d'/' -f1)"
ACCESS_TOKEN="$(curl -fsS -H 'Metadata-Flavor: Google' \
  'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' | jq -r '.access_token')"

echo "$ACCESS_TOKEN" | sudo docker login -u oauth2accesstoken --password-stdin "https://${AR_HOST}"
sudo docker pull "$DEPLOY_IMAGE"

sudo tee /etc/systemd/system/symphony.service >/dev/null <<UNIT
[Unit]
Description=Maestro Symphony mainnet indexer/API
After=network-online.target docker.service
Wants=network-online.target
Requires=docker.service

[Service]
Type=simple
Restart=always
RestartSec=10
ExecStartPre=-/usr/bin/docker rm -f symphony
ExecStartPre=/usr/bin/docker pull ${DEPLOY_IMAGE}
ExecStart=/usr/bin/docker run --rm --name symphony --network host \
  -v /var/lib/symphony:/var/lib/symphony \
  -v /etc/symphony/mainnet.toml:/etc/symphony/mainnet.toml:ro \
  ${DEPLOY_IMAGE} /etc/symphony/mainnet.toml run
ExecStop=/usr/bin/docker stop symphony
TimeoutStopSec=45

[Install]
WantedBy=multi-user.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable symphony
sudo systemctl restart symphony

sudo systemctl --no-pager --full status symphony | sed -n '1,40p'
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_CONFIG" "${SYMPHONY_VM}:/tmp/symphony-mainnet.toml"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${SYMPHONY_VM}:/tmp/symphony-bootstrap.sh"

gcloud compute ssh "$SYMPHONY_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/symphony-bootstrap.sh && /tmp/symphony-bootstrap.sh '$DEPLOY_IMAGE' '/tmp/symphony-mainnet.toml'"

log "Symphony deployment refreshed on ${SYMPHONY_VM} using image ${DEPLOY_IMAGE}"
