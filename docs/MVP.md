# Desktop-First MVP Spec (WGPUI + NIP-90 + Spark + Spacetime)

Status: draft spec for immediate implementation alignment
Date: 2026-02-25
Owner lanes: `owner:autopilot`, `owner:runtime`, `owner:protocol`, `owner:wallet`

## 1) Product Definition

This MVP is a desktop-first OpenAgents product where:

1. The primary UI is the native desktop app built with WGPUI.
2. A user can toggle "Go Online" so their Autopilot can accept NIP-90 compute requests.
3. The user has an Autopilot agent with a Spark Bitcoin wallet that can send/receive/pay.
4. The Autopilot can chat and interact with other agents over the retained Spacetime sync lane.

This is the minimum useful product loop:

1. User opens desktop app.
2. User authenticates and sees their Autopilot + wallet state.
3. User toggles online provider mode.
4. User receives/executes NIP-90 work and earns sats.
5. User chats/interacts with other agents via Spacetime-backed sync.

## 2) Authority and Invariants

This MVP must obey active architecture constraints:

1. Rust-only retained implementation (`ADR-0001`).
2. Proto-first cross-boundary contract authority (`ADR-0002`, `INV-01`).
3. Spacetime-only retained live sync transport (`ADR-0007`, `INV-03`).
4. Sync transport is delivery/replay only, not authority mutation (`INV-06`).
5. Authority mutations remain authenticated command boundaries (`INV-02`, `INV-04`, `INV-05`).

## 3) User Stories

## 3.1 Operator / User

1. I can open one desktop app and use chat, provider mode, and wallet in one surface.
2. I can turn online mode on/off and clearly see whether I am accepting NIP-90 jobs.
3. I can receive and pay BTC through Spark and see deterministic transaction history.
4. I can see my agent conversations and cross-agent interactions update live.

## 3.2 Agent Provider

1. When online, my Autopilot advertises provider capability on Nostr.
2. My Autopilot can accept, execute, and return NIP-90 results.
3. I can see job count, earnings, and failure reasons in desktop UI.

## 3.3 Safety / Operations

1. If sync disconnects, desktop reconnects with replay safety and no duplicate apply.
2. If a cursor is stale, desktop deterministically rebootstrapes.
3. If wallet/network fails, the UI shows explicit error states and no silent success.

## 4) MVP Scope

## In Scope

1. Desktop shell + chat thread UX in WGPUI.
2. Online/offline provider toggle and provider runtime lifecycle.
3. NIP-90 request submission and provider response path.
4. Spark wallet status, address/invoice generation, and payment execution.
5. Spacetime subscribe/replay/live chat/event synchronization.
6. Basic provider telemetry in-app: running state, jobs completed, earnings msats.

## Out of Scope (for this MVP)

1. Web-first product UX parity.
2. L402 paywall marketplace productization and ops workflows.
3. Advanced Hydra/Aegis policy and underwriting UX.
4. Multi-surface admin/reporting consoles.
5. Historical compatibility-lane behavior beyond required retained APIs.

## 5) Functional Requirements

## FR-1 Desktop Shell (WGPUI)

1. Desktop process boots into WGPUI app shell and initializes workspace/session.
2. Chat panel, provider controls, wallet panel, and activity feed are visible in one workflow.
3. UI state transitions are deterministic and recoverable after restart.

Acceptance:

1. App starts without web dependency for local-first flows.
2. User can create/select thread and submit a message.
3. Provider and wallet panels render live status updates.

## FR-2 "Go Online" Provider Toggle

1. User toggles provider online state from desktop UI.
2. App initializes provider runtime and provider identity.
3. App publishes provider presence/capability and starts heartbeat loop.
4. App can stop provider cleanly and report offline state.

Acceptance:

1. Toggle ON transitions to running provider status.
2. Toggle OFF transitions to not-running provider status.
3. Heartbeat failures are surfaced and do not fake running state.

## FR-3 NIP-90 Request Handling

1. Desktop can submit NIP-90 jobs to configured relays.
2. Desktop provider can accept processable jobs and emit results.
3. Job telemetry is tracked in desktop state (`jobs_completed`, `earnings_msats`, errors).

