# OpenAgents Documentation Index

This directory tracks the active OpenAgents cross-platform stack and Rust-only migration path.
Canonical architecture intent is Rust-only and defined in `ARCHITECTURE-RUST.md` + `ARCHITECTURE-RUST-ROADMAP.md`.

## Start Here

- Product and mission: `MANIFESTO.md`
- Canonical architecture (Rust-only endstate): `ARCHITECTURE-RUST.md`
- Canonical migration sequencing/issues: `ARCHITECTURE-RUST-ROADMAP.md`
- Historical hybrid architecture snapshot: `ARCHITECTURE.md`
- Progressive disclosure map: `AGENT_MAP.md`
- Terminology: `GLOSSARY.md`
- Repository map (current): `PROJECT_OVERVIEW.md`
- Current roadmap: `ROADMAP.md`
- ADR index/process: `adr/INDEX.md`, `adr/README.md`
- Agent contract and engineering rules: `../AGENTS.md`

## Control-Plane and Runtime Docs

- Rust control service canary/rollback runbook: `../apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- Runtime architecture plan: `plans/active/elixir-agent-runtime-gcp-implementation-plan.md`
- Runtime internal API contract: `../apps/runtime/docs/RUNTIME_CONTRACT.md`
- Runtime operations runbook: `../apps/runtime/docs/OPERATIONS.md`
- Runtime Cloud Run deploy runbook: `../apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- Zero-downtime schema evolution playbook (control + runtime + proto): `SCHEMA_EVOLUTION_PLAYBOOK.md`
- Runtime restart/reconnect chaos drill runbook: `../apps/runtime/docs/RESTART_RECONNECT_CHAOS.md`
- Runtime WS/auth/stale-cursor incident runbook: `../apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`
  - Canonical deploy + migration command:
    ```bash
    GCP_PROJECT=openagentsgemini \
    GCP_REGION=us-central1 \
    RUNTIME_SERVICE=runtime \
    MIGRATE_JOB=runtime-migrate \
    IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> \
    apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
    ```
- DS-Elixir runtime contract: `../apps/runtime/docs/DS_ELIXIR_RUNTIME_CONTRACT.md`
- DS-Elixir operations runbook: `../apps/runtime/docs/DS_ELIXIR_OPERATIONS.md`

## Codex Architecture

- Canonical unified plan: `codex/unified-runtime-desktop-plan.md`
- Khala sync spec (runtime-owned WS sync engine): `sync/thoughts.md`
- Khala delivery roadmap: `sync/ROADMAP.md`
- Khala surface contract: `sync/SURFACES.md`
- Khala sync-layer plan: `plans/active/khala-self-hosting-runtime-sync-plan.md`
- Master execution roadmap: `plans/active/khala-runtime-codex-master-roadmap.md`
- Codex docs index: `codex/README.md`

## Contracts (Canonical Specs)

- Execution artifacts + replay: `execution/`
- Protocol surface: `protocol/`
- DSE/compiler contracts: `dse/`
- Proto generation policy + verification command: `../proto/README.md` (`./scripts/verify-proto-generate.sh`)
- Proto package ownership map: `../proto/PACKAGE_MAP.md`

## Plans

- Repo-wide plans hub: `plans/`
- Rust migration execution board + owner map: `plans/active/rust-migration-execution-control-plane.md`
- Rust migration legacy dependency inventory: `plans/active/rust-migration-legacy-dependency-inventory.md`
- Rust migration invariant gates: `plans/active/rust-migration-invariant-gates.md`
- Rust migration KPI dashboard/reporting: `plans/active/rust-migration-kpi-dashboard.md`

## Product Surfaces

- Web app runbooks: `autopilot/`
- Rust web shell source/build: `../apps/openagents.com/web-shell/`
- Rust web shell JS boundary policy: `../apps/openagents.com/web-shell/HOST_SHIM_BOUNDARY.md`
- Shared cross-surface UI core crate: `../crates/openagents-ui-core/`
- Shared cross-surface app state crate: `../crates/openagents-app-state/`
- Web production E2E testing: `autopilot/testing/PROD_E2E_TESTING.md`
- Web stream testing: `autopilot/testing/STREAM_TESTING.md`
- Web trace retrieval and debugging: `autopilot/testing/TRACE_RETRIEVAL.md`
- Cross-surface contract harness (web/desktop/iOS): `autopilot/testing/CROSS_SURFACE_CONTRACT_HARNESS.md`
- iOS/desktop runtime handshake runbook: `../apps/autopilot-ios/docs/real-device-codex-handshake-runbook.md`
- Pane system docs: `autopilot/reference/EFFUSE_PANES.md`
- iOS app source: `../apps/autopilot-ios/`
- Rust desktop Codex app source: `../apps/autopilot-desktop/`
- Rust desktop Codex app local auth/run guide: `../apps/autopilot-desktop/README.md`
- Onyx notes app source: `../apps/onyx/`

## Local Development

- Laravel web (control plane): `../apps/openagents.com/` — see `plans/active/laravel-rebuild.md`; typically `cd apps/openagents.com && composer run dev`.
- Elixir runtime (execution): `../apps/runtime/` — `cd apps/runtime && mix phx.server`.
- Local CI policy + hooks: `LOCAL_CI.md`
- iOS local dev: `../apps/autopilot-ios/docs/README.md`
- Rust desktop Codex local dev: `../apps/autopilot-desktop/` — typically `cargo run -p autopilot-desktop`.
- Storybook and component docs: `STORYBOOK.md`

## Audits

- Architecture and technical audits: `audits/`

## Historical Archive

- Deprecated docs were moved to backroom archives.
