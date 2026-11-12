#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: gcloud, ssh and scp around the relay's VM)
# The Coax relay on a GCE VM for relay.openagents.com (#13361; runbook
# nostr/book/ch103-replace-relay-openagents-com.md). Every resource it makes:
#   VM        oa-relay-1, e2-small, Debian 12, us-central1-a, network tag oa-relay
#   address   oa-relay-1-ip (static external, us-central1)
#   firewall  oa-relay-1-wss: tcp 80 and 443 from anywhere to tag oa-relay
#             (443 is the relay, wss:// and NIP-11; 80 only answers ACME renewals)
#   disk      oa-relay-1-data, 10 GB pd-balanced at /var/lib/nostr-relay (the store)
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
#   nostr/relay/vm.sh cert              a certificate for relay, relay-staging and relay-coax.openagents.com
#                                       by DNS-01 (below); cert-renew-by-http moves renewals to HTTP-01
#   nostr/relay/vm.sh start | stop | restart | status | logs [N]
#   nostr/relay/vm.sh ssh -- CMD...     run CMD on the VM (as root)
#   nostr/relay/vm.sh staging-rust up | down   the rollback rehearsal's load balancer in front of the
#                                       Rust relay for relay-staging.openagents.com (below)
#
# Cost per month (list prices, us-central1): e2-small $12.23; boot disk 10 GB and data
# disk 10 GB pd-balanced $2.00; the address in use $3.65; hourly snapshots of a store under
# 1 GB, a few cents; the bucket, under a cent. About $18. GCP is over its monthly cap, so
# nothing larger unless the store's size needs it (#13361: the store is in memory, 2 KiB a
# record). staging-rust is billed by the hour (its forwarding
# rule $0.025) and is deleted after the rehearsal, so it carries no monthly cost.
set -euo pipefail
export LC_ALL=C
[[ -n "${CLOUDSDK_CONFIG:-}" ]] || export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
project=openagentsgemini; region=us-central1; zone=us-central1-a
vm=oa-relay-1; machine=e2-small; ip=oa-relay-1-ip; fw=oa-relay-1-wss; tag=oa-relay
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
    printf '{"rule":[{"action":{"type":"Delete"},"condition":{"age":8}}]}\n' > "${TMPDIR:-/tmp}/oa-relay-lifecycle.json"
    gcloud storage buckets update "gs://$bucket" --lifecycle-file "${TMPDIR:-/tmp}/oa-relay-lifecycle.json"
    yes y | gcloud storage buckets update "gs://$bucket" --lock-retention-period   # it asks, and --quiet answers no
  fi
  gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$sa_email" --role roles/storage.objectCreator >/dev/null
  gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$sa_email" --role roles/storage.objectViewer >/dev/null
  exists g compute resource-policies describe "$sched" --region "$region" || g compute resource-policies create snapshot-schedule "$sched" \
    --region "$region" --hourly-schedule 1 --start-time 00:00 --max-retention-days 2 --on-source-disk-delete keep-auto-snapshots
  exists g compute disks describe "$disk" --zone "$zone" || g compute disks create "$disk" --zone "$zone" --size 10GB --type pd-balanced
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
# A new object each time: the retention lock keeps every object 7 days, so none
# can be overwritten; the newest name under relay.cp/ is the current checkpoint.
gcloud storage cp --quiet "\$last.tmp" gs://'"$bucket"'/relay.cp/\$(date -u +%Y%m%dT%H%M%SZ)
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

