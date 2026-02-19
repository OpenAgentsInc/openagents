#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

PROJECT_ID="${PROJECT_ID:-openagentsgemini}"
REGION="${REGION:-us-central1}"
CONVEX_BACKEND_SERVICE="${CONVEX_BACKEND_SERVICE:-oa-convex-backend-nonprod}"
CONVEX_ADMIN_KEY_SECRET="${CONVEX_ADMIN_KEY_SECRET:-oa-convex-nonprod-admin-key}"
CONVEX_CLI_DIR="${CONVEX_CLI_DIR:-$REPO_ROOT/apps/mobile}"
DRILL_OUTPUT_DIR="${DRILL_OUTPUT_DIR:-/tmp}"

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 1
  fi
}

require_cmd gcloud
require_cmd npx
require_cmd mktemp

if [[ ! -d "$CONVEX_CLI_DIR" ]]; then
  echo "CONVEX_CLI_DIR does not exist: $CONVEX_CLI_DIR" >&2
  exit 1
fi

if [[ ! -f "$CONVEX_CLI_DIR/package.json" ]]; then
  echo "CONVEX_CLI_DIR must contain package.json: $CONVEX_CLI_DIR" >&2
  exit 1
fi

mkdir -p "$DRILL_OUTPUT_DIR"

BACKEND_URL="$(gcloud run services describe "$CONVEX_BACKEND_SERVICE" --project "$PROJECT_ID" --region "$REGION" --format='value(status.url)')"
ADMIN_KEY="$(gcloud secrets versions access latest --secret="$CONVEX_ADMIN_KEY_SECRET" --project "$PROJECT_ID")"

ENV_FILE="$(mktemp /tmp/convex-self-hosted-env-XXXXXX)"
trap 'rm -f "$ENV_FILE"' EXIT

cat > "$ENV_FILE" <<ENV
CONVEX_SELF_HOSTED_URL="$BACKEND_URL"
CONVEX_SELF_HOSTED_ADMIN_KEY="$ADMIN_KEY"
ENV

TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
EXPORT_PATH="$DRILL_OUTPUT_DIR/convex-nonprod-export-$TIMESTAMP.zip"

START_TS="$(date -u +%s)"
(
  cd "$CONVEX_CLI_DIR"
  npx convex export --env-file "$ENV_FILE" --path "$EXPORT_PATH"
  npx convex import --env-file "$ENV_FILE" --append "$EXPORT_PATH"
)
END_TS="$(date -u +%s)"

DURATION_SECONDS=$((END_TS - START_TS))

echo "Backup/restore drill completed."
echo "Project:           $PROJECT_ID"
echo "Region:            $REGION"
echo "Backend URL:       $BACKEND_URL"
echo "Convex CLI dir:    $CONVEX_CLI_DIR"
echo "Export snapshot:   $EXPORT_PATH"
echo "Duration seconds:  $DURATION_SECONDS"
