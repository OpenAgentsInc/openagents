# Autopilot Earn MVP (Beacon Launch Spec)

Date: 2026-03-04  
Status: Draft MVP spec

## 1) Product Intent

This launch is a market signal, not a feature dump.

Core message to users and the ecosystem:

> Paid NIP-90 jobs are live right now, and providers are earning real sats.

The MVP is optimized for three moments only:

1. "I can sell my compute."
2. "There are real paid jobs."
3. "I just earned sats."

If those three moments are not obvious in the first session, MVP failed.

This document is intentionally the compute-provider cut of Earn. It does not attempt to ship the full multi-lane economy in one pass.

## 2) MVP Outcome

A fresh desktop user can complete this loop in minutes:

1. Open app.
2. Press `Go Online`.
3. Receive and execute at least one paid NIP-90 job.
4. See wallet-confirmed sats increase.
5. Withdraw by paying a Lightning invoice.

No synthetic payout-only states count as success.

### 2.1) Multi-Lane Context (Canonical, Scope-Gated)

Autopilot Earn is a provider marketplace with multiple revenue lanes:

1. **Compute provider** (this MVP): execute paid NIP-90 jobs.
2. **Liquidity solver** (future Hydra lane): fill liquidity intents using capital + execution and earn fees/spreads.

`Go Online` should continue to mean "I'm available to earn," but this MVP binds that to compute only. Liquidity solver mode must remain an explicit future opt-in and never activate automatically.

Earn roadmap note: future liquidity solver earnings run through an OpenAgents-native solver market under Hydra, not third-party solver networks.

## 3) UX Spec: Mission Control First

### 3.1 First screen (offline)

```text
Autopilot - Mission Control

Status: OFFLINE

[ GO ONLINE ]

Network Stats
Providers Online: <n>
Jobs Completed: <n>
Sats Paid: <btc>

Your Earnings
Today: 0 sats
Total: 0 sats

Recent Jobs
(empty)

Wallet: Connected
```

### 3.2 After pressing `GO ONLINE`

```text
Status: ONLINE
Listening for jobs...

[ GO OFFLINE ]

Your Earnings
Today: 0 sats
Total: 0 sats

Recent Jobs
Waiting for first job...
```

### 3.3 During a job

```text
Job #8123
Type: AI Inference
Reward: 50 sats
Status: Running...
```

### 3.4 Completion + payout moment

```text
Job #8123
Type: AI Inference
Reward: 50 sats
Status: Completed

Your Earnings
Today: 50 sats
Total: 50 sats
```

### 3.5 UI rules

- No onboarding maze on first run.
- No settings-first interaction.
- One primary action button (`Go Online`/`Go Offline`).
- Advanced panes may remain available via command palette, but not required for first earnings loop.
- Future provider modes may be added later, but compute remains the default active lane for this MVP.

## 4) Scope Alignment To Current Repo

This spec aligns to current MVP authority docs:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PANES.md`

Ownership constraints:

- Product workflow/UI behavior lives in `apps/autopilot-desktop`.
- `crates/wgpui` remains product-agnostic UI infrastructure.
- Wallet primitives remain in `crates/spark`.

Existing pane/state surfaces already cover most required fields:

- provider mode: `ProviderRuntimeState`
- earnings: `EarningsScoreboardState`
- request intake: `JobInboxState`
- in-flight lifecycle: `ActiveJobState`
- receipts: `JobHistoryState`
- wallet authority: `SparkPaneState`

## 5) Minimal System Architecture

### 5.1 Desktop provider app (Rust)

Responsibilities:

- connect to relays,
- ingest NIP-90 requests,
- accept and execute supported jobs locally,
- publish result/feedback events,
- settle payment to wallet,
- render deterministic job/payment state.

### 5.2 Seed-demand buyer agent (server)

Purpose: bootstrap demand so first-run users reliably get a paid job.

Loop:

```text
while true:
  publish_paid_nip90_job()
  pay_provider_invoice()
  sleep(3s)
