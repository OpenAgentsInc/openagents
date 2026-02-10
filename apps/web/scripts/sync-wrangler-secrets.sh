#!/usr/bin/env bash
# Push env vars from .env.production to Cloudflare Worker secrets.
# Run from apps/web: npm run wrangler:secrets
# Requires: .env.production present with the required keys set.

set -euo pipefail
cd "$(dirname "$0")/.."

ENV_FILE=".env.production"
if [[ ! -f "$ENV_FILE" ]]; then
  echo "Missing $ENV_FILE. Create it with WORKOS_CLIENT_ID, WORKOS_API_KEY, etc." >&2
  exit 1
fi

# Load .env.production (export KEY=value or KEY=value)
set -a
# shellcheck source=/dev/null
source "$ENV_FILE"
set +a

# Secrets the Worker needs at runtime (from wrangler.jsonc comments + AuthKit)
# PRELAUNCH_BYPASS_KEY: optional; when set, ?key={value} on /autopilot bypasses prelaunch (set via wrangler secret put PRELAUNCH_BYPASS_KEY)
SECRET_NAMES=(
  WORKOS_CLIENT_ID
  WORKOS_API_KEY
  WORKOS_COOKIE_PASSWORD
  AUTOPILOT_CODEX_SECRET
  OA_INTERNAL_KEY
  # Inference
  OPENROUTER_API_KEY
  # E2E auth bypass + Convex JWT minting (prod testing)
  OA_E2E_BYPASS_SECRET
  OA_E2E_JWT_PRIVATE_JWK
  # Headless DSE ops (overnight runner)
  OA_DSE_ADMIN_SECRET
  PUBLIC_API_URL
  PRELAUNCH_BYPASS_KEY
)

for name in "${SECRET_NAMES[@]}"; do
  value="${!name:-}"
  if [[ -z "$value" ]]; then
    echo "Skip $name (not set in $ENV_FILE)"
    continue
  fi
  echo "Setting secret: $name"
  printf '%s' "$value" | npx wrangler secret put "$name"
done

echo "Done. Deploy worker with: npm run deploy (or wrangler deploy after build)"
