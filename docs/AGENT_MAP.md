# Agent Map

Start with canonical Rust docs, then drill into the service/app surface you are changing.

## First Files to Read

- `AGENTS.md`
- `docs/README.md`
- `docs/PROJECT_OVERVIEW.md`
- `docs/GLOSSARY.md`
- `docs/ROADMAP.md`

## Architecture and Contract Authority

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `docs/adr/INDEX.md`
- `docs/execution/README.md`
- `docs/protocol/README.md`
- `docs/dse/README.md`

## Planning and Sequencing

- `docs/plans/README.md`
- `docs/plans/active/rust-migration-invariant-gates.md`
- `docs/plans/active/rust-migration-execution-control-plane.md`
- `docs/SYNTHESIS.md`

## Product and Runtime Surfaces

- `apps/openagents.com/service/`
- `apps/openagents.com/web-shell/`
- `apps/runtime/`
- `apps/autopilot-desktop/`
- `apps/autopilot-ios/`
- `apps/onyx/`
- `apps/lightning-ops/`
- `apps/lightning-wallet-executor/`
- `crates/`
- `proto/`

## Operations and Testing

- `docs/LOCAL_CI.md`
- `docs/RUST_STAGING_PROD_VALIDATION.md`
- `docs/RUST_LEGACY_INFRA_DECOMMISSION.md`
- `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`

## Quick Routing by Task

- Contract/wire changes: `proto/` + `docs/protocol/`
- Runtime behavior: `apps/runtime/` + `docs/execution/`
- Web control/shell behavior: `apps/openagents.com/service/`, `apps/openagents.com/web-shell/`
- Desktop behavior: `apps/autopilot-desktop/`
- iOS behavior: `apps/autopilot-ios/`
- Architecture/boundaries: `docs/ARCHITECTURE-RUST.md` + new ADR in `docs/adr/`
