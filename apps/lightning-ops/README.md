# lightning-ops (Rust)

Rust-native operational CLI/service for hosted L402 gateway workflows.

This is the canonical runtime lane for `apps/lightning-ops`.

Legacy TypeScript/Effect implementation is archived under:

- `apps/lightning-ops/archived-ts/`

## Commands

From repo root:

```bash
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:compile --json --mode mock
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- compile:api --json
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- reconcile:api --json
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:security --json --mode mock
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:settlement --json --mode mock
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:observability --json --mode mock
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:staging --json --mode mock
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:full-flow --json --mode mock --allow-missing-local-artifact
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:ep212-routes --json --mode mock
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:ep212-full-flow --json --mode mock
```

## Output shape

`smoke:compile` / `compile:api` emit:

- `configHash`
- `ruleCount`
- `valid`
- `deploymentStatus`
- `deploymentId`
- `diagnostics[]`

`reconcile:api` / `smoke:staging --mode api` emit:

- `requestId`
- `executionPath`
- `configHash`
- `deploymentStatus`
- `challengeOk`
- `proxyOk`
- `healthOk`

`smoke:full-flow` and `smoke:ep212-full-flow` always write:

- `events.jsonl`
- `summary.json`

Default artifact directories:

- `output/lightning-ops/full-flow/<requestId>/`
- `output/lightning-ops/ep212-full-flow/<requestId>/`

## Environment variables

API-backed mode (`--mode api`) requires:

- `OA_LIGHTNING_OPS_API_BASE_URL`
- `OA_LIGHTNING_OPS_SECRET`

Gateway probe defaults (can be overridden):

- `OA_LIGHTNING_OPS_GATEWAY_BASE_URL` (default `https://l402.openagents.com`)
- `OA_LIGHTNING_OPS_CHALLENGE_URL` (default `https://l402.openagents.com/staging`)
- `OA_LIGHTNING_OPS_PROXY_URL` (default `https://l402.openagents.com/staging`)
- `OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH` (default `/healthz`)
- `OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN` (optional)
- `OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER` (optional)

EP212 live smoke overrides:

- `OA_LIGHTNING_OPS_EP212_ROUTE_A_URL`
- `OA_LIGHTNING_OPS_EP212_ROUTE_B_URL`
- `OA_LIGHTNING_OPS_EP212_SATS4AI_URL`

## Verification

From repo root:

```bash
cargo test --manifest-path apps/lightning-ops/Cargo.toml
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:compile --json --mode mock
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:staging --json --mode mock
```
