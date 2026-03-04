# Autopilot Earn Integration Audit and Gap Analysis

Date: 2026-03-04  
Author: Codex  
Status: Complete audit pass (docs + code + tests reviewed)

## Objective

Assess the real implementation state of Autopilot Earn against the MVP promise in `docs/MVP.md`, with emphasis on:

- What is actually implemented and trustworthy now
- What remains partial/simulated
- What should be built next to complete the MVP earn loop

## Scope Reviewed

Documents:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/EARN.md`
- `docs/AUTOPILOT_EARN_MVP.md`
- `docs/AUTOPILOT_EARNINGS_AUTOMATION.md`
- `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
- `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`
- `docs/SOLVER.md`
- `docs/AUTOPILOT_EARN_MVP_IMPLEMENTATION_LOG.md`
- `docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md`
- `docs/AUTOPILOT_EARN_MVP_TEST_HARNESS.md`
- `docs/plans/hydra-x.md`
- `docs/plans/hydra-liquidity-engine.md`
- `docs/plans/aegis.md`

Code:

- `apps/autopilot-desktop/src/{render,input,app_state,runtime_lanes}.rs`
- `apps/autopilot-desktop/src/input/{actions,reducers/*}.rs`
- `apps/autopilot-desktop/src/state/{provider_runtime,operations,job_inbox,earnings_gate,wallet_reconciliation}.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/Cargo.toml`
- `crates/nostr/core/src/nip90/*`
- `crates/nostr/client/src/dvm.rs`
- `crates/nostr/client/tests/dvm_submit_await_e2e.rs`
- `scripts/lint/autopilot-earnings-epic-test-gate.sh`

## Programmatic Verification

Executed and passed:

1. `./scripts/lint/autopilot-earnings-epic-test-gate.sh`
2. `cargo test -p nostr-client --test dvm_submit_await_e2e`
3. `cargo test -p autopilot-desktop --bin autopilot-desktop mission_control_earn_loop_wallet_confirmed_end_to_end`
4. `cargo test -p autopilot-desktop --bin autopilot-desktop state::earnings_gate::tests::accepts_wallet_backed_earnings_evidence`
5. `cargo test -p autopilot-desktop --bin autopilot-desktop state::earnings_gate::tests::rejects_synthetic_payout_pointers`

Note:

- Gate/test runs are green, but they mostly validate deterministic local state transitions and reconciliation contracts. They do not prove full relay-backed live-market execution in the desktop app.
- Test runs emit non-fatal dead-code warnings in `autopilot-desktop` (59 warnings during the gate run), indicating cleanup debt but not blocking correctness gates.

## Executive Summary

Autopilot Earn has solid foundations but is not yet fully end-to-end integrated as a real external provider loop.

Green:

- Wallet integration and wallet-authoritative earning gates are real and well tested.
- NIP-90 model + client helper substrate exists and is test-covered.
- Mission Control UX and local lifecycle scaffolding are present.

Yellow:

- Core user flow is represented in UI/state, but much of runtime authority is in-process and synthetic.

Red:

- Desktop provider loop is not yet clearly wired to real relay-driven NIP-90 job intake/execution/result publication in the primary path.
- “Global network” stats are currently computed from local state, not an authoritative external network source.

## Execution Status Updates

- 2026-03-04: `#2877` (desktop relay-backed NIP-90 provider lane wiring) implemented.
  - Added `nostr-client` ingress worker in `apps/autopilot-desktop/src/provider_nip90_lane.rs`.
  - Wired `Go Online` and relay/settings sync to configure and toggle live ingress in app runtime.
  - Added automated coverage for relay ingress (`provider_nip90_lane::tests::worker_ingests_live_relay_request`) and included it in `scripts/lint/autopilot-earnings-epic-test-gate.sh`.
