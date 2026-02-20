#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PROJECT_ID="${PROJECT_ID:-openagentsgemini}"
REGION="${REGION:-us-central1}"
KHALA_BACKEND_SERVICE="${KHALA_BACKEND_SERVICE:-oa-khala-backend-nonprod}"
KHALA_DASHBOARD_SERVICE="${KHALA_DASHBOARD_SERVICE:-oa-khala-dashboard-nonprod}"
OA_KHALA_ROLLBACK_DRILL_APPLY="${OA_KHALA_ROLLBACK_DRILL_APPLY:-0}"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 1
  fi
}

require_cmd gcloud
require_cmd jq

current_traffic_revision() {
  local service="$1"

  gcloud run services describe "$service" \
    --project "$PROJECT_ID" \
    --region "$REGION" \
    --format=json \
    | jq -r '.status.traffic[] | select(.percent == 100) | .revisionName' \
    | head -n 1
}

previous_ready_revision() {
  local service="$1"

  gcloud run revisions list \
    --project "$PROJECT_ID" \
    --region "$REGION" \
    --service "$service" \
    --format='value(metadata.name,status.conditions[0].status)' \
    | awk '$2=="True" {print $1}' \
    | sed -n '2p'
}

CURRENT_BACKEND="$(current_traffic_revision "$KHALA_BACKEND_SERVICE")"
CURRENT_DASHBOARD="$(current_traffic_revision "$KHALA_DASHBOARD_SERVICE")"
ROLLBACK_BACKEND="$(previous_ready_revision "$KHALA_BACKEND_SERVICE")"
ROLLBACK_DASHBOARD="$(previous_ready_revision "$KHALA_DASHBOARD_SERVICE")"

if [[ -z "$CURRENT_BACKEND" || -z "$CURRENT_DASHBOARD" ]]; then
  echo "Unable to resolve current 100% traffic revisions." >&2
  exit 1
fi

if [[ -z "$ROLLBACK_BACKEND" || -z "$ROLLBACK_DASHBOARD" ]]; then
  echo "Unable to resolve previous ready revisions for rollback drill." >&2
  exit 1
fi

if [[ "$OA_KHALA_ROLLBACK_DRILL_APPLY" != "1" ]]; then
  echo "Rollback drill plan (dry-run):"
  echo "  backend current:   $CURRENT_BACKEND"
  echo "  backend rollback:  $ROLLBACK_BACKEND"
  echo "  dashboard current: $CURRENT_DASHBOARD"
  echo "  dashboard rollback:$ROLLBACK_DASHBOARD"
  echo
  echo "To execute:"
  echo "  OA_KHALA_ROLLBACK_DRILL_APPLY=1 $SCRIPT_DIR/run-rollback-drill.sh"
  exit 0
fi

RESTORE_NEEDED=0
restore_current_revisions() {
  if [[ "$RESTORE_NEEDED" != "1" ]]; then
    return 0
  fi

  gcloud run services update-traffic "$KHALA_BACKEND_SERVICE" \
    --project "$PROJECT_ID" \
    --region "$REGION" \
    --to-revisions "${CURRENT_BACKEND}=100" >/dev/null || true

  gcloud run services update-traffic "$KHALA_DASHBOARD_SERVICE" \
    --project "$PROJECT_ID" \
    --region "$REGION" \
    --to-revisions "${CURRENT_DASHBOARD}=100" >/dev/null || true
}
trap restore_current_revisions EXIT

DRILL_START="$(date -u +%s)"

# Switch traffic to previous ready revisions.
gcloud run services update-traffic "$KHALA_BACKEND_SERVICE" \
  --project "$PROJECT_ID" \
  --region "$REGION" \
  --to-revisions "${ROLLBACK_BACKEND}=100" >/dev/null

gcloud run services update-traffic "$KHALA_DASHBOARD_SERVICE" \
  --project "$PROJECT_ID" \
  --region "$REGION" \
  --to-revisions "${ROLLBACK_DASHBOARD}=100" >/dev/null

RESTORE_NEEDED=1

"$SCRIPT_DIR/check-nonprod-health.sh" >/dev/null
ROLLBACK_READY="$(date -u +%s)"

# Restore original revisions.
gcloud run services update-traffic "$KHALA_BACKEND_SERVICE" \
  --project "$PROJECT_ID" \
  --region "$REGION" \
  --to-revisions "${CURRENT_BACKEND}=100" >/dev/null

gcloud run services update-traffic "$KHALA_DASHBOARD_SERVICE" \
  --project "$PROJECT_ID" \
  --region "$REGION" \
  --to-revisions "${CURRENT_DASHBOARD}=100" >/dev/null

"$SCRIPT_DIR/check-nonprod-health.sh" >/dev/null
RESTORE_READY="$(date -u +%s)"
RESTORE_NEEDED=0

ROLLBACK_RTO_SECONDS=$((ROLLBACK_READY - DRILL_START))
RESTORE_SECONDS=$((RESTORE_READY - ROLLBACK_READY))
TOTAL_SECONDS=$((RESTORE_READY - DRILL_START))

echo "Rollback drill completed."
echo "Project:               $PROJECT_ID"
echo "Region:                $REGION"
echo "Backend current:       $CURRENT_BACKEND"
echo "Backend rollback:      $ROLLBACK_BACKEND"
echo "Dashboard current:     $CURRENT_DASHBOARD"
echo "Dashboard rollback:    $ROLLBACK_DASHBOARD"
echo "Rollback RTO seconds:  $ROLLBACK_RTO_SECONDS"
echo "Restore seconds:       $RESTORE_SECONDS"
echo "Total drill seconds:   $TOTAL_SECONDS"
