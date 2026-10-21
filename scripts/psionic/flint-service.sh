#!/usr/bin/env bash
# Install a commit-named native Flint executable on a dedicated GCP host.
set -euo pipefail
if [[ $# != 2 ]]; then echo 'usage: flint-service.sh COMMIT BINARY' >&2; exit 2; fi
commit=$1
binary=$(realpath "$2")
[[ $commit =~ ^[0-9a-f]{10,40}$ ]] || { echo 'commit must be a Git hash' >&2; exit 2; }
user=$(id -un)
destination="$HOME/flint-builds/$commit/psionic-flint"
mkdir -p "$(dirname "$destination")"
if [[ $binary != "$destination" ]]; then cp "$binary" "$destination"; fi
chmod 755 "$destination"
sudo tee /etc/systemd/system/psionic-flint.service >/dev/null <<EOF
[Unit]
Description=Psionic native Flint
After=network-online.target

[Service]
User=$user
Environment=LD_LIBRARY_PATH=/usr/local/cuda/lib64
ExecStart=$destination serve --weights $HOME/flint-weights --backend cuda --context 8192 --listen 127.0.0.1:8080
Restart=on-failure
RestartSec=5
TimeoutStopSec=30
KillMode=control-group
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable --now psionic-flint
echo "Installed Flint from $commit; GPU capacity and VM expiry are managed by cloud-budget.py."
