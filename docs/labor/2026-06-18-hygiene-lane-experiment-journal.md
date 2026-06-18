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

## 2026-06-18 - Receipt Identity Should Be Digest-Bound

Evidence:

- #5372 added the hygiene-lane settlement dispatch route.
- #5388 hardens the route so `idempotencyRef` is SHA-256 hashed before ledger
  receipt refs and idempotency hashes are derived.

Lesson:

Readable refs are useful, but truncation should not be the collision boundary
for settlement-adjacent records. The readable part can be public and stable; the
identity boundary should come from a digest.

Operating rule:

For receipt-first money paths, derive idempotency keys and settlement refs from
hashed request material, then expose only a public-safe digest suffix.

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

## 2026-06-18 - Verification Must Dereference Before Settlement

Evidence:

- Orrery flagged on #5372 that contributor-asserted local test runs are not a
  receipt a settlement authority can independently resolve.
- Raynor and AtlantisPleb caught that GitHub-hosted Actions violate the
  repository's no-GitHub-hosted-CI invariant.
- #5391 now documents that test/typecheck-based hygiene evidence must
  dereference to an OpenAgents-owned runner check-run or an independent
  verifier replay, not a GitHub-hosted Actions run.

Lesson:

The payout gate should not depend on a worker saying "tests passed." Local runs
are useful while developing, but real settlement needs a public verification
artifact that another actor can inspect without trusting the worker.

Operating rule:

Before treating tests or typechecks as payout-grade hygiene evidence, require a
green OpenAgents-owned runner check-run or an independent verifier receipt.
Local worker logs and GitHub-hosted Actions runs are useful coordination
signals, not settlement evidence.

## 2026-06-18 - Settlement Means A Public Receipt, Not A Merge

Evidence:

- Raynor settled the first real hygiene-lane Bitcoin payment for #5358.
- The public receipt
  `receipt.nexus.hygiene_lane_settlement.sha256_c81865d82fd5d3ac33757e7935e5ed8fd895ed13ba8deff2c6e34c60d7b6d7a3`
  dereferences through `/api/public/nexus-pylon/receipts/<ref>`.
- The public projection reports `amountSats: 75`, `movementMode:
  real_bitcoin`, `realBitcoinMoved: true`, `verificationBasis:
  hygiene_merged_reviewed`, `state: settled`, and
  `mergedPrRef: pr.public.github.openagentsinc_openagents.5358`.
- Raynor reported that same-idempotency retry and new-idempotency/same-key
  replay both returned `409 duplicate_replay`.

Lesson:

The lane's money loop is now proven end to end: a funded debt receipt can move
from verified merged work to an honest public settlement receipt with real
Bitcoin movement, while the retire-once key prevents double-pay. This worked
because merge, review, payable classification, and settlement stayed distinct.

Operating rule:

Do not call hygiene work paid until a public settlement receipt exists and
reports real movement. After settlement, the `DebtReceiptKey` is retired; any
same-key replay is duplicate work, not a second payable claim.
