#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

export GCP_PROJECT="${GCP_PROJECT:-openagentsgemini}"
export GCP_REGION="${GCP_REGION:-us-central1}"
export GCP_ZONE="${GCP_ZONE:-us-central1-a}"

export OA_VPC="${OA_VPC:-oa-lightning}"
export OA_SUBNET="${OA_SUBNET:-oa-lightning-us-central1}"

export BITCOIND_VM="${BITCOIND_VM:-oa-bitcoind}"
export BITCOIND_RPC_HOST="${BITCOIND_RPC_HOST:-10.42.0.2}"
export BITCOIND_RPC_PORT="${BITCOIND_RPC_PORT:-8332}"
export BITCOIND_P2P_HOST="${BITCOIND_P2P_HOST:-10.42.0.2}"
export BITCOIND_P2P_PORT="${BITCOIND_P2P_PORT:-8333}"
export BITCOIND_RPC_SECRET="${BITCOIND_RPC_SECRET:-oa-bitcoind-rpc-creds}"

export SYMPHONY_VM="${SYMPHONY_VM:-symphony-mainnet-1}"
export SYMPHONY_MACHINE_TYPE="${SYMPHONY_MACHINE_TYPE:-n2-standard-8}"
export SYMPHONY_SERVICE_ACCOUNT_NAME="${SYMPHONY_SERVICE_ACCOUNT_NAME:-symphony-mainnet}"
export SYMPHONY_SERVICE_ACCOUNT_EMAIL="${SYMPHONY_SERVICE_ACCOUNT_EMAIL:-${SYMPHONY_SERVICE_ACCOUNT_NAME}@${GCP_PROJECT}.iam.gserviceaccount.com}"
export SYMPHONY_TAG="${SYMPHONY_TAG:-symphony-node}"

export SYMPHONY_DATA_DISK="${SYMPHONY_DATA_DISK:-symphony-data-mainnet}"
export SYMPHONY_DATA_DISK_SIZE_GB="${SYMPHONY_DATA_DISK_SIZE_GB:-512}"
export SYMPHONY_DATA_DISK_TYPE="${SYMPHONY_DATA_DISK_TYPE:-pd-ssd}"

export SYMPHONY_ARTIFACT_REPO="${SYMPHONY_ARTIFACT_REPO:-openagents-symphony}"
export SYMPHONY_IMAGE_NAME="${SYMPHONY_IMAGE_NAME:-symphony}"
export SYMPHONY_IMAGE_TAG="${SYMPHONY_IMAGE_TAG:-latest}"
export SYMPHONY_IMAGE="${SYMPHONY_IMAGE:-${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${SYMPHONY_ARTIFACT_REPO}/${SYMPHONY_IMAGE_NAME}:${SYMPHONY_IMAGE_TAG}}"

export SYMPHONY_CONFIG_PATH="${SYMPHONY_CONFIG_PATH:-/etc/symphony/mainnet.toml}"
export SYMPHONY_SERVER_BIND="${SYMPHONY_SERVER_BIND:-0.0.0.0:8080}"

log() {
  printf '[symphony-deploy] %s\n' "$*" >&2
}

die() {
  printf '[symphony-deploy] ERROR: %s\n' "$*" >&2
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
    secretmanager.googleapis.com \
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

fetch_bitcoind_rpc_creds() {
  local payload
  payload="$(gcloud secrets versions access latest --secret "$BITCOIND_RPC_SECRET" --project "$GCP_PROJECT")"

  export BITCOIND_RPC_USER
  export BITCOIND_RPC_PASS
  BITCOIND_RPC_USER="$(printf '%s\n' "$payload" | awk -F= '/^rpcuser=/{print $2}' | tail -n1)"
  BITCOIND_RPC_PASS="$(printf '%s\n' "$payload" | awk -F= '/^rpcpassword=/{print $2}' | tail -n1)"

  [[ -n "$BITCOIND_RPC_USER" ]] || die "Could not parse rpcuser from secret ${BITCOIND_RPC_SECRET}"
  [[ -n "$BITCOIND_RPC_PASS" ]] || die "Could not parse rpcpassword from secret ${BITCOIND_RPC_SECRET}"
}
