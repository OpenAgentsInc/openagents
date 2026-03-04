# Autopilot Earn MVP Epic Tracker

Date: 2026-03-04
Epic issue: [#2814](https://github.com/OpenAgentsInc/openagents/issues/2814)
Spec source: `docs/AUTOPILOT_EARN_MVP.md`

## Purpose

Track the full Autopilot Earn MVP execution program from issue creation through implementation closeout.

Scope note: this tracker covers the compute-provider MVP lane. Future liquidity-solver work should be tracked as a separate Hydra-aligned program.

## Acceptance Gates

The epic is complete only when all are true:

1. Mission Control first-run loop is implemented and default.
2. Provider runtime is relay-backed for NIP-90 request/result/feedback flow.
3. Wallet-confirmed payout gate is enforced in all earnings surfaces.
4. Seed-demand buyer lane is operational with spend controls.
5. Public stats lane is live and consistent with wallet-confirmed payouts.
6. Reliability/test/rollout gates are green.

## Current Reconciliation Status (2026-03-04)

Epic issue `#2814` is historically closed, but the 2026-03-04 code audit opened a follow-on reconciliation track (`#2877` to `#2890`) to align claims with merged code and test evidence.

Current gate status:

| Gate | Status | Evidence |
| --- | --- | --- |
| Mission Control first-run loop | ✅ Implemented | Closed implementation stream through `#2821`-`#2828` |
| Relay-backed provider runtime (request/result/feedback) | ✅ Implemented | `#2877`, `#2878`, `#2879` |
| Wallet-confirmed payout gate | ✅ Implemented | `#2880` |
| Seed-demand buyer lane with controls | ✅ Implemented | `#2882`, `#2883` |
| Public stats lane consistent with wallet-confirmed payouts | ✅ Implemented | `#2884`, `#2885` |
| Reliability/test/rollout and canonical-status closure | ⏳ In progress | Open `#2888`, `#2889`, `#2890` |

The epic should not be treated as fully complete until the open reconciliation issues above are closed with code and verification evidence.

## Reconciliation Follow-On Issues

| # | Title | State |
| --- | --- | --- |
| [#2877](https://github.com/OpenAgentsInc/openagents/issues/2877) | Earn P0: Desktop relay-backed NIP-90 provider lane wiring | CLOSED |
| [#2878](https://github.com/OpenAgentsInc/openagents/issues/2878) | Earn P0: Provider job lifecycle external authority mapping | CLOSED |
| [#2879](https://github.com/OpenAgentsInc/openagents/issues/2879) | Earn P0: Result/feedback publishing from desktop execution lane | CLOSED |
| [#2880](https://github.com/OpenAgentsInc/openagents/issues/2880) | Earn P0: Wallet-authoritative mission-control totals | CLOSED |
| [#2881](https://github.com/OpenAgentsInc/openagents/issues/2881) | Earn P0: Desktop E2E harness (relay -> execute -> publish -> wallet confirm) | CLOSED |
| [#2882](https://github.com/OpenAgentsInc/openagents/issues/2882) | Earn P1: Starter-demand generator with budget/kill-switch controls | CLOSED |
| [#2883](https://github.com/OpenAgentsInc/openagents/issues/2883) | Earn P1: Starter job provenance and receipt tagging | CLOSED |
| [#2884](https://github.com/OpenAgentsInc/openagents/issues/2884) | Earn P1: Authoritative network stats pipeline | CLOSED |
| [#2885](https://github.com/OpenAgentsInc/openagents/issues/2885) | Earn P1: Relay connectivity truth model | CLOSED |
| [#2886](https://github.com/OpenAgentsInc/openagents/issues/2886) | Earn P1: Reconcile epic tracker closure with code evidence | CLOSED |
| [#2887](https://github.com/OpenAgentsInc/openagents/issues/2887) | Earn P2: Failure taxonomy + user-facing diagnostics | CLOSED |
| [#2888](https://github.com/OpenAgentsInc/openagents/issues/2888) | Earn P2: Loop integrity SLO metrics and alerts | OPEN |
| [#2889](https://github.com/OpenAgentsInc/openagents/issues/2889) | Earn P2: Simulation path isolation | OPEN |
| [#2890](https://github.com/OpenAgentsInc/openagents/issues/2890) | Earn P2: Docs consolidation for canonical status | OPEN |

## Historical Issue List (`#2814 - #2876`)

| # | Title | State |
| --- | --- | --- |
| [#2814](https://github.com/OpenAgentsInc/openagents/issues/2814) | [Epic] Autopilot Earn MVP Full Implementation Program | CLOSED |
| [#2815](https://github.com/OpenAgentsInc/openagents/issues/2815) | Backroom Harvest Audit: NIP-90/Provider/Wallet Assets | CLOSED |
| [#2816](https://github.com/OpenAgentsInc/openagents/issues/2816) | Restore Candidate Port: InProcessPylon Provider Domain | CLOSED |
| [#2817](https://github.com/OpenAgentsInc/openagents/issues/2817) | Restore Candidate Port: Spark Wallet Domain Bridge | CLOSED |
| [#2818](https://github.com/OpenAgentsInc/openagents/issues/2818) | Restore Candidate Port: NIP-90 Submit + Await Result Helper | CLOSED |
| [#2819](https://github.com/OpenAgentsInc/openagents/issues/2819) | Retained Runtime Placement Decision (App vs New Crate) | CLOSED |
| [#2820](https://github.com/OpenAgentsInc/openagents/issues/2820) | Backroom Provenance and Migration Notes | CLOSED |
| [#2821](https://github.com/OpenAgentsInc/openagents/issues/2821) | Mission Control Single-Screen Shell | CLOSED |
| [#2822](https://github.com/OpenAgentsInc/openagents/issues/2822) | Primary CTA: One-Button Go Online/Go Offline | CLOSED |
| [#2823](https://github.com/OpenAgentsInc/openagents/issues/2823) | Mission Control Network Stats Panel | CLOSED |
| [#2824](https://github.com/OpenAgentsInc/openagents/issues/2824) | Mission Control Earnings Panel | CLOSED |
| [#2825](https://github.com/OpenAgentsInc/openagents/issues/2825) | Mission Control Recent Jobs Feed | CLOSED |
| [#2826](https://github.com/OpenAgentsInc/openagents/issues/2826) | Global Network Earnings Header | CLOSED |
| [#2827](https://github.com/OpenAgentsInc/openagents/issues/2827) | First-Run Ready-To-Earn Flow | CLOSED |
| [#2828](https://github.com/OpenAgentsInc/openagents/issues/2828) | No-Jobs Waiting UX | CLOSED |
| [#2829](https://github.com/OpenAgentsInc/openagents/issues/2829) | Provider Runtime Promotion: Simulated to Relay-Backed | CLOSED |
| [#2830](https://github.com/OpenAgentsInc/openagents/issues/2830) | NIP-90 Request Subscription Layer | CLOSED |
| [#2831](https://github.com/OpenAgentsInc/openagents/issues/2831) | Job Admission and Capability Matching | CLOSED |
| [#2832](https://github.com/OpenAgentsInc/openagents/issues/2832) | Feedback Event Pipeline (kind 7000) | CLOSED |
| [#2833](https://github.com/OpenAgentsInc/openagents/issues/2833) | Deterministic Job Executors (Hash/JSON/Benchmark) | CLOSED |
| [#2834](https://github.com/OpenAgentsInc/openagents/issues/2834) | Minimal Inference Executor (kind 5050) | CLOSED |
| [#2835](https://github.com/OpenAgentsInc/openagents/issues/2835) | Result Publishing Pipeline (6000-6999) | CLOSED |
| [#2836](https://github.com/OpenAgentsInc/openagents/issues/2836) | Job Correlation Model (request-feedback-result-payment) | CLOSED |
| [#2837](https://github.com/OpenAgentsInc/openagents/issues/2837) | Active Job Pane Authority Wiring | CLOSED |
| [#2838](https://github.com/OpenAgentsInc/openagents/issues/2838) | Job History Receipt Authority Wiring | CLOSED |
| [#2839](https://github.com/OpenAgentsInc/openagents/issues/2839) | Remove Synthetic Success Paths for Earnings | CLOSED |
| [#2840](https://github.com/OpenAgentsInc/openagents/issues/2840) | Per-Job Invoice Contract | CLOSED |
| [#2841](https://github.com/OpenAgentsInc/openagents/issues/2841) | Buyer Invoice Payment Worker | CLOSED |
| [#2842](https://github.com/OpenAgentsInc/openagents/issues/2842) | Wallet Receive Confirmation Ingestion | CLOSED |
| [#2843](https://github.com/OpenAgentsInc/openagents/issues/2843) | Wallet-Job Reconciliation Projection | CLOSED |
| [#2844](https://github.com/OpenAgentsInc/openagents/issues/2844) | Authoritative Payout Gate Enforcement | CLOSED |
| [#2845](https://github.com/OpenAgentsInc/openagents/issues/2845) | Withdrawal Flow Hardening | CLOSED |
| [#2846](https://github.com/OpenAgentsInc/openagents/issues/2846) | Payment Failure Lifecycle | CLOSED |
| [#2847](https://github.com/OpenAgentsInc/openagents/issues/2847) | Synthetic Payment Pointer Hard Gate | CLOSED |
| [#2848](https://github.com/OpenAgentsInc/openagents/issues/2848) | Seed Demand Buyer Service (MVP) | CLOSED |
| [#2849](https://github.com/OpenAgentsInc/openagents/issues/2849) | Seed Job Templates and Pricing Matrix | CLOSED |
| [#2850](https://github.com/OpenAgentsInc/openagents/issues/2850) | Seed Pool Budget and Kill Switch Controls | CLOSED |
| [#2851](https://github.com/OpenAgentsInc/openagents/issues/2851) | Starter vs Open-Network Labeling | CLOSED |
| [#2852](https://github.com/OpenAgentsInc/openagents/issues/2852) | First-Earnings SLA Monitor | CLOSED |
| [#2853](https://github.com/OpenAgentsInc/openagents/issues/2853) | Seed Demand Reliability Backpressure | CLOSED |
| [#2854](https://github.com/OpenAgentsInc/openagents/issues/2854) | Canonical Earn Metrics Schema | CLOSED |
| [#2855](https://github.com/OpenAgentsInc/openagents/issues/2855) | Metrics Emission from Desktop and Buyer | CLOSED |
| [#2856](https://github.com/OpenAgentsInc/openagents/issues/2856) | Stats Aggregation Service Contract | CLOSED |
| [#2857](https://github.com/OpenAgentsInc/openagents/issues/2857) | openagents.com/stats Implementation | CLOSED |
| [#2858](https://github.com/OpenAgentsInc/openagents/issues/2858) | In-App Network Stats Hydration | CLOSED |
| [#2859](https://github.com/OpenAgentsInc/openagents/issues/2859) | Global Earnings Today Computation | CLOSED |
| [#2860](https://github.com/OpenAgentsInc/openagents/issues/2860) | Metric Integrity Checks | CLOSED |
| [#2861](https://github.com/OpenAgentsInc/openagents/issues/2861) | Replay-Safe Apply for Job/Payment Events | CLOSED |
| [#2862](https://github.com/OpenAgentsInc/openagents/issues/2862) | Duplicate Suppression Keys | CLOSED |
| [#2863](https://github.com/OpenAgentsInc/openagents/issues/2863) | Stale Cursor Rebootstrap for Earn Lanes | CLOSED |
| [#2864](https://github.com/OpenAgentsInc/openagents/issues/2864) | Crash Recovery for In-Flight Jobs | CLOSED |
| [#2865](https://github.com/OpenAgentsInc/openagents/issues/2865) | Relay Outage Degraded Mode UX | CLOSED |
| [#2866](https://github.com/OpenAgentsInc/openagents/issues/2866) | Payout Mismatch Incident Runbook | CLOSED |
| [#2867](https://github.com/OpenAgentsInc/openagents/issues/2867) | Rollout Flags and Cohort Controls for Earn | CLOSED |
| [#2868](https://github.com/OpenAgentsInc/openagents/issues/2868) | NIP-90 Builder/Parser Unit Coverage | CLOSED |
| [#2869](https://github.com/OpenAgentsInc/openagents/issues/2869) | Integration Test: Request to Result to Payment | CLOSED |
| [#2870](https://github.com/OpenAgentsInc/openagents/issues/2870) | Desktop Mission Control State Tests | CLOSED |
| [#2871](https://github.com/OpenAgentsInc/openagents/issues/2871) | E2E Test: First Sats Moment | CLOSED |
| [#2872](https://github.com/OpenAgentsInc/openagents/issues/2872) | Chaos Test: Relay Loss + Recovery | CLOSED |
| [#2873](https://github.com/OpenAgentsInc/openagents/issues/2873) | Chaos Test: Wallet Error + Recovery | CLOSED |
| [#2874](https://github.com/OpenAgentsInc/openagents/issues/2874) | Stress Harness: 3-Second Job Cadence | CLOSED |
| [#2875](https://github.com/OpenAgentsInc/openagents/issues/2875) | Earn MVP Merge Gate Script | CLOSED |
| [#2876](https://github.com/OpenAgentsInc/openagents/issues/2876) | Launch Rehearsal and Production Signoff | CLOSED |

## Closeout Rules

- Close issues only after code/docs/tests for that issue are merged to `main`.
- Link the closing commit(s) and a short verification summary in the issue comment.
- If scope changes, update both the issue and this tracker in the same PR/commit.
