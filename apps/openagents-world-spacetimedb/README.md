# openagents-world SpacetimeDB Module

This is the minimal SpacetimeDB module for the OpenAgents world projection.
It is deployed to the self-hosted database named `openagents-world` at
`https://spacetime.openagents.com`.

The module is a projection and interaction layer only. It does not own
settlement, payout, training truth, product promises, receipt validation,
wallet state, private prompts, private repositories, or provider credentials.

## Tables

Public projection tables:

- `training_run`
- `run_entity`
- `world_edge`
- `proof_ref`
- `settlement_ref`
- `world_event`
- `projection_cursor`
- `bridge_health`

Private authority tables:

- `module_owner`
- `service_identity`

Only allowlisted service identities may call projection reducers. The `init`
reducer stores the publishing identity as owner and service identity. Future
bridge identities can be added with `authorize_service_identity`.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo build --manifest-path apps/openagents-world-spacetimedb/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release
```

The WASM artifact is:

```text
apps/openagents-world-spacetimedb/target/wasm32-unknown-unknown/release/openagents_world.wasm
```

Publish through the VM-local SpacetimeDB CLI as documented in
`docs/game/2026-06-17-spacetimedb-admin-runbook.md`.
