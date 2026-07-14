#!/usr/bin/env bash
set -euo pipefail

# Deploy the openagents.com monolith to Google Cloud Run (CFG-9, #8524).
#
# Usage:
#   scripts/deploy-cloudrun.sh staging      # openagents-monolith-staging
#   scripts/deploy-cloudrun.sh production   # openagents-monolith
#
# Build happens here on the deploy machine (aiur/oa-updates pattern):
#   1. pnpm run build:start (TanStack Start client + server artifacts)
#   2. vp pack src/cloudrun/server.ts + preload.ts → dist-cloudrun/
#   3. render the non-secret env YAML from wrangler.jsonc vars
#   4. gcloud run deploy --source . (Dockerfile in this directory)
#
# Secrets ride --set-secrets from GCP Secret Manager (created out of band —
# never from tracked files; see the CFG-9 secret map on issue #8524):
#   openagents-monolith-database-url-<env>   direct Cloud SQL Postgres URL
#                                            (khala_app; kills Hyperdrive)
#   openagents-monolith-cron-token-<env>     bearer for POST /internal/cron
#   openagents-monolith-admin-token-<env>    OPENAGENTS_ADMIN_API_TOKEN
#   openagents-audio-token-secret            shared AUDIO-2 grant HMAC
#   khala-live-hub-token                     shared LiveHub service bearer
#   openagents-gemini-api-key / openagents-openrouter-api-key /
#   openagents-fireworks-api-key / openagents-exa-api-key /
#   openagents-resend-api-key / openagents-vertex-sa-key
#   openagents-github-client-secret          (production only; NEEDS-OWNER
#                                            until re-supplied — see #8524)
#
# Cloud Scheduler: pass --with-scheduler to (re)create the per-minute
# /internal/cron job for the target env after deploy.

TARGET="${1:-}"
if [[ "$TARGET" != "staging" && "$TARGET" != "production" ]]; then
  echo "usage: $0 (staging|production) [--with-scheduler]" >&2
  exit 2
fi
WITH_SCHEDULER="${2:-}"

PROJECT="${OPENAGENTS_GCP_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_GCP_REGION:-us-central1}"

if [[ "$TARGET" == "production" ]]; then
  SERVICE="openagents-monolith"
  ENV_SUFFIX="prod"
else
  SERVICE="openagents-monolith-staging"
  ENV_SUFFIX="staging"
fi

API_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$(cd "$API_DIR/../.." && pwd)"   # apps/openagents.com
REPO_ROOT="$(cd "$API_DIR/../../../.." && pwd)"

cd "$APP_DIR"
echo "==> Building retained Start application (apps/start/dist)"
pnpm run build:start >/dev/null

cd "$API_DIR"
echo "==> Bundling Node server + preload (Vite Plus pack)"
rm -rf dist-cloudrun
vp pack src/cloudrun/server.ts --out-dir dist-cloudrun --format esm \
  --platform node --target node24 >/dev/null
vp pack src/cloudrun/preload.ts src/cloudrun/cloudflare-workers-stub.ts \
  --out-dir dist-cloudrun --format esm --platform node --target node24 \
  --no-clean >/dev/null
# `vp pack` cleans its output directory by default. The preload pass must
# preserve server.mjs and any split server chunks produced by the first pass.
if [[ ! -f dist-cloudrun/server.mjs || ! -f dist-cloudrun/preload.mjs || \
      ! -f dist-cloudrun/cloudflare-workers-stub.mjs ]]; then
  echo "FATAL: Vite Plus Cloud Run bundles are incomplete" >&2
  exit 1
fi

# T3 Code's `vp pack` pattern bundles selected app packages and ships runtime
# dependencies with the app. This image is built from workers/api alone, so
# stage a portable production node_modules tree into the Cloud Build context.
RUNTIME_DEPLOY_DIR="$(mktemp -d)"
START_RUNTIME_DEPLOY_DIR=""
trap 'rm -rf "$RUNTIME_DEPLOY_DIR" "${START_RUNTIME_DEPLOY_DIR:-}"' EXIT
(cd "$REPO_ROOT" && CI=true pnpm --config.node-linker=hoisted \
  --filter @openagentsinc/api-worker deploy "$RUNTIME_DEPLOY_DIR" \
  --prod --legacy >/dev/null)
