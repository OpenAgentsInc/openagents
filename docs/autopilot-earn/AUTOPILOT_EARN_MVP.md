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

Launch sequencing constraint:

1. The first in-app “it works” moment can and should come from earning itself.
2. A fresh user should be able to reach Mission Control and press `Go Online` immediately.
3. The first sats earned should be celebrated explicitly with milestone feedback, not buried in a ledger row.

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

`Go Online` should continue to mean "I'm available to earn," but this MVP binds that to compute only. Liquidity solver mode must remain an explicit future opt-in and never activate automatically. Provider online state should also require a fresh explicit click each app session; MVP should not auto-restore online mode on launch.

Earn roadmap note: future liquidity solver earnings run through an OpenAgents-native solver market under Hydra, not third-party solver networks.

## 3) UX Spec: Mission Control First

The launch cut should let a fresh user enter provider mode immediately.

The first-run sequence is:

1. Install with minimal friction.
2. Land in Mission Control with a clear prompt to start earning.
3. Press `Go Online`.
4. Get first sats and see them celebrated.

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

Market Activity
Job #8123  AI Inference   50 sats   Open
Job #8119  JSON Repair    20 sats   Open
Job #8111  Starter Job    15 sats   OpenAgents

You are browsing live demand. Go Online to start accepting jobs.

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
- Offline mode should still show live or recently observed market activity; the list should not look empty just because provider mode is off.
- Offline market rows are preview-only. They must be visibly read-only until the user explicitly goes online.
- Advanced panes may remain available via command palette, but not required for first earnings loop.
- Future provider modes may be added later, but compute remains the default active lane for this MVP.
- First sats milestones should be visibly celebrated. Initial milestone set: `10`, `25`, `50`, `100`.

### 3.6 Buy Mode smoke test

For internal verification and staged rollout testing, Mission Control should
optionally expose a small `Buy Mode` block. This is not a second buyer product.
It is a one-click smoke test for the real NIP-90 + Lightning path.

Contract:

- hidden unless `OPENAGENTS_ENABLE_BUY_MODE=1`
- one in-flight outbound request at a time
- publishes one fixed `kind: 5050` request with a tiny prompt template chosen
  for cheap validation
- fixed spend: `2 sats`
- waits for feedback, result, invoice pay, and terminal settlement inline in
  Mission Control
- reuses the existing app-owned `NetworkRequestsState` instead of introducing a
  second buyer-state model

This path complements hosted starter demand. It does not replace the OpenAgents
starter buyer loop that guarantees first provider earnings.

## 4) Scope Alignment To Current Repo

This spec aligns to current MVP authority docs:

- `../MVP.md`
- `../OWNERSHIP.md`
- `../PANES.md`

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

- connect to relays, with the default OpenAgents-hosted Nexus as primary and a curated default public relay set as additional transport,
- ingest and deduplicate NIP-90 requests from the full configured reachable relay set, not just the Nexus relay,
- surface observable market activity to the desktop even before provider mode is enabled,
- auto-accept matching jobs by default subject to policy/capacity and execute them locally,
- publish capability, result, and feedback events to every healthy configured relay by default, using best-effort fanout rather than blocking on every relay ack,
- settle payment to wallet,
- render deterministic job/payment state.

### 5.2 Seed-demand buyer agent (server)

Purpose: bootstrap demand so first-run users reliably get a paid job.

Initial deployment rule: this starter-demand loop runs on the OpenAgents-hosted Nexus only. A self-hosted Nexus should still participate in the open marketplace path and should be public/open by default. Closed/private Nexus modes can come later, but they are not near-term scope. A self-hosted Nexus should not be assumed to provide starter jobs unless that operator explicitly adds its own seed-demand service. The OpenAgents-hosted Nexus remains anon/open for general marketplace traffic, but OpenAgents starter jobs target Autopilot users only and are available only to providers connected to the OpenAgents-hosted Nexus itself.

UI presentation rule: starter jobs should appear in the same normal provider job flow as any other job. In main earn surfaces they should be marked with a visible source indicator such as a badge, label, or star rather than split into a separate primary queue.

Proof rule for starter-demand eligibility:

