#!/usr/bin/env bash
# Install the Rust VM-recovery loop on its small on-demand control VM.
set -euo pipefail
if [[ $# != 3 ]]; then echo 'usage: flint-control-service.sh COMMIT BINARY GPU_INSTANCE_ID' >&2; exit 2; fi
commit=$1
binary=$(realpath "$2")
instance_id=$3
[[ $commit =~ ^[0-9a-f]{10,40}$ && $instance_id =~ ^[0-9]+$ ]] || { echo 'Use a Git hash and numeric instance ID.' >&2; exit 2; }
user=$(id -un)
destination="$HOME/flint-builds/$commit/pylon-vm-recovery"
mkdir -p "$(dirname "$destination")"
if [[ $binary != "$destination" ]]; then cp "$binary" "$destination"; fi
chmod 755 "$destination"
sudo tee /etc/systemd/system/flint-vm-recovery.service >/dev/null <<UNIT
[Unit]
Description=Restart the maintained Flint GPU VM after a preemption
Wants=network-online.target
After=network-online.target
StartLimitIntervalSec=0

[Service]
User=$user
ExecStart=$destination openagentsgemini asia-northeast1-b oa-flint-12419 $instance_id
Restart=always
RestartSec=10
MemoryMax=128M
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
UNIT
sudo systemctl daemon-reload
sudo systemctl enable flint-vm-recovery
sudo systemctl restart flint-vm-recovery
