# lightning-ops

Effect-native operational service for hosted L402 gateway workflows.

## Scope in Phase 2A

- Pull hosted paywall control-plane state from Convex.
- Compile deterministic `aperture.yaml` artifacts + stable `configHash`.
- Validate route/policy state and emit typed diagnostics.
- Persist compile/deployment intent records back to Convex.

## Commands

```bash
npm run typecheck
npm test
npm run smoke:compile -- --json
npm run smoke:security -- --json
npm run smoke:settlement -- --json
npm run reconcile:convex
npm run smoke:staging -- --json
```

`smoke:compile -- --json` prints machine-readable JSON with:

- `configHash`
- `ruleCount`
- `valid`

`smoke:staging -- --json` emits reconcile output with:

- `challengeOk`
- `proxyOk`
- `configHash`
- `deploymentStatus`

`smoke:settlement -- --json` emits deterministic settlement ingest output with:

- `settlementIds`
- `paymentProofRefs`
- `correlationRefs` (request/task/route correlation tuples)

`smoke:security -- --json` verifies security controls with machine-readable output:

- fail-closed credential validation
- global pause/owner kill-switch denial behavior
- rotation/revocation/activation lifecycle + recovery state

Environment variables for Convex-backed operation:

- `OA_LIGHTNING_OPS_CONVEX_URL`
- `OA_LIGHTNING_OPS_SECRET`

Environment variables for hosted staging smoke (`--mode convex`):

- `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`
- `OA_LIGHTNING_OPS_CHALLENGE_URL`
- `OA_LIGHTNING_OPS_PROXY_URL`
- `OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN` (optional)
- `OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH` (optional)
- `OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER` (optional)

Default `smoke:staging` mode is deterministic in-memory (`--mode mock`) so CI can run non-interactively.
