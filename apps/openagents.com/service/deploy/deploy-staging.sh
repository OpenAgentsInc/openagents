#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
APP_DIR="$(cd "${SERVICE_DIR}/.." && pwd)"

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null || true)}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-control-service-staging}"
IMAGE="${IMAGE:-}"

if [[ -z "${PROJECT}" || "${PROJECT}" == "(unset)" ]]; then
  echo "error: PROJECT is required (or set active gcloud project)" >&2
  exit 1
fi

if [[ -z "${IMAGE}" ]]; then
  echo "error: IMAGE is required (Rust control-service image URI)." >&2
  exit 1
fi

echo "[staging] project=${PROJECT} region=${REGION} service=${SERVICE}"

PROJECT="${PROJECT}" REGION="${REGION}" SERVICE="${SERVICE}" IMAGE="${IMAGE}" \
  "${SCRIPT_DIR}/deploy-production.sh"

echo "[staging] run smoke checks:"
echo "  OPENAGENTS_BASE_URL=https://staging.openagents.com ${SCRIPT_DIR}/smoke-health.sh"
echo "  OPENAGENTS_BASE_URL=https://staging.openagents.com ${SCRIPT_DIR}/smoke-control.sh"
