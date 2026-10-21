#!/usr/bin/env bash
# Install Pylon beside the supervised native Flint worker on a maintained GPU host.
set -euo pipefail
if [[ $# != 2 ]]; then echo 'usage: flint-pylon-service.sh COMMIT PYLON_BINARY' >&2; exit 2; fi
commit=$1
binary=$(realpath "$2")
[[ $commit =~ ^[0-9a-f]{10,40}$ ]] || { echo 'commit must be a Git hash' >&2; exit 2; }
user=$(id -un)
destination="$HOME/flint-builds/$commit/pylon"
mkdir -p "$(dirname "$destination")" "$HOME/.openagents/pylon-flint"
chmod 700 "$HOME/.openagents/pylon-flint"
if [[ $binary != "$destination" ]]; then cp "$binary" "$destination"; fi
chmod 755 "$destination"
sudo tee /etc/systemd/system/pylon-flint.service >/dev/null <<EOF
[Unit]
Description=Flint Pylon on GCP
Wants=network-online.target psionic-flint.service
After=network-online.target psionic-flint.service
StartLimitIntervalSec=0

[Service]
User=$user
WorkingDirectory=$HOME
Environment=OPENAGENTS_PYLON_HOME=$HOME/.openagents/pylon-flint
ExecStart=$destination serve --text-profile flint --engine http://127.0.0.1:8080 --pylon flint-gcp --label "Flint on GCP" --dedicated --allow-any --rate 10 --total-rate 6
Restart=always
RestartSec=5
TimeoutStopSec=120
KillMode=control-group
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable pylon-flint
sudo systemctl restart pylon-flint
echo "Installed Flint Pylon from $commit; its beacon is published on relay.openagents.com."