- do not rely on a Nostr `client` tag alone,
- prefer OpenAgents-hosted-Nexus proof from an authenticated Autopilot session plus bound Nostr identity/presence,
- allow `client` tags as optional observability/debugging metadata only.
- defer stronger anti-spoofing attestation and device-bound proof hardening until after MVP.

Loop:

```text
while true:
  publish_paid_nip90_job()
  pay_provider_invoice()
  sleep(3s)
```

### 5.3 Mission Control Buy Mode (desktop, feature-gated)

Purpose: give operators and QA a way to originate a real paid request from the
desktop itself and verify the buyer path without leaving Mission Control.

Responsibilities:

- build one fixed `kind: 5050` request with a tiny validation-friendly prompt
- set spend to exactly `2 sats`
- publish over the same configured relay set used by the desktop buyer lane
- track `7000` feedback and `6050` result lifecycle inline
- pay the provider invoice from the built-in Spark wallet
- show terminal success only after the buyer payment reaches a terminal wallet
  success state
- stay manual and one-shot; no repeating loop, scheduler, or generic prompt
  composer for `v0.1`

### 5.4 Lightning wallet lane

MVP settlement model:

- provider returns a built-in Spark wallet invoice,
- buyer pays invoice,
- provider UI only marks payout success once wallet receive evidence exists.

## 6) Protocol Flow (NIP-90 Canonical)

Use the in-repo NIP-90 model (`5000-5999` requests, `6000-6999` results, `7000` feedback).

Do not introduce custom `90/91/92/93` kinds for MVP.

1. Buyer publishes job request (`kind: 5000-5999`, example `5050` or `5930`).
2. Provider sees request and auto-accepts if policy and local capacity allow; otherwise it ignores or rejects.
3. Provider emits feedback (`kind: 7000`) with `status=processing` (optional but recommended).
4. Provider publishes result (`kind: request_kind + 1000`) with result payload.
5. Buyer pays invoice.
6. Provider records success only when wallet-confirmed payment pointer is present.

Minimum tag expectations:

- request: `i`, `param`, `bid`, optional `relays`
- result: `e` (request id), `p` (customer), `amount` (+ `bolt11`), `status=success`
- feedback: `status` (`processing`/`payment-required`/`success`/`error`)

Contention model:

- ordinary open-network NIP-90 jobs may be seen by many providers; MVP does not attempt a fake global lock across public relays,
- providers should keep duplicate-work risk bounded with strict local admission controls (`max_inflight`, ttl freshness, minimum reward, per-buyer caps, cheap preflight); for MVP `max_inflight` means concurrent active jobs and should default to `1` until multi-job desktop execution is proven safe,
- OpenAgents starter jobs should use a hosted-Nexus single-assignee lease with an aggressive start-confirm ttl (roughly `10-15s`) and reassignment on timeout, failed start, or lost heartbeat, while allowing a separate more forgiving execution window after work has clearly begun,
- if OpenAgents later wants shared live visibility for those leases in Spacetime, that is a projection/coordination enhancement and should not be confused with current MVP authority.

Buyer resolution modes:

- `starter-lease`: OpenAgents starter jobs are not open race jobs. The OpenAgents-hosted Nexus assigns one provider at a time with a very short start-confirm lease and may reassign quickly if work does not begin. After start is confirmed, the provider gets a more forgiving execution window backed by heartbeat/lease renewal.
- `race` (MVP default for public OpenAgents-posted jobs): first valid result wins, later duplicate results are unpaid. Use this for tiny deterministic jobs where verification is cheap and speed is an acceptable incentive. When the buyer can correlate slower or late duplicate results, it should emit explicit terminal unpaid feedback rather than silently doing nothing.
- `windowed` (later): the job declares a submission window such as `5 minutes`; providers can submit during that window and the buyer evaluates after the deadline using an explicit policy. Use this when quality, diversity, or non-speed criteria matter more than raw latency.

Important constraint:

- a job poster can observe some partial coordination signals, such as `kind 7000` `processing` feedback, but cannot assume it has perfect visibility into all workers on all relays,
- so open-market exclusivity should not be inferred from seeing one `processing` event,
- and `windowed` mode should be treated as a buyer policy mode that tolerates concurrent work rather than trying to eliminate it.
- for `race` mode, the preferred loser path is a terminal feedback event with an unpaid reason in `status_extra` (for example `lost-race` or `late-result-unpaid`) whenever the buyer can correlate the losing result.

