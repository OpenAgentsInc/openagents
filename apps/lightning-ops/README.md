# lightning-ops

Effect-native operational service for hosted L402 gateway workflows.

## Scope in Phase 2A

- Pull hosted paywall control-plane state from Convex or Laravel internal API.
- Compile deterministic `aperture.yaml` artifacts + stable `configHash`.
- Validate route/policy state and emit typed diagnostics.
- Persist compile/deployment intent records to the selected control-plane backend.

## Commands

```bash
npm run typecheck
npm test
npm run smoke:compile -- --json
npm run smoke:security -- --json
npm run smoke:settlement -- --json
npm run smoke:full-flow -- --json
npm run compile:api
npm run reconcile:api
npm run reconcile:convex
npm run smoke:staging -- --json
npm run smoke:ep212-routes -- --json --mode mock
npm run smoke:ep212-full-flow -- --json --mode mock
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

`smoke:ep212-full-flow -- --json --mode mock` runs a deterministic buyer-flow harness with local fixtures:

- sats4ai-compatible paid request (`Authorization: L402 <macaroon>:<preimage>`)
- sats4ai cache hit (no second payment)
- OpenAgents route paid success
- over-cap policy block before payment

It always writes machine-readable artifacts:

- `events.jsonl` (ordered stage events)
- `summary.json` (result with payer-call and cache assertions)

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

Environment variables for API-backed operation:

- `OA_LIGHTNING_OPS_API_BASE_URL` (for example `https://openagents.com`)
- `OA_LIGHTNING_OPS_SECRET`

Laravel API-side requirement for API mode:

- `apps/openagents.com` must set `OA_LIGHTNING_OPS_SECRET` (same value as `apps/lightning-ops`).

Control-plane mode selection (bake-in + rollback):

- `OA_LIGHTNING_OPS_CONTROL_PLANE_MODE=api|convex|mock` (default: `api` for control-plane smoke/compile commands)
- Per-command override still works with `--mode ...` (for example `--mode convex` for rollback).

Environment variables for hosted staging smoke (`--mode convex` or `--mode api`). **Gateway URLs default** to `https://l402.openagents.com` and `https://l402.openagents.com/staging` (in `staging-reconcile.sh` and in the smoke program). You only need to set:

- `OA_LIGHTNING_OPS_SECRET` (required for `--mode convex`)
- `OA_LIGHTNING_OPS_CONVEX_URL` (required only for `--mode convex`)
- `OA_LIGHTNING_OPS_API_BASE_URL` (required only for `--mode api`)

Optional overrides:

- `OA_LIGHTNING_OPS_GATEWAY_BASE_URL`, `OA_LIGHTNING_OPS_CHALLENGE_URL`, `OA_LIGHTNING_OPS_PROXY_URL`
- `OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN`, `OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH`, `OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER`

Environment variables for `smoke:ep212-routes -- --mode live`:

- `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL` (required)
- `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN` (optional)
- `OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS` (optional, default `60000`)
- `OA_LIGHTNING_OPS_EP212_ROUTE_A_URL` (optional, default `https://l402.openagents.com/ep212/premium-signal`)
- `OA_LIGHTNING_OPS_EP212_ROUTE_B_URL` (optional, default `https://l402.openagents.com/ep212/expensive-signal`)
- `OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS` (optional, default `100000`)

Additional environment variables for `smoke:ep212-full-flow -- --mode live`:

- `OA_LIGHTNING_OPS_EP212_SATS4AI_URL` (optional, default `https://sats4ai.com/api/l402/text-generation`)
- `OA_LIGHTNING_OPS_EP212_ROUTE_A_URL` (optional, default `https://l402.openagents.com/ep212/premium-signal`)
- `OA_LIGHTNING_OPS_EP212_ROUTE_B_URL` (optional, default `https://l402.openagents.com/ep212/expensive-signal`)
- `OA_LIGHTNING_OPS_EP212_MAX_SPEND_MSATS` (optional, default `100000`)
- `OA_LIGHTNING_WALLET_EXECUTOR_BASE_URL` (required for `--mode live`)
- `OA_LIGHTNING_WALLET_EXECUTOR_AUTH_TOKEN` (optional)
- `OA_LIGHTNING_WALLET_EXECUTOR_TIMEOUT_MS` (optional, default `60000`)

Example: copy `env.staging.example` to `.env.staging`, set the two required vars, then `source .env.staging && ./scripts/staging-reconcile.sh`.

**Full operator checklist (reconcile, CI, product, changing Aperture routes):** `docs/lightning/status/20260212-0753-status.md` §12.

Default `smoke:staging` mode is deterministic in-memory (`--mode mock`) so CI can run non-interactively.
