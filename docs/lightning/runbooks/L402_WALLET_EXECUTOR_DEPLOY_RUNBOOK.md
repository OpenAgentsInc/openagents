# L402 Wallet Executor Deploy Runbook

Status: Active  
Date: 2026-02-12  
Scope: Deploy `apps/lightning-wallet-executor` to Cloud Run and wire `apps/web` Worker runtime.

## 1. Overview

`apps/lightning-wallet-executor` is a narrow HTTP service that executes wallet operations for L402 buyer flows:

1. `GET /healthz`
2. `GET /status`
3. `POST /pay-bolt11`

It is designed to keep payment execution separate from browser code and Worker orchestration logic.

## 2. Required Secrets / Env

GCP Secret Manager secrets (project `openagentsgemini`):

1. `l402-wallet-executor-spark-api-key`
2. `l402-wallet-executor-mnemonic`
3. `l402-wallet-executor-auth-token`

Cloud Run env vars:

1. `OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark`
2. `OA_LIGHTNING_WALLET_EXECUTOR_HOST=0.0.0.0`
3. `OA_LIGHTNING_WALLET_EXECUTOR_PORT=8080`
4. `OA_LIGHTNING_SPARK_NETWORK=mainnet`
5. `OA_LIGHTNING_WALLET_ALLOWED_HOSTS=sats4ai.com,l402.openagents.com`
6. `OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=gcp`
7. `OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION=projects/openagentsgemini/secrets/l402-wallet-executor-mnemonic/versions/latest`
8. `OA_LIGHTNING_WALLET_ID=openagents-ep212-agent`

Cloud Run secret env mappings:

1. `OA_LIGHTNING_SPARK_API_KEY=l402-wallet-executor-spark-api-key:latest`
2. `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN=l402-wallet-executor-auth-token:latest`

## 3. Build + Push Image

From repo root:

```bash
gcloud builds submit --project openagentsgemini \
  --config docs/lightning/deploy/cloudbuild-wallet-executor.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" .
```

Image pushed:

- `us-central1-docker.pkg.dev/openagentsgemini/l402/wallet-executor:latest`

## 4. Deploy Cloud Run Service

Deploy in `us-central1`:

```bash
gcloud run deploy l402-wallet-executor \
  --project openagentsgemini \
  --region us-central1 \
  --platform managed \
  --image us-central1-docker.pkg.dev/openagentsgemini/l402/wallet-executor:latest \
  --allow-unauthenticated \
  --set-env-vars OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark,OA_LIGHTNING_WALLET_EXECUTOR_HOST=0.0.0.0,OA_LIGHTNING_WALLET_EXECUTOR_PORT=8080,OA_LIGHTNING_SPARK_NETWORK=mainnet,OA_LIGHTNING_WALLET_ALLOWED_HOSTS=sats4ai.com,l402.openagents.com,OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER=gcp,OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION=projects/openagentsgemini/secrets/l402-wallet-executor-mnemonic/versions/latest,OA_LIGHTNING_WALLET_ID=openagents-ep212-agent \
  --set-secrets OA_LIGHTNING_SPARK_API_KEY=l402-wallet-executor-spark-api-key:latest,OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN=l402-wallet-executor-auth-token:latest \
  --memory 1Gi \
  --cpu 1 \
  --port 8080 \
  --min-instances 0 \
  --max-instances 2
```

## 5. Wire `apps/web` Worker Runtime

Set Cloudflare Worker secrets for `autopilot-web`:

```bash
cd apps/web
printf '%s' 'https://<l402-wallet-executor-url>' | npx wrangler secret put OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL --name autopilot-web
printf '%s' '<same-bearer-token-as-gcp-secret>' | npx wrangler secret put OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN --name autopilot-web
printf '%s' '60000' | npx wrangler secret put OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS --name autopilot-web
printf '%s' 'sats4ai.com,l402.openagents.com' | npx wrangler secret put OA_LIGHTNING_L402_ALLOWED_HOSTS --name autopilot-web
npm run deploy:worker
```

## 6. Verification

Health/status:

```bash
curl -i -H "Authorization: Bearer <token>" https://<l402-wallet-executor-url>/status
```

Note: on Cloud Run, `/healthz` may be intercepted and return a platform 404 before app routing. Use `/status` as the operator readiness probe.

EP212 route smoke:

```bash
OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="https://<l402-wallet-executor-url>" \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="<token>" \
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:ep212-routes --json --mode live
```

## 7. Notes

1. If wallet balance is insufficient, `/status` may be healthy while paid route smoke fails at payment step.
2. Keep `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN` set to require bearer auth on all non-health endpoints.
3. Rotate mnemonic/API key/token through Secret Manager and redeploy when credentials change.
