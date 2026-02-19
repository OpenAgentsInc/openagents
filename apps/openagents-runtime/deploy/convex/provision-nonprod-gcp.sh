#!/usr/bin/env bash
set -euo pipefail

# Provision self-hosted Convex non-prod on GCP using Cloud Run + Cloud SQL Postgres.
#
# Default mode is dry-run. Set OA_CONVEX_APPLY=1 to execute.

PROJECT_ID="${PROJECT_ID:-openagentsgemini}"
REGION="${REGION:-us-central1}"

CLOUD_SQL_INSTANCE="${CLOUD_SQL_INSTANCE:-oa-convex-nonprod-pg}"
CLOUD_SQL_TIER="${CLOUD_SQL_TIER:-db-custom-1-3840}"
CLOUD_SQL_DATABASE_VERSION="${CLOUD_SQL_DATABASE_VERSION:-POSTGRES_16}"
CLOUD_SQL_STORAGE_GB="${CLOUD_SQL_STORAGE_GB:-20}"

CONVEX_DB_NAME="${CONVEX_DB_NAME:-convex_nonprod}"
CONVEX_DB_USER="${CONVEX_DB_USER:-convex}"
CONVEX_DB_PASSWORD="${CONVEX_DB_PASSWORD:-$(openssl rand -hex 24)}"

CONVEX_BACKEND_SERVICE="${CONVEX_BACKEND_SERVICE:-oa-convex-backend-nonprod}"
CONVEX_DASHBOARD_SERVICE="${CONVEX_DASHBOARD_SERVICE:-oa-convex-dashboard-nonprod}"

CONVEX_BACKEND_SA="${CONVEX_BACKEND_SA:-${CONVEX_BACKEND_SERVICE}-sa}"
CONVEX_DASHBOARD_SA="${CONVEX_DASHBOARD_SA:-${CONVEX_DASHBOARD_SERVICE}-sa}"

# Pinned image digests from ghcr manifests (2026-02-19)
CONVEX_BACKEND_IMAGE="${CONVEX_BACKEND_IMAGE:-ghcr.io/get-convex/convex-backend@sha256:fde1830745d1c2c69dd731ff1a245591e3aba380df990df2d390f2338b574d73}"
CONVEX_DASHBOARD_IMAGE="${CONVEX_DASHBOARD_IMAGE:-ghcr.io/get-convex/convex-dashboard@sha256:f809827d55bc53f617199f7ec0962b6c261f774188fbc10c62737869ed3c631b}"

INSTANCE_SECRET="${INSTANCE_SECRET:-$(openssl rand -hex 32)}"
INSTANCE_NAME="${INSTANCE_NAME:-openagents-convex-nonprod}"

POSTGRES_URL_SECRET="${POSTGRES_URL_SECRET:-oa-convex-nonprod-postgres-url}"
INSTANCE_SECRET_SECRET="${INSTANCE_SECRET_SECRET:-oa-convex-nonprod-instance-secret}"

APPLY="${OA_CONVEX_APPLY:-0}"

CLOUD_SQL_CONNECTION_NAME="${PROJECT_ID}:${REGION}:${CLOUD_SQL_INSTANCE}"
POSTGRES_URL="postgresql://${CONVEX_DB_USER}:${CONVEX_DB_PASSWORD}@/${CONVEX_DB_NAME}?host=/cloudsql/${CLOUD_SQL_CONNECTION_NAME}"
BACKEND_SA_EMAIL="${CONVEX_BACKEND_SA}@${PROJECT_ID}.iam.gserviceaccount.com"
DASHBOARD_SA_EMAIL="${CONVEX_DASHBOARD_SA}@${PROJECT_ID}.iam.gserviceaccount.com"

run_cmd() {
  local cmd="$1"
  if [[ "$APPLY" == "1" ]]; then
    echo "+ $cmd"
    eval "$cmd"
  else
    echo "[dry-run] $cmd"
  fi
}

ensure_secret_version() {
  local secret_name="$1"
  local secret_value="$2"

  if [[ "$APPLY" != "1" ]]; then
    echo "[dry-run] upsert secret ${secret_name}"
    return 0
  fi

  if ! gcloud secrets describe "$secret_name" --project "$PROJECT_ID" >/dev/null 2>&1; then
    gcloud secrets create "$secret_name" --replication-policy automatic --project "$PROJECT_ID" >/dev/null
  fi

  printf '%s' "$secret_value" | gcloud secrets versions add "$secret_name" --data-file=- --project "$PROJECT_ID" >/dev/null
}

ensure_service_account() {
  local account_id="$1"
  local display_name="$2"

  if [[ "$APPLY" != "1" ]]; then
    echo "[dry-run] ensure service account ${account_id}"
    return 0
  fi

  if ! gcloud iam service-accounts describe "${account_id}@${PROJECT_ID}.iam.gserviceaccount.com" --project "$PROJECT_ID" >/dev/null 2>&1; then
    gcloud iam service-accounts create "$account_id" --display-name "$display_name" --project "$PROJECT_ID" >/dev/null
  fi
}

if [[ "$APPLY" == "1" ]]; then
  gcloud auth print-access-token >/dev/null
fi

run_cmd "gcloud config set project ${PROJECT_ID}"

run_cmd "gcloud services enable run.googleapis.com sqladmin.googleapis.com secretmanager.googleapis.com iam.googleapis.com --project ${PROJECT_ID}"

