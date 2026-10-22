#!/usr/bin/env bash
# The lasting Kiln machine (#12454): one small Google Compute VM that runs
# kiln-runner (Kiln's remote cargo test) and, once it has a token, the GitHub
# bridge (kiln-git bridge). It holds nothing else. Read
# docs/deployment/kiln-admin.md.
#
#   scripts/deploy/kiln-vm.sh create            make the machine (e2-standard-4, 100 GB)
#   scripts/deploy/kiln-vm.sh install [COMMIT]  toolchain, build, services (default origin/main)
#   scripts/deploy/kiln-vm.sh update [COMMIT]   rebuild at COMMIT, refetch crates, restart
#   scripts/deploy/kiln-vm.sh keys              print the runner key and repository address
#   scripts/deploy/kiln-vm.sh status | logs | delete
#
# KILN_VM (oa-kiln), KILN_PROJECT (openagentsgemini), KILN_ZONE (us-central1-a)
# override the defaults. The machine only dials out to
# the relay, GitHub, and crates.io, and nothing listens but SSH. Reach it
# with gcloud compute ssh.
set -euo pipefail

vm=${KILN_VM:-oa-kiln}
project=${KILN_PROJECT:-openagentsgemini}
zone=${KILN_ZONE:-us-central1-a}
repo=https://github.com/OpenAgentsInc/openagents.git

