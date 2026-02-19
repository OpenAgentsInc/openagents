#!/usr/bin/env bash
set -euo pipefail

PROJECT_ID="${PROJECT_ID:-openagentsgemini}"
REGION="${REGION:-us-central1}"
CONVEX_BACKEND_SERVICE="${CONVEX_BACKEND_SERVICE:-oa-convex-backend-nonprod}"
CONVEX_DASHBOARD_SERVICE="${CONVEX_DASHBOARD_SERVICE:-oa-convex-dashboard-nonprod}"
CLOUD_SQL_INSTANCE="${CLOUD_SQL_INSTANCE:-oa-convex-nonprod-pg}"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for health checks" >&2
  exit 1
fi

BACKEND_URL="$(gcloud run services describe "$CONVEX_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
DASHBOARD_URL="$(gcloud run services describe "$CONVEX_DASHBOARD_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
SQL_REGION="$(gcloud sql instances describe "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" --format='value(region)')"

if [[ "$SQL_REGION" != "$REGION" ]]; then
  echo "Cloud SQL region mismatch: expected $REGION got $SQL_REGION" >&2
  exit 1
fi

echo "Checking backend version endpoint..."
BACKEND_VERSION_JSON="$(curl -fsS "${BACKEND_URL}/version")"
echo "$BACKEND_VERSION_JSON" | jq . >/dev/null

echo "Checking dashboard health..."
DASHBOARD_STATUS="$(curl -s -o /dev/null -w '%{http_code}' "$DASHBOARD_URL")"
if [[ "$DASHBOARD_STATUS" != "200" && "$DASHBOARD_STATUS" != "302" ]]; then
  echo "Dashboard returned unexpected status: $DASHBOARD_STATUS" >&2
  exit 1
fi

echo "Checking Cloud Run readiness..."
gcloud run services describe "$CONVEX_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.conditions[0].status)' | rg -q '^True$'
gcloud run services describe "$CONVEX_DASHBOARD_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.conditions[0].status)' | rg -q '^True$'

echo "Non-prod Convex health checks passed."
echo "Backend URL:   $BACKEND_URL"
echo "Dashboard URL: $DASHBOARD_URL"
echo "Cloud SQL region: $SQL_REGION"
