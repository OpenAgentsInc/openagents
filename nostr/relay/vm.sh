#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: gcloud, ssh and scp around the relay's VM)
# The Coax relay on a GCE VM for relay.openagents.com (#13361; runbook
# nostr/book/ch103-replace-relay-openagents-com.md). Every resource it makes:
#   VM        oa-relay-1, e2-medium, Debian 12, us-central1-a, network tag oa-relay
#   address   oa-relay-1-ip (static external, us-central1)
#   firewall  oa-relay-1-wss: tcp 80 and 443 from anywhere to tag oa-relay
#             (443 is the relay, wss:// and NIP-11; 80 only answers ACME renewals)
#   disk      oa-relay-1-data, 20 GB pd-balanced at /var/lib/nostr-relay (the store)
#   snapshots oa-relay-1-hourly: the data disk every hour, kept 2 days
#   bucket    gs://openagentsgemini-relay-checkpoint: versioned, 7-day retention
#             policy, locked (decision 4): the freshness checkpoint's copy
#   account   oa-relay-1@openagentsgemini.iam.gserviceaccount.com: may add and read
#             objects in that bucket, nothing else
# The checkpoint itself is on the boot disk (/var/lib/nostr-relay-cp/relay.cp),
# which the snapshots don't reach, and oa-relay-cp.timer copies it to the bucket
# every minute it changes, so a restored older store is refused (store_fresh.coax).
#
#   nostr/relay/vm.sh create            the resources above (each one skipped if it exists)
#   nostr/relay/vm.sh setup             packages, user, data disk, units (idempotent)
#   nostr/relay/vm.sh ship BINARY       install a relay binary (nostr/relay/build.sh's, built on a
#                                       Debian 12 build host) as /opt/nostr-relay/relay-SHA, then current
#   nostr/relay/vm.sh start | stop | restart | status | logs [N]
#   nostr/relay/vm.sh ssh -- CMD...     run CMD on the VM (as root)
#
# Cost per month (list prices, us-central1): e2-medium $24.46; boot disk 10 GB and data
# disk 20 GB pd-balanced $3.00; the address in use $3.65; hourly snapshots of a store under
# 1 GB, a few cents; the bucket, under a cent. About $31.
set -euo pipefail
export LC_ALL=C
[[ -n "${CLOUDSDK_CONFIG:-}" ]] || export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
project=openagentsgemini; region=us-central1; zone=us-central1-a
vm=oa-relay-1; machine=e2-medium; ip=oa-relay-1-ip; fw=oa-relay-1-wss; tag=oa-relay
disk=oa-relay-1-data; sched=oa-relay-1-hourly; bucket=openagentsgemini-relay-checkpoint
sa=oa-relay-1; sa_email="$sa@$project.iam.gserviceaccount.com"
g() { gcloud --project "$project" "$@"; }
on() { g compute ssh "$vm" --zone "$zone" --tunnel-through-iap --quiet --command "sudo bash -c $(printf '%q' "$1")"; }
exists() { "$@" >/dev/null 2>&1; }

create() {
  exists g compute addresses describe "$ip" --region "$region" || g compute addresses create "$ip" --region "$region"
  exists g compute firewall-rules describe "$fw" || g compute firewall-rules create "$fw" --network default \
    --direction INGRESS --allow tcp:80,tcp:443 --source-ranges 0.0.0.0/0 --target-tags "$tag"
  exists g iam service-accounts describe "$sa_email" || g iam service-accounts create "$sa" --display-name "Coax relay oa-relay-1"
  if ! exists gcloud storage buckets describe "gs://$bucket"; then
    gcloud storage buckets create "gs://$bucket" --project "$project" --location "$region" \
      --uniform-bucket-level-access --public-access-prevention --retention-period 7d
    gcloud storage buckets update "gs://$bucket" --versioning
    printf '{"rule":[{"action":{"type":"Delete"},"condition":{"isLive":false,"daysSinceNoncurrentTime":8}}]}\n' > "${TMPDIR:-/tmp}/oa-relay-lifecycle.json"
    gcloud storage buckets update "gs://$bucket" --lifecycle-file "${TMPDIR:-/tmp}/oa-relay-lifecycle.json"
    gcloud storage buckets update "gs://$bucket" --lock-retention-period
  fi
  gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$sa_email" --role roles/storage.objectCreator >/dev/null
  gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$sa_email" --role roles/storage.objectViewer >/dev/null
  exists g compute resource-policies describe "$sched" --region "$region" || g compute resource-policies create snapshot-schedule "$sched" \
    --region "$region" --hourly-schedule 1 --start-time 00:00 --max-retention-days 2 --on-source-disk-delete keep-auto-snapshots
  exists g compute disks describe "$disk" --zone "$zone" || g compute disks create "$disk" --zone "$zone" --size 20GB --type pd-balanced
  g compute disks add-resource-policies "$disk" --zone "$zone" --resource-policies "$sched" 2>/dev/null || true
  exists g compute instances describe "$vm" --zone "$zone" || g compute instances create "$vm" --zone "$zone" \
    --machine-type "$machine" --image-family debian-12 --image-project debian-cloud --boot-disk-size 10GB \
    --boot-disk-type pd-balanced --address "$ip" --tags "$tag" --service-account "$sa_email" \
    --scopes https://www.googleapis.com/auth/devstorage.read_write --disk "name=$disk,device-name=relay-data,mode=rw,boot=no" \
    --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring
  g compute addresses describe "$ip" --region "$region" --format 'value(address)'
}

