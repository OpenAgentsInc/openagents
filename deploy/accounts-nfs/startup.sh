#!/bin/bash
# The account-store NFS server's startup script
# (docs/deployment/account-storage.md). GCE runs it as root at every boot
# of oa-accounts-nfs-<env>; running it again changes nothing.
#
# It formats the data disk (device name `accounts`) the first time only,
# mounts it at /srv/accounts, makes the two directories the web stack
# mounts, `stack` (the gateway and the chat worker) and `web` (the site's
# saved own-Claude keys), owned by the containers' user (uid 10001, mode
# 0700), and exports /srv/accounts over NFS to the Cloud Run subnet named
# by the instance metadata key `allowed-cidr`. `sync` means a write is on
# the disk before the client is told it landed.
set -euo pipefail

meta() {
    curl -fsS -H 'Metadata-Flavor: Google' \
        "http://metadata.google.internal/computeMetadata/v1/instance/attributes/$1"
}

cidr=$(meta allowed-cidr)
case "$cidr" in
    */*) ;;
    *)
        echo "allowed-cidr metadata is missing or not a CIDR" >&2
        exit 1
        ;;
esac
dev=/dev/disk/by-id/google-accounts
root=/srv/accounts

if ! blkid "$dev" > /dev/null 2>&1; then
    mkfs.ext4 -m 0 -E lazy_itable_init=0,lazy_journal_init=0,discard -L accounts "$dev"
fi
mkdir -p "$root"
if ! grep -q " $root " /etc/fstab; then
    echo "LABEL=accounts $root ext4 defaults,discard,nofail 0 2" >> /etc/fstab
fi
mountpoint -q "$root" || mount "$root"

if ! command -v exportfs > /dev/null 2>&1; then
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends nfs-kernel-server
fi

for dir in stack web; do
    mkdir -p "$root/$dir"
    chown 10001:10001 "$root/$dir"
    chmod 0700 "$root/$dir"
done
chmod 0755 "$root"

# mountd on a fixed port, so the firewall can name it for NFSv3 clients.
mkdir -p /etc/nfs.conf.d /etc/exports.d
printf '[mountd]\nport=20048\n' > /etc/nfs.conf.d/accounts.conf
printf '%s %s(rw,sync,no_subtree_check,root_squash)\n' "$root" "$cidr" \
    > /etc/exports.d/accounts.exports
systemctl enable nfs-server
systemctl restart nfs-server
exportfs -ra
echo "accounts NFS: $root exported to $cidr"
