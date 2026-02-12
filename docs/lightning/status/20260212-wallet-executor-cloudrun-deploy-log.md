# Wallet Executor Cloud Run Deploy Log (2026-02-12)

Owner: Codex (terminal session)  
Scope: Deploy `apps/lightning-wallet-executor`, wire `apps/web` Worker secrets, verify end-to-end readiness for hosted L402 buyer flow.

## 1. Code and Infra Changes Made

## 1.1 Executor security/runtime

1. Added optional bearer auth enforcement in wallet executor:
   - `apps/lightning-wallet-executor/src/runtime/config.ts`
     - new config field: `authToken: string | null`
     - env: `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`
   - `apps/lightning-wallet-executor/src/http/server.ts`
     - `/status` and `/pay-bolt11` now require `Authorization: Bearer <token>` when token configured
     - `/healthz` left unauthenticated in app code
2. Added integration test coverage:
   - `apps/lightning-wallet-executor/test/http-server.integration.test.ts`
   - new test: `enforces bearer auth when configured`

## 1.2 Executor packaging/deploy assets

1. Added wallet executor container build assets:
   - `docs/lightning/deploy/Dockerfile.wallet-executor`
   - `docs/lightning/deploy/cloudbuild-wallet-executor.yaml`
2. Updated deploy docs index:
   - `docs/lightning/deploy/README.md`
3. Added dedicated runbook:
   - `docs/lightning/runbooks/L402_WALLET_EXECUTOR_DEPLOY_RUNBOOK.md`

## 1.3 Worker secret sync plumbing

1. Updated secret sync script:
   - `apps/web/scripts/sync-wrangler-secrets.sh`
   - includes `OA_LIGHTNING_WALLET_EXECUTOR_*` and `OA_LIGHTNING_L402_*` keys
2. Updated production env example:
   - `apps/web/.env.production.example`
   - added optional wallet executor variables

## 1.4 Build/runtime dependency fix

1. Added `better-sqlite3` dependency required by Breez Spark Node storage:
   - `apps/lightning-wallet-executor/package.json`
   - `apps/lightning-wallet-executor/package-lock.json`
2. Added container native build deps for `better-sqlite3`:
   - `python3`, `make`, `g++` in `Dockerfile.wallet-executor`

## 2. Verification Run (Local CI-Style)

1. Executor package checks:
   - `cd apps/lightning-wallet-executor`
   - `npm run typecheck`
   - `npm test`
   - Result: PASS
2. Worker integration regression:
   - `cd apps/web`
   - `npm test -- tests/worker/lightning-tool-chat.test.ts`
   - Result: PASS

## 3. GCP Actions Performed

Project: `openagentsgemini`  
Region: `us-central1`

## 3.1 Created/updated Secret Manager secrets

1. `l402-wallet-executor-spark-api-key` (version added)
2. `l402-wallet-executor-mnemonic` (version added)
3. `l402-wallet-executor-auth-token` (version added)

## 3.2 Built/pushed wallet executor image

Command used:

```bash
gcloud builds submit --project openagentsgemini \
  --config docs/lightning/deploy/cloudbuild-wallet-executor.yaml \
  --substitutions _TAG=<git-short-sha> .
```

Latest successful build id:

- `4781fa8e-4d02-4cd9-9189-b1952b1f0e66`

Image:

- `us-central1-docker.pkg.dev/openagentsgemini/l402/wallet-executor:latest`

## 3.3 Deployed Cloud Run service

Service:

- `l402-wallet-executor`
- URL: `https://l402-wallet-executor-157437760789.us-central1.run.app`
- Revision: `l402-wallet-executor-00003-zfq`

Runtime mode/env:

1. `OA_LIGHTNING_WALLET_EXECUTOR_MODE=spark`
2. `OA_LIGHTNING_SPARK_NETWORK=mainnet`
3. `OA_LIGHTNING_WALLET_ALLOWED_HOSTS=sats4ai.com,l402.openagents.com`
4. mnemonic provider = GCP secret version reference

Secret env bindings:

1. `OA_LIGHTNING_SPARK_API_KEY` -> `l402-wallet-executor-spark-api-key:latest`
2. `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN` -> `l402-wallet-executor-auth-token:latest`

## 3.4 Cloud Run verification

1. `/status` with bearer token: HTTP 200
2. `/status` without bearer token: HTTP 401
3. `/pay-bolt11` invalid invoice with bearer token: HTTP 502 with typed Spark error JSON
4. Wallet status snapshot currently reports:
   - `ready: true`
   - `mode: spark`
   - `network: mainnet`
   - `balanceSats: 0`

## 4. Cloudflare Worker Wiring Performed

Worker: `autopilot-web`

Secrets set:

1. `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL`
2. `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN`
3. `OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS`
4. `OA_LIGHTNING_L402_ALLOWED_HOSTS`

Deploy command executed:

```bash
cd apps/web
npm run deploy:worker
```

Deployed version id:

- `cbf7a9e9-8283-49b9-a5f6-0a0f96cff357`

## 5. Live Smoke Outcome (Current)

Command executed:

```bash
cd apps/lightning-ops
OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL=<cloud-run-url> \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN=<token> \
npm run smoke:ep212-routes -- --json --mode live
```

Result:

- FAIL at challenge stage due endpoint response mismatch:
  - expected 402 challenge
  - got 400 from `https://l402.openagents.com/ep212/premium-signal`
  - response body: `Client sent an HTTP request to an HTTPS server.`

Additional observed blocker for paid live flow:

- executor wallet balance currently `0 sats`, so real paid route success is not possible until funded.
- `smoke:ep212-full-flow --mode live` fails on first real payment attempt with:
  - `spark send payment failed ... insufficient funds`
  - confirms executor path is live and trying to pay, but funding is missing.

## 6. Remaining Work Required for Fully Live Paid Demo

1. Fix Aperture/OpenAgents route A (`/ep212/premium-signal`) so it returns proper L402 challenge (402 + `WWW-Authenticate`) instead of HTTP/HTTPS mismatch 400.
2. Fund the deployed Spark wallet used by `l402-wallet-executor` so invoice payment succeeds.
3. Re-run:
   - `npm run smoke:ep212-routes -- --json --mode live`
   - `npm run smoke:ep212-full-flow -- --json --mode live`
4. Capture final successful artifacts in this status folder once route + funding are complete.