setup() {
  on '
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq && apt-get install -y -qq libssl3 certbot >/dev/null
id nostr-relay >/dev/null 2>&1 || useradd --system --home /var/lib/nostr-relay --shell /usr/sbin/nologin nostr-relay
dev=/dev/disk/by-id/google-relay-data
blkid "$dev" >/dev/null 2>&1 || mkfs.ext4 -q -L relaydata "$dev"
mkdir -p /var/lib/nostr-relay
grep -q relaydata /etc/fstab || echo "LABEL=relaydata /var/lib/nostr-relay ext4 defaults,nofail,discard 0 2" >> /etc/fstab
mountpoint -q /var/lib/nostr-relay || mount /var/lib/nostr-relay
install -d -o nostr-relay -g nostr-relay -m 0700 /var/lib/nostr-relay/store /var/lib/nostr-relay-cp /etc/nostr-relay/tls
install -d -m 0755 /opt/nostr-relay /etc/letsencrypt/renewal-hooks/deploy
cat > /etc/letsencrypt/renewal-hooks/deploy/nostr-relay <<EOF
#!/bin/sh
# Copies the renewed certificate where the relay (user nostr-relay) reads it, 0400, and restarts it.
set -e
src=/etc/letsencrypt/live/relay.openagents.com
install -o nostr-relay -g nostr-relay -m 0400 "\$src/fullchain.pem" /etc/nostr-relay/tls/fullchain.pem
install -o nostr-relay -g nostr-relay -m 0400 "\$src/privkey.pem" /etc/nostr-relay/tls/privkey.pem
systemctl try-restart nostr-relay
EOF
chmod 0755 /etc/letsencrypt/renewal-hooks/deploy/nostr-relay
cat > /usr/local/bin/oa-relay-cp-push <<EOF
#!/bin/sh
# Copies the freshness checkpoint to its versioned, retention-locked object when it changed.
set -e
cp=/var/lib/nostr-relay-cp/relay.cp; last=/var/lib/nostr-relay-cp/.pushed
[ -f "\$cp" ] || exit 0
if [ -f "\$last" ] && cmp -s "\$cp" "\$last"; then exit 0; fi
cp "\$cp" "\$last.tmp"
gcloud storage cp --quiet "\$last.tmp" gs://'"$bucket"'/relay.cp
mv "\$last.tmp" "\$last"
EOF
chmod 0755 /usr/local/bin/oa-relay-cp-push
cat > /etc/systemd/system/oa-relay-cp.service <<EOF
[Unit]
Description=Copy the Coax relay freshness checkpoint to GCS
[Service]
Type=oneshot
ExecStart=/usr/local/bin/oa-relay-cp-push
EOF
cat > /etc/systemd/system/oa-relay-cp.timer <<EOF
[Unit]
Description=Copy the Coax relay freshness checkpoint to GCS every minute
[Timer]
OnBootSec=60
OnUnitActiveSec=60
[Install]
WantedBy=timers.target
EOF
[ -f /etc/nostr-relay/flags ] || echo "RELAY_FLAGS=" > /etc/nostr-relay/flags
cat > /etc/systemd/system/nostr-relay.service <<EOF
[Unit]
Description=Coax Nostr relay (relay.openagents.com, #13361)
After=network-online.target
Wants=network-online.target
RequiresMountsFor=/var/lib/nostr-relay
[Service]
User=nostr-relay
Group=nostr-relay
EnvironmentFile=/etc/nostr-relay/flags
ExecStart=/opt/nostr-relay/current/relay --listen 0.0.0.0 --port 443 --url wss://relay.openagents.com --store /var/lib/nostr-relay/store/relay.store --checkpoint /var/lib/nostr-relay-cp/relay.cp --tls-cert /etc/nostr-relay/tls/fullchain.pem --tls-key /etc/nostr-relay/tls/privkey.pem \$RELAY_FLAGS
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
NoNewPrivileges=true
LimitNOFILE=8192
Restart=always
RestartSec=2
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable --now oa-relay-cp.timer >/dev/null 2>&1
echo setup done'
}

ship() {
  local bin="$1" sha
  sha="$(shasum -a 256 "$bin" | cut -c1-12)"
  g compute scp "$bin" "$vm:/tmp/relay-$sha" --zone "$zone" --tunnel-through-iap --quiet
  on "install -m 0755 /tmp/relay-$sha /opt/nostr-relay/relay-$sha && rm -f /tmp/relay-$sha && mkdir -p /opt/nostr-relay/b-$sha && ln -sfn /opt/nostr-relay/relay-$sha /opt/nostr-relay/b-$sha/relay && ln -sfn /opt/nostr-relay/b-$sha /opt/nostr-relay/current.new && mv -T /opt/nostr-relay/current.new /opt/nostr-relay/current && echo installed relay-$sha"
}

case "${1:-}" in
  create) create ;;
  setup) setup ;;
  ship) ship "${2:?BINARY}" ;;
  start|stop|restart) on "systemctl $1 nostr-relay" ;;
  status) on "systemctl status nostr-relay --no-pager -n 0; systemctl is-active oa-relay-cp.timer" ;;
  logs) on "journalctl -u nostr-relay --no-pager -n ${2:-40}" ;;
  ssh) shift; [[ "${1:-}" == "--" ]] && shift; on "$*" ;;
  *) sed -n '2,30p' "$0"; exit 2 ;;
esac
