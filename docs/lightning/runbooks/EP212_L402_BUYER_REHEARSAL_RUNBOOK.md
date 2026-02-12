# EP212 L402 Buyer Rehearsal Runbook

This runbook is the deterministic verification path for EP212 buyer flows (no seller build-out required in this step).

## 1) Deterministic local/CI run

Run the full buyer sequence against local fixture endpoints:

```bash
cd apps/lightning-ops
npm run smoke:ep212-full-flow -- --json --mode mock
```

What this validates in one command:

1. sats4ai-compatible paid request (`Authorization: L402 <macaroon>:<preimage>`)
2. sats4ai cache hit (no second payment)
3. OpenAgents route paid success
4. over-cap policy block (no payment call)

Artifacts are written under:

- `output/lightning-ops/ep212-full-flow/<requestId>/summary.json`
- `output/lightning-ops/ep212-full-flow/<requestId>/events.jsonl`

## 2) Production dry run (real endpoints)

Use this when rehearsing against live infra:

```bash
cd apps/lightning-ops
OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="https://<wallet-executor-host>" \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="<optional-bearer>" \
OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS="12000" \
OA_LIGHTNING_OPS_EP212_SATS4AI_URL="https://sats4ai.com/api/l402/text-generation" \
OA_LIGHTNING_OPS_EP212_ROUTE_A_URL="https://l402.openagents.com/ep212/premium-signal" \
OA_LIGHTNING_OPS_EP212_ROUTE_B_URL="https://l402.openagents.com/ep212/expensive-signal" \
OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS="100000" \
npm run smoke:ep212-full-flow -- --json --mode live
```

Expected pass conditions in JSON output:

- `sats4ai.firstPaid = true`
- `sats4ai.cacheHit = true`
- `overCap.blocked = true`
- `openAgentsRoute.paidStatusCode = 200`

## 3) Route-only live check (OpenAgents gateway)

If you only need OpenAgents route verification:

```bash
cd apps/lightning-ops
OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL="https://<wallet-executor-host>" \
OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN="<optional-bearer>" \
npm run smoke:ep212-routes -- --json --mode live
```
