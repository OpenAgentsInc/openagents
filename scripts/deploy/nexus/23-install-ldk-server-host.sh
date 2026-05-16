#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

if [[ "${NEXUS_LDK_INSTALL_DRY_RUN}" == "true" ]]; then
  log "Dry-running LDK Server host install. Set NEXUS_LDK_INSTALL_DRY_RUN=false to apply."
else
  require_cmd gcloud
  ensure_gcloud_context
  instance_exists "$NEXUS_LDK_VM" || die "LDK VM does not exist: ${NEXUS_LDK_VM}. Run 22-provision-ldk-topology.sh first."
fi

REMOTE_SCRIPT="$(mktemp)"
cleanup() {
  rm -f "$REMOTE_SCRIPT"
}
trap cleanup EXIT

cat >"$REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[ldk-host] %s\n' "$*" >&2
}

require_root() {
  if [[ "$(id -u)" -ne 0 ]]; then
    printf '[ldk-host] ERROR: remote install must run as root\n' >&2
    exit 1
  fi
}

require_root

: "${NEXUS_LDK_SERVER_REF:?missing NEXUS_LDK_SERVER_REF}"
: "${NEXUS_LDK_STORAGE_DIR:?missing NEXUS_LDK_STORAGE_DIR}"
: "${NEXUS_LDK_CONFIG_DIR:?missing NEXUS_LDK_CONFIG_DIR}"
: "${NEXUS_LDK_CONFIG_PATH:?missing NEXUS_LDK_CONFIG_PATH}"
: "${NEXUS_LDK_DATA_DISK_DEVICE_NAME:?missing NEXUS_LDK_DATA_DISK_DEVICE_NAME}"
: "${NEXUS_LDK_NETWORK:?missing NEXUS_LDK_NETWORK}"
: "${NEXUS_LDK_ALIAS:?missing NEXUS_LDK_ALIAS}"
: "${NEXUS_LDK_GRPC_PORT:?missing NEXUS_LDK_GRPC_PORT}"
: "${NEXUS_LDK_P2P_PORT:?missing NEXUS_LDK_P2P_PORT}"
: "${NEXUS_BITCOIND_RPC_HOST:?missing NEXUS_BITCOIND_RPC_HOST}"
: "${NEXUS_BITCOIND_RPC_PORT:?missing NEXUS_BITCOIND_RPC_PORT}"
: "${NEXUS_BITCOIND_RPC_USER:?missing NEXUS_BITCOIND_RPC_USER}"
: "${NEXUS_BITCOIND_RPC_PASSWORD_PATH:?missing NEXUS_BITCOIND_RPC_PASSWORD_PATH}"
: "${NEXUS_LDK_INSTALL_ALLOW_PLACEHOLDER_BITCOIND:?missing NEXUS_LDK_INSTALL_ALLOW_PLACEHOLDER_BITCOIND}"

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
  build-essential \
  ca-certificates \
  clang \
  cmake \
  curl \
  git \
  jq \
  libssl-dev \
  logrotate \
  pkg-config \
  protobuf-compiler \
  xxd

if ! command -v cargo >/dev/null 2>&1; then
  log "Installing rustup toolchain for ldk-server build"
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs |
    sh -s -- -y --profile minimal --default-toolchain stable
  . /root/.cargo/env
else
  . /root/.cargo/env 2>/dev/null || true
fi

if ! id -u ldk-server >/dev/null 2>&1; then
  useradd --system --home-dir "$NEXUS_LDK_STORAGE_DIR" --shell /usr/sbin/nologin ldk-server
fi

mkdir -p "$NEXUS_LDK_STORAGE_DIR" "$NEXUS_LDK_CONFIG_DIR" /opt/ldk-server /var/log/ldk-server

DEVICE_PATH="/dev/disk/by-id/google-${NEXUS_LDK_DATA_DISK_DEVICE_NAME}"
if [[ -e "$DEVICE_PATH" ]]; then
  if ! blkid "$DEVICE_PATH" >/dev/null 2>&1; then
    log "Formatting LDK data disk ${DEVICE_PATH}"
    mkfs.ext4 -F "$DEVICE_PATH"
  fi
  if ! grep -q "$NEXUS_LDK_STORAGE_DIR" /etc/fstab; then
    printf '%s %s ext4 defaults,nofail 0 2\n' "$DEVICE_PATH" "$NEXUS_LDK_STORAGE_DIR" >>/etc/fstab
  fi
  mountpoint -q "$NEXUS_LDK_STORAGE_DIR" || mount "$NEXUS_LDK_STORAGE_DIR"
