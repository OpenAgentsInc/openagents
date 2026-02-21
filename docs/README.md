# OpenAgents Documentation Index

Canonical architecture is Rust-only:
- `ARCHITECTURE-RUST.md`
- `ARCHITECTURE-RUST-ROADMAP.md`
- `DEPLOYMENT_RUST_SERVICES.md`

`ARCHITECTURE.md` is historical context only.

## Start Here

- `MANIFESTO.md`
- `AGENT_MAP.md`
- `PROJECT_OVERVIEW.md`
- `ROADMAP.md`
- `GLOSSARY.md`
- `adr/INDEX.md`
- `../AGENTS.md`

## Core Service Docs

- Rust deploy/process matrix: `DEPLOYMENT_RUST_SERVICES.md`
- Rust staging/prod validation gate: `RUST_STAGING_PROD_VALIDATION.md`
- Rust control service runbook: `../apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- Rust runtime API contract: `../apps/runtime/docs/RUNTIME_CONTRACT.md`
- Runtime observability and operations: `../apps/runtime/docs/OBSERVABILITY.md`
- Runtime deploy and migration gate: `../apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- Runtime WS threat model: `../apps/runtime/docs/KHALA_WS_THREAT_MODEL.md`
- Runtime reconnect/chaos drills: `../apps/runtime/docs/RESTART_RECONNECT_CHAOS.md`
- Runtime incident runbook (WS/auth/stale cursor): `../apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`
- Schema evolution playbook: `SCHEMA_EVOLUTION_PLAYBOOK.md`

Canonical runtime deploy + migrate command:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

## Sync and Codex

- Unified Codex plan: `codex/unified-runtime-desktop-plan.md`
- Sync architecture/invariants: `sync/thoughts.md`
- Sync roadmap: `sync/ROADMAP.md`
- Sync surface contract: `sync/SURFACES.md`
- Runtime/Codex cutover runbook: `sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
- Sync docs index: `sync/README.md`

## Contracts

- Execution artifacts/replay: `execution/`
- Protocol contracts: `protocol/`
- DSE/compiler contracts: `dse/`
- Proto generation policy: `../proto/README.md`
- Proto package map: `../proto/PACKAGE_MAP.md`

## Plans

- Plans hub: `plans/`
- Active plans: `plans/active/`
- Completed plans: `plans/completed/`
- Archived plans: `plans/archived/`

## Product Surfaces

- Web service + shell: `../apps/openagents.com/service/`, `../apps/openagents.com/web-shell/`
- Desktop app: `../apps/autopilot-desktop/`
- iOS app: `../apps/autopilot-ios/`
- Onyx app: `../apps/onyx/`
- Shared Rust crates: `../crates/`

## Local Development

- Rust control service: `cargo run --manifest-path ../apps/openagents.com/service/Cargo.toml`
- Rust runtime: `cargo run --manifest-path ../apps/runtime/Cargo.toml --bin openagents-runtime-service`
- Desktop app: `cargo run -p autopilot-desktop`
- iOS docs: `../apps/autopilot-ios/docs/README.md`
- Local CI policy: `LOCAL_CI.md`

## Audits and History

- Audit index: `audits/README.md`
- One-off audit reports: `audit/`
- Historical architecture snapshot: `ARCHITECTURE.md`
