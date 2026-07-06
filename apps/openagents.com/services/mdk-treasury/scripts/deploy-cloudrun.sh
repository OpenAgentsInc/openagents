#!/usr/bin/env bash
set -euo pipefail

# Deploy the MDK/Spark treasury daemon to Google Cloud Run (CFG-15, EPIC #8515).
#
# Follows the apps/oa-updates Cloud Run pattern (`gcloud run deploy --source`).
# Secrets come from GCP Secret Manager (project `openagentsgemini`), never
# from tracked files or flags. See the rehearsed cutover runbook before ANY
# production use: docs/cloud/2026-07-06-mdk-treasury-cloudrun-cutover-runbook.md
#
# SAFETY (standing invariant): NEVER run two live daemons on one mnemonic.
# The production treasury mnemonic must never be mounted here while the
# Cloudflare `MdkTreasuryContainer` for the same mnemonic can still be woken.
# The default service name is the STAGING service; deploying the production
# service requires ALLOW_PRODUCTION_MONEY_PATH_DEPLOY=yes (owner-gated).
#
# Required before running:
#   gcloud auth login
#   gcloud config set project openagentsgemini
#   # create the Secret Manager secrets this service reads (see runbook)
#
# Optional overrides:
#   SERVICE_NAME     (default: oa-mdk-treasury-staging; prod: oa-mdk-treasury)
#   SECRET_PREFIX    (default: staging- ; prod: empty string)
#   REGION           (default: us-central1)
#   MIN_INSTANCES    (default: 0 for staging; runbook mandates 1 for prod)

SERVICE_NAME="${SERVICE_NAME:-oa-mdk-treasury-staging}"
SECRET_PREFIX="${SECRET_PREFIX-staging-}"
REGION="${REGION:-us-central1}"
MIN_INSTANCES="${MIN_INSTANCES:-0}"
SERVICE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "${SERVICE_NAME}" != *staging* && "${ALLOW_PRODUCTION_MONEY_PATH_DEPLOY:-}" != "yes" ]]; then
  echo "REFUSING: '${SERVICE_NAME}' is not a staging service name." >&2
  echo "Production money-path deploys are owner-gated (CFG-15 runbook)." >&2
  echo "Set ALLOW_PRODUCTION_MONEY_PATH_DEPLOY=yes only during the rehearsed cutover." >&2
  exit 1
fi

# Secret Manager secret name -> container env var. Only secrets that exist in
# the project are mounted, so a staging stack with a subset (e.g. no MDK
# access token yet) still deploys and reports honest healthz flags.
declare -a candidates=(
  "MDK_TREASURY_MNEMONIC=${SECRET_PREFIX}mdk-treasury-mnemonic"
  "MDK_TREASURY_ACCESS_TOKEN=${SECRET_PREFIX}mdk-treasury-access-token"
  "MDK_TREASURY_SERVICE_TOKEN=${SECRET_PREFIX}mdk-treasury-service-token"
  "SPARK_TREASURY_API_KEY=${SECRET_PREFIX}spark-treasury-api-key"
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

if [[ ${#secret_flags[@]} -eq 0 ]]; then
  echo "REFUSING: no Secret Manager secrets found for prefix '${SECRET_PREFIX}'." >&2
  exit 1
fi

secret_csv="$(IFS=,; echo "${secret_flags[*]}")"

# --max-instances 1 is LOAD-BEARING: the daemon is the single writer against
# the treasury wallet state (MDK VSS + Spark local storage). Never raise it.
# --no-cpu-throttling: the MDK node drains payment events between requests.
gcloud run deploy "${SERVICE_NAME}" \
  --source "${SERVICE_DIR}" \
  --region "${REGION}" \
  --allow-unauthenticated \
  --port 8080 \
  --execution-environment gen2 \
  --no-cpu-throttling \
  --min-instances "${MIN_INSTANCES}" \
  --max-instances 1 \
  --memory 1Gi \
  --cpu 1 \
  --concurrency 20 \
  --timeout 120 \
  --set-env-vars "SPARK_TREASURY_STORAGE_DIR=/tmp/spark-treasury" \
  --set-secrets "${secret_csv}"

echo
echo "Deployed ${SERVICE_NAME}. Verify:"
echo "  curl -fsS \"\$(gcloud run services describe ${SERVICE_NAME} --region ${REGION} --format='value(status.url)')/health\""
