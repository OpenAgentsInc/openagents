# Spacetime Broader Audit: Original Vision, Current Gap, and Reintroduction Plan

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, issue states, and implementation-status claims here may be superseded by later commits.


Date: 2026-03-04
Author: Codex
Status: Full audit (broader scope)

## Objective

Document:
1. What Spacetime was originally envisioned to do in OpenAgents.
2. What the current `openagents` repo actually has today.
3. What Spacetime would be high-leverage for now (especially Earn).
4. What appears already planned in retained/backroom artifacts.
5. A concrete path to add Spacetime back safely.

## Explicit Current-State Finding (No Spacetime in Active Implementation)

Current repo (`/Users/christopherdavid/code/openagents`) does **not** have a wired Spacetime implementation today.

Evidence:
1. Workspace members in `Cargo.toml` do not include a Spacetime client crate.
2. `crates/` has no `autopilot-spacetime` crate in this repo.
3. No `spacetime/` module directory exists in this repo.
4. Code search finds no active Spacetime transport/reducer client usage in `apps/` or `crates/`.
5. `POST /api/sync/token` appears in docs (`docs/MVP.md`) but there is no active desktop call path in this repo.
6. Sync pane labels say "Spacetime", but values are local projections (`apps/autopilot-desktop/src/state/operations.rs` + `apps/autopilot-desktop/src/pane_renderer.rs`).

This is a hard implementation gap vs product spec language.

## Sources Reviewed

Current repo:
1. `docs/MVP.md`
2. `docs/PANES.md`
3. `docs/NIP_SA_SKL_AC_TEST_MATRIX_RUNBOOK.md`
4. `apps/autopilot-desktop/src/state/operations.rs`
5. `apps/autopilot-desktop/src/pane_renderer.rs`
6. `apps/autopilot-desktop/src/pane_registry.rs`
7. `apps/autopilot-desktop/src/app_state.rs`
8. `apps/autopilot-desktop/src/input/actions.rs`
9. Workspace `Cargo.toml`

Backroom archive:
1. `docs/sync/README.md`
2. `docs/sync/ROADMAP.md`
3. `docs/sync/SPACETIME_CLIENT_CRATE.md`
4. `docs/sync/SPACETIME_RUNTIME_PUBLISH_MIRROR.md`
5. `docs/sync/SPACETIME_DESKTOP_APPLY_ENGINE.md`
6. `docs/sync/SPACETIME_DESKTOP_CONNECTION_LIFECYCLE.md`
7. `docs/sync/SPACETIME_TOPIC_STREAM_CURSOR_CONTINUITY.md`
8. `docs/sync/SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`
9. `docs/protocol/SPACETIME_SYNC_TRANSPORT_MAPPING.md`
10. `spacetime/modules/autopilot-sync/spacetimedb/src/lib.rs`
11. `spacetime/modules/autopilot-sync/README.md`
12. `crates/autopilot-spacetime/src/*`
13. `apps/autopilot-desktop/src/{main.rs,sync_lifecycle.rs,sync_apply_engine.rs,runtime_codex_proto.rs}`
14. `scripts/spacetime/*`

## What Spacetime Was Initially Envisioned For

Across retained architecture/spec/runbook material, Spacetime was envisioned as the canonical live sync and replay substrate, with explicit non-authority discipline for core money/policy mutations.

## 1) Retained sync/replay transport

Primary role:
1. One sync lane for reconnect-safe replay and live tail delivery.
2. Ordered apply keyed by `(stream_id, seq)`.
3. Duplicate-safe, stale-cursor-aware client behavior.

Backroom implementation already had:
1. Topic-to-stream mapping and cursor migration rules.
2. Replay bootstrap, live update handling, and stale reset paths.
3. Checkpoint persistence + rewind logic.

## 2) Desktop continuity and reliability semantics

Planned desktop behavior:
1. Subscribe with resume cursor.
2. Apply deterministic deltas.
3. Rebootstrap on stale/out-of-window cursor.
4. Expose true sync lifecycle/retry telemetry in UI.

Backroom had concrete code/docs for this lifecycle, apply engine, checkpoint storage, and health metrics.

## 3) Runtime projection mirror (not authority replacement)

Original doctrine:
1. Runtime/control remain authority for commands and money/policy.
2. Spacetime receives projection/delivery events for client sync.
3. Idempotency and ordered append guard replay correctness.

Backroom had explicit mirror docs and reducer calls (`append_sync_event`, `ack_stream_checkpoint`).

## 4) Presence and identity binding

Backroom module included:
1. `active_connection` lifecycle (`client_connected`, `client_disconnected`, `heartbeat`).
2. Nostr challenge-response identity binding (`request_nostr_presence_challenge`, `bind_nostr_presence_identity`).
3. SQL visibility for connected-user snapshots.

This is directly relevant to provider online registration and network counters.

## 5) Operational governance and rollout discipline

Backroom had:
1. Token scope/rotation contract.
2. Canary/prod rollout docs.
3. Chaos/parity harnesses.
4. Publish/promote scripts with schema/reducer checks.

So the prior program was not only protocol-level; it had ops guardrails and release gating.

## Current Repo: Where Spacetime-Semantic Expectations Exist But Are Unfulfilled

## 1) Spec/doctrine expects Spacetime sync lane

`docs/MVP.md` explicitly defines:
1. Spacetime-backed continuity.
2. Replay-safe reconnect.
3. Sync-health pane semantics.
4. Sync token issuance (`POST /api/sync/token`).

## 2) Product pane inventory expects real Spacetime diagnostics

`docs/PANES.md` describes Sync Health as:
1. connection state,
2. subscription state,
3. stale cursor detection,
4. replay and duplicate-drop visibility.

## 3) Actual implementation is local proxy today

