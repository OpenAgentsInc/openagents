#!/usr/bin/env bash
# Prepare write-only backup storage and a separate private watchtower host.
set -euo pipefail
cd "$(dirname "$0")/.."
project=openagentsgemini
account=oa-lnd-1@$project.iam.gserviceaccount.com
gc() { gcloud --project="$project" --quiet "$@"; }
bucket=gs://$project-lnd-scb
if ! gc storage buckets describe "$bucket" >/dev/null 2>&1; then
  gc storage buckets create "$bucket" --location=us-central1 --uniform-bucket-level-access --public-access-prevention
fi
gc storage buckets add-iam-policy-binding "$bucket" --member="serviceAccount:$account" --role=roles/storage.objectCreator >/dev/null
if ! gc secrets describe lnd-oa-lnd-1-backup-age-key >/dev/null 2>&1; then
  gc secrets create lnd-oa-lnd-1-backup-age-key --replication-policy=automatic
fi
gc secrets add-iam-policy-binding lnd-oa-lnd-1-backup-age-key --member="serviceAccount:$account" --role=roles/secretmanager.secretVersionAdder >/dev/null
tower=oa-lnd-tower-1
tower_account=$tower@$project.iam.gserviceaccount.com
if ! gc iam service-accounts describe "$tower_account" >/dev/null 2>&1; then gc iam service-accounts create "$tower"; fi
for attempt in 1 2 3 4 5; do
  if gc secrets add-iam-policy-binding oa-bitcoind-rpc-creds --member="serviceAccount:$tower_account" --role=roles/secretmanager.secretAccessor >/dev/null; then break; fi
  [[ $attempt != 5 ]] || exit 1
  sleep 5
done
for rule in deny ssh tower; do
  name="$tower-$rule"
  if ! gc compute firewall-rules describe "$name" >/dev/null 2>&1; then
    case $rule in
      deny) gc compute firewall-rules create "$name" --network=oa-lightning --direction=INGRESS --priority=900 --action=DENY --rules=all --source-ranges=0.0.0.0/0 --target-tags=oa-lnd-tower;;
      ssh) gc compute firewall-rules create "$name" --network=oa-lightning --direction=INGRESS --priority=800 --action=ALLOW --rules=tcp:22 --source-ranges=35.235.240.0/20 --target-tags=oa-lnd-tower;;
      tower) gc compute firewall-rules create "$name" --network=oa-lightning --direction=INGRESS --priority=800 --action=ALLOW --rules=tcp:9911 --source-ranges=10.42.0.192/32 --target-tags=oa-lnd-tower;;
    esac
  fi
done
if ! gc compute instances describe "$tower" --zone=us-central1-b >/dev/null 2>&1; then
  gc compute instances create "$tower" --zone=us-central1-b --machine-type=e2-small --image-family=debian-12 --image-project=debian-cloud \
    --subnet=oa-lightning-us-central1 --boot-disk-size=20GB --no-boot-disk-auto-delete \
    --service-account="$tower_account" --scopes=cloud-platform --tags=oa-lnd-tower \
    --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring --deletion-protection --labels=purpose=lnd-watchtower,issue=11753
fi
gc compute instances describe "$tower" --zone=us-central1-b --format='value(networkInterfaces[0].networkIP)'