mv "$RUNTIME_DEPLOY_DIR/node_modules" dist-cloudrun/node_modules
START_RUNTIME_DEPLOY_DIR="$(mktemp -d)"
(cd "$REPO_ROOT" && CI=true pnpm --config.node-linker=hoisted \
  --filter @openagentsinc/openagents-com-start deploy "$START_RUNTIME_DEPLOY_DIR" \
  --prod --legacy >/dev/null)
cp -R "$START_RUNTIME_DEPLOY_DIR/node_modules/." dist-cloudrun/node_modules/
# Legacy deploy mutates the workspace install mode while materializing the
# portable tree. Restore the development install before later build/smoke
# commands invoke pnpm again.
(cd "$REPO_ROOT" && CI=true pnpm install --frozen-lockfile >/dev/null)
node scripts/cloudrun/assert-self-contained-bundle.mjs dist-cloudrun
cp -R "$APP_DIR/apps/start/dist/client" dist-cloudrun/start-client
cp -R "$APP_DIR/apps/start/dist/server" dist-cloudrun/start-server
# Sarah removed at owner direction 2026-07-10 (epic #8610): the former
# sarah-ui / sarah-agent / sarah-clips bundle steps are gone with apps/sarah.
# #8652 PORTAL-1: /portal Effect Native bundle (authored in apps/start).
mkdir -p dist-cloudrun/portal-ui
(cd "$APP_DIR" && vp pack apps/start/src/portal-entry.ts --platform browser \
  --format iife --target es2022 --minify \
  --out-dir "$API_DIR/dist-cloudrun/portal-ui") >/dev/null
mv dist-cloudrun/portal-ui/portal-entry.iife.js dist-cloudrun/portal-ui/app.js
# #8634/#8635 scope 5: /forum* Effect Native bundle (authored in apps/start).
mkdir -p dist-cloudrun/forum-ui
(cd "$APP_DIR" && vp pack apps/start/src/forum-entry.ts --platform browser \
  --format iife --target es2022 --minify \
  --out-dir "$API_DIR/dist-cloudrun/forum-ui") >/dev/null
mv dist-cloudrun/forum-ui/forum-entry.iife.js dist-cloudrun/forum-ui/app.js

echo "==> Rendering env vars from wrangler.jsonc ($TARGET)"
node --import tsx scripts/cloudrun/render-env-yaml.ts "$TARGET"

SET_SECRETS=(
  "KHALA_SYNC_DATABASE_URL=openagents-monolith-database-url-${ENV_SUFFIX}:latest"
  "CLOUD_RUN_CRON_TOKEN=openagents-monolith-cron-token-${ENV_SUFFIX}:latest"
  "OPENAGENTS_ADMIN_API_TOKEN=openagents-monolith-admin-token-${ENV_SUFFIX}:latest"
  "KHALA_SYNC_LIVE_HUB_TOKEN=khala-live-hub-token:latest"
  "GEMINI_API_KEY=openagents-gemini-api-key:latest"
  # Sarah removed 2026-07-10 (epic #8610). The following Secret Manager
  # entries are retained in GCP as history but are no longer mounted:
  #   sarah-liveavatar-api-key, sarah-avatar-llm-bearer,
  #   sarah-avatar-llm-config-id, sarah-avatar-id,
  #   sarah-render-service-token, sarah-tts-service-token,
  #   sarah-inference-gateway-token(-staging)
  # OPENROUTER_API_KEY is DROPPED on BOTH staging AND production (owner decision
  # 2026-07-09): OpenRouter is no longer a platform Khala supply lane — it was
  # removed from every plan in model-router.ts and the primary lane is now our
  # own Google Cloud (Vertex). Omitting the secret means the platform lane cannot
  # silently re-lead even if a plan is later mis-edited (same staging-proven
  # omission pattern from AC-1 #8503). The OpenRouter ADAPTER stays registered
  # for BYOK caller keys, which supply the caller's own key per request and never
  # need this platform secret. See
  # docs/incidents/2026-07-08-khala-502-openrouter-credit-exhaustion-aar.md.
  "FIREWORKS_API_KEY=openagents-fireworks-api-key:latest"
  "EXA_API_KEY=openagents-exa-api-key:latest"
  "RESEND_API_KEY=openagents-resend-api-key:latest"
  "VERTEX_SA_KEY=openagents-vertex-sa-key:latest"
  # Cloud coding sessions control-plane bearer (oa-cloud-run-bridge).
  "OA_CLOUD_CONTROL_TOKEN=oa-cloud-run-bridge-control-token:latest"
  # CFG-8 GCS artifacts (bucket name is a committed wrangler var).
  "ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID=oa-artifacts-gcs-hmac-access-key-id:latest"
  "ARTIFACTS_GCS_HMAC_SECRET=oa-artifacts-gcs-hmac-secret:latest"
  "AGENT_REGISTRATION_SECRET=openagents-agent-registration-secret:latest"
  # Encrypted provider-account custody (Codex/Claude local-auth imports and
  # per-turn Agent Computer materialization). Raw auth material never enters
  # Cloud Run env files or source; only this Secret Manager key is mounted.
  "PROVIDER_TOKEN_CUSTODY_AES_KEY_B64=provider-token-custody-aes-key-b64:latest"
  "ARTANIS_AGENT_TOKEN=openagents-artanis-agent-token:latest"
  # Desktop audio grants are issued by this monolith and verified by the
  # grant-gated audio edge. Keep the shared HMAC mounted across every deploy.
  "OPENAGENTS_AUDIO_TOKEN_SECRET=openagents-audio-token-secret:latest"
  # CFG-14 (2026-07-07): khala_app password for the Cloud SQL Auth Connector
  # socket path (PGHOST/PGUSER are non-secret wrangler vars; the db name rides
  # the authority-less KHALA_SYNC_DATABASE_URL secret). khala_app is
  # instance-wide, so the same secret serves prod + staging. WITHOUT this the
  # deploy drops PGPASSWORD and the socket connection fails — and with public
  # ingress closed the DB is unreachable.
  "PGPASSWORD=openagents-monolith-pgpassword:latest"
)

