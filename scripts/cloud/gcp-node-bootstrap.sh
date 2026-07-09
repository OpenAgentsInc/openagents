#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-node-bootstrap.sh --project PROJECT_ID [--region us-central1] [--zone us-central1-a] [--env dev] [--apply]

Creates or prints the first managed Cloud node GCP test substrate:
  - required APIs
  - Artifact Registry repo for oa-node / oa-workroomd images
  - service account for the managed test VM
  - least-broad IAM for logs, monitoring, artifact reads, and scoped storage
  - private test network and subnet
  - firewall rule for SSH from IAP only
  - state, artifact, receipt, and log buckets

The default mode is dry-run. Pass --apply to execute gcloud commands.
USAGE
}

project_id=""
region="us-central1"
zone="us-central1-a"
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
    --zone)
      zone="${2:-}"
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
node_sa="${node_sa_name}@${project_id}.iam.gserviceaccount.com"
state_bucket="${project_id}-oa-node-state-${env_name}"
artifact_bucket="${project_id}-oa-workroom-artifacts-${env_name}"
receipt_bucket="${project_id}-oa-workroom-receipts-${env_name}"
log_bucket="${project_id}-oa-node-logs-${env_name}"
firewall_iap="oa-cloud-${env_name}-iap-ssh"
secret_prefix="oa-node-${env_name}"

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

run gcloud services enable \
  compute.googleapis.com \
  artifactregistry.googleapis.com \
  storage.googleapis.com \
  iam.googleapis.com \
  logging.googleapis.com \
  monitoring.googleapis.com \
  secretmanager.googleapis.com \
  oslogin.googleapis.com

run gcloud artifacts repositories create "$repo_name" \
  --project "$project_id" \
  --repository-format docker \
  --location "$region" \
  --description "OpenAgents managed cloud node images"

run gcloud compute networks create "$network_name" \
  --project "$project_id" \
  --subnet-mode custom

run gcloud compute networks subnets create "$subnet_name" \
  --project "$project_id" \
  --network "$network_name" \
  --region "$region" \
  --range "10.42.0.0/24" \
  --enable-private-ip-google-access

run gcloud compute firewall-rules create "$firewall_iap" \
  --project "$project_id" \
  --network "$network_name" \
  --allow tcp:22 \
  --source-ranges 35.235.240.0/20 \
  --target-tags "oa-cloud-node-${env_name}"

run gcloud iam service-accounts create "$node_sa_name" \
  --project "$project_id" \
  --display-name "OpenAgents managed cloud node ${env_name}"

for bucket in "$state_bucket" "$artifact_bucket" "$receipt_bucket" "$log_bucket"; do
  run gcloud storage buckets create "gs://${bucket}" \
    --project "$project_id" \
    --location "$region" \
    --uniform-bucket-level-access
done

run gcloud artifacts repositories add-iam-policy-binding "$repo_name" \
  --project "$project_id" \
  --location "$region" \
  --member "serviceAccount:${node_sa}" \
  --role roles/artifactregistry.reader

for bucket in "$state_bucket" "$artifact_bucket" "$receipt_bucket" "$log_bucket"; do
  run gcloud storage buckets add-iam-policy-binding "gs://${bucket}" \
    --member "serviceAccount:${node_sa}" \
    --role roles/storage.objectAdmin
done

run gcloud projects add-iam-policy-binding "$project_id" \
  --member "serviceAccount:${node_sa}" \
  --role roles/logging.logWriter
run gcloud projects add-iam-policy-binding "$project_id" \
  --member "serviceAccount:${node_sa}" \
  --role roles/monitoring.metricWriter

run gcloud secrets create "${secret_prefix}-placeholder" \
  --project "$project_id" \
  --replication-policy automatic
run gcloud secrets add-iam-policy-binding "${secret_prefix}-placeholder" \
  --project "$project_id" \
  --member "serviceAccount:${node_sa}" \
  --role roles/secretmanager.secretAccessor

cat <<SUMMARY

OpenAgents Cloud node ${env_name} substrate:
  Artifact Registry: ${region}-docker.pkg.dev/${project_id}/${repo_name}
  Network:           ${network_name}
  Subnet:            ${subnet_name}
  Zone:              ${zone}
  Node SA:           ${node_sa}
  State bucket:      gs://${state_bucket}
  Artifact bucket:   gs://${artifact_bucket}
  Receipt bucket:    gs://${receipt_bucket}
  Log bucket:        gs://${log_bucket}
  SSH ingress:       IAP-only firewall ${firewall_iap}

Mode apply=${apply}
SUMMARY
