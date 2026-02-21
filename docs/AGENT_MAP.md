# Agent Map

Start from canonical docs, then drill down by surface.

## First Files to Read

- `AGENTS.md`
- `docs/README.md`
- `docs/PROJECT_OVERVIEW.md`
- `docs/GLOSSARY.md`
- `docs/ROADMAP.md`
- `docs/DEPLOYMENT_RUST_SERVICES.md`

## Architecture and Contract Authority

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `docs/adr/INDEX.md`
- `docs/execution/README.md`
- `docs/protocol/README.md`
- `docs/dse/README.md`

Historical only:
- `docs/ARCHITECTURE.md`

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

## Plans and Tracking

- `docs/plans/README.md`
- `docs/plans/active/`
- `docs/plans/completed/`
- `docs/plans/archived/`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`

## Operations and Testing

- `docs/LOCAL_CI.md`
- `docs/RUST_STAGING_PROD_VALIDATION.md`
- `docs/RUST_LEGACY_INFRA_DECOMMISSION.md`
- `docs/autopilot/testing/PROD_E2E_TESTING.md`
- `docs/autopilot/testing/TRACE_RETRIEVAL.md`
- `docs/autopilot/testing/CROSS_SURFACE_CONTRACT_HARNESS.md`
- `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`

## Quick Routing by Task

- Protocol/wire changes: `proto/` + `docs/protocol/`
- Runtime behavior: `apps/runtime/` + `docs/execution/`
- Web shell/control service behavior: `apps/openagents.com/service/`, `apps/openagents.com/web-shell/`
- Desktop behavior: `apps/autopilot-desktop/`
- iOS behavior: `apps/autopilot-ios/`
- Architecture or boundaries: `docs/ARCHITECTURE-RUST.md` + new ADR in `docs/adr/`
