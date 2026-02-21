# Wallet Executor Key Rotation Runbook

Status: Active  
Last updated: 2026-02-21  
Owner lane: `owner:infra`

## Scope

Operational procedure for rotating wallet-executor auth token and Spark credential material without breaking control-plane payment calls.

## Prerequisites

1. Access to GCP project hosting wallet executor and control service.
2. Access to secret manager versions for:
   - executor auth token
   - Spark API key
   - mnemonic secret (if using `gcp` provider)
3. Deploy permissions for wallet executor and control services.
4. Staging environment validation complete before production rollout.

## Rotation Order (Required)

1. Stage new secrets in secret manager.
2. Deploy wallet executor with new secret refs (accepting new token).
3. Deploy control service with matching outbound token.
4. Verify status/auth and payment smoke checks.
5. Revoke old secret versions.

Do not revoke old token/secrets before both services are updated and verified.

## Procedure: Rotate Executor Auth Token

1. Generate a new random token (at least 32 bytes entropy).
2. Publish new secret version for executor token.
3. Increment `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN_VERSION`.
4. Deploy wallet executor with:
   - `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN=<new-token-secret>`
   - `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN_VERSION=<new-version>`
5. Deploy control service to use the same new token.
6. Verify:
   - `GET /status` returns `authMode=bearer_static`, `authEnforced=true`, and updated `authTokenVersion`.
   - authorized `/status` and `/pay-bolt11` calls succeed from control plane.
7. Disable/revoke old token secret version.

## Procedure: Rotate Spark API Key

1. Create new key in Spark provider.
2. Add new key as latest secret version for `OA_LIGHTNING_SPARK_API_KEY`.
3. Deploy wallet executor referencing the new secret version.
4. Run payment smoke test (`/status`, `/pay-bolt11`) in staging then production.
5. Revoke old Spark key once successful.

## Procedure: Rotate Mnemonic Secret (High-Risk)

Mnemonic rotation changes wallet identity and may alter balance/account lineage. Treat as controlled migration.

1. Confirm business approval and downstream reconciliation plan.
2. Create new mnemonic secret version.
3. Deploy wallet executor to staging with new mnemonic, validate identity/balance behavior.
4. If production rotation is approved:
   - schedule maintenance window,
   - deploy new mnemonic version,
   - run settlement + reconciliation checks.
5. Keep old mnemonic version disabled but recoverable until reconciliation signoff.

## Verification Checklist

1. `GET /status` healthy and auth metadata updated.
2. Unauthorized request returns deterministic `401 unauthorized`.
3. Authorized `POST /pay-bolt11` succeeds and returns receipt hash/id.
4. No secret material appears in logs/receipt payloads.
5. Control-plane to executor payment path remains green.

## Tabletop Drill (Quarterly)

1. Simulate expired/compromised auth token.
2. Execute full token rotation in staging.
3. Capture timings for:
   - secret publish
   - service deploys
   - verification completion
4. Record gaps and update this runbook.