```

### 5.3 Lightning wallet lane

MVP settlement model:

- provider returns invoice,
- buyer pays invoice,
- provider UI only marks payout success once wallet receive evidence exists.

## 6) Protocol Flow (NIP-90 Canonical)

Use the in-repo NIP-90 model (`5000-5999` requests, `6000-6999` results, `7000` feedback).

Do not introduce custom `90/91/92/93` kinds for MVP.

1. Buyer publishes job request (`kind: 5000-5999`, example `5050` or `5930`).
2. Provider sees request and claims/starts work.
3. Provider emits feedback (`kind: 7000`) with `status=processing` (optional but recommended).
4. Provider publishes result (`kind: request_kind + 1000`) with result payload.
5. Buyer pays invoice.
6. Provider records success only when wallet-confirmed payment pointer is present.

Minimum tag expectations:

- request: `i`, `param`, `bid`, optional `relays`
- result: `e` (request id), `p` (customer), `amount` (+ `bolt11`), `status=success`
- feedback: `status` (`processing`/`payment-required`/`success`/`error`)

## 7) Launch Job Types (Controlled By Buyer)

| Job Type | Kind | Local task | Reward |
| --- | --- | --- | --- |
| Hash computation | `5930` | `sha256(random_string)` | 10 sats |
| Tiny inference | `5050` | short prompt inference | 50 sats |
| JSON transform | `5930` | deterministic transform/normalize | 20 sats |
| 5s benchmark | `5930` | fixed compute loop for 5s | 20 sats |

Start with deterministic jobs first (hash/json/benchmark), then add inference.

## 8) Seed Demand + Stress Strategy

Suggested test pool: `200,000 sats`.

Dispatch cadence: every `3 seconds` while pool remains.

Track:

- provider join rate,
- time-to-first-job,
- completion latency,
- payout success rate,
- fail/error distribution.

## 9) Metrics And Public Signal

Primary in-app and public stats:

- providers online,
- jobs posted,
- jobs completed,
- median completion time,
- sats paid,
- failed jobs.

Public endpoint/page target: `openagents.com/stats`.

Add top-line multiplayer signal in app header:

```text
Global Network Earnings Today: <btc>
```

## 10) 24-Hour MVP Cut (Build Order)

1. Ship one-screen Mission Control UI with `Go Online` as the dominant action.
2. Wire status and counters from existing provider/wallet/job state.
3. Ensure job lifecycle visible in one list: waiting -> running -> completed -> paid.
4. Wire seed-demand buyer to continuously publish paid jobs.
5. Enforce wallet-confirmed payout gate in displayed earnings.
6. Add simple public stats aggregation endpoint.

## 11) Out Of Scope (Hard No For MVP)

- complex dashboards,
- distributed scheduler/orchestration,
- reputation economy,
- escrow/dispute systems,
- plugin marketplace UX,
- broad multi-model infra controls,
- Hydra liquidity-solver execution or capital-routing UX.

## 12) Acceptance Criteria

MVP is ready when all are true:

1. User can go from app launch to `Go Online` in one click.
2. First paid job appears quickly (target: < 60s with seed demand active).
3. Job completes and earnings UI increments.
4. Increment is backed by real wallet receive evidence.
5. User can withdraw by paying an external invoice.
6. Stats page reflects live economic activity.

## 13) Narrative Check (Launch Message)

The launch story is:

> Paid NIP-90 jobs are happening now. Providers are earning sats now.

Not:

> Here is a complex provider dashboard.

Everything in this MVP must reinforce that difference.

## 14) Relationship To Existing Earnings Docs

This spec is the first-run provider-beacon cut.

- It complements (does not replace) `docs/MVP.md`.
- It aligns with the phased liquidity strategy in:
  - `docs/plans/hydra-x.md`
  - `docs/plans/hydra-liquidity-engine.md`
- It is narrower than autonomous goal/swap operations documented in:
  - `docs/AUTOPILOT_EARNINGS_AUTOMATION.md`
  - `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
  - `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`

## 15) Full GitHub Issue Backlog (Name + Description)

This is the full issue set required to implement Autopilot Earn end-to-end from current state.

Backroom review completed before this list. Relevant candidate restore sources include:

- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/autopilot-desktop/src/provider_domain.rs`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/autopilot-desktop/src/wallet_domain.rs`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/autopilot-desktop/src/main.rs` (`submit_nip90_text_generation`)
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/pylon/`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/compute/`

### Program + Backroom Harvest

1. **Issue name:** `[Epic] Autopilot Earn MVP Full Implementation Program`  
   **Description:** Create umbrella epic covering Mission Control UX, NIP-90 provider runtime, real Lightning settlement, seed demand, observability, tests, and rollout gates.

2. **Issue name:** `Backroom Harvest Audit: NIP-90/Provider/Wallet Assets`  
   **Description:** Catalog portable artifacts from backroom code, including API contracts, state models, tests, and migration risk; produce keep/drop decisions per file.

3. **Issue name:** `Restore Candidate Port: InProcessPylon Provider Domain`  
   **Description:** Port minimal provider lifecycle patterns from backroom `provider_domain.rs` into MVP app-layer runtime owner without reintroducing legacy product scope.

4. **Issue name:** `Restore Candidate Port: Spark Wallet Domain Bridge`  
   **Description:** Port wallet bridge patterns from backroom `wallet_domain.rs` for authoritative balance/history/invoice/send integration.

5. **Issue name:** `Restore Candidate Port: NIP-90 Submit + Await Result Helper`  
   **Description:** Port and harden backroom `submit_nip90_text_generation` flow as reusable buyer/provider test utility in current desktop/runtime stack.

6. **Issue name:** `Retained Runtime Placement Decision (App vs New Crate)`  
   **Description:** Decide final ownership for provider runtime code under `docs/OWNERSHIP.md`; document exact boundaries before importing large backroom modules.

7. **Issue name:** `Backroom Provenance and Migration Notes`  
   **Description:** Add retained provenance notes for every restored block, including source path, modifications, and deleted legacy behavior.

### Mission Control Product Surface

8. **Issue name:** `Mission Control Single-Screen Shell`  
   **Description:** Implement one-screen `Autopilot - Mission Control` layout as the default startup surface with no first-run pane maze.

9. **Issue name:** `Primary CTA: One-Button Go Online/Go Offline`  
   **Description:** Promote `Go Online` to dominant action and wire directly to provider runtime state transitions with explicit pending/error states.

10. **Issue name:** `Mission Control Network Stats Panel`  
    **Description:** Show providers online, jobs completed, sats paid from canonical metrics lane with stale-state visibility.

11. **Issue name:** `Mission Control Earnings Panel`  
    **Description:** Show today/total sats from wallet-confirmed projection, never from synthetic-only entries.

12. **Issue name:** `Mission Control Recent Jobs Feed`  
    **Description:** Show compact lifecycle rows (waiting/running/completed/paid) tied to authoritative job IDs and timestamps.

13. **Issue name:** `Global Network Earnings Header`  
    **Description:** Add top-line global network earnings ticker with clear source attribution and refresh semantics.

14. **Issue name:** `First-Run Ready-To-Earn Flow`  
    **Description:** Ensure first launch reaches online-capable state in minimal steps (identity, wallet, relay readiness), with blockers shown inline.

15. **Issue name:** `No-Jobs Waiting UX`  
    **Description:** Replace dead-looking idle state with explicit online heartbeat, wait status, and seed-demand expectation hints.

### Provider Runtime and NIP-90 Execution

16. **Issue name:** `Provider Runtime Promotion: Simulated to Relay-Backed`  
    **Description:** Replace simulated provider lifecycle updates with real relay-backed status and event correlation.

17. **Issue name:** `NIP-90 Request Subscription Layer`  
    **Description:** Subscribe to configured request kind ranges (`5000-5999`) with stable filters, reconnect handling, and duplicate suppression.

18. **Issue name:** `Job Admission and Capability Matching`  
    **Description:** Evaluate request kind/params/cost/policy before accept; expose deterministic reject reasons.

19. **Issue name:** `Feedback Event Pipeline (kind 7000)`  
    **Description:** Publish `processing`, `payment-required`, `success`, and `error` feedback events with request linkage and timestamps.

20. **Issue name:** `Deterministic Job Executors (Hash/JSON/Benchmark)`  
    **Description:** Implement deterministic local executors for initial paid jobs with reproducible outputs and timing.

21. **Issue name:** `Minimal Inference Executor (kind 5050)`  
    **Description:** Add bounded local inference execution path for text-generation jobs with model/timeout controls.

22. **Issue name:** `Result Publishing Pipeline (6000-6999)`  
    **Description:** Publish NIP-90 result events with canonical `e/p/amount/status` tagging and payload integrity hashes.

23. **Issue name:** `Job Correlation Model (request-feedback-result-payment)`  
    **Description:** Enforce a single correlation key strategy across inbox, active job, history, and wallet reconciliation lanes.

24. **Issue name:** `Active Job Pane Authority Wiring`  
    **Description:** Source active-job stage transitions from runtime events, not local manual stage stepping.

25. **Issue name:** `Job History Receipt Authority Wiring`  
    **Description:** Source history rows from authoritative runtime/wallet settlement evidence and keep immutable receipt metadata.

26. **Issue name:** `Remove Synthetic Success Paths for Earnings`  
    **Description:** Eliminate any path where local-only state can mark a paid success without wallet-confirmed settlement.

### Lightning Settlement and Wallet Authority

27. **Issue name:** `Per-Job Invoice Contract`  
    **Description:** Define and enforce invoice generation contract per accepted job, including amount, expiry, and correlation metadata.

28. **Issue name:** `Buyer Invoice Payment Worker`  
    **Description:** Implement buyer-side payment worker that settles provider invoices and records payment outcomes with retries.

29. **Issue name:** `Wallet Receive Confirmation Ingestion`  
    **Description:** Ingest receive confirmations into desktop state so payout status can be derived from wallet evidence.

30. **Issue name:** `Wallet-Job Reconciliation Projection`  
    **Description:** Build deterministic reconciliation between job receipts and wallet receives, including mismatch reason codes.

31. **Issue name:** `Authoritative Payout Gate Enforcement`  
    **Description:** Enforce payout-complete only when reconciliation confirms wallet receipt linkage for the corresponding job.

32. **Issue name:** `Withdrawal Flow Hardening`  
    **Description:** Improve pay-invoice withdraw UX with clear terminal states, retry guidance, and deterministic status refresh.

33. **Issue name:** `Payment Failure Lifecycle`  
    **Description:** Add payment timeout, retry budget, and terminal-failure handling with user-visible reason codes.

34. **Issue name:** `Synthetic Payment Pointer Hard Gate`  
    **Description:** Reject synthetic pointers (`pay:*`, pending placeholders) at ingest and UI layers for completed payout claims.

### Seed Demand and Buyer Lane

35. **Issue name:** `Seed Demand Buyer Service (MVP)`  
    **Description:** Stand up minimal buyer loop service that posts paid NIP-90 jobs and settles provider invoices on cadence.

36. **Issue name:** `Seed Job Templates and Pricing Matrix`  
    **Description:** Define controlled starter job templates (hash/json/benchmark/inference) and sats pricing used by buyer loop.

37. **Issue name:** `Seed Pool Budget and Kill Switch Controls`  
    **Description:** Add configurable sats pool, spend limits, and immediate disable controls for safe launch operations.

38. **Issue name:** `Starter vs Open-Network Labeling`  
    **Description:** Label and track starter jobs distinctly from open-network demand in UI, metrics, and operator reports.

39. **Issue name:** `First-Earnings SLA Monitor`  
    **Description:** Measure and alert on time-to-first-paid-job for newly online providers.

40. **Issue name:** `Seed Demand Reliability Backpressure`  
    **Description:** Add dispatch backpressure controls based on provider availability, payout success, and queue latency.

### Metrics, Stats, and Public Beacon

41. **Issue name:** `Canonical Earn Metrics Schema`  
    **Description:** Define canonical metrics for providers online, jobs posted/completed, completion latency, sats paid, and failures.

42. **Issue name:** `Metrics Emission from Desktop and Buyer`  
    **Description:** Emit structured telemetry from provider app and buyer service using stable metric names and dimensions.

43. **Issue name:** `Stats Aggregation Service Contract`  
    **Description:** Implement aggregation/storage contract for live and historical market metrics powering public stats.

44. **Issue name:** `openagents.com/stats Implementation`  
    **Description:** Implement public stats endpoint/page showing live beacon metrics for ecosystem visibility.

45. **Issue name:** `In-App Network Stats Hydration`  
    **Description:** Hydrate Mission Control network stats from aggregator with stale/error handling.

46. **Issue name:** `Global Earnings Today Computation`  
    **Description:** Compute and cache global daily sats paid with deterministic rollup boundaries and correction logic.

47. **Issue name:** `Metric Integrity Checks`  
    **Description:** Add consistency checks between wallet-confirmed payout totals and stats aggregation outputs.

### Reliability, Sync, and Operations

48. **Issue name:** `Replay-Safe Apply for Job/Payment Events`  
    **Description:** Extend retained apply engine for idempotent replay-safe processing of job and settlement events.

49. **Issue name:** `Duplicate Suppression Keys`  
    **Description:** Define deterministic duplicate keys for request/result/feedback/payment events and enforce them in projections.

50. **Issue name:** `Stale Cursor Rebootstrap for Earn Lanes`  
    **Description:** Ensure stale cursor recovery for job and wallet projections with no duplicate earnings side effects.

51. **Issue name:** `Crash Recovery for In-Flight Jobs`  
    **Description:** Recover in-flight jobs after restart with explicit resumed/failed states and no double settlement.

52. **Issue name:** `Relay Outage Degraded Mode UX`  
    **Description:** Improve degraded-mode transparency and actionable recovery guidance during relay/network incidents.

53. **Issue name:** `Payout Mismatch Incident Runbook`  
    **Description:** Add runbook for payout mismatch triage, evidence capture, containment, and user-facing status updates.

54. **Issue name:** `Rollout Flags and Cohort Controls for Earn`  
    **Description:** Add staged rollout controls, health thresholds, and rollback actions specific to Mission Control earn loop.

### Test Matrix and Launch Gates

55. **Issue name:** `NIP-90 Builder/Parser Unit Coverage`  
    **Description:** Add/expand unit tests for request/result/feedback serialization and validation in canonical kind ranges.

56. **Issue name:** `Integration Test: Request to Result to Payment`  
    **Description:** Add integration test proving end-to-end request handling, execution, result publish, invoice pay, and wallet confirmation.

57. **Issue name:** `Desktop Mission Control State Tests`  
    **Description:** Add deterministic tests for offline/connecting/online/degraded transitions and earnings UI updates.

58. **Issue name:** `E2E Test: First Sats Moment`  
    **Description:** Add full end-to-end test that validates first-run flow through wallet-increment proof.

59. **Issue name:** `Chaos Test: Relay Loss + Recovery`  
    **Description:** Validate replay-safe reconnect behavior under relay disconnects and ensure no duplicate receipts.

60. **Issue name:** `Chaos Test: Wallet Error + Recovery`  
    **Description:** Validate payout-gate behavior and UI degradation/recovery when wallet operations fail.

61. **Issue name:** `Stress Harness: 3-Second Job Cadence`  
    **Description:** Add load harness for seed-demand cadence with latency, failure, and settlement success reporting.

62. **Issue name:** `Earn MVP Merge Gate Script`  
    **Description:** Add deterministic test/lint gate script covering critical earn-loop acceptance checks.

63. **Issue name:** `Launch Rehearsal and Production Signoff`  
    **Description:** Run staged rehearsal with fixed sats pool and publish signoff evidence before broad enablement.

### Existing Issues To Reuse/Extend

The existing issue sequence in current repo docs should be reused where it overlaps:

- `#2708` through `#2732` from `docs/audits/2026-03-02-autopilot-goal-automation-epic-tracker.md`.
- These issues cover major portions of scheduler, payout gating, reconciliation, swap, and rollout hardening.
- The new backlog above adds the missing Mission Control-first surface and backroom restore tracks needed for this specific Earn MVP beacon launch.
