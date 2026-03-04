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
Evidence: active job stage progression is local in `apps/autopilot-desktop/src/app_state.rs:2248-2284`; synthetic invoice/settlement IDs in `apps/autopilot-desktop/src/app_state.rs:2269-2277`.

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
- Evidence: `apps/autopilot-desktop/src/app_state.rs:2206-2235`, `apps/autopilot-desktop/src/app_state.rs:2248-2284`.
- Delivered/Paid stages stamp synthetic IDs when absent.
- Evidence: `apps/autopilot-desktop/src/app_state.rs:2269-2277`.

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
