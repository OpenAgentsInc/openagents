#!/usr/bin/env bash
# Give OCI builds their own bounded store and runtime, apart from Kubernetes.
set -euo pipefail
[[ $(id -u) == 0 && $(hostname -s) == oa-space-service-builder ]]
mountpoint -q /space
test -x /space/k3s/data/current/bin/containerd
if [[ -e /var/lib/containerd && ! -L /var/lib/containerd ]]; then
  [[ -d /var/lib/containerd && -z $(ls -A /var/lib/containerd) ]]
  rmdir /var/lib/containerd
fi
install -d -m 0700 /space/build-containerd
[[ -L /var/lib/containerd ]] || ln -s /space/build-containerd /var/lib/containerd
[[ $(readlink -f /var/lib/containerd) == /space/build-containerd ]]
cat >/space/config/build-containerd.toml <<'CONFIG'
version = 3
root = "/var/lib/containerd"
state = "/run/containerd"
disabled_plugins = ["io.containerd.cri.v1.images", "io.containerd.cri.v1.runtime"]
[grpc]
  address = "/run/containerd/containerd.sock"
CONFIG
cat >/etc/systemd/system/oa-space-build-containerd.service <<'UNIT'
[Unit]
Description=Isolated SPACE template build runtime
After=oa-space-k3s.service
Requires=oa-space-k3s.service
RequiresMountsFor=/space
[Service]
Type=notify
Environment=PATH=/space/k3s/data/current/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
ExecStart=/space/k3s/data/current/bin/containerd --config /space/config/build-containerd.toml
Restart=on-failure
RestartSec=5
Delegate=yes
KillMode=process
TasksMax=4096
MemoryMax=2G
CPUQuota=200%
[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable --now oa-space-build-containerd
systemctl is-active --quiet oa-space-build-containerd
printf '%s\n' 'Builder OCI runtime started on its separate durable-disk store.'
