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

- `docs/adr/INDEX.md` — architecture decisions and compatibility constraints.
- `docs/execution/README.md` — artifact/replay contract entrypoint.
- `docs/execution/ARTIFACTS.md` — `PR_SUMMARY.md` + `RECEIPT.json`.
- `docs/execution/REPLAY.md` — `REPLAY.jsonl` event contract.
- `docs/protocol/README.md` — protocol surfaces, reason taxonomy, comms contracts.
- `docs/protocol/PROTOCOL_SURFACE.md` — canonical field semantics.
- `docs/protocol/reasons/runtime-policy-reason-codes.v1.json` — canonical reason codes.
- `docs/dse/README.md` — DSE/compiler contract index.
- `docs/codex/unified-runtime-desktop-plan.md` — canonical Codex desktop+runtime integration plan.
- `docs/plans/active/convex-self-hosting-runtime-sync-plan.md` — Convex self-hosted sync-layer architecture plan.

### 3) Product and runtime code surfaces

- `apps/openagents.com/` — core web app (Laravel + Inertia + React).
- `apps/openagents.com/README.md` — web dev/test/deploy entrypoint.
- `apps/openagents-runtime/` — Elixir runtime (execution, policy, replay, contracts).
- `apps/openagents-runtime/README.md` — runtime setup and test entrypoint.
- `apps/mobile/README.md` — mobile app.
- `apps/autopilot-desktop/` — Rust desktop Codex app.
- `apps/desktop/README.md` — Electron desktop Lightning app.
- `packages/` — shared libraries (`effuse*`, `dse`, `lightning-effect`, `hud`).

### 4) Plans, specs, and implementation tracking

- `docs/plans/README.md` — plans structure and conventions.
- `docs/plans/active/` — active execution plans.
- `docs/plans/completed/` — completed plans and decision history.
- `docs/plans/TEMPLATE.md` — standard plan format.

### 5) Runbooks, testing, and operations

- `docs/autopilot/testing/PROD_E2E_TESTING.md` — production-safe validation flow.
- `docs/autopilot/testing/TRACE_RETRIEVAL.md` — trace/debug retrieval workflow.
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
- If you are changing runtime execution behavior: start at `apps/openagents-runtime/` plus `docs/execution/REPLAY.md`.
- If you are changing web behavior/UI/integrations: start at `apps/openagents.com/` plus `docs/plans/active/laravel-rebuild.md`.
- If you are changing Codex runtime/desktop integration: start at `docs/codex/unified-runtime-desktop-plan.md`.
- If you are changing Convex sync integration: start at `docs/plans/active/convex-self-hosting-runtime-sync-plan.md` and `apps/openagents-runtime/docs/CONVEX_SYNC.md`.
- If you are changing terminology or naming: update `docs/GLOSSARY.md` first.
- If you are changing architecture invariants: author/update an ADR in `docs/adr/`.
