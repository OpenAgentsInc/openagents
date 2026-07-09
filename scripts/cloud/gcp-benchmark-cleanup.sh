#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-benchmark-cleanup.sh --project PROJECT_ID [--region us-central1] [--env dev] [--apply]

Deletes the dev Benchmark Cloud resources created by gcp-benchmark-bootstrap.sh.
The default mode is dry-run. Pass --apply to execute destructive commands.
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

repo_name="oa-benchmark-runners"
buckets=(
  "${project_id}-oa-benchmark-specs-${env_name}"
  "${project_id}-oa-benchmark-datasets-${env_name}"
  "${project_id}-oa-benchmark-artifacts-${env_name}"
  "${project_id}-oa-benchmark-proofs-${env_name}"
)
topics=(
  "benchmark-task-events-${env_name}"
  "benchmark-run-events-${env_name}"
)
service_accounts=(
  "bench-controller-${env_name}@${project_id}.iam.gserviceaccount.com"
  "bench-runner-${env_name}@${project_id}.iam.gserviceaccount.com"
)
secret_id="oa-benchmark-provider-placeholder-${env_name}"

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

for topic in "${topics[@]}"; do
  run gcloud pubsub topics delete "$topic" --project "$project_id" --quiet
done

for bucket in "${buckets[@]}"; do
  run gcloud storage rm --recursive "gs://${bucket}/**"
  run gcloud storage buckets delete "gs://${bucket}" --quiet
done

run gcloud artifacts repositories delete "$repo_name" \
  --project "$project_id" \
  --location "$region" \
  --quiet

for service_account in "${service_accounts[@]}"; do
  run gcloud iam service-accounts delete "$service_account" \
    --project "$project_id" \
    --quiet
done
