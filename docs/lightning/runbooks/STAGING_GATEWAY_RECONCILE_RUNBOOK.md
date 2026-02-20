# Staging Gateway Reconcile Runbook (Phase 2B)

This runbook describes the non-interactive deploy/reconcile loop for hosted L402 staging infrastructure:

- Aperture on GCP (Cloud Run)
- Voltage-backed LND for challenge/invoice flow
- OpenAgents control plane in Khala
- `apps/lightning-ops` reconcile pipeline

## 1. Required environment

Set these variables before running reconcile from `apps/lightning-ops`. **Gateway URLs default** to the canonical staging route; you only need to set Khala URL and secret for Khala-backed runs:

```bash
export OA_LIGHTNING_OPS_KHALA_URL="https://<deployment>.khala.cloud"
export OA_LIGHTNING_OPS_SECRET="<ops-secret>"
# Optional (defaults set by staging-reconcile.sh):
# OA_LIGHTNING_OPS_GATEWAY_BASE_URL="https://l402.openagents.com"
# OA_LIGHTNING_OPS_CHALLENGE_URL="https://l402.openagents.com/staging"
# OA_LIGHTNING_OPS_PROXY_URL="https://l402.openagents.com/staging"
# Optional:
# export OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN="<bearer-token>"
# export OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH="/healthz"
# export OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER="L402 <prepaid-proof>"
```

**Gateway in use:** `https://l402.openagents.com` (staging path: `/staging`). **Full operator checklist (reconcile, CI, product, changing routes):** `docs/lightning/status/20260212-0753-status.md` §12. Env example: `apps/lightning-ops/env.staging.example`.

## 2. Reconcile and verify (non-interactive)

```bash
cd apps/lightning-ops
# Set only KHALA_URL and SECRET; gateway URLs are defaulted
export OA_LIGHTNING_OPS_KHALA_URL="https://<deployment>.khala.cloud"
export OA_LIGHTNING_OPS_SECRET="<ops-secret>"
./scripts/staging-reconcile.sh
```

Expected JSON output includes:

- `challengeOk`
- `proxyOk`
- `configHash`
- `deploymentStatus`

## 3. What gets persisted

`apps/lightning-ops` writes deterministic records to Khala:

1. `l402GatewayDeployments`
- `configHash`
- status (`applied`, `failed`, `rolled_back`)
- rollback markers (`rolledBackFrom`) when relevant
- correlation metadata with `executionPath=hosted-node`

2. `l402GatewayEvents`
- health/challenge/proxy reconcile events
- request/deployment/config correlation IDs

## 4. Failure taxonomy

Deployment/reconcile failures are mapped to deterministic codes:

- `compile_validation_failed`
- `active_lookup_failed`
- `deploy_apply_failed`
- `health_check_failed`
- `challenge_check_failed`
- `proxy_check_failed`
- `rollback_failed`

Rollback is attempted automatically when a previous deployment snapshot is available.

## 5. GCP + Voltage notes

1. Keep Aperture image digest pinned per rollout revision.
2. Keep Voltage credentials in Secret Manager and inject via Cloud Run runtime env.
3. Run reconcile only from CI/agent runtime with non-interactive env injection.
4. Verify `402` issuance from `OA_LIGHTNING_OPS_CHALLENGE_URL` and authenticated proxy success at `OA_LIGHTNING_OPS_PROXY_URL` as part of every staging run.

## 6. Observability correlation

Use the cross-path field contract and triage checklist in:

- `docs/lightning/runbooks/L402_OBSERVABILITY_REHEARSAL_RUNBOOK.md`
