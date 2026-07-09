#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/gcp-benchmark-bootstrap.sh --project PROJECT_ID [--region us-central1] [--env dev] [--apply]

Creates or prints the GCP Benchmark Cloud substrate:
  - required APIs
  - Artifact Registry runner repo
  - task spec, dataset, artifact, and proof buckets
  - Pub/Sub task/run topics
  - bench-controller and bench-runner service accounts
  - narrow IAM bindings for specs, artifacts, events, and placeholder secrets

The default mode is dry-run. Pass --apply to execute gcloud commands.
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
spec_bucket="${project_id}-oa-benchmark-specs-${env_name}"
dataset_bucket="${project_id}-oa-benchmark-datasets-${env_name}"
artifact_bucket="${project_id}-oa-benchmark-artifacts-${env_name}"
proof_bucket="${project_id}-oa-benchmark-proofs-${env_name}"
task_topic="benchmark-task-events-${env_name}"
run_topic="benchmark-run-events-${env_name}"
controller_sa="bench-controller-${env_name}@${project_id}.iam.gserviceaccount.com"
runner_sa="bench-runner-${env_name}@${project_id}.iam.gserviceaccount.com"
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

run gcloud services enable \
  batch.googleapis.com \
  run.googleapis.com \
  artifactregistry.googleapis.com \
  storage.googleapis.com \
  pubsub.googleapis.com \
  secretmanager.googleapis.com \
  cloudbuild.googleapis.com \
  logging.googleapis.com \
  monitoring.googleapis.com

run gcloud artifacts repositories create "$repo_name" \
  --project "$project_id" \
  --repository-format docker \
  --location "$region" \
  --description "OpenAgents benchmark runner images"

for bucket in "$spec_bucket" "$dataset_bucket" "$artifact_bucket" "$proof_bucket"; do
  run gcloud storage buckets create "gs://${bucket}" \
    --project "$project_id" \
    --location "$region" \
    --uniform-bucket-level-access
done

run gcloud pubsub topics create "$task_topic" --project "$project_id"
run gcloud pubsub topics create "$run_topic" --project "$project_id"

run gcloud iam service-accounts create "bench-controller-${env_name}" \
  --project "$project_id" \
  --display-name "Benchmark Controller ${env_name}"
run gcloud iam service-accounts create "bench-runner-${env_name}" \
  --project "$project_id" \
  --display-name "Benchmark Runner ${env_name}"

run gcloud projects add-iam-policy-binding "$project_id" \
  --member "serviceAccount:${controller_sa}" \
  --role roles/batch.jobsEditor
run gcloud projects add-iam-policy-binding "$project_id" \
  --member "serviceAccount:${controller_sa}" \
  --role roles/run.developer

run gcloud iam service-accounts add-iam-policy-binding "$runner_sa" \
  --project "$project_id" \
  --member "serviceAccount:${controller_sa}" \
  --role roles/iam.serviceAccountUser

run gcloud storage buckets add-iam-policy-binding "gs://${spec_bucket}" \
  --member "serviceAccount:${controller_sa}" \
  --role roles/storage.objectAdmin
run gcloud storage buckets add-iam-policy-binding "gs://${spec_bucket}" \
  --member "serviceAccount:${runner_sa}" \
  --role roles/storage.objectViewer
run gcloud storage buckets add-iam-policy-binding "gs://${dataset_bucket}" \
  --member "serviceAccount:${runner_sa}" \
  --role roles/storage.objectViewer
run gcloud storage buckets add-iam-policy-binding "gs://${artifact_bucket}" \
  --member "serviceAccount:${runner_sa}" \
  --role roles/storage.objectAdmin
run gcloud storage buckets add-iam-policy-binding "gs://${proof_bucket}" \
  --member "serviceAccount:${runner_sa}" \
  --role roles/storage.objectAdmin

run gcloud pubsub topics add-iam-policy-binding "$task_topic" \
  --project "$project_id" \
  --member "serviceAccount:${runner_sa}" \
  --role roles/pubsub.publisher
run gcloud pubsub topics add-iam-policy-binding "$run_topic" \
  --project "$project_id" \
  --member "serviceAccount:${controller_sa}" \
  --role roles/pubsub.publisher

run gcloud secrets create "$secret_id" \
  --project "$project_id" \
  --replication-policy automatic
run gcloud secrets add-iam-policy-binding "$secret_id" \
  --project "$project_id" \
  --member "serviceAccount:${runner_sa}" \
  --role roles/secretmanager.secretAccessor

cat <<SUMMARY

Benchmark Cloud ${env_name} substrate:
  Artifact Registry: ${region}-docker.pkg.dev/${project_id}/${repo_name}
  Spec bucket:       gs://${spec_bucket}
  Dataset bucket:    gs://${dataset_bucket}
  Artifact bucket:   gs://${artifact_bucket}
  Proof bucket:      gs://${proof_bucket}
  Task topic:        ${task_topic}
  Run topic:         ${run_topic}
  Controller SA:     ${controller_sa}
  Runner SA:         ${runner_sa}

Mode: ${apply}
SUMMARY
