#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

BASE_URL="${OPENAGENTS_EPISODE_223_NEXUS_BASE_URL:-https://nexus.openagents.com}"
ARTIFACT_BUCKET_URI="${OPENAGENTS_EPISODE_223_ARTIFACT_BUCKET_URI:-}"
DESKTOP_CLIENT_ID="${OPENAGENTS_EPISODE_223_DESKTOP_CLIENT_ID:-episode223-cs336-a1-seeder}"
DEVICE_NAME="${OPENAGENTS_EPISODE_223_DEVICE_NAME:-Episode 223 Seeder}"
CLIENT_VERSION="${OPENAGENTS_EPISODE_223_CLIENT_VERSION:-episode223-cs336-a1-seeder/v1}"

if [[ -z "$ARTIFACT_BUCKET_URI" ]]; then
  if [[ "$BASE_URL" == "https://nexus.openagents.com" ]]; then
    ARTIFACT_BUCKET_URI="gs://openagentsgemini-openagents-training-prod"
  else
    ARTIFACT_BUCKET_URI="gs://bucket"
  fi
fi

echo "Seeding Episode 223 CS336 A1 Demo"
echo "  base_url=$BASE_URL"
echo "  artifact_bucket_uri=$ARTIFACT_BUCKET_URI"

cargo run -p nexus-control --bin episode223-seed-cs336-a1-demo -- \
  --base-url "$BASE_URL" \
  --artifact-bucket-uri "$ARTIFACT_BUCKET_URI" \
  --desktop-client-id "$DESKTOP_CLIENT_ID" \
  --device-name "$DEVICE_NAME" \
  --client-version "$CLIENT_VERSION"
