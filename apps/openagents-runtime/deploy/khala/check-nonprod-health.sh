#!/usr/bin/env bash
set -euo pipefail

PROJECT_ID="${PROJECT_ID:-openagentsgemini}"
REGION="${REGION:-us-central1}"
KHALA_BACKEND_SERVICE="${KHALA_BACKEND_SERVICE:-oa-khala-backend-nonprod}"
KHALA_DASHBOARD_SERVICE="${KHALA_DASHBOARD_SERVICE:-oa-khala-dashboard-nonprod}"
CLOUD_SQL_INSTANCE="${CLOUD_SQL_INSTANCE:-oa-khala-nonprod-pg}"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for health checks" >&2
  exit 1
fi

BACKEND_URL="$(gcloud run services describe "$KHALA_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
DASHBOARD_URL="$(gcloud run services describe "$KHALA_DASHBOARD_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
SQL_REGION="$(gcloud sql instances describe "$CLOUD_SQL_INSTANCE" --project "$PROJECT_ID" --format='value(region)')"

if [[ "$SQL_REGION" != "$REGION" ]]; then
  echo "Cloud SQL region mismatch: expected $REGION got $SQL_REGION" >&2
  exit 1
fi

echo "Checking backend version endpoint..."
BACKEND_VERSION_JSON="$(curl -fsS "${BACKEND_URL}/version")"
if echo "$BACKEND_VERSION_JSON" | jq . >/dev/null 2>&1; then
  :
elif [[ -n "$BACKEND_VERSION_JSON" ]]; then
  :
else
  echo "Backend /version returned an empty response" >&2
  exit 1
fi

echo "Checking backend root health..."
BACKEND_ROOT="$(curl -fsS "${BACKEND_URL}")"
echo "$BACKEND_ROOT" | rg -q "Khala deployment is running"

echo "Checking dashboard health..."
DASHBOARD_STATUS="$(curl -s -o /dev/null -w '%{http_code}' "$DASHBOARD_URL")"
if [[ "$DASHBOARD_STATUS" != "200" && "$DASHBOARD_STATUS" != "302" ]]; then
  echo "Dashboard returned unexpected status: $DASHBOARD_STATUS" >&2
  exit 1
fi

echo "Checking Cloud Run readiness..."
gcloud run services describe "$KHALA_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format=json | jq -e '.status.conditions[] | select(.type == "Ready" and .status == "True")' >/dev/null
gcloud run services describe "$KHALA_DASHBOARD_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format=json | jq -e '.status.conditions[] | select(.type == "Ready" and .status == "True")' >/dev/null

echo "Non-prod Khala health checks passed."
echo "Backend URL:   $BACKEND_URL"
echo "Dashboard URL: $DASHBOARD_URL"
echo "Cloud SQL region: $SQL_REGION"
