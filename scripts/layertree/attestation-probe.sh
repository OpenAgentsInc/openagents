#!/usr/bin/env bash
# Short-lived attestation probes for #11787; no Bitcoin signing or funds.
set -euo pipefail
project=${LAYERTREE_PROJECT:-openagentsgemini}
zone=${LAYERTREE_ZONE:-us-central1-a}
issue=${LAYERTREE_ISSUE:-11787}
[[ $issue =~ ^[0-9]+$ ]] || { echo 'Use a GitHub issue number.' >&2; exit 2; }
action=${1:?use create or delete}
route=${2:?use snp, tdx, snp-space, or tdx-space}
case "$route" in
  snp|snp-space) machine=n2d-standard-2; technology=SEV_SNP ;;
  tdx|tdx-space) machine=c3-standard-4; technology=TDX ;;
  *) echo 'Unknown attestation route.' >&2; exit 2 ;;
esac
vm="oa-layertree-$route-probe"
case "$action" in
  create)
    args=(--no-service-account --no-scopes --tags=oa-lnd-builder
      --image=projects/ubuntu-os-cloud/global/images/ubuntu-2404-noble-amd64-v20261009)
    if [[ $route == *-space ]]; then
      image=${LAYERTREE_PROBE_IMAGE:?set the digest-pinned research image}
      [[ $image =~ ^us-central1-docker.pkg.dev/openagentsgemini/layertree-research/attestation-probe@sha256:[a-f0-9]{64}$ ]] || { echo 'Use the pinned research image.' >&2; exit 2; }
      args=(--service-account=oa-layertree-probe@openagentsgemini.iam.gserviceaccount.com
        --scopes=cloud-platform
        --image=projects/confidential-space-images/global/images/confidential-space-260900
        --metadata="^~^tee-image-reference=$image~tee-container-log-redirect=true~tee-restart-policy=Never")
    fi
    gcloud compute instances create "$vm" --project "$project" --zone "$zone" \
      --machine-type "$machine" --confidential-compute-type "$technology" \
      --maintenance-policy TERMINATE --shielded-secure-boot \
      --boot-disk-size 20GB --boot-disk-type pd-balanced \
      --labels "issue=$issue,purpose=attestation-spike" \
      --max-run-duration 7200s --instance-termination-action DELETE \
      "${args[@]}" --quiet
    if [[ -n ${OA_RUN_ID:-} ]]; then
      python3 scripts/dev/agent-run.py label-vm "$vm" "$zone"
    fi
    ;;
  delete)
    label=$(gcloud compute instances describe "$vm" --project "$project" --zone "$zone" --format='value(labels.issue)')
    [[ $label == "$issue" ]] || { echo 'The instance is not this issue’s probe.' >&2; exit 2; }
    gcloud compute instances delete "$vm" --project "$project" --zone "$zone" --quiet
    ;;
  *) echo 'Use create or delete.' >&2; exit 2 ;;
esac
