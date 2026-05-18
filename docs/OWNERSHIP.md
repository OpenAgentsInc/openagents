# Ownership Boundaries (MVP)

This document defines what each active surface in this pruned repo owns, and what it must not own.

Scope note: this file covers the `openagents` repo. The web product deployed
at `autopilot.openagents.com` lives in the separate private `autopilot` repo.
Blueprint owns durable CRM, CEO, Voice, Memory, Program, Action, Source
Authority, evidence, and receipt truth consumed by that web product.

## Purpose

- Keep the MVP codebase understandable.
- Prevent responsibility bleed between product code and reusable crates.
- Enforce stable architecture while we iterate quickly.

## Active Surfaces

## `apps/autopilot`

Owns:

- Tauri-based Autopilot desktop shell prototype.
- TypeScript product UI for the next Autopilot shell.
- Tauri IPC boundary for request/response commands and runtime state streams.
- Desktop shell composition that projects Rust-owned runtime, wallet, provider,
  Nexus, Pylon, Probe, and Psionic state.

Must not own:

- The `autopilot.openagents.com` web application, CRM/CEO/Voice/Memory HUD
  surfaces, or web BFF behavior owned by the sibling `autopilot` repo.
- Blueprint Program Type, Module Version, Program Run, Action Submission,
  Source Authority, evidence, or receipt truth.
- Coding-runtime truth that belongs in `probe`.
- Work lifecycle truth that belongs in `forge`.
- Execution substrate truth that belongs in `psionic`.
- Wallet, payout, provider admission, or accepted-outcome authority in
  TypeScript frontend state.
- Generic WGPUI rendering primitives or legacy pane behavior from
  `apps/autopilot-deprecated`.

## `apps/autopilot-deprecated`

Owns:

- App wiring and product behavior.
- Pane orchestration, app-level event routing, and UX flows.
- Composition of reusable crates (`wgpui`, `nostr`, `openagents-provider-substrate`).
- App-owned execution snapshots, payout UX, inventory presentation, and provider orchestration.
- App-owned coding-shell behavior: chat/thread/workspace/project state,
  operator controls, remote supervision, and product-specific tool projection
  whether the underlying engine is Codex today or Probe later.

Must not own:

- Reusable protocol/wallet primitives that belong in crates.
- Generic UI toolkit primitives that belong in `wgpui`.
- Reusable provider lifecycle or launch-product derivation logic that belongs in `openagents-provider-substrate`.

## `crates/wgpui`

Owns:

- Compatibility facade over split WGPUI crates.
- Product-agnostic UI APIs consumed by apps.

Must not own:

- OpenAgents product workflows.
- App-specific business logic from `apps/autopilot-deprecated`.
- Dependencies on app crates.

## `crates/wgpui-core`

Owns:

- Core primitives (geometry/color/input/scene/curve).
- Product-agnostic types that render/components build on.

Must not own:

- GPU pipeline orchestration.
- Product/app behavior.

## `crates/wgpui-render`

Owns:

- GPU renderer implementation and SVG rasterization pipeline.
- Render metrics and texture preparation internals.

Must not own:

- Product/app behavior.
- Widget-level business workflows.

## `crates/openagents-ios-domain`

Owns:

- iOS app-domain mission/Codex data models.
- Filtering/severity semantics for mission event views.

Must not own:

- Rendering/GPU platform concerns.
- Shared WGPUI primitives.

## `crates/nostr/core`

Owns:

- Reusable Nostr identity + key derivation primitives.

Must not own:

- App UI logic.
- Wallet/payout logic.

## Retired payment-provider wallet code

Owns:

- Nothing in the active Nexus/Pylon production path.

Must not own:

- New payment, funding, payout, registration, recovery, release, or operator
  behavior.
- Any normal Nexus/Pylon deploy dependency or build artifact.
- App UI logic.
- Nostr pane/product orchestration logic.

Notes:

- Nexus and Pylon payment work is LDK-only. Historical retired-provider
  references are audit material or old desktop code awaiting deletion outside
  the production Nexus/Pylon build graph.

## `crates/openagents-provider-substrate`

Owns:

- Narrow reusable provider-domain semantics shared by `Autopilot` and future provider binaries.
- Provider backend identity and backend health models.
- Launch compute-product derivation from detected backends.
- Provider inventory control primitives and product metadata.
- Provider lifecycle state-machine helpers that remain product-agnostic.

Must not own:

- Pane-facing product UX.
- Mission Control orchestration.
- Buyer workflows, payout UX, or app-specific execution snapshots.
- App-to-kernel control flow that depends on `apps/autopilot-deprecated`.
- Long-term sandbox runtime engines, clustered transport, or validator logic.

## `crates/openagents-compiled-agent`