usage() { sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }
[[ $# -ge 1 ]] || usage

on_vm() { gcloud compute ssh "$vm" --project "$project" --zone "$zone" --quiet --command "$1"; }

commit_of() {
  local want=${1:-}
  if [[ -z $want ]]; then
    git ls-remote "$repo" refs/heads/main | cut -f1
  else
    [[ $want =~ ^[0-9a-f]{40}$ ]] || { echo "give a full 40-character commit" >&2; exit 2; }
    echo "$want"
  fi
}

# Runs as root on the machine: the kiln user, packages, Rust, the build, the
# crate cache the runner reads offline, and the services.
provision() {
  local commit=$1
  cat <<EOF
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
if ! command -v bwrap >/dev/null; then
  apt-get update -q
  apt-get install -yq git build-essential pkg-config libssl-dev bubblewrap ca-certificates curl
fi
id kiln >/dev/null 2>&1 || useradd --create-home --shell /bin/bash kiln
install -d -o kiln -g kiln -m 0700 /var/lib/kiln /var/lib/kiln/runner /var/lib/kiln/bridge
install -d -o root -g kiln -m 0750 /etc/kiln
for f in allow admins openers; do [ -e /etc/kiln/\$f ] || install -o root -g kiln -m 0640 /dev/null /etc/kiln/\$f; done
[ -e /etc/kiln/crates ] || printf '%s\n' 'memory-stream' 'xp-ledger' 'atif timeout=900' > /etc/kiln/crates
chown root:kiln /etc/kiln/crates && chmod 0640 /etc/kiln/crates
sudo -u kiln -H bash <<'INNER'
cd ~
  set -euo pipefail
  if [ ! -x ~/.cargo/bin/rustup ]; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
  fi
  . ~/.cargo/env
  if [ ! -d ~/src/.git ]; then git clone --quiet --filter=blob:none $repo ~/src; fi
  cd ~/src && git fetch --quiet origin && git checkout --quiet --detach $commit
  rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install --profile minimal
  CARGO_BUILD_JOBS=3 cargo build --quiet --release -p kiln-runner -p kiln-git
  install -m 0755 target/release/kiln-runner target/release/kiln-git ~/.cargo/bin/
  # The runner runs cargo offline, so every crate main needs is fetched here.
  cargo fetch --quiet
INNER
cat > /etc/systemd/system/kiln-runner.service <<'UNIT'
[Unit]
Description=Kiln runner: remote cargo test for Kiln over Nostr
After=network-online.target
Wants=network-online.target

[Service]
User=kiln
Environment=HOME=/home/kiln
Environment=PATH=/home/kiln/.cargo/bin:/usr/local/bin:/usr/bin:/bin
# With the owner's announcement in /etc/kiln/repository, take patches to it
# and stop announcing the demo repository.
ExecStart=/bin/sh -c 'set -- --state /var/lib/kiln/runner --jobs 3 --timeout 1200 --memory 12000 --allow /etc/kiln/allow --admins /etc/kiln/admins --crates-file /etc/kiln/crates; if [ -s /etc/kiln/repository ]; then set -- "\$\$@" --repository "\$\$(cat /etc/kiln/repository)" --no-announce; fi; exec kiln-runner "\$\$@"'
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
UNIT
cat > /etc/systemd/system/kiln-bridge.service <<'UNIT'
[Unit]
Description=Kiln GitHub bridge: draft pull requests from maintainer-opened patches
After=network-online.target
ConditionPathExists=/etc/kiln/github-token
ConditionPathExists=/etc/kiln/repository

[Service]
User=kiln
Environment=HOME=/home/kiln
Environment=PATH=/home/kiln/.cargo/bin:/usr/local/bin:/usr/bin:/bin
ExecStartPre=/bin/sh -c 'test -d /var/lib/kiln/bridge/repo/.git || git clone --quiet --depth 1 $repo /var/lib/kiln/bridge/repo'
ExecStart=/bin/sh -c 'exec kiln-git bridge --repository "\$\$(cat /etc/kiln/repository)" --key /var/lib/kiln/bridge/key --token-file /etc/kiln/github-token --checkout /var/lib/kiln/bridge/repo --state /var/lib/kiln/bridge --openers /etc/kiln/openers'
Restart=always
RestartSec=30

[Install]
WantedBy=multi-user.target
UNIT
cat > /etc/systemd/system/kiln-fetch.service <<'UNIT'
[Unit]
Description=Fetch the crates main needs, for the runner's offline builds

[Service]
Type=oneshot
User=kiln
ExecStart=/bin/bash -lc '. ~/.cargo/env && { [ -d ~/main/.git ] || git clone --quiet --depth 1 $repo ~/main; } && cd ~/main && git fetch --quiet --depth 1 origin main && git reset --quiet --hard origin/main && rustup toolchain install --profile minimal && cargo fetch --quiet'
UNIT
cat > /etc/systemd/system/kiln-fetch.timer <<'UNIT'
[Unit]
Description=Fetch main's crates every six hours

[Timer]
OnBootSec=10min
OnUnitActiveSec=6h

[Install]
WantedBy=timers.target
UNIT
# The bridge's own Nostr key, made once on the machine.
if [ ! -e /var/lib/kiln/bridge/key ]; then
  sudo -u kiln sh -c 'umask 077; od -An -tx1 -N32 /dev/urandom | tr -d " \n" > /var/lib/kiln/bridge/key; echo >> /var/lib/kiln/bridge/key'
fi
systemctl daemon-reload
systemctl enable --now kiln-fetch.timer
systemctl enable kiln-runner kiln-bridge
systemctl restart kiln-runner
systemctl restart kiln-bridge || true
EOF
}

case $1 in
  create)
    gcloud compute instances create "$vm" --project "$project" --zone "$zone" \
      --machine-type e2-standard-4 \
      --boot-disk-size 100GB --boot-disk-type pd-balanced \
      --image-family debian-12 --image-project debian-cloud \
      --labels purpose=kiln,issue=12454 \
      --metadata enable-oslogin=TRUE \
      --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring
    ;;
  install|update)
    commit=$(commit_of "${2:-}")
    echo "building $commit on $vm (a first install takes about 20 minutes)"
    provision "$commit" | on_vm 'sudo bash -s'
    "$0" keys
    ;;
  keys)
    on_vm 'sudo cat /var/lib/kiln/runner/pubkey /var/lib/kiln/runner/repository; printf "bridge "; sudo -u kiln /home/kiln/.cargo/bin/kiln-git announce --key /var/lib/kiln/bridge/key --maintainer 0000000000000000000000000000000000000000000000000000000000000001 --euc 0000000000000000000000000000000000000000 --dry-run 2>/dev/null | grep -m1 "\"pubkey\"" | tr -d " ,\""'
    ;;
  status)
    gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='value(status,machineType.basename(),labels)'
    on_vm 'systemctl --no-pager --lines 0 status kiln-runner kiln-bridge kiln-fetch.timer || true'
    ;;
  logs)
    on_vm 'sudo journalctl --no-pager -n 80 -u kiln-runner -u kiln-bridge'
    ;;
  delete)
    gcloud compute instances delete "$vm" --project "$project" --zone "$zone" --quiet
    ;;
  *) usage ;;
esac
