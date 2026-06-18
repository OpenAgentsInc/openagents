# Debt Receipt Hygiene Lane

Date: 2026-06-18

Issue #5335 should be treated as a product lane, not a broad invitation to
refactor. The sellable unit is a **debt receipt**: one externally named,
budget-capped, benchmark-verified piece of codebase debt that can be retired
once and paid once.

## Money Thesis

Coding agents make maintenance cheaper, but they also make churn cheap. The
business opportunity is not "more refactoring." It is a clearing layer for
measurable debt:

1. Find a debt item with a current baseline.
2. Convert it into a funded receipt with a cap, scope, target, verifier, and
   stop condition.
3. Let workers compete or claim it through the labor market.
4. Pay only after the verifier proves behavior stayed constant and the target
   metric improved.
5. Retire the receipt so the same cleanup cannot be sold twice.

This creates finite inventory out of hygiene work. That is what buyers can
understand and budget for.

## Product Offer

OpenAgents can sell this as **AI Churn Cleanup**:

- intake: churn probe or buyer report identifies the worst debt;
- receipt design: owner/allocator turns the finding into a funded receipt;
- execution: Pylon/local agents or contributors do the bounded work;
- verification: tests, benchmark replay, regenerate-and-diff, or perf checks;
- settlement: accepted work releases escrow and records a public-safe receipt;
- reporting: buyer receives before/after metrics and the retired receipt ref.

The first buyer can be OpenAgents itself. The first canonical receipt is #5334:
Tassadar generated fixtures committed in duplicate `.ts` and `.json` formats.

## Pricing Shape

Start with simple pricing before inventing a marketplace curve:

- audit/setup fee for a repo churn report;
- per-receipt budget cap for each accepted finding;
- platform take rate on settled receipt budgets;
- optional lane-lead share for curating receipts and reviewer packets;
- optional reviewer fee for acceptance and settlement authorization.

No public copy should promise passive revenue share until the split terms,
eligibility refs, payout policy, and settlement receipts exist. For #5335, the
lane lead should ask for pinned economics before accepting responsibility:
fixed receipt bounties now, and separate written terms for any standing lane
share.

## Debt Receipt Contract

Every payable receipt needs these public-safe refs:

- source: issue, churn-probe finding, or buyer request;
- baseline metric: current debt measurement;
- target metric: what must improve;
- scope: files, package, subsystem, or artifact class;
- budget cap: maximum payable sats or buyer credit;
- stop condition: when the receipt is retired;
- verifier: command or benchmark that proves behavior did not regress;
- accepted-work evidence: review or validator result;
- hygiene delta: measured improvement;
- no-new-debt check: no equal-or-worse debt introduced elsewhere;
- settlement approval: separate from the worker;
- settlement receipt: required before calling it paid or settled.

Workers may propose findings, but a separate allocator has to fund them.
Workers do not receive spend authority, deployment authority, settlement
authority, or authority to mint payable follow-ups.

The adapter in
`apps/openagents.com/workers/api/src/debt-receipt-work-request.ts` keeps this
contract hexagonal: debt-receipt economics are projected and checked first,
then a funded receipt is translated into the existing ref-only Forum
work-request contract. The Forum market still sees only objective, verifier,
budget, deadline, repo, and capability refs.

## First Receipt: #5334

Proposed public-safe receipt packet:

```text
debtReceiptRef: receipt.public.debt.5334.tassadar_fixture_dedup
sourceRef: issue.public.github.openagentsinc_openagents.5334
baselineMetricRef: metric.public.debt.5334.dual_format_generated_lines_460k
targetMetricRef: metric.public.debt.5334.committed_generated_churn_0
scopeRef: scope.public.debt.5334.tassadar_dense_fixture_pairs
verificationCommandRef: command.public.debt.5334.regenerate_and_diff
stopConditionRef: stop.public.debt.5334.retire_once_after_dedup
duplicateFingerprintRef: fingerprint.public.debt.5334.scope_patch_digest_v1
```

Acceptance should require:

- the `.json` fixture remains the canonical committed data if generation from
  Psionic is out of scope;
- any `.ts` twin derives from the canonical data instead of duplicating it;
- exported object digests remain identical;
- targeted Tassadar fixture tests pass;
- large generated-fixture churn no longer appears in routine commits;
- the receipt is retired after settlement.

If this checkout does not contain the generated fixture twins, the worker must
not fake the proof. The correct action is to leave #5334 open for the checkout
or branch that contains the artifacts, while landing the policy/tests that make
the receipt payable when the implementation diff exists.

## Loop Guards

- Discovery is inventory, not spend.
- Funding requires an allocator or owner route.
- Verification is not settlement.
- Settlement happens once, then the receipt is retired.
- Replays against a retired fingerprint are duplicate work, not payable work.
- After repeated rejected or revision-required attempts, the receipt/scope goes
  human-review-only until the benchmark, budget, or scope changes.

Regression coverage for these invariants lives in
`apps/openagents.com/workers/api/src/debt-receipt-policy.test.ts` and
`apps/openagents.com/workers/api/src/debt-receipt-work-request.test.ts`.