- 2026-03-04: `#2878` (provider lifecycle external authority mapping) implemented.
  - Active-job stage transitions now hard-fail without external authority references (`running` requires request authority, `delivered` requires result authority, `paid` requires wallet-authoritative pointer) in `apps/autopilot-desktop/src/app_state.rs`.
  - Removed synthetic delivered/paid authority stamping from active-job stage advancement.
  - Relay ingress now maps request event id into `sa_tick_request_event_id` to preserve stage authority provenance (`apps/autopilot-desktop/src/provider_nip90_lane.rs`).
  - Added/updated tests for authority-gated transitions and wallet-confirmed settlement behavior (`apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/provider_nip90_lane.rs`).
- 2026-03-04: `#2879` (result/feedback publishing from desktop execution lane) implemented.
  - Added canonical NIP-90 publish command/outcome support in provider relay lane, including relay acceptance/rejection accounting and runtime degraded-state mapping for publish failures (`apps/autopilot-desktop/src/provider_nip90_lane.rs`).
  - Added request-kind provenance to inbox/active-job records and used it to derive canonical result kind (`request_kind + 1000`) and job-linked feedback/result tags (`apps/autopilot-desktop/src/state/job_inbox.rs`, `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/input/reducers/jobs.rs`).
  - Active-job stage actions now sign and queue canonical NIP-90 result/feedback publishes for running, paid, and abort flows via provider lane commands (`apps/autopilot-desktop/src/input/reducers/jobs.rs`, `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`).
  - Added/updated automated coverage for signed publish behavior (`provider_nip90_lane::tests::worker_publishes_signed_feedback_event_to_connected_relay`) and verified via `./scripts/lint/autopilot-earnings-epic-test-gate.sh`.
- 2026-03-04: `#2880` (wallet-authoritative mission-control totals) implemented.
  - Mission-control and earnings scoreboard payouts now derive from wallet-reconciled settled receive evidence only (job-history row + settled wallet payment id match), not raw wallet balance or unreconciled receipt rows (`apps/autopilot-desktop/src/app_state.rs`).
  - Added wallet-reconciled payout projection (`wallet_reconciled_payout_rows`) and used it for `sats_today`, `lifetime_sats`, and `jobs_today` scoreboard counters (`apps/autopilot-desktop/src/app_state.rs`).
  - Updated mission-control “Recent Payouts” rows to render wallet-reconciled payout entries only (`apps/autopilot-desktop/src/render.rs`).
  - Added automated coverage for reconciled vs unreconciled scoreboard behavior (`app_state::tests::earnings_scoreboard_refreshes_from_wallet_and_history`, `app_state::tests::earnings_scoreboard_ignores_unreconciled_history_rows`).
- 2026-03-04: `#2881` (desktop E2E harness: relay -> execute -> publish -> wallet confirm) implemented.
  - Added full-loop desktop integration harness test covering relay ingress, local job lifecycle progression, result/feedback publishing, and wallet-confirmed scoreboard settlement in one path (`apps/autopilot-desktop/src/provider_nip90_lane.rs`: `desktop_earn_harness_relay_execute_publish_wallet_confirm_end_to_end`).
  - Added relay mock harness helper that supports both live request delivery and publish-event capture/ack in a single websocket session (`apps/autopilot-desktop/src/provider_nip90_lane.rs`).
  - Wired the new harness into the Earn regression gate so this loop is continuously enforced (`scripts/lint/autopilot-earnings-epic-test-gate.sh`).
  - Updated harness documentation to reflect the new end-to-end desktop flow coverage (`docs/AUTOPILOT_EARN_MVP_TEST_HARNESS.md`).
