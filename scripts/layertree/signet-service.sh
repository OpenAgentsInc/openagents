#!/usr/bin/env bash
# Bounded signet-only TDX service; create, inspect, or delete its own instance.
set -euo pipefail
project=openagentsgemini
zone=us-central1-a
case ${LAYERTREE_SIGNET_SLOT:-primary} in
  primary) vm=oa-layertree-signet-tdx; issue=11789; duration=777600s ;;
  halt) vm=oa-layertree-halt-tdx; issue=11793; duration=86400s ;;
  *) echo 'Use the primary or halt research slot.' >&2; exit 2 ;;
esac
case ${1:?use create, status, or delete} in
  create)
    image=${LAYERTREE_SERVICE_IMAGE:?set the logged digest-pinned service image}
    [[ $image =~ ^us-central1-docker.pkg.dev/openagentsgemini/layertree-research/attestation-probe@sha256:[a-f0-9]{64}$ ]] || { echo 'Use the digest-pinned research image.' >&2; exit 2; }
    # A higher-priority allow admits only the research builder, then the deny
    # closes every other inbound port, including SSH and Core RPC.
    if ! gcloud compute firewall-rules describe oa-layertree-signet-deny --project "$project" >/dev/null 2>&1; then
      gcloud compute firewall-rules create oa-layertree-signet-deny --project "$project" --network default --direction INGRESS --priority 800 --action DENY --rules all --source-ranges 0.0.0.0/0 --target-tags oa-layertree-signet --quiet
    fi
    if ! gcloud compute firewall-rules describe oa-layertree-signet-client --project "$project" >/dev/null 2>&1; then
      gcloud compute firewall-rules create oa-layertree-signet-client --project "$project" --network default --direction INGRESS --priority 700 --action ALLOW --rules tcp:8443 --source-tags oa-lnd-builder --target-tags oa-layertree-signet --quiet
    fi
    gcloud compute instances create "$vm" --project "$project" --zone "$zone" \
      --machine-type c3-standard-4 --confidential-compute-type TDX \
      --maintenance-policy TERMINATE --shielded-secure-boot \
      --boot-disk-size 40GB --boot-disk-type pd-balanced \
      --service-account oa-layertree-probe@openagentsgemini.iam.gserviceaccount.com --scopes cloud-platform \
      --image projects/confidential-space-images/global/images/confidential-space-260900 \
      --tags oa-layertree-signet --labels "issue=$issue",purpose=signet-service \
      --metadata "^~^tee-image-reference=$image~tee-container-log-redirect=true~tee-restart-policy=Never" \
      --max-run-duration "$duration" --instance-termination-action DELETE --quiet
    if [[ -n ${OA_RUN_ID:-} ]]; then python3 scripts/dev/agent-run.py label-vm "$vm" "$zone"; fi
    ;;
  status) gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='json(id,status,creationTimestamp,networkInterfaces,scheduling,labels)' ;;
  delete)
    label=$(gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='value(labels.issue)')
    [[ $label == "$issue" ]] || { echo 'This instance is not the signet research service.' >&2; exit 2; }
    gcloud compute instances delete "$vm" --project "$project" --zone "$zone" --quiet
    ;;
  *) echo 'Use create, status, or delete.' >&2; exit 2 ;;
esac
