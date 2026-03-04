# Autopilot Earn MVP Implementation Log

Date: 2026-03-04
Issues: historical stream `#2815 - #2876`, reconciliation stream `#2877 - #2890`

This log records implementation evidence for the historical Autopilot Earn MVP issue stream and the follow-on reconciliation stream opened by the 2026-03-04 audit.

Scope note: this log is compute-lane specific (NIP-90 provider earnings). It does not include future liquidity-solver implementation work.

Current-status note: this file is an evidence ledger, not a completion assertion. Canonical live status is tracked in:
- `docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md`
- `docs/audits/2026-03-04-autopilot-earn-integration-audit-gap-analysis.md`

## Reconciliation Addendum (2026-03-04)

Follow-on issues created from audit gap analysis:

| # | State | Scope |
| --- | --- | --- |
| [#2877](https://github.com/OpenAgentsInc/openagents/issues/2877) | CLOSED | Relay-backed provider ingress wiring |
| [#2878](https://github.com/OpenAgentsInc/openagents/issues/2878) | CLOSED | External authority mapping for lifecycle stages |
| [#2879](https://github.com/OpenAgentsInc/openagents/issues/2879) | CLOSED | Desktop publish path for feedback/result events |
| [#2880](https://github.com/OpenAgentsInc/openagents/issues/2880) | CLOSED | Wallet-authoritative mission-control totals |
| [#2881](https://github.com/OpenAgentsInc/openagents/issues/2881) | CLOSED | Desktop relay->execute->publish->wallet harness |
| [#2882](https://github.com/OpenAgentsInc/openagents/issues/2882) | CLOSED | Starter-demand controls and kill switch |
| [#2883](https://github.com/OpenAgentsInc/openagents/issues/2883) | CLOSED | Starter provenance tagging into receipts/history |
| [#2884](https://github.com/OpenAgentsInc/openagents/issues/2884) | CLOSED | Aggregate network stats service pipeline |
| [#2885](https://github.com/OpenAgentsInc/openagents/issues/2885) | CLOSED | Relay connectivity truth model |
| [#2886](https://github.com/OpenAgentsInc/openagents/issues/2886) | CLOSED | Tracker/log claim reconciliation |
| [#2887](https://github.com/OpenAgentsInc/openagents/issues/2887) | CLOSED | Failure taxonomy + diagnostics |
| [#2888](https://github.com/OpenAgentsInc/openagents/issues/2888) | OPEN | Loop integrity SLO metrics/alerts |
| [#2889](https://github.com/OpenAgentsInc/openagents/issues/2889) | OPEN | Simulation path isolation |
| [#2890](https://github.com/OpenAgentsInc/openagents/issues/2890) | OPEN | Docs consolidation for canonical status |

## Program + Backroom

- #2815 Backroom harvest audit: `docs/AUTOPILOT_EARN_BACKROOM_HARVEST_AUDIT.md`
- #2816 Provider domain port patterns: `apps/autopilot-desktop/src/state/provider_runtime.rs`, `apps/autopilot-desktop/src/render.rs`
- #2817 Wallet bridge port patterns: `apps/autopilot-desktop/src/spark_wallet.rs`, `apps/autopilot-desktop/src/state/earnings_gate.rs`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`
- #2818 NIP-90 submit+await helper: `crates/nostr/client/src/dvm.rs`, `crates/nostr/client/src/pool.rs`
- #2819 Runtime placement decision: `docs/AUTOPILOT_EARN_RUNTIME_PLACEMENT_DECISION.md`
- #2820 Provenance notes: `docs/AUTOPILOT_EARN_BACKROOM_PROVENANCE.md`

## Mission Control Surface

- #2821 Single-screen shell: `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/app_state.rs`
- #2822 Primary one-button CTA: `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/input.rs`
- #2823 Network stats panel: `apps/autopilot-desktop/src/render.rs`
- #2824 Earnings panel: `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/app_state.rs`
- #2825 Recent jobs feed: `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/app_state.rs`
- #2826 Global earnings header: `apps/autopilot-desktop/src/render.rs`
- #2827 First-run ready-to-earn flow: `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/render.rs`
- #2828 No-jobs waiting UX: `apps/autopilot-desktop/src/render.rs`

## Provider Runtime + NIP-90 Execution

- #2829 Provider runtime state authority: `apps/autopilot-desktop/src/state/provider_runtime.rs`, `apps/autopilot-desktop/src/runtime_lanes.rs`
- #2830 Request subscription layer: `crates/nostr/client/src/dvm.rs`, `crates/nostr/client/src/relay.rs`
- #2831 Admission/capability matching: `apps/autopilot-desktop/src/state/job_inbox.rs`
- #2832 Feedback event pipeline (kind 7000): `crates/nostr/core/src/nip90/model.rs`, `crates/nostr/core/src/nip90/builders.rs`
- #2833 Deterministic executors (hash/json/benchmark): `apps/autopilot-desktop/src/state/operations.rs`, `apps/autopilot-desktop/src/app_state.rs`
- #2834 Minimal inference executor (5050): `crates/nostr/core/src/nip90/kinds.rs`, `crates/nostr/client/src/dvm.rs`
- #2835 Result publishing pipeline (6000-6999): `crates/nostr/client/src/dvm.rs`, `crates/nostr/core/src/nip90/kinds.rs`
- #2836 Correlation model: `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`
- #2837 Active job authority wiring: `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/pane_renderer.rs`
- #2838 Job history authority wiring: `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/state/earnings_gate.rs`
- #2839 Remove synthetic success paths: `apps/autopilot-desktop/src/state/earnings_gate.rs`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`

## Lightning Settlement + Wallet Authority

- #2840 Per-job invoice contract: `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/spark_wallet.rs`
- #2841 Buyer invoice payment worker: `apps/autopilot-desktop/src/spark_wallet.rs`
- #2842 Wallet receive confirmation ingestion: `apps/autopilot-desktop/src/spark_wallet.rs`, `apps/autopilot-desktop/src/state/earnings_gate.rs`
- #2843 Wallet-job reconciliation projection: `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`
- #2844 Authoritative payout gate: `apps/autopilot-desktop/src/state/earnings_gate.rs`
- #2845 Withdrawal hardening: `apps/autopilot-desktop/src/spark_wallet.rs`, `apps/autopilot-desktop/src/pane_renderer.rs`
- #2846 Payment failure lifecycle: `apps/autopilot-desktop/src/spark_wallet.rs`, `apps/autopilot-desktop/src/state/operations.rs`
- #2847 Synthetic pointer hard gate: `apps/autopilot-desktop/src/state/earnings_gate.rs`, `apps/autopilot-desktop/src/state/operations.rs`

## Seed Demand + Buyer Lane

- #2848 Seed demand buyer service: `apps/autopilot-desktop/src/state/operations.rs`, `apps/autopilot-desktop/src/pane_renderer.rs`
- #2849 Seed job templates/pricing: `apps/autopilot-desktop/src/state/operations.rs`, `docs/AUTOPILOT_EARN_MVP.md`
- #2850 Seed pool budget/kill switch: `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`, `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
- #2851 Starter vs open-network labeling: `apps/autopilot-desktop/src/pane_renderer.rs`, `docs/MVP.md`
- #2852 First-earnings SLA monitor: `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`, `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
- #2853 Seed demand backpressure: `docs/AUTOPILOT_EARNINGS_AUTOMATION.md`, `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`

## Metrics + Public Beacon

- #2854 Canonical metrics schema: `docs/MVP.md`, `docs/PANES.md`, `apps/autopilot-desktop/src/app_state.rs`
- #2855 Metrics emission desktop+buyer: `apps/autopilot-desktop/src/app_state.rs`, `apps/autopilot-desktop/src/state/operations.rs`
- #2856 Stats aggregation contract: `docs/AUTOPILOT_EARN_MVP.md`, `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`
- #2857 openagents.com/stats implementation contract: `docs/AUTOPILOT_EARN_MVP.md`
- #2858 In-app stats hydration: `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/app_state.rs`
- #2859 Global earnings today computation: `apps/autopilot-desktop/src/render.rs`, `apps/autopilot-desktop/src/state/earnings_gate.rs`
- #2860 Metric integrity checks: `apps/autopilot-desktop/src/state/earnings_gate.rs`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`

## Reliability + Operations

- #2861 Replay-safe apply: `apps/autopilot-desktop/src/state/operations.rs`, `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`
- #2862 Duplicate suppression keys: `apps/autopilot-desktop/src/state/operations.rs`, `docs/NIP_SA_SKL_AC_TEST_MATRIX_RUNBOOK.md`
- #2863 Stale cursor rebootstrap: `apps/autopilot-desktop/src/state/operations.rs`
- #2864 Crash recovery in-flight jobs: `apps/autopilot-desktop/src/state/autopilot_goals.rs`, `docs/AUTOPILOT_EARNINGS_AUTOMATION.md`
- #2865 Relay outage degraded UX: `apps/autopilot-desktop/src/state/provider_runtime.rs`, `apps/autopilot-desktop/src/render.rs`
- #2866 Payout mismatch runbook: `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
- #2867 Rollout flags/cohorts: `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`

## Tests + Launch Gates

- #2868 NIP-90 builder/parser unit coverage: `crates/nostr/core/src/nip90/tests.rs`, `crates/nostr/core/tests/nip90_integration.rs`
- #2869 Integration request->result->payment: `apps/autopilot-desktop/src/state/earnings_gate.rs` tests + `apps/autopilot-desktop/src/state/wallet_reconciliation.rs` tests
- #2870 Mission Control state tests: `apps/autopilot-desktop/src/app_state.rs` tests
- #2871 E2E first sats moment: `docs/AUTOPILOT_EARN_MVP.md` acceptance + app lane state tests
- #2872 Chaos relay loss/recovery: `apps/autopilot-desktop/src/state/operations.rs` + `docs/NIP_SA_SKL_AC_TEST_MATRIX_RUNBOOK.md`
- #2873 Chaos wallet error/recovery: `apps/autopilot-desktop/src/spark_wallet.rs` + `apps/autopilot-desktop/src/state/earnings_gate.rs`
- #2874 Stress harness 3s cadence: `docs/AUTOPILOT_EARN_MVP.md` cadence contract + starter job lane controls
- #2875 Merge gate script: `scripts/lint/clippy-regression-check.sh`, `scripts/lint/ownership-boundary-check.sh`
- #2876 Launch rehearsal + signoff: `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`, `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`
