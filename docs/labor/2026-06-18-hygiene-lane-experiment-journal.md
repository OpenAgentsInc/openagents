# Hygiene Lane Experiment Journal

Date started: 2026-06-18

This journal records public-safe lessons from the #5335 hygiene/refactoring lane
experiment. It is intentionally not a heartbeat log. Add entries only when an
experiment changes how contributors, reviewers, or settlement authorities should
operate.

## 2026-06-18 - Funded Work Needs State Labels

Evidence:

- #5335 adopted the debt-receipt process.
- #5372 owner-authorized and funded the hygiene-lane settlement path.
- #5373 codified the state ladder in docs and invariants.

Lesson:

Merge, review, and green tests are accepted-work evidence, not payment evidence.
The lane needs explicit state names so contributors do not treat a merged PR as
paid work.

Operating rule:

Use `discovery`, `accepted_verified`, `credit_class`, `payable_class`,
`payable_pending_settlement`, and `settled`. Only `settled` means paid.

## 2026-06-18 - A Payout Formula Should Be A Receipt

Evidence:

- #5369 requested `churn_tax.v0.backtest`.
- #5385 implements a replayable fixture set, payout multiplier, denial reason,
  and public-safe conflict override refs.

Lesson:

A narrative size/depth heuristic is too easy to tune after the fact. A formula
that pays hygiene work should be deterministic, replayable, and small enough for
reviewers to inspect.

Operating rule:

For funded hygiene settlement, publish the formula ref, multiplier, denial
reason, and replay cases. Churn without measured debt reduction pays zero.

## 2026-06-18 - The Monitoring Loop Should Be Quiet By Default

Evidence:

- The thread automation originally woke every 10 minutes and was useful for
  polling, but too noisy for routine updates.
- The automation now polls GitHub and the forum quietly and reports only when a
  new reply, blocker, receipt, PR, or process change materially matters.

Lesson:

Agent persistence is valuable when it preserves attention. Reporting cadence
should be based on signal, not on the timer.

Operating rule:

Poll regularly, post publicly when it helps coordination, and notify Trigger
only for material changes or approval needs.

## 2026-06-18 - Local Secrets Need Two Layers Of Protection

Evidence:

- The workspace-level Git status showed local Forum/API credential files and
  scratch worktrees.
- Local excludes now hide parent-workspace credential and scratch noise.
- #5386 adds repo-level ignore patterns for `.openagents-*-agent.json` and
  `.openagents-agent.json`.

Lesson:

Private operational files should be protected both where the agent actually
runs and in the repository, because credentials are easy to copy into the wrong
directory during live work.

Operating rule:

Keep credentials in local-only files, never print or commit them, and add narrow
ignore patterns for known OpenAgents credential filenames.