if [[ "$TARGET" == "production" ]]; then
  SET_SECRETS+=(
    # OPENROUTER_API_KEY was dropped here (owner decision 2026-07-09) — OpenRouter
    # is no longer a platform Khala lane on prod OR staging. See the common block
    # above. The BYOK caller-key path does not need this platform secret.
    "GITHUB_CLIENT_SECRET=openagents-github-client-secret:latest"
    # SHC live dispatch (config validation requires the bearer when
    # SHC_DISPATCH_MODE=live).
    "SHC_CONTROL_API_BEARER_TOKEN=openagents-shc-control-api-bearer:latest"
    "SHC_RUNNER_CALLBACK_TOKEN=openagents-shc-runner-callback-token:latest"
    # D1-over-HTTP bridge for not-yet-migrated CFG-4 domains (typed 503 when
    # the daily free-tier quota is exhausted — see #8524).
    "CLOUDFLARE_API_TOKEN=openagents-monolith-cf-d1-token:latest"
    # Hydralisk GPT-OSS lanes (secondary; 120B base URL is CF-only — see the
    # #8524 NEEDS-OWNER catalogue).
    "HYDRALISK_BASE_URL=hydralisk-gptoss20b-base-url:latest"
    "HYDRALISK_BEARER_TOKEN=hydralisk-gptoss20b-bearer:latest"
    "HYDRALISK_GPT_OSS_120B_BEARER_TOKEN=hydralisk-gptoss120b-bearer:latest"
  )
  # Hydralisk GLM-5.2-REAP-504B fleet (the Khala primary backing): one
  # BASE_URL + BEARER_TOKEN pair per replica id from the committed
  # HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS wrangler var.
  GLM_REPLICAS=(
    g4-4g-b-20260625154532
    g4-4g-central1f-spot-20260625203000
    g4-4g-east1b-spot-20260625203000
    g4-4g-east1d-spot-20260625203000
    g4-4g-east5a-spot-20260625203000
    g4-4g-east5b-spot-20260625203000
    g4-4g-east5c-spot-20260625211500
    g4-4g-south1b-spot-20260625211500
    g4-4g-west1a-spot-20260625203000
    g4-8g-b-20260624214500
  )
  for replica in "${GLM_REPLICAS[@]}"; do
    suffix="$(echo "$replica" | tr '[:lower:]-' '[:upper:]_')"
    SET_SECRETS+=(
      "HYDRALISK_GLM_52_REAP_504B_${suffix}_BASE_URL=hydralisk-glm-${replica}-base-url:latest"
      "HYDRALISK_GLM_52_REAP_504B_${suffix}_BEARER_TOKEN=hydralisk-glm-${replica}-bearer:latest"
    )
  done
