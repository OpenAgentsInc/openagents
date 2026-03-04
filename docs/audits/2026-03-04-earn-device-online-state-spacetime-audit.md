# Earn Device Online-State Audit (Current vs Spacetime App DB Target)

Date: 2026-03-04  
Author: Codex  
Status: Full audit + target design

## Objective

Audit how Earn currently marks devices as "online" in `openagents`, then define how it should work if Spacetime becomes the app DB authority for online presence.

## Scope Reviewed

Current repo (`openagents`):
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/runtime_lanes.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/input/reducers/{sa.rs,provider_ingress.rs}`
- `apps/autopilot-desktop/src/state/{provider_runtime.rs,operations.rs}`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/render.rs`

Backroom reference (`~/code/backroom/openagents-prune-20260225-205724-wgpui-mvp`):
- `spacetime/modules/autopilot-sync/spacetimedb/src/lib.rs`
- `spacetime/modules/autopilot-sync/README.md`
- `crates/autopilot-spacetime/src/{client.rs,reducers.rs,schema.rs,mapping.rs,subscriptions.rs}`
- `apps/autopilot-desktop/src/{main.rs,sync_lifecycle.rs,runtime_codex_proto.rs}`
- `scripts/spacetime/{publish-promote.sh,maincloud-handshake-smoke.sh}`
- `docs/sync/*` (connection lifecycle, client crate, roadmap)

## Executive Summary

Current state works for local MVP simulation but is not a clean network-authoritative online registry.

Main findings:
1. `Go Online` currently toggles two local lanes (SA lane + NIP-90 relay ingress lane), and UI mode is mostly projected from local state, not a shared presence authority.
2. `providers_online` is currently `connected_relays` for this desktop instance, not global providers on network.
3. Sync Health pane is labeled "Spacetime" but derives from provider mode + relay rows + heartbeat age in local state.
4. Backroom already contains a strong Spacetime presence foundation (`active_connection`, challenge/identity bind, heartbeat, connection lifecycle, SQL presence queries).

Recommendation:
- Move online-state authority to Spacetime presence tables/reducers.
- Keep wallet settlement authority unchanged (Spark + reconciliation).
- Use local lanes as execution telemetry inputs, but derive final online truth from Spacetime app DB rows.

## Current Implementation Audit (OpenAgents)

## 1) How "online" is currently set

`Go Online` click path (`input.rs`) does two writes:
1. `ProviderNip90LaneCommand::SetOnline { online: wants_online }`
2. `SaLifecycleCommand::SetRunnerOnline { online: wants_online }`

What this means:
- Relay ingress lane tracks websocket relay connectivity and request ingress.
- SA lane is an in-process runtime lane simulation with its own mode/heartbeat loop.

## 2) Who controls `provider_runtime.mode`

`provider_runtime.mode` is written by multiple reducers:
- SA lane snapshot reducer maps SA mode to `offline/connecting/online`.
- Provider ingress reducer can force `degraded` on relay ingress/publish failures and can set `online` when ingress is online.

This is a multi-writer projection, not one authoritative source.

## 3) Heartbeat semantics today

`ProviderRuntimeState.last_heartbeat_at` is fed from SA lane heartbeat ticks (`runtime_lanes.rs`), not from a shared network presence authority.

Implication:
- A device can appear "online" in UI due to SA heartbeat even when relay ingress is degraded/disconnected, unless explicit relay error paths override mode.

## 4) Preflight blockers are advisory only

`provider_blockers()` computes blockers (identity, wallet, trust, credit), and Go Online pane renders them, but the toggle path does not hard-block on these conditions.

Implication:
- Device can be switched into online flow despite unmet preconditions.

## 5) Network counters are local projections

`NetworkAggregateCountersState::refresh_from_sources` currently computes:
- `providers_online` from local `provider_nip90_lane.connected_relays`
- `jobs_completed/sats_paid/global_earnings_today` from local reconciled wallet/job history

Implication:
- Mission Control global counters are not globally authoritative network values.

## 6) Sync Health pane is not true Spacetime health

`SyncHealthState.refresh_from_runtime` maps:
- `spacetime_connection` <- `provider_runtime.mode`
- `subscription_state` <- whether relay rows are connected
- cursor staleness <- provider heartbeat age

Implication:
- Pane naming implies Spacetime subscription health, but data is primarily local provider/relay projection.

## 7) Strong points in current implementation

- Relay ingress lane has real relay transport states and publish outcomes.
- Wallet-confirmed payout gates are strong and should remain authoritative.
- Failure taxonomy (`relay/execution/payment/reconciliation`) is already useful for operator diagnostics.

## Backroom Spacetime Findings (What We Can Reuse)

## 1) Existing Spacetime module already models live connections

`autopilot-sync` module includes:
- `active_connection` table
- `client_connected` / `client_disconnected` reducers
- `heartbeat` reducer

It also includes challenge-bound Nostr identity association:
- `request_nostr_presence_challenge`
- `bind_nostr_presence_identity`

This is already the core of an online registry.

## 2) Existing module supports deterministic sync primitives

Also present:
- `sync_event` + `stream_head` + `stream_checkpoint`
- `append_sync_event` with idempotency/sequence conflict behavior
- `ack_stream_checkpoint`

This aligns with replay-safe state continuity.

## 3) Existing desktop/backroom code already queries presence

Backroom desktop code queries:
- `SELECT COUNT(*) AS connected_users FROM active_connection`
- identity list from `active_connection`

This is close to the counter path needed for a global providers-online signal.

## 4) Existing lifecycle design is production-oriented