Acceptance:

1. Known-good relay set can receive submitted jobs.
2. Provider can execute at least text-generation class jobs.
3. End-to-end request/response path is observable in desktop logs and UI.

## FR-4 Autopilot Agent Actions

1. Agent can execute local-first actions (Codex/local runtime) with fallback policy.
2. Runtime/shared execution is optional fallback, not replacement for local-first.
3. Actions emit structured events suitable for replay/sync.

Acceptance:

1. Lane ordering remains local-first.
2. Failures include lane-specific reason and fallback behavior.

## FR-5 Spark Wallet

1. Desktop loads Spark wallet from configured signer/mnemonic context.
2. User can view balance, Spark address, and payment history.
3. User can create invoice and pay request.
4. Wallet errors are explicit and never reported as success.

Acceptance:

1. Wallet status shows network/connectivity and balance fields.
2. Invoice creation returns usable payment request.
3. Payment call returns canonical status update in UI.

## FR-6 Spacetime Chat and Agent Interaction

1. Desktop subscribes to retained Spacetime stream with authenticated token.
2. Desktop applies replay/live events with `(stream_id, seq)` monotonic idempotency.
3. Desktop can recover from disconnect and stale cursor deterministically.
4. Agent-to-agent chat/event interaction flows through the same retained Spacetime lane.

Acceptance:

1. Subscribe/bootstrap/live updates process without duplicate apply.
2. Legacy websocket/Phoenix frames are not accepted.
3. Stale cursor triggers deterministic rebootstrap path.

## 6) Protocol and Interface Contracts

## 6.1 Control/Auth Contract (retained)

Required retained control endpoints for desktop MVP:

1. Auth/session endpoints for desktop identity bootstrap.
2. `POST /api/sync/token` for Spacetime claim issuance.
3. Runtime worker control endpoints used by desktop execution lane.

## 6.2 Spacetime Contract (retained)

1. Subscribe target shape: `/v1/database/:name_or_identity/subscribe`.
2. Protocol parsing supports retained Spacetime message forms only.
3. Replay/apply key is `(stream_id, seq)`.

## 6.3 NIP-90 Contract (interop)

1. Provider advertises supported kinds/capabilities.
2. Request ingestion, execution, and response publishing are deterministic.
3. Provider identity and relay set are explicit config inputs.

## 6.4 Wallet Contract (Spark)

1. Wallet signer and network config are explicit env/config inputs.
2. Balance/address/invoice/pay operations expose typed outcomes.
3. Receipt/payment status is retained in UI-visible history.

## 7) State Model

## Local desktop state (minimum)

1. Auth/session state.
2. Thread/chat state and current worker context.
3. Provider online/running status + counters.
4. Wallet status snapshot and recent operations.
5. Sync checkpoint/watermark state.

## Remote authority state (minimum)

1. Runtime/control command authority (authenticated API).
2. Spacetime replay/live stream state for sync delivery.
3. Nostr relay event state for NIP-90 provider participation.

## 8) UX Requirements

1. Online toggle is prominent, binary, and always shows current effective state.
2. Wallet panel exposes: balance, receive invoice, send payment, history.
3. Chat panel remains first-class (not hidden behind provider/wallet workflows).
4. Error UX is actionable: disconnected, stale cursor, relay unavailable, wallet failure.
5. Status badges for `offline`, `connecting`, `online`, `degraded`.

## 9) Observability and Test Gates

Required evidence lanes:

1. Desktop sync reconnect/resume tests.
2. Spacetime parse/apply tests (reject legacy frame formats).
3. Provider online/offline lifecycle tests.
4. Wallet create-invoice/pay/status tests.
5. Desktop-to-runtime control request tests for local-first fallback behavior.

## 10) Crates and Code Required for This MVP

## 10.1 Required apps/services

1. `apps/autopilot-desktop` (primary product surface).
2. `apps/openagents.com` (retained auth/session + sync-token/control APIs used by desktop).
3. `apps/runtime` (retained execution authority + sync projection publishing).

Optional for MVP hardening (not required for first ship):

1. `apps/lightning-wallet-executor` (custody-externalized signing mode).

## 10.2 Required core crates