- 2026-03-04: `#2882` (starter-demand generator with budget/kill-switch controls) implemented.
  - Added deterministic starter-demand dispatch state with explicit controls for sats budget cap, dispatch interval, max inflight quests, and a manual kill switch (`apps/autopilot-desktop/src/state/operations.rs`).
  - Added automatic starter-demand generator tick in desktop background loop that dispatches starter quests only while provider is online and relay ingress is connected (`apps/autopilot-desktop/src/input.rs`, `apps/autopilot-desktop/src/input/actions.rs`).
  - Added controlled starter-demand request publication path that queues AC credit intent, records network request submission, and injects starter ingress requests into the provider inbox (`apps/autopilot-desktop/src/input/actions.rs`).
  - Added starter-pane kill-switch UI action and status rendering (`apps/autopilot-desktop/src/pane_system.rs`, `apps/autopilot-desktop/src/pane_renderer.rs`, `apps/autopilot-desktop/src/input/tool_bridge.rs`).
  - Added automated coverage for budget/interval/kill-switch/rollback behavior and wired it into the earnings epic gate (`apps/autopilot-desktop/src/app_state.rs`, `scripts/lint/autopilot-earnings-epic-test-gate.sh`).
  - Updated harness documentation with starter-demand controls coverage (`docs/AUTOPILOT_EARN_MVP_TEST_HARNESS.md`).
- 2026-03-04: `#2883` (starter job provenance and receipt tagging) implemented.
  - Added explicit demand-source tagging across job ingest and lifecycle records (`open-network` vs `starter-demand`) in inbox, active-job, and job-history state models (`apps/autopilot-desktop/src/state/job_inbox.rs`, `apps/autopilot-desktop/src/app_state.rs`).
  - Wired source tagging on ingress paths: live relay requests are tagged open-network, while seeded starter dispatch requests are tagged starter-demand (`apps/autopilot-desktop/src/provider_nip90_lane.rs`, `apps/autopilot-desktop/src/input/actions.rs`).
  - Extended UI rows/details to render source provenance in Job Inbox, Active Job, Job History, and starter-job labeling (`apps/autopilot-desktop/src/pane_renderer.rs`).
  - Added provenance in result publication payload and automated propagation coverage (`apps/autopilot-desktop/src/input/reducers/jobs.rs`, `apps/autopilot-desktop/src/app_state.rs`).
  - Added regression gate coverage for starter provenance tagging (`scripts/lint/autopilot-earnings-epic-test-gate.sh`).
- 2026-03-04: `#2884` (authoritative network stats pipeline) implemented.
  - Added dedicated aggregate-counters service state for Mission Control network/global counters with explicit source/load/error semantics (`apps/autopilot-desktop/src/app_state.rs`).
  - Wired aggregate refresh into background pump and removed ad-hoc inline Mission Control stats math in sidebar render path (`apps/autopilot-desktop/src/input.rs`, `apps/autopilot-desktop/src/input/actions.rs`, `apps/autopilot-desktop/src/render.rs`).
  - Aggregate counters now derive from provider lane connectivity snapshot + wallet-reconciled payout rows (`jobs completed`, `sats paid`, and global today sats) instead of raw unreconciled history rows.
  - Added automated coverage for reconciled/unreconciled/error aggregate-counter behavior and wired it into the earnings gate script (`apps/autopilot-desktop/src/app_state.rs`, `scripts/lint/autopilot-earnings-epic-test-gate.sh`).
- 2026-03-04: `#2885` (relay connectivity truth model) implemented.
  - Added transport-derived per-relay health rows to provider ingress snapshots (`connected/connecting/disconnected/error`, latency, last-seen, last-error) and wired lane loop telemetry updates from real relay transport state (`apps/autopilot-desktop/src/provider_nip90_lane.rs`).
  - Replaced strict all-relay subscribe/connect path with per-relay connect+subscribe handling and reconnect attempts so one failing relay does not force total ingress failure (`apps/autopilot-desktop/src/provider_nip90_lane.rs`).
  - Relay pane reducer now hydrates relay rows directly from provider lane transport snapshots, preserving selection while removing pane-local connected simulation (`apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`).
  - Relay retry action now sets `connecting` only and queues reconnect sync, leaving connected/latency state to transport authority (`apps/autopilot-desktop/src/state/operations.rs`, `apps/autopilot-desktop/src/input/actions.rs`).
  - Added/updated automated coverage for transport relay snapshot wiring and retry semantics; wired relay-retry regression into the earnings gate script (`apps/autopilot-desktop/src/provider_nip90_lane.rs`, `apps/autopilot-desktop/src/app_state.rs`, `scripts/lint/autopilot-earnings-epic-test-gate.sh`).