Backroom `sync_lifecycle.rs` includes:
- explicit states (`idle/connecting/live/backoff`)
- disconnect reason classification
- stale-cursor handling with rebootstrap
- bounded backoff and token refresh handling

This is materially better than current local proxy semantics.

## 5) Existing ops scripts already enforce schema contract

Backroom scripts verify required tables/reducers and run handshake smoke checks on `active_connection` counts.

This reduces rollout risk if restored/wired carefully.

## Gap Analysis: Current vs Desired

1. Source of truth:
- Current: local in-memory multi-writer state.
- Desired: Spacetime app DB row(s) per connected provider session/device.

2. Online definition:
- Current: mostly SA mode + relay-side error overrides.
- Desired: explicit DB-backed online predicate with heartbeat freshness + relay capability signals.

3. Global counters:
- Current: local projection.
- Desired: DB query/aggregate from network-visible presence + settled earnings feeds.

4. Device identity binding:
- Current: implicit/local.
- Desired: challenge-bound identity binding persisted in Spacetime presence records.

5. Observability semantics:
- Current: pane labels suggest Spacetime, but data is local proxy.
- Desired: pane state values sourced from actual Spacetime lifecycle + subscription telemetry.

## Target Design: Spacetime as App DB for Online Presence

## 1) Presence Authority Model

Treat Spacetime as authoritative for device online registration.

Canonical online predicate for a provider session:
- connection row exists,
- identity binding is valid,
- provider mode is `online`,
- last heartbeat within TTL,
- relay connectivity/capability minimum met.

## 2) Minimal table/reducer extension plan

Reuse `active_connection`, extend with provider-facing fields (or add a companion table):
- `worker_id` / `device_id`
- `provider_mode`
- `relay_connected_count`
- `relay_required_count`
- `last_provider_heartbeat_unix_ms`
- `last_error_class` / `last_error_code`

Reducers:
- `set_provider_online(...)`
- `provider_heartbeat(...)`
- `set_provider_offline(...)`

Keep existing:
- `client_connected/client_disconnected`
- challenge + bind reducers

## 3) Desktop write path

When user clicks `Go Online`:
1. ensure auth/session token with provider presence scopes,
2. request and bind Nostr challenge if not bound,
3. call `set_provider_online` reducer,
4. start periodic `provider_heartbeat` reducer calls,
5. keep relay/SA lane telemetry mirrored into DB fields.

When user clicks `Go Offline` (or app exits):
- call `set_provider_offline`, then disconnect subscription.
- still rely on TTL as safety net for unclean disconnects.

## 4) Desktop read path

Replace local proxy counters/flags with DB-backed queries/subscriptions:
- `providers_online`: count rows matching online predicate
- current-device status: row for current identity+worker
- connected identities/providers list: query by recency

Use this row for Mission Control status chip and Go Online button semantics.

## 5) Keep payout truth unchanged

Do not move payout settlement truth to presence table.
Continue using:
- Spark wallet receive evidence,
- job history reconciliation,
- synthetic-pointer rejection.

Presence authority and payout authority should remain separate.

## Implementation Plan (Phased)

## Phase 1: Mirror-only (no behavior flip)

- Add Spacetime presence client in current repo.
- Write mirrored presence updates from current local lanes.
- Render both local vs Spacetime status side-by-side in diagnostics (temporary).

Exit criteria:
- no regressions in existing Earn loop,
- Spacetime mirror matches local status in normal cases.

## Phase 2: Read authority cutover for online status

- Mission Control `Status` and `providers_online` read from Spacetime.
- Keep local lane status as fallback diagnostics only.

Exit criteria:
- status transitions remain deterministic,
- stale sessions age out by TTL and no zombie online rows persist.

## Phase 3: Sync Health semantic correction

- Wire Sync Health pane to true Spacetime lifecycle/subscription telemetry (state, retry, disconnect reason, replay).
- Remove misleading proxy mapping from provider mode.

Exit criteria:
- pane labels and data source are consistent,
- stale-cursor and reconnect states are observable and actionable.

## Phase 4: Tighten preflight gating

- Block Go Online transition when critical blockers fail.
- Surface blocker remediation inline before mutating presence state.

Exit criteria:
- no online registration without required prerequisites.

## Test Plan Additions

1. Presence registration lifecycle test
- online -> heartbeat updates -> offline -> row transitions correctly.

2. Crash/restart stale cleanup test
- abrupt stop marks stale/offline after TTL.

3. Multi-device same identity test
- deterministic counting and per-device status semantics.

4. Relay partition test
- relay count drops => status degrades in DB and UI.

5. Sync lifecycle test
- token refresh, stale cursor, reconnect backoff all reflected in UI from real lifecycle state.

## Key Risks / Decisions Needed

1. Authority doctrine change
- Current MVP doc language says sync lane is delivery/replay only. If Spacetime is app DB authority for online presence, this must be explicitly ratified in ADR/docs.

2. Session cardinality
- Decide whether online counts are by device, worker, or identity.

3. Heartbeat budget
- Choose heartbeat cadence and TTL to balance freshness vs load.

4. Scope/security
- Ensure reducer scope/stream grants are minimum necessary for presence updates.

## Concrete Next Step Recommendation

Restore minimal Spacetime presence stack first:
1. `autopilot-sync` module (`active_connection` + challenge/bind + heartbeat),
2. typed client calls for reducer + SQL presence query,
3. Mission Control read-only `providers_online` from Spacetime,
4. keep existing local lanes unchanged during mirror phase.

This gives immediate truthful online registration with low blast radius.
