#!/usr/bin/env bash
# W1 CPU qualification infrastructure. Run only on its dedicated guarded VM.
# Original weights stay in Google Cloud. No reference Python runs inference here.
set -euo pipefail
SSD=/mnt/ssd
MODELS=gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1
ORACLE=gs://openagents-psionic-oracle/deepseek-v41/1bae0e1
setup() {
  if ! mountpoint -q "$SSD"; then
    [ -b /dev/disk/by-id/google-data ] || { echo 'Dedicated data disk missing' >&2; exit 1; }
    sudo mkfs.ext4 -q -F -E lazy_itable_init=1,lazy_journal_init=1 /dev/disk/by-id/google-data
    sudo mkdir -p "$SSD"
    sudo mount -o noatime /dev/disk/by-id/google-data "$SSD"
    sudo chown "$(id -u):$(id -g)" "$SSD"
  fi
  mkdir -p "$SSD/checkpoint" "$SSD/oracle" "$SSD/out" "$SSD/source"
  # The regional GCE Ubuntu mirror can be unavailable while the primary works.
  sudo sed -i -E 's|http://|https://|g;s|[a-z0-9-]+\.gce\.archive\.ubuntu\.com|archive.ubuntu.com|g' /etc/apt/sources.list.d/ubuntu.sources
  sudo apt-get -o Acquire::Retries=1 -o Acquire::https::Timeout=15 -qq update
  sudo apt-get -qq install -y build-essential pkg-config libssl-dev curl git >/dev/null
  if ! [ -x "$HOME/.cargo/bin/rustup" ]; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --default-toolchain 1.97.1
  fi
  export PATH="$HOME/.cargo/bin:$PATH"
  rustup toolchain install 1.97.1 --profile minimal
  gcloud storage rsync -r "$MODELS" "$SSD/checkpoint" > "$SSD/out/copy-weights.log" 2>&1 &
  local weights_pid=$!
  gcloud storage rsync -r "$ORACLE/dump" "$SSD/oracle" > "$SSD/out/copy-oracle.log" 2>&1 &
  local oracle_pid=$!
  wait "$weights_pid"
  wait "$oracle_pid"
  date -u +%FT%TZ > "$SSD/out/copy-complete.txt"
  uname -a > "$SSD/out/machine.txt"
  lscpu >> "$SSD/out/machine.txt"
  rustc +1.97.1 -vV >> "$SSD/out/machine.txt"
}
case "${1:-}" in setup) setup;; *) echo 'Usage: cpu_vm.sh setup' >&2; exit 2;; esac
