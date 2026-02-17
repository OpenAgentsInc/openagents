# Laravel Staging E2E Validation (EP212/L402 API)

Date: 2026-02-17 (UTC)

## Environment

- App service: `openagents-web-staging`
- URL: `https://openagents-web-staging-ezxz4mgdsq-uc.a.run.app`
- Cloud Run revision: `openagents-web-staging-00005-bvs`
- Project / region: `openagentsgemini` / `us-central1`
- DB: `openagents_web_staging` (Cloud SQL instance `l402-aperture-db`)
- Wallet executor: `https://l402-wallet-executor-ezxz4mgdsq-uc.a.run.app`
- Invoice payer mode: `spark_wallet`
- API signup bootstrap: enabled (`OA_API_SIGNUP_ENABLED=true`)
- Allowed signup domains: `openagents.com`
- Staging debug mode: `APP_DEBUG=0`

## Changes Applied During This Run

1. Fixed staging API chat 500 due to missing PostHog API key:
   - Updated `apps/openagents.com/app/Services/PostHogService.php` to no-op safely when PostHog is disabled **or** `POSTHOG_API_KEY` is blank.

2. Added regression coverage:
   - `apps/openagents.com/tests/Feature/Api/V1/ChatApiTest.php`
   - New test verifies `/api/chats` creation still works when PostHog is enabled but API key is empty.

3. Ensured Docker image applies OpenAPI vendor patch in image builds:
   - Updated `apps/openagents.com/Dockerfile` to run `php scripts/patch-oooas.php` in both build/runtime stages.
   - This removed deprecation noise leaking into JSON API responses in staging.

4. Updated API/OpenAPI parity fixtures:
   - `apps/openagents.com/tests/Feature/Api/V1/ApiRouteCoverageManifestTest.php`
   - `apps/openagents.com/tests/Feature/Api/V1/OpenApiGenerationTest.php`
   - Regenerated and minified `apps/openagents.com/public/openapi.json`.

## Test Coverage Executed

- `php artisan test tests/Feature/Api/V1`
  - Result: passing (`24 passed`).

Also verified smoke checks on deployed staging revision:

- `apps/openagents.com/deploy/smoke/health.sh` => `ok`
- `apps/openagents.com/deploy/smoke/stream.sh` => `ok: stream done (7 deltas)`

## Full Staging E2E Scenario (Fresh Accounts)

Run ID: `20260217T084857Z`

Artifacts were captured during the run under a local temp directory:

- `/var/folders/mz/1lvnhfd91qlbyc8_5q7n3_bc0000gn/T/tmp.zlGsJMmYnS`

### 1) Programmatic account + autopilot bootstrap

Created 2 fresh users via `POST /api/auth/register` with `createAutopilot=true`:

- `stage-final-u1-20260217T084857Z@openagents.com`
  - handle: `stage-final-u1-20260217t084857z`
  - autopilot: `stage-final-u1-agent-1`
- `stage-final-u2-20260217T084857Z@openagents.com`
  - handle: `stage-final-u2-20260217t084857z`
  - autopilot: `stage-final-u2-agent-1`

Also minted admin token for `chris@openagents.com` via same bootstrap endpoint.

### 2) Wallet provisioning

Ensured Spark wallets for both users and admin via `POST /api/agent-payments/wallet`.

Observed wallet addresses:

- U1 spark address: `oa-stg-user-10`
- U2 spark address: `oa-stg-user-11`

### 3) Wallet funding

- Created U1 invoice for 1000 sats (`POST /api/agent-payments/invoice`)
- Admin wallet had 0 sats, so funding fallback used node payment:
  - `gcloud compute ssh oa-lnd ... lncli payinvoice --force <invoice>`
- Funding succeeded via node route (`paidVia=oa_lnd_node`)
- Verified U1 balance reached 1000 sats (`GET /api/agent-payments/balance`)

### 4) Cross-user payment interaction

Executed direct user-to-user payment flow:

- U2 created 200 sat invoice
- U1 paid U2 invoice via `POST /api/agent-payments/pay`
- Verified balance movement:
  - U1: `1000 -> 798`
  - U2: `0 -> 200`
- Payment reference:
  - paymentId: `019c6aca-f4c2-7832-b50c-f8e26cc9b85b`
  - proof: `preimage:0d47e0175e94afa9`

### 5) Shouts

- U1 posted shout in zone `ep212`
- U2 posted shout in zone `global`
- Public feed query `GET /api/shouts?zone=ep212&limit=5` returned expected entries.

### 6) Whispers

- U1 whispered U2 by handle via `POST /api/whispers`
- U2 fetched thread via `GET /api/whispers?with=<u1-handle>`
- Verified latest thread message id: `4`

### 7) Paywall creator/consumer API behavior

- Admin created paywall via `POST /api/l402/paywalls`:
  - paywallId: `019c6acb-37d1-718d-86c5-7cffca53369b`
  - HTTP: `201`
- Consumer (U2) successfully queried `GET /api/l402/paywalls` (`200`).

Note: paywall creation reconciles in current staging mode as `snapshot-only` (recorded deployment metadata), not full Aperture push from this staging app instance.

### 8) Real L402 consumer flow with wallet payment

Validated full 2-step L402 consumer flow against `sats4ai` using staged user wallet:

1. Initial request produced `402` with `www-authenticate` macaroon+invoice.
2. Invoice paid through app API (`POST /api/agent-payments/pay`) from U1 wallet.
3. Second request with `Authorization: L402 <macaroon>:<preimage>` returned `HTTP 200` and content.

Recorded proof reference:

- `preimage:3d68c826a59a308a`

## Final Outcome

Staging now supports and validates:

- Programmatic signup + autopilot bootstrap
- Token-authenticated wallet lifecycle endpoints
- Wallet funding + wallet-to-wallet invoice payment interactions
- Shouts and whispers API flows between fresh staged users
- Admin paywall creation path and consumer read path
- Real L402 two-step payment/authorization flow using staged wallet context

Known nuance from this run: direct `send-spark` by `sparkAddress` is environment/provider-sensitive; wallet-to-wallet interaction was validated using invoice payment path, which succeeded deterministically on staging.