else
  # Staging-only secrets.
  SET_SECRETS+=(
    # AC-1 (#8503): the org-cloud runtime no-meter shared secret. A request
    # carrying x-openagents-org-cloud-runtime-no-meter matching this value
    # bypasses metering + the customer balance(402)/spend-cap gates so the
    # in-microVM gateway inference call is org-capacity (no_debit). PROD stays
    # fail-closed by NOT mounting this secret (bypass inert without it). Durable
    # across redeploys — do not rely on live-revision arming.
    "OA_CLOUD_RUNTIME_NO_METER_SECRET=oa-cloud-runtime-no-meter-secret-staging:latest"
  )
fi

SECRET_FLAG="$(IFS=,; echo "${SET_SECRETS[*]}")"

echo "==> Deploying $SERVICE to Cloud Run ($REGION)"
gcloud run deploy "$SERVICE" \
  --project "$PROJECT" \
  --source . \
  --region "$REGION" \
  --allow-unauthenticated \
  --port 8080 \
  --min-instances 1 \
  --max-instances 4 \
  --cpu 2 \
  --memory 2Gi \
  --timeout 3600 \
  --concurrency 80 \
  --env-vars-file "dist-cloudrun/env-${TARGET}.yaml" \
  --set-secrets "$SECRET_FLAG" \
  --add-cloudsql-instances "openagentsgemini:us-central1:khala-sync-pg"

SERVICE_URL="$(gcloud run services describe "$SERVICE" --project "$PROJECT" --region "$REGION" --format='value(status.url)')"
echo "==> Deployed: $SERVICE_URL"

if [[ "$WITH_SCHEDULER" == "--with-scheduler" ]]; then
  echo "==> Ensuring per-minute Cloud Scheduler cron ($SERVICE-cron)"
  CRON_TOKEN="$(gcloud secrets versions access latest --secret "openagents-monolith-cron-token-${ENV_SUFFIX}" --project "$PROJECT")"
  gcloud scheduler jobs delete "$SERVICE-cron" --project "$PROJECT" --location "$REGION" --quiet 2>/dev/null || true
  # NOTE(#8564): `--format='value(name)'` is REQUIRED, not cosmetic. Without it
  # `gcloud scheduler jobs create` prints the created job resource to stdout,
  # including `httpTarget.headers.Authorization: "Bearer <CRON_TOKEN>"` — i.e.
  # it would leak the live cron bearer into the deploy log / CI output / any
  # captured task transcript. Narrowing the output to just the job name keeps
  # the secret out of stdout. Do not remove.
  gcloud scheduler jobs create http "$SERVICE-cron" \
    --project "$PROJECT" \
    --location "$REGION" \
    --schedule "* * * * *" \
    --uri "${SERVICE_URL}/internal/cron" \
    --http-method POST \
    --headers "Authorization=Bearer ${CRON_TOKEN}" \
    --attempt-deadline 300s \
    --format='value(name)'
fi

echo "==> Smoke: /internal/healthz"
curl -fsS "${SERVICE_URL}/internal/healthz"
echo

# Sarah removed 2026-07-10 (epic #8610): the avatar e2e smoke died with the
# surface. Prove the tombstone instead: /sarah must be 404, never a page.
echo "==> Smoke: /sarah tombstone (expect 404)"
SARAH_STATUS="$(curl -s -o /dev/null -w '%{http_code}' "${SERVICE_URL}/sarah")"
if [[ "$SARAH_STATUS" != "404" ]]; then
  echo "FATAL: GET /sarah returned $SARAH_STATUS (expected 404 — Sarah is removed)" >&2
  exit 1
fi
echo "==> /sarah -> 404 confirmed"

# Portal REAL-BROWSER smoke (#8652 reopen): headless Chromium loads /portal on
# the deployed service and proves the logged-out login gate pixel-level
# (screenshot receipt). Curl checks alone shipped a broken owner-visible state
# once; never again. Logged-in states (empty-with-identity / engagement) are
# the pre-owner-handoff gate — see docs/DEPLOYMENT.md and
# scripts/portal-browser-smoke.ts. Requires `pnpm exec playwright install chromium`
# once per machine; skip with PORTAL_SKIP_BROWSER_SMOKE=1.
if [[ "${PORTAL_SKIP_BROWSER_SMOKE:-}" != "1" ]]; then
  echo "==> Smoke: portal real-browser (logged-out login gate)"
  pnpm exec tsx "$API_DIR/scripts/portal-browser-smoke.ts" \
    --base-url "$SERVICE_URL" \
    --state logged-out \
    --out-dir "${PORTAL_SMOKE_OUT_DIR:-/tmp/portal-smoke-${TARGET}}"
fi
echo "==> Done."
