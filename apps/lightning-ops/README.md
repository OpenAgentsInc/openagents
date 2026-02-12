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
npm run smoke:full-flow -- --json
npm run reconcile:convex
npm run smoke:staging -- --json
npm run smoke:ep212-routes -- --json --mode mock
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

`smoke:ep212-routes -- --json --mode live` verifies the two episode routes on `l402.openagents.com`:

- route A (`/ep212/premium-signal`): challenge shape + paid success (`status 200`)
- route B (`/ep212/expensive-signal`): challenge shape + over-cap policy block (no payment call)

`smoke:settlement -- --json` emits deterministic settlement ingest output with:

- `settlementIds`
- `paymentProofRefs`
- `correlationRefs` (request/task/route correlation tuples)

`smoke:security -- --json` verifies security controls with machine-readable output:

- fail-closed credential validation
- global pause/owner kill-switch denial behavior
- rotation/revocation/activation lifecycle + recovery state

`smoke:full-flow -- --json` executes the hosted-path end-to-end harness and writes:

- `events.jsonl` (ordered stage events)
- `summary.json` (machine-readable pass/fail + coverage summary)
- parity checks against local-node smoke artifact correlation keys

Default artifact paths:

- hosted output: `output/lightning-ops/full-flow/<requestId>/`
- local-node parity source: `output/l402-local-node-smoke-artifact.json`

Override flags:

- `--artifact-dir <path>`
- `--local-artifact <path>`
- `--allow-missing-local-artifact` (disables strict local parity requirement)

Environment variables for Convex-backed operation:

- `OA_LIGHTNING_OPS_CONVEX_URL`
- `OA_LIGHTNING_OPS_SECRET`

Environment variables for hosted staging smoke (`--mode convex`). **Gateway URLs default** to `https://l402.openagents.com` and `https://l402.openagents.com/staging` (in `staging-reconcile.sh` and in the smoke program). You only need to set:

- `OA_LIGHTNING_OPS_CONVEX_URL` (required for `--mode convex`)
- `OA_LIGHTNING_OPS_SECRET` (required for `--mode convex`)

Optional overrides:

- `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`, `OA_LIGHTNING_OPS_CHALLENGE_URL`, `OA_LIGHTNING_OPS_PROXY_URL`
- `OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN`, `OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH`, `OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER`

Environment variables for `smoke:ep212-routes -- --mode live`:

- `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL` (required)
- `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN` (optional)
- `OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS` (optional, default `12000`)
- `OA_LIGHTNING_OPS_EP212_ROUTE_A_URL` (optional, default `https://l402.openagents.com/ep212/premium-signal`)
- `OA_LIGHTNING_OPS_EP212_ROUTE_B_URL` (optional, default `https://l402.openagents.com/ep212/expensive-signal`)
- `OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS` (optional, default `100000`)

Example: copy `env.staging.example` to `.env.staging`, set the two required vars, then `source .env.staging && ./scripts/staging-reconcile.sh`.

**Full operator checklist (reconcile, CI, product, changing Aperture routes):** `docs/lightning/status/20260212-0753-status.md` §12.

Default `smoke:staging` mode is deterministic in-memory (`--mode mock`) so CI can run non-interactively.
