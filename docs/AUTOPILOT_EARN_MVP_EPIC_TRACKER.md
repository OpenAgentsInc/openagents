# Autopilot Earn MVP Epic Tracker

Date: 2026-03-04
Epic issue: [#2814](https://github.com/OpenAgentsInc/openagents/issues/2814)
Spec source: `docs/AUTOPILOT_EARN_MVP.md`

## Purpose

Track the full Autopilot Earn MVP execution program from issue creation through implementation closeout.

## Acceptance Gates

The epic is complete only when all are true:

1. Mission Control first-run loop is implemented and default.
2. Provider runtime is relay-backed for NIP-90 request/result/feedback flow.
3. Wallet-confirmed payout gate is enforced in all earnings surfaces.
4. Seed-demand buyer lane is operational with spend controls.
5. Public stats lane is live and consistent with wallet-confirmed payouts.
6. Reliability/test/rollout gates are green.

## Issue List

| # | Title | State |
| --- | --- | --- |
| [#2814](https://github.com/OpenAgentsInc/openagents/issues/2814) | [Epic] Autopilot Earn MVP Full Implementation Program | OPEN |
| [#2815](https://github.com/OpenAgentsInc/openagents/issues/2815) | Backroom Harvest Audit: NIP-90/Provider/Wallet Assets | OPEN |
| [#2816](https://github.com/OpenAgentsInc/openagents/issues/2816) | Restore Candidate Port: InProcessPylon Provider Domain | OPEN |
| [#2817](https://github.com/OpenAgentsInc/openagents/issues/2817) | Restore Candidate Port: Spark Wallet Domain Bridge | OPEN |
| [#2818](https://github.com/OpenAgentsInc/openagents/issues/2818) | Restore Candidate Port: NIP-90 Submit + Await Result Helper | OPEN |
| [#2819](https://github.com/OpenAgentsInc/openagents/issues/2819) | Retained Runtime Placement Decision (App vs New Crate) | OPEN |
| [#2820](https://github.com/OpenAgentsInc/openagents/issues/2820) | Backroom Provenance and Migration Notes | OPEN |
| [#2821](https://github.com/OpenAgentsInc/openagents/issues/2821) | Mission Control Single-Screen Shell | OPEN |
| [#2822](https://github.com/OpenAgentsInc/openagents/issues/2822) | Primary CTA: One-Button Go Online/Go Offline | OPEN |
| [#2823](https://github.com/OpenAgentsInc/openagents/issues/2823) | Mission Control Network Stats Panel | OPEN |
| [#2824](https://github.com/OpenAgentsInc/openagents/issues/2824) | Mission Control Earnings Panel | OPEN |
| [#2825](https://github.com/OpenAgentsInc/openagents/issues/2825) | Mission Control Recent Jobs Feed | OPEN |
| [#2826](https://github.com/OpenAgentsInc/openagents/issues/2826) | Global Network Earnings Header | OPEN |
| [#2827](https://github.com/OpenAgentsInc/openagents/issues/2827) | First-Run Ready-To-Earn Flow | OPEN |
| [#2828](https://github.com/OpenAgentsInc/openagents/issues/2828) | No-Jobs Waiting UX | OPEN |
| [#2829](https://github.com/OpenAgentsInc/openagents/issues/2829) | Provider Runtime Promotion: Simulated to Relay-Backed | OPEN |
| [#2830](https://github.com/OpenAgentsInc/openagents/issues/2830) | NIP-90 Request Subscription Layer | OPEN |
| [#2831](https://github.com/OpenAgentsInc/openagents/issues/2831) | Job Admission and Capability Matching | OPEN |
| [#2832](https://github.com/OpenAgentsInc/openagents/issues/2832) | Feedback Event Pipeline (kind 7000) | OPEN |
| [#2833](https://github.com/OpenAgentsInc/openagents/issues/2833) | Deterministic Job Executors (Hash/JSON/Benchmark) | OPEN |
| [#2834](https://github.com/OpenAgentsInc/openagents/issues/2834) | Minimal Inference Executor (kind 5050) | OPEN |
| [#2835](https://github.com/OpenAgentsInc/openagents/issues/2835) | Result Publishing Pipeline (6000-6999) | OPEN |
| [#2836](https://github.com/OpenAgentsInc/openagents/issues/2836) | Job Correlation Model (request-feedback-result-payment) | OPEN |
| [#2837](https://github.com/OpenAgentsInc/openagents/issues/2837) | Active Job Pane Authority Wiring | OPEN |
| [#2838](https://github.com/OpenAgentsInc/openagents/issues/2838) | Job History Receipt Authority Wiring | OPEN |
| [#2839](https://github.com/OpenAgentsInc/openagents/issues/2839) | Remove Synthetic Success Paths for Earnings | OPEN |
| [#2840](https://github.com/OpenAgentsInc/openagents/issues/2840) | Per-Job Invoice Contract | OPEN |
| [#2841](https://github.com/OpenAgentsInc/openagents/issues/2841) | Buyer Invoice Payment Worker | OPEN |
| [#2842](https://github.com/OpenAgentsInc/openagents/issues/2842) | Wallet Receive Confirmation Ingestion | OPEN |
| [#2843](https://github.com/OpenAgentsInc/openagents/issues/2843) | Wallet-Job Reconciliation Projection | OPEN |
| [#2844](https://github.com/OpenAgentsInc/openagents/issues/2844) | Authoritative Payout Gate Enforcement | OPEN |
| [#2845](https://github.com/OpenAgentsInc/openagents/issues/2845) | Withdrawal Flow Hardening | OPEN |
| [#2846](https://github.com/OpenAgentsInc/openagents/issues/2846) | Payment Failure Lifecycle | OPEN |
| [#2847](https://github.com/OpenAgentsInc/openagents/issues/2847) | Synthetic Payment Pointer Hard Gate | OPEN |
| [#2848](https://github.com/OpenAgentsInc/openagents/issues/2848) | Seed Demand Buyer Service (MVP) | OPEN |
| [#2849](https://github.com/OpenAgentsInc/openagents/issues/2849) | Seed Job Templates and Pricing Matrix | OPEN |
| [#2850](https://github.com/OpenAgentsInc/openagents/issues/2850) | Seed Pool Budget and Kill Switch Controls | OPEN |
| [#2851](https://github.com/OpenAgentsInc/openagents/issues/2851) | Starter vs Open-Network Labeling | OPEN |
| [#2852](https://github.com/OpenAgentsInc/openagents/issues/2852) | First-Earnings SLA Monitor | OPEN |
| [#2853](https://github.com/OpenAgentsInc/openagents/issues/2853) | Seed Demand Reliability Backpressure | OPEN |
| [#2854](https://github.com/OpenAgentsInc/openagents/issues/2854) | Canonical Earn Metrics Schema | OPEN |
| [#2855](https://github.com/OpenAgentsInc/openagents/issues/2855) | Metrics Emission from Desktop and Buyer | OPEN |
| [#2856](https://github.com/OpenAgentsInc/openagents/issues/2856) | Stats Aggregation Service Contract | OPEN |
| [#2857](https://github.com/OpenAgentsInc/openagents/issues/2857) | openagents.com/stats Implementation | OPEN |
| [#2858](https://github.com/OpenAgentsInc/openagents/issues/2858) | In-App Network Stats Hydration | OPEN |
| [#2859](https://github.com/OpenAgentsInc/openagents/issues/2859) | Global Earnings Today Computation | OPEN |
| [#2860](https://github.com/OpenAgentsInc/openagents/issues/2860) | Metric Integrity Checks | OPEN |
| [#2861](https://github.com/OpenAgentsInc/openagents/issues/2861) | Replay-Safe Apply for Job/Payment Events | OPEN |
| [#2862](https://github.com/OpenAgentsInc/openagents/issues/2862) | Duplicate Suppression Keys | OPEN |
| [#2863](https://github.com/OpenAgentsInc/openagents/issues/2863) | Stale Cursor Rebootstrap for Earn Lanes | OPEN |
| [#2864](https://github.com/OpenAgentsInc/openagents/issues/2864) | Crash Recovery for In-Flight Jobs | OPEN |
| [#2865](https://github.com/OpenAgentsInc/openagents/issues/2865) | Relay Outage Degraded Mode UX | OPEN |
| [#2866](https://github.com/OpenAgentsInc/openagents/issues/2866) | Payout Mismatch Incident Runbook | OPEN |
| [#2867](https://github.com/OpenAgentsInc/openagents/issues/2867) | Rollout Flags and Cohort Controls for Earn | OPEN |
| [#2868](https://github.com/OpenAgentsInc/openagents/issues/2868) | NIP-90 Builder/Parser Unit Coverage | OPEN |
| [#2869](https://github.com/OpenAgentsInc/openagents/issues/2869) | Integration Test: Request to Result to Payment | OPEN |
| [#2870](https://github.com/OpenAgentsInc/openagents/issues/2870) | Desktop Mission Control State Tests | OPEN |
| [#2871](https://github.com/OpenAgentsInc/openagents/issues/2871) | E2E Test: First Sats Moment | OPEN |
| [#2872](https://github.com/OpenAgentsInc/openagents/issues/2872) | Chaos Test: Relay Loss + Recovery | OPEN |
| [#2873](https://github.com/OpenAgentsInc/openagents/issues/2873) | Chaos Test: Wallet Error + Recovery | OPEN |
| [#2874](https://github.com/OpenAgentsInc/openagents/issues/2874) | Stress Harness: 3-Second Job Cadence | OPEN |
| [#2875](https://github.com/OpenAgentsInc/openagents/issues/2875) | Earn MVP Merge Gate Script | OPEN |
| [#2876](https://github.com/OpenAgentsInc/openagents/issues/2876) | Launch Rehearsal and Production Signoff | OPEN |

## Closeout Rules

- Close issues only after code/docs/tests for that issue are merged to `main`.
- Link the closing commit(s) and a short verification summary in the issue comment.
- If scope changes, update both the issue and this tracker in the same PR/commit.
