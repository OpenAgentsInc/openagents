#!/usr/bin/env bash
# Provision the experimental node. Funding and traffic switches are separate.
set -euo pipefail
cd "$(dirname "$0")/.."
project=${LND_GCP_PROJECT:-openagentsgemini}
region=us-central1
zone=us-central1-a
node=oa-lnd-1
account="$node@$project.iam.gserviceaccount.com"
mode=${1:-}
gc() { gcloud --project="$project" --quiet "$@"; }
if [[ $mode == check ]]; then
  gc compute instances describe "$node" --zone="$zone" --format='json(name,status,machineType,networkInterfaces,tags,shieldedInstanceConfig,deletionProtection)'
  gc compute ssh "$node" --zone="$zone" --tunnel-through-iap --command='sudo /opt/openagents-lnd/lncli --lnddir=/var/lib/openagents-lnd/lnd --network=mainnet getinfo'
  exit
fi
if [[ $mode != provision ]]; then
  echo 'Usage: scripts/lnd-gcp.sh provision|check' >&2
  exit 2
fi
# Recheck the chain before creating a dependent host. This prints public chain
# state only; bitcoin-cli reads the RPC credential on its existing host.
gc compute ssh oa-bitcoind --zone="$zone" --tunnel-through-iap --command='sudo /usr/local/bin/bitcoin-cli -conf=/etc/bitcoin/bitcoin.conf getblockchaininfo' \
  | python3 -c 'import json,sys; s=json.load(sys.stdin); assert s["chain"]=="main" and not s["initialblockdownload"] and s["blocks"]==s["headers"]; print("Bitcoin Core is synced at",s["blocks"])'
gc services enable cloudkms.googleapis.com secretmanager.googleapis.com
if ! gc iam service-accounts describe "$account" >/dev/null 2>&1; then
  gc iam service-accounts create "$node" --display-name='Experimental LND node'
fi
if ! gc kms keyrings describe openagents-lnd --location="$region" >/dev/null 2>&1; then
  gc kms keyrings create openagents-lnd --location="$region"
fi
if ! gc kms keys describe "$node-password" --keyring=openagents-lnd --location="$region" >/dev/null 2>&1; then
  gc kms keys create "$node-password" --keyring=openagents-lnd --location="$region" --purpose=encryption
fi
gc kms keys add-iam-policy-binding "$node-password" --keyring=openagents-lnd --location="$region" \
  --member="serviceAccount:$account" --role=roles/cloudkms.cryptoKeyEncrypterDecrypter >/dev/null
for suffix in password-kms seed tls-cert macaroon-invoice macaroon-readonly macaroon-payout macaroon-ops; do
  secret="lnd-$node-$suffix"
  if ! gc secrets describe "$secret" >/dev/null 2>&1; then gc secrets create "$secret" --replication-policy=automatic; fi
  gc secrets add-iam-policy-binding "$secret" --member="serviceAccount:$account" --role=roles/secretmanager.secretVersionAdder >/dev/null
done
for secret in "lnd-$node-password-kms" oa-bitcoind-rpc-creds; do
  gc secrets add-iam-policy-binding "$secret" --member="serviceAccount:$account" --role=roles/secretmanager.secretAccessor >/dev/null
done
if ! gc compute addresses describe "$node-p2p" --region="$region" >/dev/null 2>&1; then
  gc compute addresses create "$node-p2p" --region="$region"
fi
external=$(gc compute addresses describe "$node-p2p" --region="$region" --format='value(address)')
pay_ip=$(gc compute instances describe oa-pay-1 --zone="$zone" --format='value(networkInterfaces[0].networkIP)')
# The explicit deny overrides the default VPC's broad internal/SSH allows.
if ! gc compute firewall-rules describe "$node-deny-ingress" >/dev/null 2>&1; then
  gc compute firewall-rules create "$node-deny-ingress" --network=default --direction=INGRESS --priority=900 --action=DENY --rules=all --source-ranges=0.0.0.0/0 --target-tags=oa-lnd-v1
fi
for rule in ssh peer pay; do
  case $rule in
    ssh) source=35.235.240.0/20; port=22;;
    peer) source=0.0.0.0/0; port=9735;;
    pay) source="$pay_ip/32"; port=10009;;
  esac
  if ! gc compute firewall-rules describe "$node-$rule" >/dev/null 2>&1; then
    gc compute firewall-rules create "$node-$rule" --network=default --direction=INGRESS --priority=800 --action=ALLOW --rules="tcp:$port" --source-ranges="$source" --target-tags=oa-lnd-v1
  fi
done
if ! gc compute disks describe "$node-data" --zone="$zone" >/dev/null 2>&1; then
  gc compute disks create "$node-data" --zone="$zone" --size=50GB --type=pd-ssd --labels=purpose=openagents-lnd,issue=11752
fi
if ! gc compute instances describe "$node" --zone="$zone" >/dev/null 2>&1; then
  gc compute instances create "$node" --zone="$zone" --machine-type=e2-standard-4 \
    --image-family=debian-12 --image-project=debian-cloud --boot-disk-size=20GB --boot-disk-type=pd-balanced \
    --disk="name=$node-data,device-name=lnd-data,mode=rw,boot=no,auto-delete=no" \
    --network-interface="subnet=default,address=$external" \
    --network-interface=subnet=oa-lightning-us-central1,no-address \
    --service-account="$account" --scopes=cloud-platform --tags=oa-lnd-v1 \
    --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring --deletion-protection \
    --labels=purpose=openagents-lnd,issue=11752
fi
printf 'Provisioned %s; peer address %s:9735. Install from a current main commit.\n' "$node" "$external"