# A Let's Encrypt certificate for the relay's names by DNS-01: certbot runs on the VM (the key
# never leaves it); its manual hook writes each challenge to /root/acme/pending-NAME and waits;
# this side puts the TXT record _acme-challenge.NAME at Cloudflare (the openagents.com zone, with
# the token in ~/work/.secrets/cloudflare-openagents.env), waits until 1.1.1.1 serves it, and
# answers. The TXT records are removed after. Once relay.openagents.com points here,
# cert-renew-by-http moves renewals to HTTP-01 on port 80, which certbot.timer runs alone.
names=(relay.openagents.com relay-staging.openagents.com relay-coax.openagents.com)
cf() {  # METHOD PATH [JSON]: one Cloudflare API call against the openagents.com zone
  local zone=bd33d951ee951a7c18fa4ab2ddcbe3a7
  ( . "$HOME/work/.secrets/cloudflare-openagents.env"
    curl -fsS -X "$1" -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" -H 'Content-Type: application/json' \
      "https://api.cloudflare.com/client/v4/zones/$zone$2" ${3:+--data "$3"} )
}
cert() {
  on 'mkdir -p /root/acme && rm -f /root/acme/pending-* /root/acme/ready-* &&
printf "%s\n" "#!/bin/sh" "echo \"\$CERTBOT_VALIDATION\" > /root/acme/pending-\$CERTBOT_DOMAIN" "i=0; while [ ! -f /root/acme/ready-\$CERTBOT_DOMAIN ] && [ \$i -lt 900 ]; do sleep 1; i=\$((i+1)); done" > /root/acme/hook && chmod 0700 /root/acme/hook'
  local d args=""; for d in "${names[@]}"; do args="$args -d $d"; done
  on "nohup certbot certonly --non-interactive --agree-tos --register-unsafely-without-email --manual --preferred-challenges dns --manual-auth-hook /root/acme/hook --cert-name relay.openagents.com $args > /root/acme/certbot.log 2>&1 &"
  local done_n=0 ids=() v rec
  while (( done_n < ${#names[@]} )); do
    for d in "${names[@]}"; do
      v="$(on "cat /root/acme/pending-$d 2>/dev/null; rm -f /root/acme/pending-$d" | tr -d '\r\n')"
      [[ -n "$v" ]] || continue
      rec="$(cf POST /dns_records "{\"type\":\"TXT\",\"name\":\"_acme-challenge.$d\",\"content\":\"\\\"$v\\\"\",\"ttl\":60}" | python3 -c 'import sys,json;print(json.load(sys.stdin)["result"]["id"])')"
      ids+=("$rec"); echo "TXT _acme-challenge.$d (Cloudflare record $rec)"
      until dig +short TXT "_acme-challenge.$d" @1.1.1.1 | grep -q -- "$v"; do sleep 5; done
      on "touch /root/acme/ready-$d"; done_n=$((done_n + 1))
    done
    sleep 3
  done
  until on 'if pgrep -x certbot >/dev/null; then exit 1; fi' >/dev/null 2>&1; do sleep 5; done
  on 'tail -5 /root/acme/certbot.log; test -f /etc/letsencrypt/live/relay.openagents.com/fullchain.pem && /etc/letsencrypt/renewal-hooks/deploy/nostr-relay && echo "certificate installed"'
  for rec in "${ids[@]}"; do cf DELETE "/dns_records/$rec" >/dev/null && echo "removed TXT record $rec"; done
}
renew_by_http() {
  on "certbot certonly --non-interactive --standalone --preferred-challenges http --cert-name relay.openagents.com --force-renewal $(printf -- '-d %s ' "${names[@]}")"
}

# The rollback rehearsal's other side (step 7): relay-staging.openagents.com can't be a Cloud
# Run domain mapping (the automation account owns no verified domain), so a global external
# load balancer stands in front of the Rust relay's Cloud Run service for it, with a Certificate
# Manager certificate authorized by DNS. Every resource is named relay-staging-rust-* and
# `staging-rust down` deletes them all (only these; the service itself is untouched).
sr=relay-staging-rust
staging_rust_up() {
  local auth_rec
  exists g compute network-endpoint-groups describe "$sr-neg" --region "$region" || g compute network-endpoint-groups create "$sr-neg" \
    --region "$region" --network-endpoint-type serverless --cloud-run-service openagents-nostr-relay
  exists g compute backend-services describe "$sr-bs" --global || { g compute backend-services create "$sr-bs" --global \
    --load-balancing-scheme EXTERNAL_MANAGED && g compute backend-services add-backend "$sr-bs" --global \
    --network-endpoint-group "$sr-neg" --network-endpoint-group-region "$region"; }
  exists g compute url-maps describe "$sr-map" --global || g compute url-maps create "$sr-map" --global --default-service "$sr-bs"
  exists g certificate-manager dns-authorizations describe "$sr-auth" || g certificate-manager dns-authorizations create "$sr-auth" --domain relay-staging.openagents.com
  auth_rec="$(g certificate-manager dns-authorizations describe "$sr-auth" --format 'value(dnsResourceRecord.name,dnsResourceRecord.data)')"
  echo "DNS authorization record: $auth_rec"
  # Certificate Manager's DNS authorization: its CNAME at Cloudflare (removed by staging-rust down)
  local an ad; read -r an ad <<< "$auth_rec"; an="${an%.}"; ad="${ad%.}"
  if ! cf GET "/dns_records?type=CNAME&name=$an" | grep -q "\"content\":\"$ad\""; then
    cf POST /dns_records "{\"type\":\"CNAME\",\"name\":\"$an\",\"content\":\"$ad\",\"proxied\":false,\"ttl\":60}" \
      | python3 -c 'import sys,json;print("Cloudflare CNAME", json.load(sys.stdin)["result"]["id"])'
  fi
  exists g certificate-manager certificates describe "$sr-cert" || g certificate-manager certificates create "$sr-cert" \
    --domains relay-staging.openagents.com --dns-authorizations "$sr-auth"
  exists g certificate-manager maps describe "$sr-certmap" || g certificate-manager maps create "$sr-certmap"
  exists g certificate-manager maps entries describe "$sr-entry" --map "$sr-certmap" || g certificate-manager maps entries create "$sr-entry" \
    --map "$sr-certmap" --certificates "$sr-cert" --hostname relay-staging.openagents.com
  exists g compute target-https-proxies describe "$sr-proxy" --global || g compute target-https-proxies create "$sr-proxy" --global \
    --url-map "$sr-map" --certificate-map "$sr-certmap"
  exists g compute addresses describe "$sr-ip" --global || g compute addresses create "$sr-ip" --global
  exists g compute forwarding-rules describe "$sr-fr" --global || g compute forwarding-rules create "$sr-fr" --global \
    --load-balancing-scheme EXTERNAL_MANAGED --address "$sr-ip" --target-https-proxy "$sr-proxy" --ports 443
  g compute addresses describe "$sr-ip" --global --format 'value(address)'
}
staging_rust_down() {
  g compute forwarding-rules delete "$sr-fr" --global --quiet || true
  g compute addresses delete "$sr-ip" --global --quiet || true
  g compute target-https-proxies delete "$sr-proxy" --global --quiet || true
  g certificate-manager maps entries delete "$sr-entry" --map "$sr-certmap" --quiet || true
  g certificate-manager maps delete "$sr-certmap" --quiet || true
  g certificate-manager certificates delete "$sr-cert" --quiet || true
  g certificate-manager dns-authorizations delete "$sr-auth" --quiet || true
  g compute url-maps delete "$sr-map" --global --quiet || true
  g compute backend-services delete "$sr-bs" --global --quiet || true
  g compute network-endpoint-groups delete "$sr-neg" --region "$region" --quiet || true
  local id
  for id in $(cf GET "/dns_records?type=CNAME&name=_acme-challenge.relay-staging.openagents.com" | python3 -c 'import sys,json;[print(r["id"]) for r in json.load(sys.stdin)["result"]]'); do
    cf DELETE "/dns_records/$id" >/dev/null && echo "removed Cloudflare CNAME $id"
  done
}

case "${1:-}" in
  staging-rust) case "${2:-}" in up) staging_rust_up ;; down) staging_rust_down ;; *) exit 2 ;; esac ;;
  cert) cert ;;
  cert-renew-by-http) renew_by_http ;;
  create) create ;;
  setup) setup ;;
  ship) ship "${2:?BINARY}" ;;
  start|stop|restart) on "systemctl $1 nostr-relay" ;;
  status) on "systemctl status nostr-relay --no-pager -n 0; systemctl is-active oa-relay-cp.timer" ;;
  logs) on "journalctl -u nostr-relay --no-pager -n ${2:-40}" ;;
  ssh) shift; [[ "${1:-}" == "--" ]] && shift; on "$*" ;;
  *) sed -n '2,30p' "$0"; exit 2 ;;
esac
