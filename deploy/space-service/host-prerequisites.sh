#!/usr/bin/env bash
# Adapt the pinned upstream host identities and layout to an owned GCE node.
set -euo pipefail
[[ $(id -u) == 0 ]] && mountpoint -q /space
case $(hostname -s) in oa-space-service-worker|oa-space-service-builder) ;; *) exit 2 ;; esac
install -d -m 0700 /space/config
space_identities=/space/config/node-identities-de87b436.sh
curl --fail --silent --show-error --location --retry 3 https://raw.githubusercontent.com/perplexityai/SPACE/de87b436c35dac056564bdd5b2a113c9046e189b/infra/space/identity/node-identities.sh -o "$space_identities"
printf '%s  %s\n' e8702503d75ab73e6abb1ed40ce874cdf87cdf034f7e56c8ce7a4075583e7d7b "$space_identities" | sha256sum --check
source "$space_identities"
for spec in $SPACE_GROUP_SPECS; do
  group=${spec%%:*}; gid=${spec##*:}
  if getent group "$group" >/dev/null; then
    [[ $(getent group "$group" | cut -d: -f3) == "$gid" ]]
  else
    ! getent group "$gid" >/dev/null
    groupadd -g "$gid" "$group"
  fi
done
while IFS=: read -r alias user uid group; do
  [[ -n $user ]] || continue
  if id "$user" >/dev/null 2>&1; then
    [[ $(id -u "$user") == "$uid" && $(id -gn "$user") == "$group" ]]
  else
    ! getent passwd "$uid" >/dev/null
    useradd --system --no-create-home --uid "$uid" --gid "$group" --shell /usr/sbin/nologin "$user"
  fi
done <<< "$SPACE_USER_SPECS"
# SPACE peer authorization resolves the local role accounts `dev` and `deploy`
# by name in every service, including each sandbox's net gateway. On GCE an
# unknown name falls through to the OS Login NSS module, which asks the metadata
# server; the gateway's sandbox network cannot reach it, so each lookup waited
# about 4.5 s and every boot and wake waited about 9 s (#12144). Locked local
# accounts answer from /etc/passwd. No process runs as them and they can't log in.
for user in dev deploy; do
  getent passwd "$user" >/dev/null || useradd --system --no-create-home --user-group --shell /usr/sbin/nologin "$user"
  passwd -l "$user" >/dev/null
done
install -d -m 0755 /space/etc/deployed /space/run/node-manager /space/run/firecracker/sandboxes /space/cache/firecracker/jails /var/run/netns
install -d -m 0750 -o root -g space-sbxclients /space/run/dependency-drain /space/run/dependency-drain/sandbox-manager
install -d -m 0755 -o space-sbxmgr -g space-sbxmgr /space/run/sandbox-host-control
install -d -m 0750 -o space-sbxmgr -g space-sbxclients /space/run/sandbox-manager /space/run/credential-manager
install -d -m 0750 -o space-tmplmgr -g space-tmplclients /space/run/template-manager
install -d -m 0750 -o space-rtmgr -g space-rtmgrclients /space/run/runtime-manager
install -d -m 3775 -o root -g space-cache-rw /space/cache /space/cache/firecracker
install -d -m 2775 -o root -g space-cache-rw /space/cache/firecracker/templates /space/cache/firecracker/sandboxes /space/cache/firecracker/builds
for gate in capability-v1.lock admission-v1.lock work-v1.lock; do
  path=/space/run/dependency-drain/sandbox-manager/$gate
  [[ -e $path ]] || install -m 0660 -o root -g space-sbxclients /dev/null "$path"
done
# Never replace a saved policy while computers might reference it.
if [[ ! -e /space/etc/sandbox.toml ]]; then
  printf 'max_memory_mib = 4096\n' >/space/etc/sandbox.toml
  chmod 0644 /space/etc/sandbox.toml
fi
install -d -m 0755 /usr/local/lib/space
cat >/usr/local/lib/space/cgroup-setup.sh <<'CGROUP'
#!/usr/bin/env bash
set -euo pipefail
printf '+io\n' >/sys/fs/cgroup/cgroup.subtree_control
mkdir -p /sys/fs/cgroup/space
printf '+cpu +memory +io +pids\n' >/sys/fs/cgroup/space/cgroup.subtree_control
printf '400000 100000\n' >/sys/fs/cgroup/space/cpu.max
printf '8589934592\n' >/sys/fs/cgroup/space/memory.max
printf '0\n' >/sys/fs/cgroup/space/memory.swap.max
printf '4096\n' >/sys/fs/cgroup/space/pids.max
space_device=$(lsblk -dn -o MAJ:MIN /dev/disk/by-id/google-space-data)
printf '%s rbps=67108864 wbps=67108864 riops=2000 wiops=2000\n' "$space_device" >/sys/fs/cgroup/space/io.max
CGROUP
chmod 0755 /usr/local/lib/space/cgroup-setup.sh
cat >/etc/systemd/system/space-cgroup.service <<'UNIT'
[Unit]
Description=Prepare the bounded SPACE guest cgroup
After=local-fs.target
Before=space-runtime-supervisor.service
RequiresMountsFor=/space
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/lib/space/cgroup-setup.sh
[Install]
WantedBy=multi-user.target
UNIT
modprobe tun
modprobe dm_crypt
modprobe dm_snapshot
install -d /mnt/hugepages
mountpoint -q /mnt/hugepages || mount -t hugetlbfs none /mnt/hugepages
printf 'tun\ndm_crypt\ndm_snapshot\n' >/etc/modules-load.d/oa-space.conf
systemctl daemon-reload
systemctl enable --now space-cgroup.service
printf '%s\n' 'Pinned host identities, permissions, and finite guest cgroup installed.'