if [[ "$APPLY" == "1" ]]; then
  if ! gcloud sql instances describe "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" >/dev/null 2>&1; then
    run_cmd "gcloud sql instances create ${CLOUD_SQL_INSTANCE} --database-version=${CLOUD_SQL_DATABASE_VERSION} --tier=${CLOUD_SQL_TIER} --storage-size=${CLOUD_SQL_STORAGE_GB} --region=${REGION} --project=${PROJECT_ID}"
  else
    echo "+ Cloud SQL instance ${CLOUD_SQL_INSTANCE} already exists"
  fi

  if ! gcloud sql databases describe "$CONVEX_DB_NAME" --instance "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" >/dev/null 2>&1; then
    run_cmd "gcloud sql databases create ${CONVEX_DB_NAME} --instance=${CLOUD_SQL_INSTANCE} --project=${PROJECT_ID}"
  else
    echo "+ Cloud SQL database ${CONVEX_DB_NAME} already exists"
  fi

  if ! gcloud sql users list --instance "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" --format='value(name)' | rg -q "^${CONVEX_DB_USER}$"; then
    run_cmd "gcloud sql users create ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='${CONVEX_DB_PASSWORD}' --project=${PROJECT_ID}"
  else
    run_cmd "gcloud sql users set-password ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='${CONVEX_DB_PASSWORD}' --project=${PROJECT_ID}"
  fi
else
  run_cmd "gcloud sql instances create ${CLOUD_SQL_INSTANCE} --database-version=${CLOUD_SQL_DATABASE_VERSION} --tier=${CLOUD_SQL_TIER} --storage-size=${CLOUD_SQL_STORAGE_GB} --region=${REGION} --project=${PROJECT_ID}"
  run_cmd "gcloud sql databases create ${CONVEX_DB_NAME} --instance=${CLOUD_SQL_INSTANCE} --project=${PROJECT_ID}"
  run_cmd "gcloud sql users create ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='***' --project=${PROJECT_ID}"
fi

ensure_service_account "$CONVEX_BACKEND_SA" "Convex Backend Non-Prod"
ensure_service_account "$CONVEX_DASHBOARD_SA" "Convex Dashboard Non-Prod"

run_cmd "gcloud projects add-iam-policy-binding ${PROJECT_ID} --member=serviceAccount:${BACKEND_SA_EMAIL} --role=roles/cloudsql.client --quiet"

ensure_secret_version "$POSTGRES_URL_SECRET" "$POSTGRES_URL"
ensure_secret_version "$INSTANCE_SECRET_SECRET" "$INSTANCE_SECRET"

run_cmd "gcloud run deploy ${CONVEX_BACKEND_SERVICE} --project=${PROJECT_ID} --region=${REGION} --platform=managed --image='${CONVEX_BACKEND_IMAGE}' --service-account='${BACKEND_SA_EMAIL}' --allow-unauthenticated --port=3210 --cpu=2 --memory=4Gi --add-cloudsql-instances='${CLOUD_SQL_CONNECTION_NAME}' --set-secrets='POSTGRES_URL=${POSTGRES_URL_SECRET}:latest,INSTANCE_SECRET=${INSTANCE_SECRET_SECRET}:latest' --set-env-vars='INSTANCE_NAME=${INSTANCE_NAME},CONVEX_CLOUD_ORIGIN=https://placeholder.invalid,CONVEX_SITE_ORIGIN=https://placeholder.invalid,REDACT_LOGS_TO_CLIENT=true,DISABLE_BEACON=true,DO_NOT_REQUIRE_SSL=1'"

if [[ "$APPLY" == "1" ]]; then
  BACKEND_URL="$(gcloud run services describe "$CONVEX_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
else
  BACKEND_URL="https://${CONVEX_BACKEND_SERVICE}-<hash>-uc.a.run.app"
fi

run_cmd "gcloud run deploy ${CONVEX_DASHBOARD_SERVICE} --project=${PROJECT_ID} --region=${REGION} --platform=managed --image='${CONVEX_DASHBOARD_IMAGE}' --service-account='${DASHBOARD_SA_EMAIL}' --allow-unauthenticated --port=6791 --cpu=1 --memory=1Gi --set-env-vars='NEXT_PUBLIC_DEPLOYMENT_URL=${BACKEND_URL},NEXT_PUBLIC_LOAD_MONACO_INTERNALLY=1'"

if [[ "$APPLY" == "1" ]]; then
  DASHBOARD_URL="$(gcloud run services describe "$CONVEX_DASHBOARD_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
else
  DASHBOARD_URL="https://${CONVEX_DASHBOARD_SERVICE}-<hash>-uc.a.run.app"
fi

run_cmd "gcloud run services update ${CONVEX_BACKEND_SERVICE} --project=${PROJECT_ID} --region=${REGION} --set-env-vars='CONVEX_CLOUD_ORIGIN=${BACKEND_URL},CONVEX_SITE_ORIGIN=${DASHBOARD_URL},INSTANCE_NAME=${INSTANCE_NAME},REDACT_LOGS_TO_CLIENT=true,DISABLE_BEACON=true,DO_NOT_REQUIRE_SSL=1'"

echo
echo "Provisioning summary:"
echo "  Project:   ${PROJECT_ID}"
echo "  Region:    ${REGION}"
echo "  Cloud SQL: ${CLOUD_SQL_INSTANCE}"
echo "  Backend:   ${CONVEX_BACKEND_SERVICE} (${BACKEND_URL})"
echo "  Dashboard: ${CONVEX_DASHBOARD_SERVICE} (${DASHBOARD_URL})"
echo
echo "Next step:"
echo "  apps/openagents-runtime/deploy/convex/check-nonprod-health.sh"