`SyncHealthState.refresh_from_runtime` currently derives:
1. `spacetime_connection` from provider mode,
2. subscription from relay connectivity count,
3. cursor age from provider heartbeat age.

That is useful telemetry, but not actual Spacetime session/subscription truth.

## 4) Other "sync-ish" UX is local composition

Activity feed and related rows are composed from local app state snapshots, not from Spacetime stream state.

## What Spacetime Could Be Good For In This Repo Now

If Spacetime is being reintroduced as app DB substrate, highest-leverage domains in this repo are:

## A) Device/provider online presence (P0)

Use Spacetime for:
1. per-device online row,
2. heartbeat freshness,
3. identity-bound presence,
4. relay capability telemetry projection.

Why high value:
1. removes local multi-writer ambiguity for "online",
2. enables truthful network-wide `providers_online`,
3. directly improves Earn trust surface.

## B) Global counters and network visibility (P0/P1)

Use Spacetime for:
1. connected providers/users counts,
2. network-level active provider stats,
3. fleet presence snapshots.

Keep money truth separate:
1. payout confirmation remains wallet + reconciliation authority.

## C) Sync Health truth and reconnect lifecycle (P1)

Use Spacetime client lifecycle telemetry for:
1. connected/reconnecting/backoff state,
2. stale-cursor reason codes,
3. replay cursor/target/progress.

This aligns pane labels with actual source semantics.

## D) Activity/event continuity across restart and multi-device (P1)

Use stream + checkpoint model for:
1. append-only event projection rows (chat/job/network/sync),
2. deterministic replay on restart,
3. duplicate-safe ordering guarantees.

## E) Job lifecycle projection stream (P1/P2)

Use Spacetime for non-monetary, replay-safe projection of:
1. intake,
2. accepted/running/delivered states,
3. payment pointer linkage metadata.

Authority boundary:
1. settlement/credit/wallet authority remains explicit command lanes.

## F) Starter-demand coordination state (P2)

Potential fit for shared app DB coordination:
1. starter quest dispatch/inflight counters,
2. kill-switch state,
3. budget telemetry projection.

Only if scope is intentionally shared and deterministic.

## G) Alert/recovery incident queue projection (P2)

Potential fit for:
1. durable incident rows,
2. ack/resolve lifecycle with audit trail,
3. replay-safe operator visibility.

## Planned/Implied Work We Already Have Artifacts For

Backroom already contains strong starting points:
1. canonical Spacetime module with sync + presence tables/reducers,
2. typed client crate with negotiation/reducer calls/resume planning,
3. desktop lifecycle/apply/checkpoint implementations,
4. token scope and refresh contract docs,
5. publish/promote/smoke scripts,
6. chaos/parity verification playbooks.

This significantly lowers bootstrap cost compared to greenfield design.

## Authority Model Clarification Needed

Current MVP doctrine says sync is delivery/replay, not authority.
Your stated direction says Spacetime should be app DB.

These can coexist if split explicitly:
1. Keep money/policy/trust authority under authenticated command owners (runtime/control/wallet).
2. Allow Spacetime authority for selected app-state domains (presence, collaboration projections, replay surfaces) with explicit reducer contracts and auditability.
3. Update docs/ADR text so "authority" is scoped and unambiguous by domain.

Without this clarification, implementation will drift and reviewers will block each other on doctrine conflicts.

## Recommended Reintroduction Plan (Practical)

## Phase 0: Governance and boundary ratification

1. Add a short ADR/update defining which state classes Spacetime can be authoritative for.
2. Keep explicit "money mutation authority remains HTTP command lanes" rule.

## Phase 1: Minimum Spacetime foundation in this repo

1. Reintroduce `spacetime/modules/autopilot-sync` (or equivalent).
2. Reintroduce `crates/autopilot-spacetime` typed client.
3. Wire canonical sync token mint path in desktop (`POST /api/sync/token` contract).
4. Add handshake smoke script and basic CI verification.

## Phase 2: Earn online-state cut-in first

1. Device presence registration + heartbeat + offline transitions.
2. Mission-control `providers_online` from Spacetime query/subscription.
3. Keep local provider lanes for execution telemetry.

## Phase 3: Sync Health truth cutover

1. Replace proxy metrics with real client lifecycle/replay telemetry.
2. Preserve existing pane UX but back with true transport state.

## Phase 4: Event continuity and projection expansion

1. Move activity feed seeds to stream-backed projections.
2. Add deterministic replay coverage for job/network/activity panes.

## Risks and Controls

1. Risk: doctrinal conflict on authority boundaries.
   Control: ADR + docs update before deep implementation.
2. Risk: overloading Spacetime with money-critical writes.
   Control: keep settlement authority outside sync reducers.
3. Risk: reconnect/replay regressions.
   Control: restore parity/chaos harnesses early.
4. Risk: token/scope drift.
   Control: one canonical token path and scope contract tests.
5. Risk: UI-label trust erosion from proxy semantics.
   Control: source-of-truth alignment for pane values.

## Acceptance Criteria for "Spacetime Added Back"

1. Repo has versioned Spacetime module and typed client crate.
2. Desktop establishes scoped Spacetime session using canonical token contract.
3. Sync Health pane fields are sourced from real Spacetime lifecycle signals.
4. Earn online state is registered in Spacetime presence rows.
5. `providers_online` no longer equals local relay count.
6. Replay/idempotency tests cover reconnect + stale cursor + duplicate delivery.

## Bottom Line

Today: no active Spacetime integration in this repo implementation.

Originally envisioned: deterministic sync/replay substrate with presence, lifecycle, and operational discipline.

Best immediate fit now: add Spacetime back first for provider/device online presence + truthful sync health + network counters, then expand to broader projection continuity, while keeping monetary authority lanes explicit and unchanged.
