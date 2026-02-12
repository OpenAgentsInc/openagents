# Staging Gateway Reconcile Runbook (Phase 2B)

This runbook describes the non-interactive deploy/reconcile loop for hosted L402 staging infrastructure:

- Aperture on GCP (Cloud Run)
- Voltage-backed LND for challenge/invoice flow
- OpenAgents control plane in Convex
- `apps/lightning-ops` reconcile pipeline

## 1. Required environment

Set these variables before running reconcile from `apps/lightning-ops`:

```bash
export OA_LIGHTNING_OPS_CONVEX_URL="https://<deployment>.convex.cloud"
export OA_LIGHTNING_OPS_SECRET="<ops-secret>"
export OA_LIGHTNING_OPS_GATEWAY_BASE_URL="https://<gateway-control-shim>"
export OA_LIGHTNING_OPS_CHALLENGE_URL="https://<aperture-domain>/<paywalled-route>"
export OA_LIGHTNING_OPS_PROXY_URL="https://<aperture-domain>/<paywalled-route>"
# Optional:
export OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN="<bearer-token>"
export OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH="/healthz"
export OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER="L402 <prepaid-proof>"
```

## 2. Reconcile and verify (non-interactive)

```bash
cd apps/lightning-ops
./scripts/staging-reconcile.sh
```

Expected JSON output includes:

- `challengeOk`
- `proxyOk`
- `configHash`
- `deploymentStatus`

## 3. What gets persisted

`apps/lightning-ops` writes deterministic records to Convex:

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

- `docs/lightning/L402_OBSERVABILITY_REHEARSAL_RUNBOOK.md`
