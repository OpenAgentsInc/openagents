#!/usr/bin/env bash
set -euo pipefail

# Provision self-hosted Convex non-prod on GCP using:
# - Cloud Run (backend + dashboard)
# - Cloud SQL Postgres
# - Cloud SQL Auth Proxy sidecar in the backend service
#
# Default mode is dry-run. Set OA_CONVEX_APPLY=1 to execute.

PROJECT_ID="${PROJECT_ID:-openagentsgemini}"
REGION="${REGION:-us-central1}"

CLOUD_SQL_INSTANCE="${CLOUD_SQL_INSTANCE:-oa-convex-nonprod-pg}"
CLOUD_SQL_TIER="${CLOUD_SQL_TIER:-db-custom-1-3840}"
CLOUD_SQL_DATABASE_VERSION="${CLOUD_SQL_DATABASE_VERSION:-POSTGRES_16}"
CLOUD_SQL_STORAGE_GB="${CLOUD_SQL_STORAGE_GB:-20}"
CLOUD_SQL_EDITION="${CLOUD_SQL_EDITION:-ENTERPRISE}"

INSTANCE_NAME="${INSTANCE_NAME:-convex-nonprod}"

CONVEX_DB_NAME="${CONVEX_DB_NAME:-${INSTANCE_NAME//-/_}}"
CONVEX_DB_USER="${CONVEX_DB_USER:-convex}"
CONVEX_DB_PASSWORD="${CONVEX_DB_PASSWORD:-$(openssl rand -hex 24)}"

CONVEX_BACKEND_SERVICE="${CONVEX_BACKEND_SERVICE:-oa-convex-backend-nonprod}"
CONVEX_DASHBOARD_SERVICE="${CONVEX_DASHBOARD_SERVICE:-oa-convex-dashboard-nonprod}"

CONVEX_BACKEND_SA="${CONVEX_BACKEND_SA:-${CONVEX_BACKEND_SERVICE}-sa}"
CONVEX_DASHBOARD_SA="${CONVEX_DASHBOARD_SA:-${CONVEX_DASHBOARD_SERVICE}-sa}"

# Pinned non-prod images mirrored in Artifact Registry (2026-02-19)
CONVEX_BACKEND_IMAGE="${CONVEX_BACKEND_IMAGE:-${REGION}-docker.pkg.dev/${PROJECT_ID}/thirdparty/convex-backend:2026-02-19-amd64}"
CONVEX_DASHBOARD_IMAGE="${CONVEX_DASHBOARD_IMAGE:-${REGION}-docker.pkg.dev/${PROJECT_ID}/thirdparty/convex-dashboard:2026-02-19-amd64}"
CONVEX_PROXY_IMAGE="${CONVEX_PROXY_IMAGE:-gcr.io/cloud-sql-connectors/cloud-sql-proxy:2.19.0}"

INSTANCE_SECRET="${INSTANCE_SECRET:-$(openssl rand -hex 32)}"

POSTGRES_URL_SECRET="${POSTGRES_URL_SECRET:-oa-convex-nonprod-postgres-url}"
INSTANCE_SECRET_SECRET="${INSTANCE_SECRET_SECRET:-oa-convex-nonprod-instance-secret}"

APPLY="${OA_CONVEX_APPLY:-0}"

CLOUD_SQL_CONNECTION_NAME="${PROJECT_ID}:${REGION}:${CLOUD_SQL_INSTANCE}"
POSTGRES_URL="postgresql://${CONVEX_DB_USER}:${CONVEX_DB_PASSWORD}@localhost:5432?sslmode=disable"
BACKEND_SA_EMAIL="${CONVEX_BACKEND_SA}@${PROJECT_ID}.iam.gserviceaccount.com"
DASHBOARD_SA_EMAIL="${CONVEX_DASHBOARD_SA}@${PROJECT_ID}.iam.gserviceaccount.com"
PROJECT_NUMBER="${PROJECT_NUMBER:-}"

run_cmd() {
  local cmd="$1"
  if [[ "$APPLY" == "1" ]]; then
    echo "+ $cmd"
    eval "$cmd"
  else
    echo "[dry-run] $cmd"
  fi
}

