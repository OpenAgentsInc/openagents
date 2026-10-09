#!/bin/sh
# Make the account-store NFS server for one environment
# (docs/deployment/account-storage.md): a Cloud Run egress subnet, a
# private e2-small VM with a 10 GB data disk exported over NFS to that
# subnet only, hourly snapshots of the disk, and the firewall rules.
# Every step is skipped when its resource is already there.
#
#   deploy/accounts-nfs/provision.sh staging
#   deploy/accounts-nfs/provision.sh production
#
# Run with the automation account:
#   CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
set -eu
env=${1:?usage: provision.sh staging|production}
case "$env" in
    staging)
        subnet=openagents-web-staging
        range=10.42.26.0/26
        address=10.42.26.2
        keep_days=7
        ;;
    production)
        subnet=openagents-web-production
        range=10.42.27.0/26
        address=10.42.27.2
        keep_days=30
        ;;
    *)
        echo "usage: provision.sh staging|production" >&2
        exit 2
        ;;
esac
project=openagentsgemini
region=us-central1
zone=us-central1-a
name=oa-accounts-nfs-$env
here=$(dirname "$0")
g() { gcloud --project "$project" --quiet "$@"; }
have() { "$@" > /dev/null 2>&1; }

have g compute networks subnets describe "$subnet" --region "$region" \
    || g compute networks subnets create "$subnet" --network default \
        --region "$region" --range "$range"

have g compute addresses describe "$name" --region "$region" \
    || g compute addresses create "$name" --region "$region" \
        --subnet "$subnet" --addresses "$address"

# NFS from the subnet only, SSH through IAP only, nothing else (this
# outranks the default network's allow-internal and allow-ssh rules).
have g compute firewall-rules describe "$name-ingress" \
    || g compute firewall-rules create "$name-ingress" --network default \
        --direction INGRESS --priority 900 --source-ranges "$range" \
        --target-tags "$name" \
        --allow tcp:111,tcp:2049,tcp:20048,udp:111,udp:2049,udp:20048
have g compute firewall-rules describe "$name-iap-ssh" \
    || g compute firewall-rules create "$name-iap-ssh" --network default \
        --direction INGRESS --priority 900 --source-ranges 35.235.240.0/20 \
        --target-tags "$name" --allow tcp:22
have g compute firewall-rules describe "$name-deny-other-ingress" \
    || g compute firewall-rules create "$name-deny-other-ingress" --network default \
        --direction INGRESS --priority 910 --source-ranges 0.0.0.0/0 \
        --target-tags "$name" --action DENY --rules all

have g compute resource-policies describe "$name-hourly" --region "$region" \
    || g compute resource-policies create snapshot-schedule "$name-hourly" \
        --region "$region" --hourly-schedule 1 --start-time 00:00 \
        --max-retention-days "$keep_days" --on-source-disk-delete keep-auto-snapshots \
        --storage-location us --snapshot-labels "app=accounts,env=$env"

have g compute disks describe "$name-data" --zone "$zone" \
    || g compute disks create "$name-data" --zone "$zone" --type pd-balanced \
        --size 10GB --resource-policies "$name-hourly" --labels "app=accounts,env=$env"

have g compute instances describe "$name" --zone "$zone" \
    || g compute instances create "$name" --zone "$zone" --machine-type e2-small \
        --subnet "$subnet" --private-network-ip "$name" --no-address \
        --image-family debian-12 --image-project debian-cloud \
        --boot-disk-size 10GB --boot-disk-type pd-balanced \
        --disk "name=$name-data,device-name=accounts,mode=rw,boot=no,auto-delete=no" \
        --no-service-account --no-scopes --tags "$name" \
        --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring \
        --deletion-protection --labels "app=accounts,env=$env" \
        --metadata "allowed-cidr=$range,enable-oslogin=TRUE,block-project-ssh-keys=TRUE" \
        --metadata-from-file "startup-script=$here/startup.sh"

# Production's site also reaches the pay host (oa-pay-1, 10.128.0.46:4400,
# docs/deployment/openagents-web.md); the default network's allow-internal
# rule covers 10.128.0.0/9 only, so this subnet needs its own rule.
if [ "$env" = production ]; then
    have g compute firewall-rules describe oa-pay-host-from-web-production \
        || g compute firewall-rules create oa-pay-host-from-web-production --network default \
            --direction INGRESS --priority 1000 --source-ranges "$range" \
            --target-tags oa-pay-host --allow tcp:4400
    # The service's other sidecar (coder-serve) reaches private addresses in
    # 10.128.0.0/9 (the Coder pool host) through the default subnet today;
    # moving its egress to this subnet keeps exactly the reach the default
    # network's allow-internal rule gives that subnet. Tagged deny rules
    # (the NFS server's, the Coder box's) still outrank it.
    have g compute firewall-rules describe allow-internal-from-web-production \
        || g compute firewall-rules create allow-internal-from-web-production --network default \
            --direction INGRESS --priority 65534 --source-ranges "$range" \
            --allow tcp:0-65535,udp:0-65535,icmp
fi

echo "$name: NFS server $address, exports /srv/accounts/{stack,web} to $range"
