#!/usr/bin/env bash
set -euo pipefail

CACHE_DISK_DEVICE_NAME="${1:?usage: remote-bootstrap-warm-builder.sh <cache-disk-device-name> <cache-mount-point> <builder-user> <rust-toolchain> <sccache-version>}"
CACHE_MOUNT_POINT="${2:?usage: remote-bootstrap-warm-builder.sh <cache-disk-device-name> <cache-mount-point> <builder-user> <rust-toolchain> <sccache-version>}"
BUILDER_USER="${3:?usage: remote-bootstrap-warm-builder.sh <cache-disk-device-name> <cache-mount-point> <builder-user> <rust-toolchain> <sccache-version>}"
RUST_TOOLCHAIN="${4:?usage: remote-bootstrap-warm-builder.sh <cache-disk-device-name> <cache-mount-point> <builder-user> <rust-toolchain> <sccache-version>}"
SCCACHE_VERSION="${5:?usage: remote-bootstrap-warm-builder.sh <cache-disk-device-name> <cache-mount-point> <builder-user> <rust-toolchain> <sccache-version>}"

log() {
  printf '[nexus-builder-bootstrap] %s\n' "$*" >&2
}

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    printf '[nexus-builder-bootstrap] ERROR: missing command: %s\n' "$cmd" >&2
    exit 1
  fi
}

install_sccache() {
  local desired_version="$1"
  local arch sccache_arch download_url tmp_dir

  if command -v sccache >/dev/null 2>&1; then
    local current_version
    current_version="$(sccache --version | awk 'NR==1 { print $2 }')"
    if [[ "$current_version" == "$desired_version" ]]; then
      return 0
    fi
  fi

  arch="$(dpkg --print-architecture)"
  case "$arch" in
    amd64) sccache_arch="x86_64-unknown-linux-musl" ;;
    arm64) sccache_arch="aarch64-unknown-linux-musl" ;;
    *)
      printf '[nexus-builder-bootstrap] ERROR: unsupported sccache architecture: %s\n' "$arch" >&2
      exit 1
      ;;
  esac

  download_url="https://github.com/mozilla/sccache/releases/download/v${desired_version}/sccache-v${desired_version}-${sccache_arch}.tar.gz"
  tmp_dir="$(mktemp -d)"
  curl -fsSL "$download_url" -o "${tmp_dir}/sccache.tar.gz"
  tar -xzf "${tmp_dir}/sccache.tar.gz" -C "$tmp_dir"
  sudo install -m 0755 "${tmp_dir}/sccache-v${desired_version}-${sccache_arch}/sccache" /usr/local/bin/sccache
  rm -rf "$tmp_dir"
}

require_cmd sudo
require_cmd curl
require_cmd dpkg

log "Installing builder packages"
sudo env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get update -y
sudo env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a \
  apt-get install -y \
  -o Dpkg::Options::=--force-confdef \
  -o Dpkg::Options::=--force-confold \
  build-essential \
  ca-certificates \
  curl \
  git \
  jq \
  pkg-config \
  libssl-dev \
  protobuf-compiler \
  python3 \
  rsync \
  unzip

DATA_DISK_PATH="/dev/disk/by-id/google-${CACHE_DISK_DEVICE_NAME}"
[[ -b "$DATA_DISK_PATH" ]] || {
  printf '[nexus-builder-bootstrap] ERROR: cache disk not found: %s\n' "$DATA_DISK_PATH" >&2
  exit 1
}

if ! sudo blkid "$DATA_DISK_PATH" >/dev/null 2>&1; then
  log "Formatting cache disk ${DATA_DISK_PATH}"
  sudo mkfs.ext4 -F "$DATA_DISK_PATH"
fi

sudo mkdir -p "$CACHE_MOUNT_POINT"
if ! grep -qE "^[^#]+[[:space:]]+${CACHE_MOUNT_POINT//\//\\/}[[:space:]]+ext4" /etc/fstab; then
  log "Recording cache disk mount in /etc/fstab"
  printf '%s %s ext4 defaults,nofail 0 2\n' "$DATA_DISK_PATH" "$CACHE_MOUNT_POINT" | sudo tee -a /etc/fstab >/dev/null
fi
if ! findmnt -n "$CACHE_MOUNT_POINT" >/dev/null 2>&1; then
  log "Mounting ${CACHE_MOUNT_POINT}"
  sudo mount "$CACHE_MOUNT_POINT" 2>/dev/null || sudo mount -a
fi

if ! id -u "$BUILDER_USER" >/dev/null 2>&1; then
  log "Creating builder user ${BUILDER_USER}"
  sudo useradd --create-home --home-dir "/home/${BUILDER_USER}" --shell /bin/bash "$BUILDER_USER"
fi

sudo install -d -o "$BUILDER_USER" -g "$BUILDER_USER" -m 0755 \
  "$CACHE_MOUNT_POINT/cargo-home" \
  "$CACHE_MOUNT_POINT/target" \
  "$CACHE_MOUNT_POINT/sccache" \
  "$CACHE_MOUNT_POINT/sources" \
  "$CACHE_MOUNT_POINT/artifacts" \
  "$CACHE_MOUNT_POINT/timings"

sudo install -d -o "$BUILDER_USER" -g "$BUILDER_USER" -m 0755 \
  "/home/${BUILDER_USER}/.cargo" \
  "/home/${BUILDER_USER}/.config"

if ! sudo -u "$BUILDER_USER" bash -lc 'test -x "$HOME/.cargo/bin/rustc"'; then
  log "Installing rustup toolchain ${RUST_TOOLCHAIN} for ${BUILDER_USER}"
  curl -fsSL https://sh.rustup.rs | sudo -u "$BUILDER_USER" bash -s -- -y --default-toolchain "$RUST_TOOLCHAIN" --profile minimal
fi

sudo -u "$BUILDER_USER" bash -lc "
  set -euo pipefail
  export PATH=\"\$HOME/.cargo/bin:\$PATH\"
  rustup toolchain install '${RUST_TOOLCHAIN}' --profile minimal >/dev/null
  rustup default '${RUST_TOOLCHAIN}' >/dev/null
"

log "Installing sccache ${SCCACHE_VERSION}"
install_sccache "$SCCACHE_VERSION"

sudo tee /etc/profile.d/nexus-builder.sh >/dev/null <<EOF
export NEXUS_BUILDER_CACHE_MOUNT_POINT=${CACHE_MOUNT_POINT}
export NEXUS_BUILDER_USER=${BUILDER_USER}
export CARGO_HOME=${CACHE_MOUNT_POINT}/cargo-home
export CARGO_TARGET_DIR=${CACHE_MOUNT_POINT}/target
export SCCACHE_DIR=${CACHE_MOUNT_POINT}/sccache
export PATH=/home/${BUILDER_USER}/.cargo/bin:\$PATH
EOF
sudo chmod 0644 /etc/profile.d/nexus-builder.sh

sudo -u "$BUILDER_USER" bash -lc "
  set -euo pipefail
  export PATH=\"\$HOME/.cargo/bin:\$PATH\"
  rustc --version
  cargo --version
"
sccache --version

log "Builder bootstrap complete"
log "cache_mount=${CACHE_MOUNT_POINT}"
log "builder_user=${BUILDER_USER}"
