# 2026-03-02 Epic Tracker: Autopilot Goal Automation

Related issue: [#2708](https://github.com/OpenAgentsInc/openagents/issues/2708)

## Scope

This epic tracks implementation of autonomous goal execution for "earn bitcoin on autopilot", including:

- chat-directed goal definition,
- autonomous iterative execution until condition,
- authoritative payout confirmation,
- BTC <-> stablesat USD swap support,
- scheduler support and recovery,
- safety policy for no-approval operation,
- telemetry, testing, and rollout.

Primary product/spec authorities:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-02-autopilot-earn-bitcoin-autonomy-flow-audit.md`

## MVP Acceptance Bar

Epic acceptance requires all of the following:

1. User can direct Autopilot to earn bitcoin until a wallet-based condition is met.
2. Relevant skills are auto-selected and attached for autonomous runs.
3. Autonomous loop runs until completion/timeout/policy stop without manual turn submission.
4. Success gates rely on authoritative wallet/payment evidence.
5. Scheduling supports manual/interval (and cron via follow-up issues).
6. BTC <-> stablesat USD swap flow is first-class with receipts and policy limits.

## Issue Sequence

1. [#2709](https://github.com/OpenAgentsInc/openagents/issues/2709) Define Goal Spec and Persistence Model in `apps/autopilot-desktop`
2. [#2710](https://github.com/OpenAgentsInc/openagents/issues/2710) Add Goal Runner State Machine (Queued/Running/Succeeded/Failed/Aborted)
3. [#2711](https://github.com/OpenAgentsInc/openagents/issues/2711) Implement Condition Evaluator for Wallet Delta, Job Count, Timeout, and Error Budget
4. [#2712](https://github.com/OpenAgentsInc/openagents/issues/2712) Add Authoritative Earnings Verification Gate (Wallet/Payment Evidence Only)
5. [#2713](https://github.com/OpenAgentsInc/openagents/issues/2713) Define BTC <-> Stablesat USD Swap Contract + Policy Model
6. [#2714](https://github.com/OpenAgentsInc/openagents/issues/2714) Implement Blink Skill First-Class Swap Scripts (Quote + Execute, Both Directions)
7. [#2715](https://github.com/OpenAgentsInc/openagents/issues/2715) Add Stablesats Quote Adapter Path (`GetQuoteToBuyUsd` / `GetQuoteToSellUsd` / `AcceptQuote`)
8. [#2716](https://github.com/OpenAgentsInc/openagents/issues/2716) Wire Swap Operations Into Autopilot Tool Bridge and Goal Runner
9. [#2717](https://github.com/OpenAgentsInc/openagents/issues/2717) Implement Skill Relevance Resolver for Earnings Goals
10. [#2718](https://github.com/OpenAgentsInc/openagents/issues/2718) Wire Auto-Attached Skill Sets Into Goal-Driven Chat Turn Submission
11. [#2719](https://github.com/OpenAgentsInc/openagents/issues/2719) Build Goal Loop Executor (Turn -> Observe -> Replan -> Retry)
12. [#2720](https://github.com/OpenAgentsInc/openagents/issues/2720) Add Goal Controls in UI (Create, Start/Stop, Status, Receipts)
13. [#2721](https://github.com/OpenAgentsInc/openagents/issues/2721) Implement Manual + Interval Scheduler for Goal Runs
14. [#2722](https://github.com/OpenAgentsInc/openagents/issues/2722) Add Cron Expression Scheduler Runtime Support
15. [#2723](https://github.com/OpenAgentsInc/openagents/issues/2723) Add Optional OS-Backed Scheduling Adapters (`launchd`/`cron`/`systemd`)
16. [#2724](https://github.com/OpenAgentsInc/openagents/issues/2724) Implement Restart Recovery + Missed-Run Semantics
17. [#2725](https://github.com/OpenAgentsInc/openagents/issues/2725) Extend Tooling Surface for Safe Automation Primitives
18. [#2726](https://github.com/OpenAgentsInc/openagents/issues/2726) Add Autonomous Mode Safety Policy (Permissions, Budgets, Kill Switch, Swap Limits)
19. [#2727](https://github.com/OpenAgentsInc/openagents/issues/2727) Replace Simulated Earn Path With Authoritative Paid Job Pipeline
20. [#2728](https://github.com/OpenAgentsInc/openagents/issues/2728) Add Wallet Reconciliation Layer for Earn Events and Swap Events
21. [#2729](https://github.com/OpenAgentsInc/openagents/issues/2729) Add End-to-End Telemetry and Audit Receipts for Each Goal Run
22. [#2730](https://github.com/OpenAgentsInc/openagents/issues/2730) Add Deterministic Test Matrix (Unit + Integration + E2E, Including Swap Roundtrips)
23. [#2731](https://github.com/OpenAgentsInc/openagents/issues/2731) Ship Docs + Operator Runbook for Autopilot Earnings Automation
24. [#2732](https://github.com/OpenAgentsInc/openagents/issues/2732) Rollout Issue: Feature Flag, Staged Enablement, and Success Metrics