run_cmd_redacted() {
  local cmd="$1"
  local display="$2"
  if [[ "$APPLY" == "1" ]]; then
    echo "+ $display"
    eval "$cmd"
  else
    echo "[dry-run] $display"
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

render_backend_manifest() {
  local manifest_path="$1"
  local cloud_origin="$2"
  local site_origin="$3"

  cat >"$manifest_path" <<EOF
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: ${CONVEX_BACKEND_SERVICE}
  namespace: '${PROJECT_NUMBER}'
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/maxScale: '100'
        run.googleapis.com/container-dependencies: '{"convex-backend":["cloud-sql-proxy"]}'
        run.googleapis.com/startup-cpu-boost: 'true'
    spec:
      containerConcurrency: 160
      serviceAccountName: ${BACKEND_SA_EMAIL}
      timeoutSeconds: 300
      containers:
      - name: convex-backend
        image: ${CONVEX_BACKEND_IMAGE}
        env:
        - name: INSTANCE_NAME
          value: ${INSTANCE_NAME}
        - name: CONVEX_CLOUD_ORIGIN
          value: '${cloud_origin}'
        - name: CONVEX_SITE_ORIGIN
          value: '${site_origin}'
        - name: REDACT_LOGS_TO_CLIENT
          value: 'true'
        - name: DISABLE_BEACON
          value: 'true'
        - name: DO_NOT_REQUIRE_SSL
          value: '1'
        - name: POSTGRES_URL
          valueFrom:
            secretKeyRef:
              key: latest
              name: ${POSTGRES_URL_SECRET}
        - name: INSTANCE_SECRET
          valueFrom:
            secretKeyRef:
              key: latest
              name: ${INSTANCE_SECRET_SECRET}
        ports:
        - name: http1
          containerPort: 3210
        resources:
          limits:
            cpu: '2'
            memory: 4Gi
        startupProbe:
          failureThreshold: 1
          periodSeconds: 240
          tcpSocket:
            port: 3210
          timeoutSeconds: 240
      - name: cloud-sql-proxy
        image: ${CONVEX_PROXY_IMAGE}
        args:
        - ${CLOUD_SQL_CONNECTION_NAME}
        - --address=0.0.0.0
        - --port=5432
        startupProbe:
          failureThreshold: 30
          periodSeconds: 2
          tcpSocket:
            port: 5432
          timeoutSeconds: 2
        resources:
          limits:
            cpu: '1'
            memory: 512Mi
  traffic:
  - latestRevision: true
    percent: 100
EOF
}

if [[ "$APPLY" == "1" ]]; then
  gcloud auth print-access-token >/dev/null
fi

run_cmd "gcloud config set project ${PROJECT_ID}"

run_cmd "gcloud services enable run.googleapis.com sqladmin.googleapis.com secretmanager.googleapis.com iam.googleapis.com artifactregistry.googleapis.com --project ${PROJECT_ID}"

if [[ "$APPLY" == "1" ]]; then
  PROJECT_NUMBER="$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')"
else
  PROJECT_NUMBER="<project-number>"
fi

if [[ "$APPLY" == "1" ]]; then
  if ! gcloud sql instances describe "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" >/dev/null 2>&1; then
    run_cmd "gcloud sql instances create ${CLOUD_SQL_INSTANCE} --database-version=${CLOUD_SQL_DATABASE_VERSION} --edition=${CLOUD_SQL_EDITION} --tier=${CLOUD_SQL_TIER} --storage-size=${CLOUD_SQL_STORAGE_GB} --region=${REGION} --project=${PROJECT_ID}"
  else
    echo "+ Cloud SQL instance ${CLOUD_SQL_INSTANCE} already exists"
  fi

  if ! gcloud sql databases describe "$CONVEX_DB_NAME" --instance "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" >/dev/null 2>&1; then
    run_cmd "gcloud sql databases create ${CONVEX_DB_NAME} --instance=${CLOUD_SQL_INSTANCE} --project=${PROJECT_ID}"
  else
    echo "+ Cloud SQL database ${CONVEX_DB_NAME} already exists"
  fi

  if ! gcloud sql users list --instance "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" --format='value(name)' | rg -q "^${CONVEX_DB_USER}$"; then
    run_cmd_redacted "gcloud sql users create ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='${CONVEX_DB_PASSWORD}' --project=${PROJECT_ID}" "gcloud sql users create ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='***' --project=${PROJECT_ID}"
  else
    run_cmd_redacted "gcloud sql users set-password ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='${CONVEX_DB_PASSWORD}' --project=${PROJECT_ID}" "gcloud sql users set-password ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='***' --project=${PROJECT_ID}"
  fi
else
  run_cmd "gcloud sql instances create ${CLOUD_SQL_INSTANCE} --database-version=${CLOUD_SQL_DATABASE_VERSION} --edition=${CLOUD_SQL_EDITION} --tier=${CLOUD_SQL_TIER} --storage-size=${CLOUD_SQL_STORAGE_GB} --region=${REGION} --project=${PROJECT_ID}"
  run_cmd "gcloud sql databases create ${CONVEX_DB_NAME} --instance=${CLOUD_SQL_INSTANCE} --project=${PROJECT_ID}"
  run_cmd "gcloud sql users create ${CONVEX_DB_USER} --instance=${CLOUD_SQL_INSTANCE} --password='***' --project=${PROJECT_ID}"
fi

ensure_service_account "$CONVEX_BACKEND_SA" "Convex Backend Non-Prod"
ensure_service_account "$CONVEX_DASHBOARD_SA" "Convex Dashboard Non-Prod"

run_cmd "gcloud projects add-iam-policy-binding ${PROJECT_ID} --member=serviceAccount:${BACKEND_SA_EMAIL} --role=roles/cloudsql.client --quiet >/dev/null"
run_cmd "gcloud projects add-iam-policy-binding ${PROJECT_ID} --member=serviceAccount:${BACKEND_SA_EMAIL} --role=roles/secretmanager.secretAccessor --quiet >/dev/null"

ensure_secret_version "$POSTGRES_URL_SECRET" "$POSTGRES_URL"
ensure_secret_version "$INSTANCE_SECRET_SECRET" "$INSTANCE_SECRET"

if [[ "$APPLY" == "1" ]]; then
  BACKEND_MANIFEST_PATH="$(mktemp)"
  trap 'rm -f "$BACKEND_MANIFEST_PATH"' EXIT
else
  BACKEND_MANIFEST_PATH="/tmp/${CONVEX_BACKEND_SERVICE}.yaml"
fi

render_backend_manifest "$BACKEND_MANIFEST_PATH" "https://placeholder.invalid" "https://placeholder.invalid"
run_cmd "gcloud run services replace ${BACKEND_MANIFEST_PATH} --project ${PROJECT_ID} --region ${REGION}"

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

render_backend_manifest "$BACKEND_MANIFEST_PATH" "$BACKEND_URL" "$DASHBOARD_URL"
run_cmd "gcloud run services replace ${BACKEND_MANIFEST_PATH} --project ${PROJECT_ID} --region ${REGION}"

if [[ "$APPLY" == "1" ]]; then
  BACKEND_URL="$(gcloud run services describe "$CONVEX_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
fi

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
