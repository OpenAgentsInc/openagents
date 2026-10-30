#!/bin/sh
# Cloud Run authenticates callers with IAM before this private listener.
# The browser/API caller's credentials never enter the Nostr mesh.
set -eu
: "${OA_MESH_IDENTITY:?deployment must pin a checkpoint}"
: "${OA_MESH_PROVIDER:?deployment must pin a provider}"
exec /usr/local/bin/psionic-fleet --identity "$OA_MESH_IDENTITY" \
  --provider "$OA_MESH_PROVIDER" --listen-host 0.0.0.0 --port "${PORT:-8080}" \
  --timeout-ms 25000 --attempt-ms 20000 --discover-ms 5000