## 7) Launch Job Types (Controlled By Buyer)

| Job Type | Kind | Local task | Reward |
| --- | --- | --- | --- |
| Hash computation | `5930` | `sha256(random_string)` | 10 sats |
| Tiny inference | `5050` | short prompt inference | 50 sats |
| JSON transform | `5930` | deterministic transform/normalize | 20 sats |
| 5s benchmark | `5930` | fixed compute loop for 5s | 20 sats |

Start with deterministic jobs first (hash/json/benchmark), then add inference.

Default buyer policy for those launch jobs is `race`. `windowed` evaluation is roadmap work for later job classes.

Mission Control `Buy Mode` is a separate smoke-test contract from the hosted
starter-demand pricing matrix above: it should always publish one fixed
`kind: 5050` request at `2 sats` for manual end-to-end verification.

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

Public endpoint/page target: default OpenAgents-hosted Nexus stats surface, for example `openagents.com/stats`.

Add top-line multiplayer signal in app header:

```text
Global Network Earnings Today: <btc>
```

## 10) 24-Hour MVP Cut (Build Order)

1. Ship one-screen Mission Control UI with `Go Online` as the dominant action.
2. Wire status and counters from existing provider/wallet/job state.
3. Ensure job lifecycle visible in one list: waiting -> running -> completed -> paid.
4. Wire seed-demand buyer to continuously publish paid jobs.
5. Wire feature-gated Mission Control `Buy Mode` to publish/pay one fixed
   `kind: 5050` request at `2 sats`.
6. Enforce wallet-confirmed payout gate in displayed earnings.
7. Add simple public stats aggregation endpoint.

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
5. User can withdraw from the built-in Spark wallet by paying an external Lightning invoice.
6. Stats page reflects live economic activity.
7. With `OPENAGENTS_ENABLE_BUY_MODE=1`, Mission Control can publish one
   `kind: 5050` / `2 sats` smoke-test request and show the terminal
   buyer-side payment outcome inline.

The user should be able to do that withdrawal while still online. Going offline is not a prerequisite for paying out from the built-in wallet.

## 13) Narrative Check (Launch Message)

The launch story is:

> Paid NIP-90 jobs are happening now. Providers are earning sats now.

Not:

> Here is a complex provider dashboard.

Everything in this MVP must reinforce that difference.

## 14) Relationship To Existing Earnings Docs

This spec is the first-run provider-beacon cut.

- It complements (does not replace) `../MVP.md`.
- It aligns with the current marketplace and liquidity-market direction in:
  - `../kernel/README.md`
  - `../kernel/liquidity-market.md`
