#!/usr/bin/env bash
# Run as root on a fresh, dedicated spike host only.
set -euo pipefail
[[ $(uname -sm) == 'Linux x86_64' && $EUID == 0 ]]
UPSTREAM=de87b436c35dac056564bdd5b2a113c9046e189b
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get -o DPkg::Lock::Timeout=120 install -y git curl ca-certificates docker.io docker-buildx btrfs-progs jq openssl python3 uidmap binutils
modprobe kvm_intel
modprobe tun
modprobe btrfs
# Kind nodes share host inotify limits. Stock Ubuntu's 128 instances can
# make kube-proxy fail with EMFILE, leaving cross-node DNS unavailable.
# https://kind.sigs.k8s.io/docs/user/known-issues/#pod-errors-due-to-too-many-open-files
cat >/etc/sysctl.d/90-space-spike.conf <<'LIMITS'
fs.inotify.max_user_instances = 1024
fs.inotify.max_user_watches = 1048576
LIMITS
sysctl -p /etc/sysctl.d/90-space-spike.conf
[[ -c /dev/kvm && -c /dev/net/tun ]]
disk=/dev/disk/by-id/google-local-nvme-ssd-0
[[ -b "$disk" ]]
mkdir -p /var/lib/space-spike
if ! mountpoint -q /var/lib/space-spike; then
  kind=$(blkid -o value -s TYPE "$disk" || true)
  if [[ -z "$kind" ]]; then mkfs.btrfs "$disk"; else [[ "$kind" == btrfs ]]; fi
  mount "$disk" /var/lib/space-spike
fi
[[ $(findmnt -n -o FSTYPE /var/lib/space-spike) == btrfs ]]
if ! id space-spike >/dev/null 2>&1; then
  useradd --create-home --home-dir /var/lib/space-spike/home --shell /bin/bash space-spike
fi
usermod -aG docker,kvm space-spike
# This is a disposable VM with no cloud service account or product credentials.
echo 'space-spike ALL=(ALL) NOPASSWD: ALL' >/etc/sudoers.d/space-spike
chmod 0440 /etc/sudoers.d/space-spike
if [[ ! -f /etc/docker/daemon.json ]]; then
  mkdir -p /etc/docker
  echo '{"data-root":"/var/lib/space-spike/docker"}' >/etc/docker/daemon.json
fi
systemctl restart docker
runuser -l space-spike -c 'git clone https://github.com/perplexityai/SPACE.git ~/SPACE' || test -d /var/lib/space-spike/home/SPACE/.git
runuser -l space-spike -c "cd ~/SPACE && git checkout --detach $UPSTREAM"
runuser -l space-spike -c 'bash -s' <<'SETUP'
set -euo pipefail
export INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
exec > >(tee -a "$HOME/install.log") 2>&1
export PATH="$INSTALL_DIR:$PATH"
export SPACE_VENDOR_ROOT="$HOME/vendor"
export SPACE_KIND_DATA_ROOT="$HOME/kind-data"
export SPACE_KIND_CLUSTER=space-local
export SPACE_KIND_TELEMETRY_MODE=off
export SPACE_KIND_OBSERVABILITY=off
unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN DD_API_KEY EPPO_SDK_KEY
cd "$HOME/SPACE"
bash infra/space/kind/scripts/install-prereqs.sh
bash infra/space/kind/scripts/install-fc-vendor.sh
bash infra/space/kind/scripts/bootstrap.sh
kubectl --context kind-space-local -n space wait --for=condition=complete job/space-seed-publish --timeout=300s
for ds in $(bash infra/space/kind/lib/node-services.sh daemonsets); do
  kubectl --context kind-space-local -n space rollout status "ds/$ds" --timeout=300s
done
export SPACE_API_URL=http://127.0.0.1:8080 SPACE_DATA_PLANE_URL=http://127.0.0.1:8090 SPACE_TEMPLATE=seed:latest
umask 077
SPACE_API_TOKEN=$(curl --noproxy '*' --fail-with-body --silent --show-error --max-time 15 \
  -H 'Content-Type: application/json' -d '{"email":"root@local","password":"password"}' \
  "$SPACE_API_URL/auth/v1/sessions" | jq -er '.token')
export SPACE_API_TOKEN
printf 'export SPACE_API_URL=%q\nexport SPACE_DATA_PLANE_URL=%q\nexport SPACE_TEMPLATE=%q\nexport SPACE_API_TOKEN=%q\n' \
  "$SPACE_API_URL" "$SPACE_DATA_PLANE_URL" "$SPACE_TEMPLATE" "$SPACE_API_TOKEN" >"$HOME/space.env"
mkdir -p "$HOME/evidence"
export TMPDIR="$HOME/evidence"
printf 'y\n' | python3 examples/first_sandbox.py >"$HOME/evidence/first-sandbox.log" 2>&1
echo 'SPACE installation and first sandbox passed.'
SETUP