else
  log "LDK data disk ${DEVICE_PATH} not found; using root disk path ${NEXUS_LDK_STORAGE_DIR}"
fi

chown -R ldk-server:ldk-server "$NEXUS_LDK_STORAGE_DIR" /var/log/ldk-server
chmod 0750 "$NEXUS_LDK_STORAGE_DIR"

if [[ ! -d /opt/ldk-server/src/.git ]]; then
  rm -rf /opt/ldk-server/src
  git clone https://github.com/lightningdevkit/ldk-server.git /opt/ldk-server/src
fi

git -C /opt/ldk-server/src fetch --tags --prune
git -C /opt/ldk-server/src checkout "$NEXUS_LDK_SERVER_REF"

log "Building ldk-server and ldk-server-cli at ${NEXUS_LDK_SERVER_REF}"
cargo build --manifest-path /opt/ldk-server/src/Cargo.toml --release -p ldk-server -p ldk-server-cli
install -m 0755 /opt/ldk-server/src/target/release/ldk-server /usr/local/bin/ldk-server
install -m 0755 /opt/ldk-server/src/target/release/ldk-server-cli /usr/local/bin/ldk-server-cli

if [[ -r "$NEXUS_BITCOIND_RPC_PASSWORD_PATH" ]]; then
  BITCOIND_RPC_PASSWORD="$(tr -d '\n' <"$NEXUS_BITCOIND_RPC_PASSWORD_PATH")"
elif [[ "$NEXUS_LDK_INSTALL_ALLOW_PLACEHOLDER_BITCOIND" == "true" ]]; then
  BITCOIND_RPC_PASSWORD="replace-with-bitcoind-rpc-password-on-host"
  install -m 0600 -o root -g root /dev/null "$NEXUS_BITCOIND_RPC_PASSWORD_PATH"
  log "Created empty bitcoind password placeholder at ${NEXUS_BITCOIND_RPC_PASSWORD_PATH}; ldk-server may not start until the real password is written."
else
  install -m 0600 -o root -g root /dev/null "$NEXUS_BITCOIND_RPC_PASSWORD_PATH"
  printf '[ldk-host] ERROR: bitcoind password file missing: %s\n' "$NEXUS_BITCOIND_RPC_PASSWORD_PATH" >&2
  printf '[ldk-host] Write the RPC password on-host, or set NEXUS_LDK_INSTALL_ALLOW_PLACEHOLDER_BITCOIND=true for a placeholder bootstrap.\n' >&2
  exit 1
fi

cat >"$NEXUS_LDK_CONFIG_PATH" <<CONFIG
[node]
network = "${NEXUS_LDK_NETWORK}"
listening_addresses = ["0.0.0.0:${NEXUS_LDK_P2P_PORT}"]
grpc_service_address = "0.0.0.0:${NEXUS_LDK_GRPC_PORT}"
alias = "${NEXUS_LDK_ALIAS}"

[storage.disk]
dir_path = "${NEXUS_LDK_STORAGE_DIR}"

[log]
level = "Info"
file = "${NEXUS_LDK_STORAGE_DIR}/${NEXUS_LDK_NETWORK}/ldk-server.log"

[tls]
hosts = ["${NEXUS_LDK_VM_HOSTNAME:-${HOSTNAME}}"]

[bitcoind]
rpc_address = "${NEXUS_BITCOIND_RPC_HOST}:${NEXUS_BITCOIND_RPC_PORT}"
rpc_user = "${NEXUS_BITCOIND_RPC_USER}"
rpc_password = "${BITCOIND_RPC_PASSWORD}"

[metrics]
enabled = true
poll_metrics_interval = 60
CONFIG

chown root:ldk-server "$NEXUS_LDK_CONFIG_PATH"
chmod 0640 "$NEXUS_LDK_CONFIG_PATH"

cat >/etc/systemd/system/ldk-server.service <<UNIT
[Unit]
Description=OpenAgents Nexus LDK Server
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
User=ldk-server
Group=ldk-server
WorkingDirectory=/opt/ldk-server/src
ExecStart=/usr/local/bin/ldk-server ${NEXUS_LDK_CONFIG_PATH}
Restart=always
RestartSec=5
TimeoutStopSec=90
LimitNOFILE=1048576
ReadWritePaths=${NEXUS_LDK_STORAGE_DIR} /var/log/ldk-server
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
UNIT

