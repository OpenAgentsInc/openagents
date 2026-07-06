#!/usr/bin/env bash
set -euo pipefail

# Deploy the MDK checkout sidecar daemon to Google Cloud Run (CFG-15, EPIC #8515).
#
# Follows the apps/oa-updates Cloud Run pattern. Secrets come from GCP Secret
# Manager (project `openagentsgemini`). Runbook (read before ANY production
# use): docs/cloud/2026-07-06-mdk-treasury-cloudrun-cutover-runbook.md
#
# SAFETY (standing invariant): NEVER run two live daemons on one mnemonic. The
# production checkout mnemonic must never be mounted here while the Cloudflare
# `MdkSidecarContainer` for the same mnemonic can still be woken. Production
# deploys are gated on ALLOW_PRODUCTION_MONEY_PATH_DEPLOY.
#
# Off-Workers the sidecar is network-reachable, so the deploy REQUIRES the
# sidecar service-token secret; the Worker sends it as
# `x-mdk-sidecar-service-token` (see workers/api/src/mdk-service-endpoints.ts).
#
# Optional overrides:
#   SERVICE_NAME     (default: oa-mdk-sidecar-staging; prod: oa-mdk-sidecar)
#   SECRET_PREFIX    (default: staging- ; prod: empty string)
#   REGION           (default: us-central1)
#   MIN_INSTANCES    (default: 0 for staging; runbook mandates 1 for prod)

SERVICE_NAME="${SERVICE_NAME:-oa-mdk-sidecar-staging}"
SECRET_PREFIX="${SECRET_PREFIX-staging-}"
REGION="${REGION:-us-central1}"
MIN_INSTANCES="${MIN_INSTANCES:-0}"
SERVICE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${SERVICE_NAME}" != *staging* && "${ALLOW_PRODUCTION_MONEY_PATH_DEPLOY:-}" != "yes" ]]; then
  echo "REFUSING: '${SERVICE_NAME}' is not a staging service name." >&2
  echo "Production money-path deploys are owner-gated (CFG-15 runbook)." >&2
  exit 1
fi

if ! gcloud secrets describe "${SECRET_PREFIX}mdk-sidecar-service-token" >/dev/null 2>&1; then
  echo "REFUSING: ${SECRET_PREFIX}mdk-sidecar-service-token secret missing." >&2
  echo "A network-reachable sidecar must gate /api/mdk with a service token." >&2
  exit 1
fi

declare -a candidates=(
  "MDK_SIDECAR_SERVICE_TOKEN=${SECRET_PREFIX}mdk-sidecar-service-token"
  "MDK_ACCESS_TOKEN=${SECRET_PREFIX}mdk-sidecar-access-token"
  "MDK_MNEMONIC=${SECRET_PREFIX}mdk-sidecar-mnemonic"
  "MDK_WEBHOOK_SECRET=${SECRET_PREFIX}mdk-sidecar-webhook-secret"
  "WITHDRAWAL_DESTINATION=${SECRET_PREFIX}mdk-sidecar-withdrawal-destination"
)

secret_flags=()
for candidate in "${candidates[@]}"; do
  env_name="${candidate%%=*}"
  secret_name="${candidate#*=}"
  if gcloud secrets describe "${secret_name}" >/dev/null 2>&1; then
    secret_flags+=("${env_name}=${secret_name}:latest")
  else
    echo "note: secret ${secret_name} not found; ${env_name} stays unset" >&2
  fi
done

secret_csv="$(IFS=,; echo "${secret_flags[*]}")"

# --max-instances 2 mirrors the Cloudflare `max_instances: 2` for the sidecar
# (checkout traffic, not the single-writer treasury/tips wallets).
gcloud run deploy "${SERVICE_NAME}" \
  --source "${SERVICE_DIR}" \
  --region "${REGION}" \
  --allow-unauthenticated \
  --port 8080 \
  --execution-environment gen2 \
  --no-cpu-throttling \
  --min-instances "${MIN_INSTANCES}" \
  --max-instances 2 \
  --memory 1Gi \
  --cpu 1 \
  --concurrency 40 \
  --timeout 120 \
  --set-secrets "${secret_csv}"

echo
echo "Deployed ${SERVICE_NAME}. Verify:"
echo "  curl -fsS \"\$(gcloud run services describe ${SERVICE_NAME} --region ${REGION} --format='value(status.url)')/health\""
