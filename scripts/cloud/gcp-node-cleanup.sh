#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-node-cleanup.sh --project PROJECT_ID [--region us-central1] [--env dev] [--apply]

Prints or removes the GCP test substrate created by gcp-node-bootstrap.sh.
The default mode is dry-run. Pass --apply to delete resources.
USAGE
}

project_id=""
region="us-central1"
env_name="dev"
apply="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project)
      project_id="${2:-}"
      shift 2
      ;;
    --region)
      region="${2:-}"
      shift 2
      ;;
    --env)
      env_name="${2:-}"
      shift 2
      ;;
    --apply)
      apply="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$project_id" ]]; then
  echo "--project is required" >&2
  usage >&2
  exit 2
fi

repo_name="oa-cloud"
network_name="oa-cloud-${env_name}"
subnet_name="oa-cloud-${env_name}-${region}"
node_sa_name="oa-node-${env_name}"
state_bucket="${project_id}-oa-node-state-${env_name}"
artifact_bucket="${project_id}-oa-workroom-artifacts-${env_name}"
receipt_bucket="${project_id}-oa-workroom-receipts-${env_name}"
log_bucket="${project_id}-oa-node-logs-${env_name}"
firewall_iap="oa-cloud-${env_name}-iap-ssh"
secret_id="oa-node-${env_name}-placeholder"

run() {
  if [[ "$apply" == "true" ]]; then
    "$@"
  else
    printf '+'
    printf ' %q' "$@"
    printf '\n'
  fi
}

run gcloud config set project "$project_id"

run gcloud secrets delete "$secret_id" --project "$project_id" --quiet

for bucket in "$state_bucket" "$artifact_bucket" "$receipt_bucket" "$log_bucket"; do
  run gcloud storage rm --recursive "gs://${bucket}/**"
  run gcloud storage buckets delete "gs://${bucket}" --quiet
done

run gcloud artifacts repositories delete "$repo_name" \
  --project "$project_id" \
  --location "$region" \
  --quiet

run gcloud iam service-accounts delete "${node_sa_name}@${project_id}.iam.gserviceaccount.com" \
  --project "$project_id" \
  --quiet

run gcloud compute firewall-rules delete "$firewall_iap" --project "$project_id" --quiet
run gcloud compute networks subnets delete "$subnet_name" --project "$project_id" --region "$region" --quiet
run gcloud compute networks delete "$network_name" --project "$project_id" --quiet
