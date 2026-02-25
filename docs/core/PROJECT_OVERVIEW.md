# OpenAgents Repository Overview

This map reflects canonical Rust-only architecture boundaries.

## Active Apps and Services (Retained Topology)

- `apps/openagents.com/service/`
  Rust control service: auth/session/device management, authorization, sync token issuance, and landing-page distribution.
  Compatibility lanes under `/api/v1/*` and legacy chat aliases are sunset-marked (`2026-06-30`) and treated as migration debt, not new authority surface.

- `apps/runtime/`
  Rust execution authority: run lifecycle, worker lifecycle, event log, projector/read models, and sync projection delivery (Spacetime canonical retained lane).

- `apps/autopilot-desktop/`
  Rust desktop surface for Codex/inbox administration, local Spark wallet operations (balance/send/receive/history), and local operator workflows.

- `apps/lightning-ops/`
  Rust Lightning policy/reconcile service.

- `apps/lightning-wallet-executor/`
  Rust Lightning payment execution service.

## Decommission Targets (Not Retained)

- `apps/onyx/`
  Archived to backroom and removed from this repo; not part of retained steady-state topology.

## Shared Code

- `crates/`
  Shared Rust workspace crates for protocols, clients, UI core, state, and utilities.

- `proto/`
  Universal schema authority for cross-process and cross-surface wire contracts.

## Authority Boundaries

- Control plane authority: `control.*` domain (identity, sessions, org/device authorization state).
- Runtime authority: `runtime.*` domain (execution events, projectors, sync journal, replay artifacts).
- Sync transport for retained lanes is SpacetimeDB.
- Legacy Khala-named compatibility surfaces are migration debt and do not perform authority mutations.
- Nostr is the interop substrate for portable events across independently-run operator domains.
- Nexus is the high-throughput intra-domain fabric for swarm coordination/streaming; a Bridge/Gateway controls what crosses between Nexus and Nostr.

## Canonical Documentation

- `docs/core/ARCHITECTURE.md`
- `docs/core/DEPLOYMENT_RUST_SERVICES.md`
- `docs/core/README.md`
- `docs/core/ROADMAP.md`
- `docs/core/AGENT_MAP.md`

## Historical/Removed Surfaces (Non-Canonical)

These are removed from active architecture and appear only in archived/historical docs:
- `apps/mobile/` (removed)
- `apps/desktop/` (removed)
- `apps/inbox-autopilot/` (removed)
- `apps/openagents-runtime/` (removed; replaced by `apps/runtime/`)
- `packages/` (removed)

`apps/openagents.com` legacy web/code lanes were archived to backroom and removed from this repo in OA-AUDIT `#2212`:
- `app/`, `bootstrap/`, `config/`, `database/`, `resources/`, `routes/`, `tests/`, and related legacy PHP/TS assets.
- Manifest: `docs/audits/2026-02-25-openagents-com-legacy-code-archive-manifest.md`.
