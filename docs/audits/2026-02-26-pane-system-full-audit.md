# Pane + Pane System Full Audit (MVP)

- Date: 2026-02-26
- Scope: `apps/autopilot-desktop` pane inventory, pane system orchestration, pane data realism vs demo/hardcoded behavior.
- Authority checked: `docs/MVP.md`, `docs/OWNERSHIP.md`.

## Executive Summary

The pane system is functional and stable, but still mostly simulation-first.

- Total panes: 19.
- Real-backed panes: 5 (`Nostr Keys`, `Spark Lightning Wallet`, `Create Lightning Invoice`, `Pay Lightning Invoice`, `Settings` persistence).
- Hybrid panes: 2 (`Earnings Scoreboard`, `Alerts and Recovery`).
- Simulated/demo panes: 12 (`Autopilot Chat`, `Go Online`, `Provider Status`, `Relay Connections`, `Sync Health`, `Network Requests`, `Starter Jobs`, `Activity Feed`, `Job Inbox`, `Active Job`, `Job History`, `Empty`).

Primary architectural risk is duplication: pane metadata, hit-testing, input routing, and rendering are repeated across three large files (`pane_system.rs`, `input.rs`, `pane_renderer.rs`). This increases change cost and drift risk as pane count grows.

## Verification Run

- `cargo check -p autopilot-desktop`: pass.
- `cargo test -p autopilot-desktop`: pass (60 tests).
- `cargo clippy -p autopilot-desktop --all-targets -- -D warnings`: fails in shared crates (`wgpui-core`, `spark`) before pane-specific linting can complete.

## Pane Reality Matrix (Real vs Simulated)

## Real-backed

1. `Nostr Keys (NIP-06)`
- Real key material load/create on startup (`render.rs:87`, `crates/nostr/core/src/identity.rs:20-33`).
- Real regeneration (`input.rs:396-417`, `crates/nostr/core/src/identity.rs:28-33`).
- Real file persistence for mnemonic (`crates/nostr/core/src/identity.rs:35-46`, `77-99`).
- Real clipboard copy of `nsec` (`input.rs:432-449`).

2. `Spark Lightning Wallet`
- Real async worker + command queue (`spark_wallet.rs:32-94`).
- Real wallet init and API calls (`spark_wallet.rs:380-422`, `296-376`, `424-462`).
- Real dependency on Nostr mnemonic path (`spark_wallet.rs:380`).

3. `Create Lightning Invoice`
- Real `CreateInvoice` command path (`input.rs:1735-1750`, `1635-1671`).
- Real Spark wallet create-invoice call (`spark_wallet.rs:296-331`).

4. `Pay Lightning Invoice`
- Real `SendPayment` command path with invoice prefix validation (`input.rs:1712-1733`, `1753-1770`).
- Real Spark wallet send-payment call (`spark_wallet.rs:333-378`).

5. `Settings`
- Real load/save to disk (`app_state.rs:1400-1431`, `1519-1529`).
- Real field validation + persistence pipeline (`app_state.rs:1433-1517`).

## Hybrid

1. `Earnings Scoreboard`
- Uses real Spark balance when present (`app_state.rs:2299-2358`).
- But downstream job signals are mostly from simulated lanes (`job_history`, `starter_jobs`).

2. `Alerts and Recovery`
- Alert list itself is deterministic seeded data (`app_state.rs:1215-1273`).
- Recovery actions include real operations for identity regen and wallet refresh (`input.rs:1273-1384`, especially `1317-1334`).

## Simulated / Hardcoded / Demo-heavy

1. `Autopilot Chat`
- Local timed status simulation only (`app_state.rs:259-336`), no runtime/Codex lane execution.

2. `Go Online`
- UI toggles a local state machine (`app_state.rs:2415-2506`), no real relay/provider runtime boot.

3. `Provider Status`
- Derived from local runtime state; includes placeholder dependency line: `relay: unknown (lane pending)` (`pane_renderer.rs:483`).

4. `Relay Connections`
- Seeded rows (`app_state.rs:416-455`) and local mutate/retry behavior (`457-547`), not actual relay IO.

5. `Sync Health`
- Seeded counters/state (`app_state.rs:583-600`), local rebootstrap (`607-618`), pseudo-refresh from local runtime (`620-654`).

6. `Network Requests`
- Seeded request list (`app_state.rs:694-732`) and local insert-only submit (`734-796`), no network send.

7. `Starter Jobs`
- Seeded queue (`app_state.rs:833-868`) and local completion path (`870-914`).

8. `Activity Feed`
- Seeded default events (`app_state.rs:1010-1062`) and locally synthesized refresh snapshots (`input.rs:1464-1560`).

9. `Job Inbox`
- Seeded request intake (`app_state.rs:1692-1742`) and local accept/reject only (`1744-1826`).

10. `Active Job`
- Seeded active job (`app_state.rs:1880-1920`) and local lifecycle advancement (`1923-2011`).

11. `Job History`
- Seeded receipts (`app_state.rs:2110-2146`) and locally recorded updates (`2211-2236`).

12. `Empty`
- Placeholder pane (`pane_renderer.rs:201-212`).

## Critical Drift / Product-Mismatch Findings