1. `crates/wgpui` (desktop UI runtime).
2. `crates/autopilot_ui` (desktop views/components).
3. `crates/autopilot_app` (desktop app state/events).
4. `crates/autopilot-core` (execution/session logic).
5. `crates/openagents-client-core` (lane ordering and runtime endpoint resolution).
6. `crates/openagents-codex-control` (typed desktop control request handling).
7. `crates/codex-client` (Codex app-server client path).
8. `crates/autopilot-spacetime` (Spacetime reducer/client integration).
9. `crates/pylon` (provider runtime for online NIP-90 operation).
10. `crates/runtime` (runtime execution abstractions used by desktop).
11. `crates/openagents-spark` (Spark wallet integration).
12. `crates/nostr/core` and `crates/nostr/client` (NIP-90/relay participation).
13. `crates/openagents-proto` and `crates/protocol` (typed contract surfaces).
14. `crates/autopilot-inbox-domain` (inbox interaction domain support).

## 10.3 Required code paths (minimum)

Desktop:

1. `apps/autopilot-desktop/src/main.rs`
2. `apps/autopilot-desktop/src/provider_domain.rs`
3. `apps/autopilot-desktop/src/wallet_domain.rs`
4. `apps/autopilot-desktop/src/sync_lifecycle.rs`
5. `apps/autopilot-desktop/src/runtime_codex_proto.rs`
6. `apps/autopilot-desktop/src/sync_apply_engine.rs`

Client/sync/wallet:

1. `crates/openagents-client-core/src/execution.rs`
2. `crates/autopilot-spacetime/src/client.rs`
3. `crates/spark/src/*`
4. `crates/nostr/core/src/nip90*` and `crates/nostr/client/src/*`

Control/runtime retained boundaries:

1. `apps/openagents.com/src/lib.rs` and `apps/openagents.com/src/openapi.rs` (retained desktop-facing API boundaries)
2. `apps/runtime/src/server.rs` and runtime internal route ownership/docs

## 11) Not Needed for This MVP (Defer / Exclude)

These are explicitly not required to ship the desktop-first MVP loop above.

## 11.1 Product surfaces not required

1. Web shell parity and HTMX/Maud product UX under `apps/openagents.com` web pages/fragments.
2. Compatibility lane UX and route-split admin controls for legacy web migrations.
3. Full operator/finance web dashboards beyond minimal retained APIs desktop needs.

## 11.2 Services not required for first MVP loop

1. `apps/lightning-ops` (ops/reconcile tooling).
2. `apps/lightning-wallet-executor` for first local-wallet MVP path (only needed for custody-externalized mode).

## 11.3 Workspace crates not required for this MVP behavior

1. `crates/openagents-l402` (L402 platform/productization is not required for this MVP loop).
2. `crates/neobank` (treasury/economics service paths are outside first desktop MVP).
3. `crates/openagents-registry` and `crates/openagents-cli` (operator/CLI tooling, not core desktop loop).
4. `crates/ws-test` (local WebSocket test utility, not product runtime requirement).
5. `crates/arrow` (test utility crate, not runtime requirement).

## 11.4 Capability areas deferred

1. Advanced Hydra/Aegis economics and underwriting UX.
2. Marketplace/L402 paywall creator product flows.
3. Multi-team/project/admin APIs beyond what desktop needs for auth/sync/control.

## 12) MVP Acceptance Checklist

Ship-ready means all are true:

1. Desktop launches and signs in with retained auth flow.
2. User can toggle online provider mode and see accurate state.
3. At least one NIP-90 request lifecycle succeeds end-to-end while online.
4. Wallet can create invoice and pay request with visible status/historical record.
5. Chat/inter-agent sync over Spacetime is live with replay-safe reconnect behavior.
6. No retained legacy websocket/Phoenix transport dependency exists in desktop sync path.

## 13) Post-MVP Expansion (Explicit)

After this MVP is stable:

1. Add custody-externalized wallet mode via wallet executor with receipt guarantees.
2. Add L402 service buying/selling loops in-product.
3. Add richer marketplace economics UX and policy controls.
4. Add stronger operator analytics surfaces after desktop loop is stable.
