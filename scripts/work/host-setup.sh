#!/usr/bin/env bash
# Make this cloud environment (a VM from the oa-coder-host image, as
# docs/cloud/dogfood-dev-on-prod.md starts one) a work host for "work on
# this issue" (#11258): it takes runs from openagents.com and runs the
# briefed agent on them (scripts/work/worker.py, docs/cloud/work-on-issues.md).
#
#   sudo -v && scripts/work/host-setup.sh [SITES]
#
# SITES (default https://openagents.com) is a comma-separated list. It
# installs numpy for the context finder, signs the environment in
# (scripts/cloud/dev-env-session.sh: GitHub and the finder's embeddings key;
# its own Claude token is never given to a run), builds the briefed agent
# and the openagents CLI into ~/.openagents/work/bin, builds the finder's
# index, and installs oa-work-worker.service, which starts at boot.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
sites=${1:-https://openagents.com}
work=$HOME/.openagents/work
mkdir -p "$work/bin" "$work/target"
export PATH=$HOME/.cargo/bin:$PATH

python3 -c 'import numpy' 2>/dev/null || sudo -n apt-get install -y -q python3-numpy > /dev/null
eval "$("$here/scripts/cloud/dev-env-session.sh")"

target=${CARGO_TARGET_DIR:-$(ls -d "$HOME"/.openagents/targets/*slot-0 2>/dev/null | head -1)}
target=${target:-$work/target}
(cd "$here" && CARGO_TARGET_DIR=$target cargo build -q -p briefed-agent -p openagents-cli --bin openagents)
install -m 755 "$target/debug/briefed-agent" "$work/bin/briefed-agent"
install -m 755 "$target/debug/openagents" "$work/bin/openagents"

(cd "$here" && python3 scripts/filefind/filefind.py index --repo . > /dev/null)

unit=/etc/systemd/system/oa-work-worker.service
sudo -n tee "$unit" > /dev/null <<UNIT
[Unit]
Description=OpenAgents work host: takes "work on this issue" runs (#11258)
After=network-online.target
Wants=network-online.target

[Service]
User=$USER
WorkingDirectory=$here
Environment=OA_WORK_SITES=$sites
Environment=OA_WORK_TARGET=$target
ExecStart=/bin/bash -c 'eval "\$($here/scripts/cloud/dev-env-session.sh)" && exec python3 $here/scripts/work/worker.py'
Restart=always
RestartSec=15
KillMode=mixed
TimeoutStopSec=60

[Install]
WantedBy=multi-user.target
UNIT
sudo -n systemctl daemon-reload
sudo -n systemctl enable --now oa-work-worker.service
sudo -n systemctl restart oa-work-worker.service
echo "work host ready: $(hostname), taking runs from $sites"
