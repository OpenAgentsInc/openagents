# Agent Map

Start with canonical Rust docs, then drill into the service/app surface you are changing.

## First Files to Read

- `AGENTS.md`
- `docs/core/README.md`
- `docs/core/PROJECT_OVERVIEW.md`
- `docs/core/GLOSSARY.md`
- `docs/core/ROADMAP.md`

## Architecture and Contract Authority

- `docs/core/ARCHITECTURE.md`
- `docs/adr/INDEX.md`
- `docs/execution/README.md`
- `docs/protocol/README.md`

## Planning and Sequencing

- `docs/plans/README.md`
- `docs/plans/rust-migration-invariant-gates.md`
- `docs/plans/rust-migration-execution-control-plane.md`
- `docs/core/SYNTHESIS.md`

## Product and Runtime Surfaces

- `apps/openagents.com/`
- `apps/runtime/`
- `apps/autopilot-desktop/`
- `apps/lightning-ops/`
- `apps/lightning-wallet-executor/`
- `crates/`
- `proto/`

## Operations and Testing

- `docs/core/LOCAL_CI.md`
- `docs/core/RUST_STAGING_PROD_VALIDATION.md`
- `docs/core/RUST_LEGACY_INFRA_DECOMMISSION.md`
- `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`

## Quick Routing by Task

- Contract/wire changes: `proto/` + `docs/protocol/`
- Runtime behavior: `apps/runtime/` + `docs/execution/`
- Web control/landing behavior: `apps/openagents.com/`
- Desktop behavior: `apps/autopilot-desktop/`
- Architecture/boundaries: `docs/core/ARCHITECTURE.md` + new ADR in `docs/adr/`