1. MVP startup surface mismatch
- Current startup opens only an empty pane (`render.rs:155`).
- MVP describes always-visible core surfaces around Autopilot + Go Online + Wallet.

2. Settings identity path drift
- `SettingsDocumentV1` default `identity_path` is `~/.openagents/nostr/identity.json` (`app_state.rs:1372`).
- Actual identity authority is mnemonic at `~/.openagents/pylon/identity.mnemonic` (`crates/nostr/core/src/identity.rs:44-46`).
- This is stale and misleading in UI.

3. Inconsistent invoice validation paths
- Pay Invoice pane validates Lightning prefixes (`input.rs:1753-1770`).
- Spark Wallet send path only checks non-empty (`input.rs:1692-1707`).
- Same domain action, different guardrails.

## Pane System Architecture Findings

1. Metadata is duplicated in multiple authorities
- Pane IDs and semantics exist in:
  - `PaneKind` enum (`app_state.rs:34-55`)
  - `PaneDescriptor` constructors (`pane_system.rs:157-328`)
  - Pane titles (`pane_system.rs:2029-2049`)
  - Command registry (`render.rs:277-334`)
  - Command action constants + dispatch (`input.rs:35-52`, `1932-2044`)
  - Hotbar labels/slots (`hotbar.rs:12-24`, `102-116`)

2. Hit-testing is pane-by-pane duplicated
- 19 topmost hit/action functions in one file (`pane_system.rs:1323-1730`).
- Every pane adds new manual branches + row loops.

3. Input routing is manually repeated across event phases
- Mouse move/down/up repeat the same dispatch chain (`input.rs:122-170`, `219-246`, `264-286`).
- Keyboard handlers are mostly copy-pattern functions (`input.rs:632-934`).

4. Render pipeline is monolithic
- 19 pane paint functions in one module (`pane_renderer.rs`), with repeated status banner + action/error rendering patterns.

5. Re-sorting by z-index is repeated
- `pane_indices_by_z_desc` sorts each call (`pane_system.rs:2016-2020`).
- Many hit tests call it independently in one input event, increasing overhead and complexity.

6. Repeated pane-state status fields
- Many pane state structs duplicate `load_state`, `last_error`, `last_action` with similar update semantics.

## Recommendations (Ordered)

## P0: Remove simulation-by-default from production pane state

1. Replace seeded defaults with empty/unknown/loading defaults in:
- `RelayConnectionsState`, `SyncHealthState`, `NetworkRequestsState`, `StarterJobsState`, `ActivityFeedState`, `AlertsRecoveryState`, `JobInboxState`, `ActiveJobState`, `JobHistoryState`.

2. Add explicit source badges in each pane:
- `source: runtime`, `source: wallet`, `source: local`, so simulated state can never be mistaken for live data.

3. Move deterministic seed fixtures to tests only.

## P0: Fix authority drift and consistency bugs

1. Remove or derive `settings.document.identity_path` from `nostr::identity_mnemonic_path`.
2. Unify Lightning request validation so Spark Wallet send and Pay Invoice send use one validator.
3. Align startup pane set with MVP (core pane set should open by default; no empty placeholder default).

## P1: Introduce single pane registry (one source of truth)

Create a `PaneSpec` registry with:
- kind
- title
- width/height
- singleton
- command id/label/description
- optional hotbar slot metadata

Use this registry to generate:
- pane descriptors
- titles
- command palette entries
- input command dispatch mappings

This removes current multi-file drift risk.

## P1: Split pane modules by domain

Refactor into per-pane modules with a shared interface:
- layout/hit
- input dispatch
- action reducer
- renderer

Target structure example:
- `panes/chat.rs`
- `panes/wallet.rs`
- `panes/relay_connections.rs`
- `panes/mod.rs`

Keep orchestrator files thin.

## P2: Centralize repeated UI primitives

Create shared helpers for:
- status banner (`load_state`, `last_action`, `last_error`)
- selectable row list rendering
- labeled field/value block rendering

This will materially reduce `pane_renderer.rs` size and improve consistency.

## P2: Consolidate input dispatch pipeline

Introduce a single `dispatch_text_inputs(state, event)` and `dispatch_pane_actions(state, point)` path instead of manual repeated chains across mouse phases.

## P3: Add integration tests for pane behavior contracts

Add tests for:
- command registry <-> dispatch parity (every command ID must resolve)
- pane singleton behavior
- hotbar-to-pane behavior parity
- input path parity (mouse click and keyboard Enter trigger same action)

Current tests are strong at local unit/layout level, but sparse at cross-module pane contract level.

## Suggested Refactor Sequence

1. Fix P0 drift/consistency bugs (identity path, validation parity, startup pane set).
2. Build `PaneSpec` registry and switch command/title/descriptor generation.
3. Extract one pilot pane module (wallet) end-to-end and stabilize the interface.
4. Migrate remaining panes incrementally.
5. Remove old duplicated branches once migration is complete.

## Bottom Line

The pane system is usable and stable, but it is still simulation-heavy and structurally monolithic. The next pass should prioritize two things:

1. Make runtime truth explicit (remove seeded illusionary defaults in production).
2. Collapse duplicated pane metadata and routing into a single registry + modular pane implementations.

