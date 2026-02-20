# Khala Lightning Production Cutover Drill

Date: 2026-02-20
Owner: OpenAgents operations
Scope: KHALA-027 production drill checklist + evidence contract for API-only lightning control-plane.

## Objective

Validate that production lightning-ops runs without Convex transport/dependencies and that rollback is no longer required for the control-plane lane.

## Required production inputs

- `OA_LIGHTNING_OPS_API_BASE_URL` (production web/API origin)
- `OA_LIGHTNING_OPS_SECRET` (shared with Laravel `OA_LIGHTNING_OPS_SECRET`)
- Optional gateway overrides:
  - `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`
  - `OA_LIGHTNING_OPS_CHALLENGE_URL`
  - `OA_LIGHTNING_OPS_PROXY_URL`

## Production drill commands

```bash
cd apps/lightning-ops
npm run typecheck
npm test
OA_LIGHTNING_OPS_API_BASE_URL=<prod-api-origin> OA_LIGHTNING_OPS_SECRET=<prod-ops-secret> npm run smoke:compile -- --json
OA_LIGHTNING_OPS_API_BASE_URL=<prod-api-origin> OA_LIGHTNING_OPS_SECRET=<prod-ops-secret> npm run smoke:security -- --json
OA_LIGHTNING_OPS_API_BASE_URL=<prod-api-origin> OA_LIGHTNING_OPS_SECRET=<prod-ops-secret> npm run smoke:settlement -- --json
OA_LIGHTNING_OPS_API_BASE_URL=<prod-api-origin> OA_LIGHTNING_OPS_SECRET=<prod-ops-secret> npm run reconcile:api
```

## Evidence to capture

- Command outputs (JSON where applicable) attached to deployment ticket.
- Reconcile output includes `configHash`, `deploymentStatus`, `challengeOk`, `proxyOk`, `healthOk`.
- Confirmation that no Convex env vars are required by lightning-ops runtime.
- Confirmation that `apps/lightning-ops/package-lock.json` contains no `convex` dependency.

## Rollback stance

- Control-plane rollback to Convex is removed in this wave.
- Operational rollback is now API-only:
  - revert to previous `apps/lightning-ops` image/tag,
  - keep Laravel authority tables/API unchanged,
  - rerun `reconcile:api` with previous known-good config.
