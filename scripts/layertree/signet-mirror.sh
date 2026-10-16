#!/usr/bin/env bash
# Three private, bounded research mirrors in separate zones.
set -euo pipefail
action=${1:?use create, status, or delete}
index=${2:?use mirror index 0, 1, or 2}
case $index in 0) zone=us-central1-a ;; 1) zone=us-central1-b ;; 2) zone=us-central1-c ;; *) echo 'Use mirror index 0, 1, or 2.' >&2; exit 2 ;; esac
project=openagentsgemini
vm=oa-layertree-mirror-$index
case $action in
  create)
    if ! gcloud compute firewall-rules describe oa-layertree-mirror-deny --project "$project" >/dev/null 2>&1; then
      gcloud compute firewall-rules create oa-layertree-mirror-deny --project "$project" --network default --direction INGRESS --priority 800 --action DENY --rules all --source-ranges 0.0.0.0/0 --target-tags oa-layertree-mirror --quiet
    fi
    if ! gcloud compute firewall-rules describe oa-layertree-mirror-iap --project "$project" >/dev/null 2>&1; then
      gcloud compute firewall-rules create oa-layertree-mirror-iap --project "$project" --network default --direction INGRESS --priority 700 --action ALLOW --rules tcp:22 --source-ranges 35.235.240.0/20 --target-tags oa-layertree-mirror --quiet
    fi
    if ! gcloud compute firewall-rules describe oa-layertree-mirror-data --project "$project" >/dev/null 2>&1; then
      gcloud compute firewall-rules create oa-layertree-mirror-data --project "$project" --network default --direction INGRESS --priority 700 --action ALLOW --rules tcp:18440 --source-tags oa-layertree-signet,oa-lnd-builder --target-tags oa-layertree-mirror --quiet
    fi
    gcloud compute instances create "$vm" --project "$project" --zone "$zone" \
      --machine-type e2-micro --no-address --no-service-account --no-scopes \
      --image projects/debian-cloud/global/images/debian-12-bookworm-v20261014 \
      --boot-disk-size 10GB --boot-disk-type pd-standard --shielded-secure-boot \
      --tags oa-layertree-mirror --labels issue=11789,purpose=signet-mirror \
      --max-run-duration 777600s --instance-termination-action DELETE --quiet
    if [[ -n ${OA_RUN_ID:-} ]]; then python3 scripts/dev/agent-run.py label-vm "$vm" "$zone"; fi
    ;;
  status) gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='json(id,status,creationTimestamp,networkInterfaces,scheduling,labels)' ;;
  delete)
    label=$(gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='value(labels.issue)')
    [[ $label == 11789 ]] || { echo 'This instance is not a signet research mirror.' >&2; exit 2; }
    gcloud compute instances delete "$vm" --project "$project" --zone "$zone" --quiet
    ;;
  *) echo 'Use create, status, or delete.' >&2; exit 2 ;;
esac
