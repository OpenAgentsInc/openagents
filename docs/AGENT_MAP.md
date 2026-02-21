# Agent Map

This repo is designed for **progressive disclosure**: start from stable entry points, then drill into the domain you are actively changing.

## Global TOC (Where Everything Is)

### 1) First five files to read

- `AGENTS.md` — non-negotiables, authority chain, ship-quality rules.
- `docs/README.md` — documentation directory index.
- `docs/PROJECT_OVERVIEW.md` — active codebase map and ownership.
- `docs/GLOSSARY.md` — canonical terms and collision resolution.
- `docs/ROADMAP.md` — current sequencing and priority phases.

### 2) Contracts and architecture authority

- `docs/ARCHITECTURE-RUST.md` — canonical architecture source of truth.
- `docs/ARCHITECTURE-RUST-ROADMAP.md` — canonical migration sequencing and OA-RUST issue map.
- `docs/ARCHITECTURE.md` — historical hybrid snapshot (not canonical).
- `docs/adr/INDEX.md` — architecture decisions and compatibility constraints.
- `docs/adr/README.md` — Rust-era ADR process and lifecycle.
- `docs/execution/README.md` — artifact/replay contract entrypoint.
- `docs/execution/ARTIFACTS.md` — `PR_SUMMARY.md` + `RECEIPT.json`.
- `docs/execution/REPLAY.md` — `REPLAY.jsonl` event contract.
- `docs/protocol/README.md` — protocol surfaces, reason taxonomy, comms contracts.
- `docs/protocol/PROTOCOL_SURFACE.md` — canonical field semantics.
- `docs/protocol/reasons/runtime-policy-reason-codes.v1.json` — canonical reason codes.
- `docs/dse/README.md` — DSE/compiler contract index.
- `docs/codex/unified-runtime-desktop-plan.md` — canonical Codex desktop+runtime integration plan.
- `docs/sync/thoughts.md` — Khala sync engine architecture (runtime-owned WS sync target).
- `docs/sync/ROADMAP.md` — Khala migration roadmap with issue-ready sequencing.
- `docs/sync/SURFACES.md` — surface/topic/hydration contract for Khala clients.
- `docs/sync/PARITY_DASHBOARD.md` — parity mismatch/lag dashboard contract for dual-publish windows.
- `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md` — staged rollout + rollback runbook for runtime/Codex Khala cutover.
- `docs/plans/active/khala-self-hosting-runtime-sync-plan.md` — Khala self-hosted sync-layer architecture plan.
- `docs/plans/active/khala-runtime-codex-master-roadmap.md` — execution roadmap with delivery gates for runtime/Khala/Codex integration.
- `docs/plans/active/rust-migration-invariant-gates.md` — non-negotiable migration invariant checks for PR/release gates.

### 3) Product and runtime code surfaces

- `apps/openagents.com/` — core web app (Laravel + Inertia + React).
- `apps/openagents.com/README.md` — web dev/test/deploy entrypoint.
- `apps/runtime/` — Elixir runtime (execution, policy, replay, contracts).
- `apps/runtime/README.md` — runtime setup and test entrypoint.
- `apps/autopilot-ios/docs/README.md` — iOS app docs and runbooks.
- `apps/autopilot-desktop/` — Rust desktop Codex app.
- `apps/onyx/` — Rust local-first notes app.
- `packages/` — shared libraries (`effuse*`, `dse`, `lightning-effect`, `hud`).

### 4) Plans, specs, and implementation tracking

- `docs/plans/README.md` — plans structure and conventions.
- `docs/plans/active/` — active execution plans.
- `docs/plans/completed/` — completed plans and decision history.
- `docs/plans/TEMPLATE.md` — standard plan format.

### 5) Runbooks, testing, and operations

- `docs/autopilot/testing/PROD_E2E_TESTING.md` — production-safe validation flow.
- `docs/autopilot/testing/TRACE_RETRIEVAL.md` — trace/debug retrieval workflow.
- `docs/autopilot/testing/CROSS_SURFACE_CONTRACT_HARNESS.md` — shared web/desktop/iOS contract harness gate.
- `apps/runtime/docs/DEPLOY_CLOUD_RUN.md` — canonical runtime Cloud Run deploy + migration image-lock runbook.
- `apps/runtime/docs/RESTART_RECONNECT_CHAOS.md` — restart/reconnect drill matrix + artifact expectations.
- `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md` — operator playbook for WS auth/reconnect/stale-cursor incidents.
- `apps/autopilot-ios/docs/real-device-codex-handshake-runbook.md` — canonical real-app iOS↔desktop handshake gate (runtime-mediated).
- `docs/autopilot/runbooks/` — operational runbooks.
- `docs/lightning/runbooks/` — Lightning-specific operational procedures.
- `docs/audits/README.md` — audit index and follow-up targets.

### 6) Local context, research, and archives

- `docs/local/` — operator notes and local environment references.
- `docs/research/` — research notes and supporting material.
- `docs/transcripts/` — transcript archives.
- `docs/logs/` — operational logs/checkpoint docs.

## By Task (Quick Routing)

- If you are changing protocol fields or IDs: start at `docs/protocol/PROTOCOL_SURFACE.md`, then `docs/adr/INDEX.md`.
- If you are changing runtime execution behavior: start at `apps/runtime/` plus `docs/execution/REPLAY.md`.
- If you are changing web behavior/UI/integrations: start at `apps/openagents.com/` plus `docs/plans/active/laravel-rebuild.md`.
- If you are changing Codex runtime/desktop integration: start at `docs/codex/unified-runtime-desktop-plan.md`.
- If you are changing Khala sync integration: start at `docs/sync/thoughts.md`, `docs/sync/ROADMAP.md`, `docs/sync/SURFACES.md`, and `apps/runtime/docs/KHALA_SYNC.md` (for current boundary/migration context).
- If you are deploying runtime to Cloud Run: use `apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh` so deploy + migration validation are chained (see `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`).
- If you are changing terminology or naming: update `docs/GLOSSARY.md` first.
- If you are changing architecture invariants: start with `docs/ARCHITECTURE-RUST.md`, then author/update an ADR in `docs/adr/`.
- If you are changing Rust migration boundaries/cutover behavior: apply `docs/plans/active/rust-migration-invariant-gates.md`.