- Historical Hydra planning notes remain archived under `../plans/deprecated/`.
- It is narrower than autonomous goal/swap operations documented in:
  - `AUTOPILOT_EARNINGS_AUTOMATION.md`
  - `AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
  - `AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`

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

3. **Issue name:** `Restore Candidate Port: Embedded Autopilot Provider Domain`  
   **Description:** Port minimal provider lifecycle patterns from backroom `provider_domain.rs` into the embedded Autopilot provider runtime without reintroducing legacy product scope.

4. **Issue name:** `Restore Candidate Port: Spark Wallet Domain Bridge`  
   **Description:** Port wallet bridge patterns from backroom `wallet_domain.rs` for authoritative balance/history/invoice/send integration.

5. **Issue name:** `Restore Candidate Port: NIP-90 Submit + Await Result Helper`  
   **Description:** Port and harden backroom `submit_nip90_text_generation` flow as reusable buyer/provider test utility in current desktop/runtime stack.

6. **Issue name:** `Retained Runtime Placement Decision (App vs New Crate)`  
   **Description:** Decide final ownership for provider runtime code under `../OWNERSHIP.md`; document exact boundaries before importing large backroom modules.

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

16. **Issue name:** `First Sats Milestone Celebration`  
    **Description:** Celebrate first earnings milestones (`10/25/50/100` sats initially) with truthful wallet-backed UI states and no synthetic progress.

17. **Issue name:** `Offline Market Preview`  
    **Description:** Show live or recently observed job activity in Mission Control and job surfaces before the user goes online, with clear preview-only/read-only state until provider mode is enabled.

18. **Issue name:** `Default Nexus Primary Relay Configuration`  
    **Description:** Preconfigure the desktop to use the OpenAgents-hosted Nexus as the primary Nostr relay plus a curated default public relay set selected from relays with meaningful recent NIP-90 job activity; allow full override to a user-run Nexus and custom relay set, while starter jobs remain tied to the OpenAgents-hosted Nexus initially.

19. **Issue name:** `Public-Open Nexus Default Posture`  
    **Description:** Treat both OpenAgents-hosted and self-hosted Nexus deployments as public/open relays by default; defer closed/private Nexus modes to later roadmap work.

### Provider Runtime and NIP-90 Execution

20. **Issue name:** `Provider Runtime Promotion: Simulated to Relay-Backed`  
    **Description:** Replace simulated provider lifecycle updates with real relay-backed status and event correlation.

21. **Issue name:** `NIP-90 Request Subscription Layer`  
    **Description:** Subscribe to configured request kind ranges (`5000-5999`) with stable filters, reconnect handling, and duplicate suppression.

22. **Issue name:** `Job Admission and Capability Matching`  
    **Description:** Evaluate request kind/params/cost/policy before accept, auto-accept matching jobs by default when capacity allows, include buyer targeting rules such as preferring or requiring OpenAgents participants / Autopilot clients, and expose deterministic reject reasons.

23. **Issue name:** `Starter Job Eligibility Proof`  
    **Description:** Enforce OpenAgents starter-job eligibility only for providers connected to the OpenAgents-hosted Nexus, using hosted-Nexus authenticated Autopilot session evidence and bound Nostr identity rather than optional Nostr client tags alone; defer stronger anti-spoofing attestation hardening to post-MVP follow-up.

24. **Issue name:** `Feedback Event Pipeline (kind 7000)`  
    **Description:** Publish `processing`, `payment-required`, `success`, and `error` feedback events with request linkage and timestamps, fanning out to every healthy configured relay by default without blocking job progression on universal relay success; include explicit terminal unpaid feedback for correlated losing/late `race` results when possible.

25. **Issue name:** `Deterministic Job Executors (Hash/JSON/Benchmark)`  
    **Description:** Implement deterministic local executors for initial paid jobs with reproducible outputs and timing.

26. **Issue name:** `Minimal Inference Executor (kind 5050)`  
    **Description:** Add bounded local inference execution path for text-generation jobs with model/timeout controls.

27. **Issue name:** `Result Publishing Pipeline (6000-6999)`  
    **Description:** Publish NIP-90 result events with canonical `e/p/amount/status` tagging and payload integrity hashes, using broad healthy-relay fanout by default and surfacing partial publish failure states explicitly.

28. **Issue name:** `Job Correlation Model (request-feedback-result-payment)`  
    **Description:** Enforce a single correlation key strategy across inbox, active job, history, and wallet reconciliation lanes.

29. **Issue name:** `Active Job Pane Authority Wiring`  
    **Description:** Source active-job stage transitions from runtime events, not local manual stage stepping.

30. **Issue name:** `Job History Receipt Authority Wiring`  
    **Description:** Source history rows from authoritative runtime/wallet settlement evidence and keep immutable receipt metadata.

31. **Issue name:** `Remove Synthetic Success Paths for Earnings`  
    **Description:** Eliminate any path where local-only state can mark a paid success without wallet-confirmed settlement.

### Lightning Settlement and Wallet Authority

32. **Issue name:** `Per-Job Invoice Contract`  
    **Description:** Define and enforce invoice generation contract per accepted job, including amount, expiry, and correlation metadata.

33. **Issue name:** `Buyer Invoice Payment Worker`  
    **Description:** Implement buyer-side payment worker that settles provider invoices and records payment outcomes with retries.

34. **Issue name:** `Wallet Receive Confirmation Ingestion`  
    **Description:** Ingest receive confirmations into desktop state so payout status can be derived from wallet evidence.

35. **Issue name:** `Wallet-Job Reconciliation Projection`  
    **Description:** Build deterministic reconciliation between job receipts and wallet receives, including mismatch reason codes.

36. **Issue name:** `Authoritative Payout Gate Enforcement`  
    **Description:** Enforce payout-complete only when reconciliation confirms wallet receipt linkage for the corresponding job.

37. **Issue name:** `Withdrawal Flow Hardening`  
    **Description:** Improve pay-invoice withdraw UX with clear terminal states, retry guidance, and deterministic status refresh.

38. **Issue name:** `Payment Failure Lifecycle`  
    **Description:** Add payment timeout, retry budget, and terminal-failure handling with user-visible reason codes.

39. **Issue name:** `Synthetic Payment Pointer Hard Gate`  
    **Description:** Reject synthetic pointers (`pay:*`, pending placeholders) at ingest and UI layers for completed payout claims.

### Seed Demand and Buyer Lane

40. **Issue name:** `Seed Demand Buyer Service (MVP)`  
    **Description:** Stand up minimal buyer loop service that posts paid NIP-90 jobs, assigns each starter job to a single eligible provider using a short-lived hosted-Nexus lease with aggressive start-confirm timeout and more forgiving execution lease semantics, and settles provider invoices on cadence.

41. **Issue name:** `Seed Job Templates and Pricing Matrix`  
    **Description:** Define controlled starter job templates (hash/json/benchmark/inference) and sats pricing used by buyer loop.

42. **Issue name:** `Seed Pool Budget and Kill Switch Controls`  
    **Description:** Add configurable sats pool, spend limits, and immediate disable controls for safe launch operations.

43. **Issue name:** `Starter vs Open-Network Labeling`  
    **Description:** Label and track starter jobs distinctly from open-network demand in UI, metrics, and operator reports, while keeping them in the normal job flow with a visible badge/marker rather than a separate primary user queue; include that OpenAgents starter jobs target Autopilot users only.

44. **Issue name:** `First-Earnings SLA Monitor`  
    **Description:** Measure and alert on time-to-first-paid-job for newly online providers.

45. **Issue name:** `Seed Demand Reliability Backpressure`  
    **Description:** Add dispatch backpressure controls based on provider availability, payout success, queue latency, aggressive starter-job start-confirm timeout health, and execution-lease reassignment health.

46. **Issue name:** `Starter Jobs Availability Gating By Nexus`  
    **Description:** Make it explicit in UI and runtime that OpenAgents starter jobs are available only when connected to the OpenAgents-hosted Nexus, not through a third-party Nexus bridge, and that third-party operators must run their own seed-demand service if they want equivalent starter jobs.

### Metrics, Stats, and Public Beacon

47. **Issue name:** `Canonical Earn Metrics Schema`  
    **Description:** Define canonical metrics for providers online, jobs posted/completed, completion latency, sats paid, and failures.

48. **Issue name:** `Metrics Emission from Desktop and Buyer`  
    **Description:** Emit structured telemetry from provider app and buyer service using stable metric names and dimensions.

49. **Issue name:** `Stats Aggregation Service Contract`  
    **Description:** Implement aggregation/storage contract for live and historical market metrics powering public stats.

50. **Issue name:** `openagents.com/stats Implementation`  
    **Description:** Implement public stats endpoint/page showing live beacon metrics for ecosystem visibility.

51. **Issue name:** `In-App Network Stats Hydration`  
    **Description:** Hydrate Mission Control network stats from aggregator with stale/error handling.

52. **Issue name:** `Global Earnings Today Computation`  
    **Description:** Compute and cache global daily sats paid with deterministic rollup boundaries and correction logic.

53. **Issue name:** `Metric Integrity Checks`  
    **Description:** Add consistency checks between wallet-confirmed payout totals and stats aggregation outputs.

### Reliability, Sync, and Operations

54. **Issue name:** `Replay-Safe Apply for Job/Payment Events`  
    **Description:** Extend retained apply engine for idempotent replay-safe processing of job and settlement events.

55. **Issue name:** `Duplicate Suppression Keys`  
    **Description:** Define deterministic duplicate keys for request/result/feedback/payment events and enforce them in projections.

56. **Issue name:** `Stale Cursor Rebootstrap for Earn Lanes`  
    **Description:** Ensure stale cursor recovery for job and wallet projections with no duplicate earnings side effects.

57. **Issue name:** `Crash Recovery for In-Flight Jobs`  
    **Description:** Recover in-flight jobs after restart with explicit resumed/failed states and no double settlement.

58. **Issue name:** `Relay Outage Degraded Mode UX`  
    **Description:** Improve degraded-mode transparency and actionable recovery guidance during relay/network incidents.

59. **Issue name:** `Payout Mismatch Incident Runbook`  
    **Description:** Add runbook for payout mismatch triage, evidence capture, containment, and user-facing status updates.

60. **Issue name:** `Rollout Flags and Cohort Controls for Earn`  
    **Description:** Add staged rollout controls, health thresholds, and rollback actions specific to Mission Control earn loop.

### Test Matrix and Launch Gates

61. **Issue name:** `NIP-90 Builder/Parser Unit Coverage`  
    **Description:** Add/expand unit tests for request/result/feedback serialization and validation in canonical kind ranges.

62. **Issue name:** `Integration Test: Request to Result to Payment`  
    **Description:** Add integration test proving end-to-end request handling, execution, result publish, invoice pay, and wallet confirmation.

63. **Issue name:** `Desktop Mission Control State Tests`  
    **Description:** Add deterministic tests for offline/connecting/online/degraded transitions and earnings UI updates.

64. **Issue name:** `E2E Test: First Sats Moment`  
    **Description:** Add full end-to-end test that validates first-run flow through wallet-increment proof.

65. **Issue name:** `Chaos Test: Relay Loss + Recovery`  
    **Description:** Validate replay-safe reconnect behavior under relay disconnects and ensure no duplicate receipts.

66. **Issue name:** `Chaos Test: Wallet Error + Recovery`  
    **Description:** Validate payout-gate behavior and UI degradation/recovery when wallet operations fail.

67. **Issue name:** `Stress Harness: 3-Second Job Cadence`  
    **Description:** Add load harness for seed-demand cadence with latency, failure, and settlement success reporting.

68. **Issue name:** `Earn MVP Merge Gate Script`  
    **Description:** Add deterministic test/lint gate script covering critical earn-loop acceptance checks.

69. **Issue name:** `Launch Rehearsal and Production Signoff`  
    **Description:** Run staged rehearsal with fixed sats pool and publish signoff evidence before broad enablement.

### Buy Mode Smoke-Test Additions

70. **Issue name:** `Mission Control Buy Mode Inline Smoke Test`
    **Description:** Add a feature-gated inline Mission Control block that
    submits one fixed `kind: 5050` request at `2 sats`, renders the
    buyer-side lifecycle from `NetworkRequestsState`, and never opens a second
    buyer pane.
    **GitHub issue:** [#3378](https://github.com/OpenAgentsInc/openagents/issues/3378)

71. **Issue name:** `Desktop 5050 / 2-Sat Buyer Pipeline`
    **Description:** Reuse the existing desktop network-request lane to publish
    the buy-mode `5050` smoke-test request, correlate `7000` feedback and
    `6050` result events, pay the provider invoice from Spark, and expose
    terminal settlement reasons.
    **GitHub issue:** [#3379](https://github.com/OpenAgentsInc/openagents/issues/3379)

72. **Issue name:** `Buy Mode End-to-End Regression Coverage`
    **Description:** Add automated coverage proving Mission Control buy mode can
    publish the fixed `5050` request, receive a result, send the `2 sats`
    payment, and render terminal success/failure deterministically.
    **GitHub issue:** [#3380](https://github.com/OpenAgentsInc/openagents/issues/3380)

### Existing Issues To Reuse/Extend

The existing issue sequence in current repo docs should be reused where it overlaps:

- `#2708` through `#2732` from `../audits/2026-03-02-autopilot-goal-automation-epic-tracker.md`.
- These issues cover major portions of scheduler, payout gating, reconciliation, swap, and rollout hardening.
- The new backlog above adds the missing Mission Control-first surface and backroom restore tracks needed for this specific Earn MVP beacon launch.
