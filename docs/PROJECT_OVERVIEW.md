# OpenAgents Repository Overview

This map reflects canonical Rust-only architecture boundaries.

## Active Apps and Services

- `apps/openagents.com/service/`
  Rust control service: auth/session/device management, authorization, sync token issuance, static asset serving.

- `apps/openagents.com/web-shell/`
  Rust/WGPUI web shell (WASM) running in-browser.

- `apps/runtime/`
  Rust execution authority: run lifecycle, worker lifecycle, event log, projector/read models, Khala delivery semantics.

- `apps/autopilot-desktop/`
  Rust desktop surface for Codex/inbox administration and local operator workflows.

- `apps/autopilot-ios/`
  iOS host surface integrating shared Rust client/runtime behavior.

- `apps/onyx/`
  Rust local-first notes surface.

- `apps/lightning-ops/`
  Rust Lightning policy/reconcile service.

- `apps/lightning-wallet-executor/`
  Rust Lightning payment execution service.

## Shared Code

- `crates/`
  Shared Rust workspace crates for protocols, clients, UI core, state, and utilities.

- `proto/`
  Universal schema authority for cross-process and cross-surface wire contracts.

## Authority Boundaries

- Control plane authority: `control.*` domain (identity, sessions, org/device authorization state).
- Runtime authority: `runtime.*` domain (execution events, projectors, sync journal, replay artifacts).
- Khala is projection/replay delivery infrastructure; it does not perform authority mutations.

## Canonical Documentation

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `docs/DEPLOYMENT_RUST_SERVICES.md`
- `docs/README.md`
- `docs/ROADMAP.md`
- `docs/AGENT_MAP.md`

## Historical/Removed Surfaces (Non-Canonical)

These are removed from active architecture and appear only in archived/historical docs:
- `apps/mobile/` (removed)
- `apps/desktop/` (removed)
- `apps/inbox-autopilot/` (removed)
- `apps/openagents-runtime/` (removed; replaced by `apps/runtime/`)
- `packages/` (removed)
