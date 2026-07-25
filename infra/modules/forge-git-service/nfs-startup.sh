#!/bin/bash
set -euo pipefail

metadata_url="http://metadata.google.internal/computeMetadata/v1/instance/attributes"
metadata() {
  curl -fsS -H "Metadata-Flavor: Google" "$metadata_url/$1"
}

repository_disk_name="$(metadata repository-disk-name)"
mount_path="$(metadata export-path)"
allowed_cidr="$(metadata allowed-cidr)"

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends nfs-kernel-server

device="/dev/disk/by-id/google-$repository_disk_name"
while [[ ! -b "$device" ]]; do
  sleep 1
done

if ! blkid "$device" >/dev/null 2>&1; then
  mkfs.ext4 -F "$device"
fi

mkdir -p "$mount_path"
uuid="$(blkid -s UUID -o value "$device")"
if ! grep -q "UUID=$uuid " /etc/fstab; then
  printf 'UUID=%s %s ext4 defaults,nofail 0 2\n' "$uuid" "$mount_path" >>/etc/fstab
fi
mountpoint -q "$mount_path" || mount "$mount_path"
chown 1000:1000 "$mount_path"
chmod 0770 "$mount_path"

install -d -m 0755 /etc/nfs.conf.d
printf '%s\n' \
  '[mountd]' \
  'port=20048' \
  '[statd]' \
  'port=32765' \
  'outgoing-port=32766' \
  >/etc/nfs.conf.d/forge-git.conf

printf '%s %s(rw,sync,no_subtree_check,all_squash,anonuid=1000,anongid=1000,fsid=0)\n' \
  "$mount_path" "$allowed_cidr" \
  >/etc/exports.d/forge-git.exports

exportfs -ra
systemctl enable --now nfs-kernel-server