- 2026-03-04: `#2886` (epic tracker + implementation log reconciliation) implemented.
  - Updated `docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md` with a reconciled gate-status matrix and explicit follow-on issue state table for `#2877` through `#2890`.
  - Clarified the original `#2814`-`#2876` list as historical scope and documented that open reconciliation issues block final completion claims.
  - Updated `docs/AUTOPILOT_EARN_MVP_IMPLEMENTATION_LOG.md` header/claims to separate historical evidence from live completion status, plus added reconciliation addendum coverage for `#2877`-`#2890`.
- 2026-03-04: `#2887` (failure taxonomy + user-facing diagnostics) implemented.
  - Added canonical provider failure classes (`relay`, `execution`, `payment`, `reconciliation`) as typed runtime state and surfaced them in Provider Status diagnostics (`apps/autopilot-desktop/src/state/provider_runtime.rs`, `apps/autopilot-desktop/src/pane_renderer.rs`).
  - Wired deterministic failure classification with precedence in scoreboard refresh path so operator diagnostics consistently map relay, execution, wallet/payment, and reconciliation mismatch failures (`apps/autopilot-desktop/src/input/actions.rs`).
  - Updated reducer error mapping to use canonical classes across ingress, active-job execution, wallet, and SA command response flows (`apps/autopilot-desktop/src/input/reducers/{provider_ingress,jobs,wallet,sa}.rs`, `apps/autopilot-desktop/src/input.rs`).
  - Added automated taxonomy classifier coverage and wired it into the earnings epic gate (`apps/autopilot-desktop/src/input/actions.rs`, `scripts/lint/autopilot-earnings-epic-test-gate.sh`).
- 2026-03-04: `#2888` (loop integrity SLO metrics + alerts) implemented.
  - Extended earnings scoreboard state to track first-job latency, completion ratio, payout success ratio, and average wallet confirmation latency from runtime/history/wallet-reconciled evidence (`apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/pane_renderer.rs`).
  - Added deterministic loop-integrity SLO alert evaluation and alert-row upsert/recovery wiring for degraded metric thresholds (`apps/autopilot-desktop/src/input/actions.rs`).
  - Added automated coverage for SLO metric projection and SLO alert threshold behavior, and wired both checks into the earnings epic gate (`apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/input/actions.rs`, `scripts/lint/autopilot-earnings-epic-test-gate.sh`).
- 2026-03-04: `#2889` (simulation path isolation) implemented.
  - Added explicit runtime gate for simulation-only panes using `OPENAGENTS_ENABLE_SIMULATION_PANES`, defaulting simulation routes off in production (`apps/autopilot-desktop/src/pane_registry.rs`, `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/app_state.rs`).
  - Isolated simulation paths from default runtime pump/action routes by skipping auto-simulation loops and blocking simulation pane actions when gate is off (`apps/autopilot-desktop/src/input.rs`, `apps/autopilot-desktop/src/input/shortcuts.rs`).
  - Restricted tool-bridge pane resolution/listing so simulation panes are not discoverable/actionable unless explicitly enabled (`apps/autopilot-desktop/src/input/tool_bridge.rs`).
  - Added deterministic coverage for runtime simulation-gate predicates and command/bridge isolation behavior; wired checks into the earnings epic gate (`apps/autopilot-desktop/src/pane_registry.rs`, `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/input/tool_bridge.rs`, `scripts/lint/autopilot-earnings-epic-test-gate.sh`).

## MVP Requirement Matrix (from `docs/MVP.md`)

Reference requirements:

- `docs/MVP.md:24-29`
- `docs/MVP.md:89-97`
- `docs/MVP.md:105-113`

Status by requirement:

1. User can click `Go Online` and observe mode transition.  
Status: Implemented (local runtime lane).  
Evidence: `apps/autopilot-desktop/src/input.rs:2355-2387`, `apps/autopilot-desktop/src/runtime_lanes.rs:510-569`.