Owns:

- Rust-native DSPy-like signature contracts for the first narrow agent graph.
- Typed compiled-module inputs and outputs for route, tool policy, tool
  arguments, grounded answer, and verify/refusal.
- Reusable graph composition primitives and module-level eval interfaces.
- Module manifests and other app-agnostic compiled-agent metadata.

Must not own:

- App UX or pane orchestration.
- Product-specific workflow policy that belongs in `apps/autopilot-deprecated`.
- Model serving, training, or execution substrate that belongs in `psionic`.
- Market settlement, wallet authority, or provider lifecycle state that belongs
  in existing product or kernel crates.

## `OpenAgentsInc/psionic`

Own:

- Reusable compute execution substrate for local inference, clustered execution,
  artifact staging, and execution evidence.
- Backend-specific runtime implementations and execution planning internals.
- Cluster topology, ordered execution-state, and machine-checkable execution
  provenance.
- Sandbox runtime ownership, including bounded execution profiles, runtime
  detection, and execution evidence.
- Later training-class execution substrate.

Must not own:

- Pane-facing UX or product shell behavior.
- Wallet, payout, or buyer/provider product orchestration.
- Canonical compute-market settlement, procurement, or index authority.
- Final collateral, claim, or adjudication authority.

`openagents` consumes these crates through pinned git dependencies rather than
through an in-repo subtree.

## `OpenAgentsInc/probe`

Own:

- Reusable coding-agent runtime ownership: sessions, turns, transcripts,
  compaction, and recovery.
- Tool registry, permission policy, approval model, and MCP / connector loading
  for the coding runtime.
- Headless and operator-facing execution surfaces such as CLI, daemon/server,
  and stable supervision protocol.
- Multi-workspace routing, per-project runtime state, and engine-owned
  subagent/task orchestration.

Must not own:

- Autopilot pane UX, chat shell behavior, or desktop-specific product
  orchestration.
- Wallet, payout, provider, marketplace, data-market, or CAD product truth.
- App-owned remote companion behavior for the desktop product surface.
- Final operator projections that belong to `apps/autopilot-deprecated`.

`openagents` should consume Probe through a stable runtime boundary rather than
through app-to-engine internal coupling.

## `crates/openagents-kernel-core` and `crates/openagents-kernel-proto`

Own:

- Reusable economic domain objects and validation rules for compute-market
  truth.
- Canonical authority client contracts and generated wire-layer contracts.
- Receipt, snapshot, and reason-code shapes that higher-level services and apps
  consume.

Must not own:

- App-owned UX flows.
- Reusable runtime execution engines that belong in Psionic.
- Service-specific storage or deployment logic.

## `apps/nexus-control`

Owns:

- Canonical authority mutation and read-model behavior for the retained compute
  market slices.
- Durable receipts, snapshots, projections, and market-policy enforcement.
- Acceptance of delivery, challenge, and settlement outcomes into canonical
  market truth.
- The public integration contract that the private `treasury` service consumes
  and the canonical payout snapshots it imports back into public stats.

Must not own:

- Desktop UX.
- Reusable execution runtimes.
- Provider-local orchestration that belongs in the desktop app or reusable
  provider crates.
- The long-lived closed-source treasury send engine, wallet-operation worker,
  or backlog-drain supervisor that now belong in the separate private
  `treasury` repo.

## Planned Compute Extension Surfaces

These are not all active crates today, but their owner split is already fixed
by `docs/adr/ADR-0003-compute-market-ownership-and-authority-split.md`.

### Validator services

Own:

- Challenge execution and proof-verification workloads.
- Supporting evidence generation for adjudication.

Must not own:

- Final settlement authority.
- Canonical collateral or claim authority.
- Desktop UX.

### Environment and eval services

Own:

- Environment package descriptors.
- Dataset, harness, rubric, and eval-run registry logic.
- Synthetic-data and evaluation pipeline helpers.

Must not own:

- Reusable low-level execution substrate that belongs in Psionic.
- Final compute-market settlement authority.
- App-owned product UX.

## Dependency Rules

- `apps/*` may depend on crates.
- `crates/*` must not path-depend on `apps/*`.
- Reusable crates must not depend on each other through app layers.

## Guardrail

- Static boundary check: `scripts/lint/ownership-boundary-check.sh`
- This guard is run from `scripts/lint/clippy-regression-check.sh`.

## Review Checklist

- Does this change introduce product-specific behavior into reusable crates?
- Does this change add a crate-to-app dependency?
- Could this logic be moved one layer closer to its true owner?
- Does this change respect domain-scoped authority boundaries in `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`?
- Does this change respect compute-market owner split in `docs/adr/ADR-0003-compute-market-ownership-and-authority-split.md`?