cat >/etc/logrotate.d/ldk-server <<LOGROTATE
${NEXUS_LDK_STORAGE_DIR}/${NEXUS_LDK_NETWORK}/ldk-server.log {
    daily
    rotate 14
    compress
    missingok
    notifempty
    postrotate
        systemctl kill --signal=HUP ldk-server.service || true
    endscript
}
LOGROTATE

systemctl daemon-reload
systemctl enable ldk-server.service
systemctl restart ldk-server.service
systemctl --no-pager --full status ldk-server.service || {
  journalctl -u ldk-server.service -n 120 --no-pager >&2
  exit 1
}

API_KEY_PATH="${NEXUS_LDK_STORAGE_DIR}/${NEXUS_LDK_NETWORK}/api_key"
TLS_CERT_PATH="${NEXUS_LDK_STORAGE_DIR}/tls.crt"
if [[ -f "$API_KEY_PATH" ]]; then
  chmod 0400 "$API_KEY_PATH"
  chown ldk-server:ldk-server "$API_KEY_PATH"
fi
if [[ -f "$TLS_CERT_PATH" ]]; then
  chmod 0444 "$TLS_CERT_PATH"
fi

mkdir -p /etc/nexus-relay/ldk-server
cat >/etc/nexus-relay/ldk-server/client-paths.env <<PATHS
NEXUS_LDK_SERVER_URL=${HOSTNAME}:${NEXUS_LDK_GRPC_PORT}
NEXUS_LDK_API_KEY_PATH=${API_KEY_PATH}
NEXUS_LDK_TLS_CERT_PATH=${TLS_CERT_PATH}
NEXUS_LDK_NETWORK=${NEXUS_LDK_NETWORK}
PATHS
chmod 0640 /etc/nexus-relay/ldk-server/client-paths.env

log "LDK Server install complete. API key and TLS cert paths were written, but secret material was not printed."
REMOTE

if [[ "${NEXUS_LDK_INSTALL_DRY_RUN}" == "true" ]]; then
  log "Would install on ${NEXUS_LDK_VM} using ${REMOTE_SCRIPT}"
  sed -n '1,60p' "$REMOTE_SCRIPT" >&2
  exit 0
fi

gcloud compute ssh "$NEXUS_LDK_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "sudo env \
    NEXUS_LDK_SERVER_REF='${NEXUS_LDK_SERVER_REF}' \
    NEXUS_LDK_STORAGE_DIR='${NEXUS_LDK_STORAGE_DIR}' \
    NEXUS_LDK_CONFIG_DIR='${NEXUS_LDK_CONFIG_DIR}' \
    NEXUS_LDK_CONFIG_PATH='${NEXUS_LDK_CONFIG_PATH}' \
    NEXUS_LDK_DATA_DISK_DEVICE_NAME='${NEXUS_LDK_DATA_DISK_DEVICE_NAME}' \
    NEXUS_LDK_NETWORK='${NEXUS_LDK_NETWORK}' \
    NEXUS_LDK_ALIAS='${NEXUS_LDK_ALIAS}' \
    NEXUS_LDK_GRPC_PORT='${NEXUS_LDK_GRPC_PORT}' \
    NEXUS_LDK_P2P_PORT='${NEXUS_LDK_P2P_PORT}' \
    NEXUS_BITCOIND_RPC_HOST='${NEXUS_BITCOIND_RPC_HOST}' \
    NEXUS_BITCOIND_RPC_PORT='${NEXUS_BITCOIND_RPC_PORT}' \
    NEXUS_BITCOIND_RPC_USER='${NEXUS_BITCOIND_RPC_USER}' \
    NEXUS_BITCOIND_RPC_PASSWORD_PATH='${NEXUS_BITCOIND_RPC_PASSWORD_PATH}' \
    NEXUS_LDK_INSTALL_ALLOW_PLACEHOLDER_BITCOIND='${NEXUS_LDK_INSTALL_ALLOW_PLACEHOLDER_BITCOIND}' \
    bash -s" <"$REMOTE_SCRIPT"

log "LDK Server host install complete on ${NEXUS_LDK_VM}"
