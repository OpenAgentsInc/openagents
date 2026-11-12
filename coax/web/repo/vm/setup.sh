#!/bin/sh
# Firm reason: 5 (a thin wrapper: installs the repo VM's packages, data disk, users and units, as root)
# Run on the repo VM by vm-deploy.sh create and setup (docs/deployment/repo-vm.md, #13326), from a
# directory holding this file and oa-repo@.service. It can run again: every step leaves what is
# already there, and it never formats a disk that holds a filesystem.
#
#   sudo OA_REPO_REPOS='slug=url;slug=url' sh setup.sh
set -eu
export DEBIAN_FRONTEND=noninteractive
repos="${OA_REPO_REPOS:-openagents=https://github.com/OpenAgentsInc/openagents.git}"
data=/var/lib/oa-repo
dev=/dev/disk/by-id/google-oa-repo-data

need=""
for p in git curl ca-certificates; do dpkg -s "$p" >/dev/null 2>&1 || need="$need $p"; done
# shellcheck disable=SC2086  # $need is a word list
if [ -n "$need" ]; then apt-get update -q >/dev/null && apt-get install -y -q --no-install-recommends $need >/dev/null; fi

# The data disk: ext4, mounted by UUID at /var/lib/oa-repo. Formatted only when it holds nothing.
[ -b "$dev" ] || { echo "setup: no data disk at $dev" >&2; exit 1; }
if ! blkid "$dev" >/dev/null 2>&1; then
  echo "setup: formatting the empty data disk"
  mkfs.ext4 -q -m 0 -L oa-repo-data -E lazy_itable_init=0,lazy_journal_init=0,discard "$dev"
fi
uuid="$(blkid -s UUID -o value "$dev")"
install -d -m 755 "$data"
grep -q "UUID=$uuid" /etc/fstab || echo "UUID=$uuid $data ext4 defaults,discard,nofail 0 2" >> /etc/fstab
mountpoint -q "$data" || mount "$data"
mountpoint -q "$data" || { echo "setup: $data is not mounted" >&2; exit 1; }

# One unprivileged user per unit. Production's tree is the disk's root; staging's is a directory in
# it that production's user can't enter, and production's files are its own (the unit's UMask 0077).
for u in production staging; do
  id "oa-repo-$u" >/dev/null 2>&1 || useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "oa-repo-$u"
done
chown oa-repo-production:oa-repo-production "$data"; chmod 711 "$data"
install -d -m 700 -o oa-repo-staging -g oa-repo-staging "$data/staging"
install -d -m 700 -o oa-repo-production -g oa-repo-production "$data/.home"
install -d -m 700 -o oa-repo-staging -g oa-repo-staging "$data/staging/.home"

install -d -m 755 /opt/oa-repo /opt/oa-repo/releases
install -d -m 755 /etc/oa-repo
for u in production staging; do
  if [ "$u" = production ]; then port=8080 dir="$data"; else port=8081 dir="$data/staging"; fi
  printf 'OA_REPO_LISTEN=0.0.0.0:%s\nOA_REPO_DIR=%s\nOA_REPO_REPOS="%s"\nOA_GIT=/usr/bin/git\nHOME=%s/.home\nGIT_TERMINAL_PROMPT=0\n' \
    "$port" "$dir" "$repos" "$dir" > "/etc/oa-repo/$u.env.new"
  chmod 644 "/etc/oa-repo/$u.env.new"; mv "/etc/oa-repo/$u.env.new" "/etc/oa-repo/$u.env"
  [ -f "/etc/oa-repo/$u.secret.env" ] || install -m 600 /dev/null "/etc/oa-repo/$u.secret.env"
done
install -m 644 oa-repo@.service /etc/systemd/system/oa-repo@.service
systemctl daemon-reload
echo "setup: done"