2. Network sends at least one paid job to provider.  
Status: Partial (local/seeded paths dominate).  
Evidence: local inbox upsert from request form in `apps/autopilot-desktop/src/input/actions.rs:3643-3690`; no desktop `DvmClient` usage (`rg` shows only crate test references).

3. Provider executes job and returns result lifecycle.  
Status: Partial (state machine present, external execution/publication not fully wired).  
Evidence: active job stage progression remains operator-driven in `apps/autopilot-desktop/src/app_state.rs`, but stage transitions now require external authority references (request/result/payment) and reject missing authority.

4. Wallet receives payment and earnings UI reflects truth.  
Status: Implemented for reconciliation/gating contracts; integration path still partial.  
Evidence: `apps/autopilot-desktop/src/state/earnings_gate.rs:23-90`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs:51-112`, `apps/autopilot-desktop/src/spark_wallet.rs:18-205`.

5. User can withdraw via Lightning payment flow.  
Status: Implemented at wallet operation layer.  
Evidence: send-payment command path in `apps/autopilot-desktop/src/spark_wallet.rs:31-35`, `apps/autopilot-desktop/src/spark_wallet.rs:195-200`.

6. Demand seeding ensures first earning moment.  
Status: Partial/Missing in production path.  
Evidence: starter job state exists, but defaults empty (`apps/autopilot-desktop/src/state/operations.rs:482-489`); no clear runtime population path in non-test flow.

7. Public/global network signal is authoritative.  
Status: Missing (currently local-derived).  
Evidence: “Global Network Earnings Today” computed from local `job_history` in `apps/autopilot-desktop/src/render.rs:454-477`.

## What Is In (Confirmed)

## 1) NIP-90 protocol and client substrate

- NIP-90 helper client exists (`DvmClient`) with publish/subscribe/await APIs: `crates/nostr/client/src/dvm.rs:10-117`.
- NIP-90 e2e mock-relay test passes: `crates/nostr/client/tests/dvm_submit_await_e2e.rs:117-172`.
- Core NIP-90 types/builders are present in `crates/nostr/core/src/nip90/*`.

## 2) Wallet lane is real and functional

- Desktop wallet worker is wired to `openagents-spark` and supports refresh/invoice/send/history.
- Evidence: `apps/autopilot-desktop/src/spark_wallet.rs:8-12`, `apps/autopilot-desktop/src/spark_wallet.rs:182-205`, `apps/autopilot-desktop/src/spark_wallet.rs:245-260`.

## 3) Wallet-authoritative earnings gate exists

- Synthetic payout pointers are rejected.
- Reconciliation maps succeeded receive payments to earned payout events.
- Evidence: `apps/autopilot-desktop/src/state/earnings_gate.rs:47-67`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs:77-112`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs:311-317`.

## 4) Mission Control surface exists

- Sidebar/status/online button/earnings/recent-jobs render logic is present.
- Evidence: `apps/autopilot-desktop/src/render.rs:442-505`.

## What Is Not In (or Not Fully In)

## 1) Desktop runtime is not clearly relay-backed for provider jobs

- Desktop app depends on `nostr` core, not `nostr-client` transport helper crate.
- Evidence: `apps/autopilot-desktop/Cargo.toml:25-43`.
- `DvmClient` is not referenced by desktop app code (only crate exports/tests).
- Evidence: repo search shows usage only in `crates/nostr/client/*` and tests.

Impact:

- The app has protocol substrate available but not visibly bridged into desktop provider execution authority.

## 2) Runtime authority lanes are in-process/synthetic by design

- Lane workers are local thread + channel loops, not relay transport drivers.
- Evidence: `apps/autopilot-desktop/src/runtime_lanes.rs:413-499`.
- Event IDs are generated via local formatter (`prefix:kind:seq`), not relay-signed event identity.
- Evidence: `apps/autopilot-desktop/src/runtime_lanes.rs:1274-1282`.

Impact:

- Good for deterministic local simulation, insufficient as authoritative external provider runtime.

## 3) “Network request to inbox” path is local injection

- Submitting a request in UI directly upserts a `JobInboxNetworkRequest` locally.
- Evidence: `apps/autopilot-desktop/src/input/actions.rs:3643-3690`.
- Request IDs are locally minted (`req-buy-XXXX`).
- Evidence: `apps/autopilot-desktop/src/state/operations.rs:356-381`.

Impact:

- Job demand currently appears generated from local UX path rather than true external buyer market ingress.

## 4) Active job lifecycle is local stage machine

- Active job created from selected inbox request and manually advanced through stages.
- Evidence: `apps/autopilot-desktop/src/app_state.rs`.
- Stage transitions are now authority-gated (request/result/payment) instead of minting synthetic delivered/paid IDs.
- Evidence: `apps/autopilot-desktop/src/app_state.rs`.

Impact:

- User-visible progress can be valid UX scaffolding, but it is not proof of real network-side execution and settlement.

## 5) Relay connection pane behavior is local state, not transport authority

- Add/retry/remove relay operations mutate local rows.
- Evidence: `apps/autopilot-desktop/src/state/operations.rs:71-134`.
- Retry marks connected with fixed latency (`96ms`), indicating simulated health.
- Evidence: `apps/autopilot-desktop/src/state/operations.rs:128-131`.

Impact:

- “Connected” can indicate local pane state rather than confirmed external relay health.

## 6) Global network metrics are local-history derived

- `Global Network Earnings Today` uses sum of local successful job rows.
- Evidence: `apps/autopilot-desktop/src/render.rs:454-477`.

Impact:

- This does not yet satisfy a public/beacon-grade network truth metric.

## 7) Starter-demand flow is incomplete as a guaranteed first-run lane

- Starter job state exists and completion is wallet-confirmation gated.
- Evidence: `apps/autopilot-desktop/src/input/actions.rs:3728-3841`.
- Default starter state is empty and no clear production seeding path was found.
- Evidence: `apps/autopilot-desktop/src/state/operations.rs:482-489`; only test fixtures push rows.

Impact:

- Time-to-first-earned-bitcoin may remain unreliable for cold-start providers.

## 8) Doc/issue closure drift vs code reality

- Epic tracker marks #2814-#2876 closed.
- Evidence: `docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md:28-90`.
- Acceptance gate still requires relay-backed provider runtime and live public stats.
- Evidence: `docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md:15-23`.
- Code evidence above indicates those requirements are only partial.

Impact:

- Planning and execution confidence degrade when closed-state signals are ahead of runtime reality.

## Test Harness Assessment

Current strengths:

- Deterministic state, reconciliation, and policy contracts are thoroughly test-gated (`scripts/lint/autopilot-earnings-epic-test-gate.sh:15-121`).
- NIP-90 transport helper has an isolated e2e mock relay test.

Current gaps:

- Missing desktop-level e2e harness that proves: real relay job ingress -> local execution -> result publish -> wallet-confirmed payout -> mission control update.
- Existing mission-control e2e test is mostly state-assembled and manually injects wallet payment pointer.
- Evidence: `apps/autopilot-desktop/src/app_state.rs:3700-3789`.

## Risk Analysis

Product risk:

- Users can experience a convincing local loop without the full external market loop being live.

Trust risk:

- If “global network” counters are local-only, the beacon narrative can be perceived as inflated.

Delivery risk:

- Closed-issue posture may hide remaining P0 wiring work and delay real launch readiness.

## Recommendations (Build Order)

## P0 (must land first)

1. Wire desktop provider runtime to real relay subscription + NIP-90 job handling path.
2. Replace synthetic event-ID progression in earn-critical paths with signed event authority mapping.
3. Ensure mission-control earnings counters are strictly wallet-reconciled in all displayed totals.
4. Add desktop integration test harness for real relay round-trip + wallet confirmation.

## P1 (immediately after P0)

1. Implement deterministic starter-demand generator/service with explicit budget and safety controls.
2. Build authoritative network stats source and hydrate Mission Control from it.
3. Reconcile issue/doc statuses with code truth and reopen unresolved items.

## P2 (hardening)

1. Failure taxonomy and operator diagnostics (relay/executor/payment/reconciliation breakdown).
2. Performance/SLO telemetry: first-job latency, payout confirmation latency, failure rate by class.
3. Remove or isolate simulation-only code paths from production UX route.

## Suggested GitHub Issues (Name + Description)

1. `Earn P0: Desktop relay-backed NIP-90 provider lane wiring`  
Description: Integrate `nostr-client` transport into desktop runtime and consume live NIP-90 requests as primary job ingress.

2. `Earn P0: Provider job lifecycle external authority mapping`  
Description: Map accepted/running/delivered/paid states to externally verifiable events, not synthetic stage-only transitions.

3. `Earn P0: Result/feedback publishing from desktop execution lane`  
Description: Publish canonical feedback/result/status events for each processed job and persist event IDs for receipts.

4. `Earn P0: Wallet-authoritative mission-control totals`  
Description: Make today/total earnings and recent payout rows derive from reconciled wallet receive evidence only.

5. `Earn P0: Desktop E2E harness (relay -> execute -> publish -> wallet confirm)`  
Description: Add integration harness proving full loop in one automated test path.

6. `Earn P1: Starter-demand generator with budget/kill-switch controls`  
Description: Seed first jobs reliably for new providers using controlled dispatch and real settlement path.

7. `Earn P1: Starter job provenance and receipt tagging`  
Description: Add explicit starter-source metadata in jobs, receipts, and UI labels to avoid confusion with open demand.

8. `Earn P1: Authoritative network stats pipeline`  
Description: Implement aggregate counters service and wire Mission Control global counters to it.

9. `Earn P1: Relay connectivity truth model`  
Description: Replace pane-local relay “connected” simulation with transport-derived health and latency reporting.

10. `Earn P1: Reconcile epic tracker closure with code evidence`  
Description: Reopen/create issues for unresolved acceptance gates and update implementation log claims.

11. `Earn P2: Failure taxonomy + user-facing diagnostics`  
Description: Standardize and surface concise failure classes across relay, execution, payment, and reconciliation.

12. `Earn P2: Loop integrity SLO metrics and alerts`  
Description: Track first-job latency, completion ratio, payout success ratio, and wallet confirmation latency.

13. `Earn P2: Simulation path isolation`  
Description: Gate simulation-only runtime behavior behind explicit dev/test flags and remove from default production path.

14. `Earn P2: Docs consolidation for canonical status`  
Description: Keep one canonical “current implementation status” doc and demote historical logs to appendix references.

## Cleanup Needed

1. Clearly label simulation-only code paths in runtime and panes.
2. Avoid terms like “network” and “global” where values are local projections.
3. Normalize Earn docs so “complete/closed” language tracks testable runtime reality.
4. Remove stale references that imply relay-backed production wiring where it is not yet present.

## Proposed Definition of Earn MVP Complete

Autopilot Earn MVP should be considered complete when all are true:

1. `Go Online` results in externally verifiable provider presence and job intake from relays.
2. At least one supported NIP-90 job type executes end-to-end in the desktop runtime.
3. Result/feedback/status are published and correlated to the originating request.
4. Earnings become visible only after wallet-confirmed receive events.
5. User can send a withdrawal payment from the same wallet without manual state surgery.
6. Starter-demand path guarantees first earnings moment within target SLA.
7. Global network counters are sourced from authoritative shared data, not local history.

## Strategic Note (Compute vs Solver Lane)

Docs correctly frame compute as the MVP revenue lane and liquidity solving as future lane:

- `docs/EARN.md:10-16`
- `docs/plans/hydra-x.md:7-11`
- `docs/plans/hydra-x.md:71-76`

No blocker found in this audit for keeping that strategy. The immediate blocker is execution realism in the compute lane itself.
