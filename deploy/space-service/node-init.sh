#!/usr/bin/env bash
# Prepare an owned node. Join credentials are supplied separately as root files.
set -euo pipefail
space_role=${1:?Choose control or worker.}
case "$space_role" in control|worker) ;; *) exit 2 ;; esac
[[ $(id -u) == 0 ]] || { echo 'Run node setup as root.' >&2; exit 1; }
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y --no-install-recommends ca-certificates curl btrfs-progs nftables jq python3
systemctl disable --now unattended-upgrades.service apt-daily.timer apt-daily-upgrade.timer 2>/dev/null || true

space_disk=/dev/disk/by-id/google-space-data
[[ -b $space_disk ]] || { echo 'The owned data disk is unavailable.' >&2; exit 1; }
space_fs=$(blkid -s TYPE -o value "$space_disk" || true)
if [[ -z $space_fs ]]; then
  # Refuse a partitioned or nonempty disk rather than treating an unfamiliar
  # signature as permission to format it.
  [[ -z $(lsblk -nr -o PTTYPE "$space_disk" | tr -d '[:space:]') ]]
  wipefs --no-act --json "$space_disk" | python3 -c 'import json,sys; assert not json.load(sys.stdin).get("signatures")'
  mkfs.btrfs -L oa-space-data "$space_disk"
elif [[ $space_fs != btrfs ]]; then
  echo 'The data disk uses an unexpected filesystem.' >&2
  exit 1
fi
mkdir -p /space
space_uuid=$(blkid -s UUID -o value "$space_disk")
if ! mountpoint -q /space; then
  mount -o noatime,compress=zstd:3 "UUID=$space_uuid" /space
fi
[[ $(findmnt -n -o UUID /space) == "$space_uuid" ]]
grep -q "^UUID=$space_uuid /space " /etc/fstab || printf 'UUID=%s /space btrfs noatime,compress=zstd:3 0 0\n' "$space_uuid" >> /etc/fstab
mkdir -p /space/config
chmod 0700 /space/config

space_k3s_version='v1.37.1+k3s1'
space_k3s_sha=92b94582eb7b34a8cf532e6006842b9ed215a6783cf56a3dd6271fd35243b2c0
if ! printf '%s  /usr/local/bin/k3s\n' "$space_k3s_sha" | sha256sum --check --status; then
  curl --fail --silent --show-error --location --retry 3 \
    "https://github.com/k3s-io/k3s/releases/download/$space_k3s_version/k3s" -o /space/config/k3s.new
  printf '%s  /space/config/k3s.new\n' "$space_k3s_sha" | sha256sum --check
  install -m 0755 /space/config/k3s.new /usr/local/bin/k3s
  rm /space/config/k3s.new
fi
if [[ $space_role == worker ]]; then
  if [[ -s /run/oa-space/k3s-join ]]; then
    install -m 0600 /run/oa-space/k3s-join /space/config/k3s-join
    rm /run/oa-space/k3s-join
  fi
  test -s /space/config/k3s-join
  python3 - <<'PY'
import fcntl, os
with open('/dev/kvm', 'r+b', buffering=0) as kvm:
    assert fcntl.ioctl(kvm, 0xae00, 0) == 12
    vm = fcntl.ioctl(kvm, 0xae01, 0)
    os.close(vm)
print('KVM API and VM creation passed.')
PY
  space_start='agent --server=https://10.219.14.10:6443 --token-file=/space/config/k3s-join --node-ip=10.219.14.11 --node-name=oa-space-service-worker --node-label=space.pplx.net/node-pool=sandbox'
else
  space_start='server --data-dir=/space/k3s --advertise-address=10.219.14.10 --node-ip=10.219.14.10 --node-name=oa-space-service-control --tls-san=10.219.14.10 --cluster-cidr=10.220.0.0/16 --service-cidr=10.221.0.0/16 --flannel-backend=wireguard-native --disable=traefik,servicelb,local-storage --secrets-encryption --node-label=space.pplx.net/infra=true --write-kubeconfig-mode=600'
fi
space_unit=/etc/systemd/system/oa-space-k3s.service
cat >/space/config/k3s-unit.new <<EOF
[Unit]
Description=OpenAgents maintained SPACE cluster
After=network-online.target
Wants=network-online.target
RequiresMountsFor=/space

[Service]
Type=notify
ExecStart=/usr/local/bin/k3s $space_start
Restart=on-failure
RestartSec=5
LimitNOFILE=1048576
LimitNPROC=infinity
TasksMax=infinity
Delegate=yes
KillMode=process

[Install]
WantedBy=multi-user.target
EOF
if ! cmp -s /space/config/k3s-unit.new "$space_unit"; then
  if systemctl is-active --quiet oa-space-k3s; then
    echo 'The running cluster configuration differs. Drain before changing it.' >&2
    exit 1
  fi
  install -m 0644 /space/config/k3s-unit.new "$space_unit"
fi
rm /space/config/k3s-unit.new
systemctl daemon-reload
systemctl enable --now oa-space-k3s
systemctl is-active --quiet oa-space-k3s
printf '%s node prepared with k3s %s; data disk %s\n' "$space_role" "$space_k3s_version" "$space_uuid"
