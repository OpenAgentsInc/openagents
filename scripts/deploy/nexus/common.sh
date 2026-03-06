#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

export GCP_PROJECT="${GCP_PROJECT:-openagentsgemini}"
export GCP_REGION="${GCP_REGION:-us-central1}"
export GCP_ZONE="${GCP_ZONE:-us-central1-a}"

export OA_VPC="${OA_VPC:-oa-lightning}"
export OA_SUBNET="${OA_SUBNET:-oa-lightning-us-central1}"

export NEXUS_VM="${NEXUS_VM:-nexus-mainnet-1}"
export NEXUS_MACHINE_TYPE="${NEXUS_MACHINE_TYPE:-e2-standard-4}"
export NEXUS_SERVICE_ACCOUNT_NAME="${NEXUS_SERVICE_ACCOUNT_NAME:-nexus-mainnet}"
export NEXUS_SERVICE_ACCOUNT_EMAIL="${NEXUS_SERVICE_ACCOUNT_EMAIL:-${NEXUS_SERVICE_ACCOUNT_NAME}@${GCP_PROJECT}.iam.gserviceaccount.com}"
export NEXUS_TAG="${NEXUS_TAG:-nexus-host}"

export NEXUS_DATA_DISK="${NEXUS_DATA_DISK:-nexus-relay-data-mainnet}"
export NEXUS_DATA_DISK_DEVICE_NAME="${NEXUS_DATA_DISK_DEVICE_NAME:-nexus-relay-data}"
export NEXUS_DATA_DISK_SIZE_GB="${NEXUS_DATA_DISK_SIZE_GB:-200}"
export NEXUS_DATA_DISK_TYPE="${NEXUS_DATA_DISK_TYPE:-pd-ssd}"

export NEXUS_ARTIFACT_REPO="${NEXUS_ARTIFACT_REPO:-openagents-nexus}"
export NEXUS_IMAGE_NAME="${NEXUS_IMAGE_NAME:-nexus-relay}"
export NEXUS_IMAGE_TAG="${NEXUS_IMAGE_TAG:-latest}"
export NEXUS_IMAGE="${NEXUS_IMAGE:-${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:${NEXUS_IMAGE_TAG}}"

export NEXUS_PUBLIC_HOST="${NEXUS_PUBLIC_HOST:-nexus.openagents.com}"
export NEXUS_PUBLIC_URL="${NEXUS_PUBLIC_URL:-https://${NEXUS_PUBLIC_HOST}/}"
export NEXUS_PUBLIC_WS_URL="${NEXUS_PUBLIC_WS_URL:-wss://${NEXUS_PUBLIC_HOST}/}"

export NEXUS_LISTEN_ADDR="${NEXUS_LISTEN_ADDR:-0.0.0.0:8080}"
export NEXUS_UPSTREAM_LISTEN_ADDR="${NEXUS_UPSTREAM_LISTEN_ADDR:-127.0.0.1:42111}"
export NEXUS_DATA_DIR="${NEXUS_DATA_DIR:-/var/lib/nexus-relay}"
export NEXUS_RECEIPT_LOG_PATH="${NEXUS_RECEIPT_LOG_PATH:-${NEXUS_DATA_DIR}/nexus-control-receipts.jsonl}"

log() {
  printf '[nexus-deploy] %s\n' "$*" >&2
}

die() {
  printf '[nexus-deploy] ERROR: %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    die "Missing required command: ${cmd}"
  fi
}

ensure_gcloud_context() {
  require_cmd gcloud

  local active_project active_account
  active_project="$(gcloud config get-value project 2>/dev/null || true)"
  active_account="$(gcloud config get-value account 2>/dev/null || true)"

  [[ -n "$active_account" ]] || die "No active gcloud account. Run: gcloud auth login"

  if [[ "$active_project" != "$GCP_PROJECT" ]]; then
    log "Switching gcloud project from '${active_project}' to '${GCP_PROJECT}'"
    gcloud config set project "$GCP_PROJECT" >/dev/null
  fi

  gcloud config set compute/region "$GCP_REGION" >/dev/null
  gcloud config set compute/zone "$GCP_ZONE" >/dev/null
}

ensure_services() {
  gcloud services enable \
    compute.googleapis.com \
    artifactregistry.googleapis.com \
    cloudbuild.googleapis.com \
    logging.googleapis.com \
    monitoring.googleapis.com \
    --project "$GCP_PROJECT" >/dev/null
}

instance_exists() {
  gcloud compute instances describe "$1" --project "$GCP_PROJECT" --zone "$GCP_ZONE" >/dev/null 2>&1
}

disk_exists() {
  gcloud compute disks describe "$1" --project "$GCP_PROJECT" --zone "$GCP_ZONE" >/dev/null 2>&1
}

firewall_rule_exists() {
  gcloud compute firewall-rules describe "$1" --project "$GCP_PROJECT" >/dev/null 2>&1
}
