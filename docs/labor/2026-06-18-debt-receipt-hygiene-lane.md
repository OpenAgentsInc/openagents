# Debt Receipt Hygiene Lane

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-18

Issue #5335 should be treated as a product lane, not a broad invitation to
refactor. The sellable unit is a **debt receipt**: one externally named,
budget-capped, benchmark-verified piece of codebase debt that can be retired
once and paid once.

Design and the first policy/test packet for this lane are credited to Trigger
(Codex Loop Guard), on branches `codex/debt-receipt-policy` and
`codex/study-hygiene-lane` (PRs #5343/#5344). This document and the landed code
incorporate that work and add the typed fingerprint-key model, the fail-closed
optional studied-knowledge gate, and the SA-3 studied-knowledge wiring (#5340).

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
- work class: code hygiene by default, or `documentation_or_journal` when the
  work is process notes, journals, docs, receipts, or discussion cleanup;
- budget cap: maximum payable sats or buyer credit;
- stop condition: when the receipt is retired;
- verifier: command or benchmark that proves behavior did not regress;
- accepted-work evidence: review or validator result;
- hygiene delta: measured improvement;
- no-new-debt check: no equal-or-worse debt introduced elsewhere;
- settlement approval: separate from the worker;
- settlement receipt: required before calling it paid or settled.

## Acceptance And Settlement States

Use distinct labels for the lane state. Collapsing them makes workers think a
merged cleanup is already money, and makes reviewers look like settlement
authorities.

- `discovery`: a worker, churn probe, buyer, or reviewer found debt. This is
  inventory, not spend.
- `accepted_verified`: a reviewer or verifier accepted the patch and evidence.
  A merged PR can be accepted-work evidence, but it is not payout authority.
  When the verifier is a test or typecheck command, the accepted-work evidence
  should dereference to an OpenAgents-owned runner check-run or independent
  verifier replay, not a worker-authored comment or GitHub-hosted Actions run.
- `credit_class`: accepted and credited work where no owner-funded hygiene-lane
  settlement path has been armed. This is recognition, not a pending Bitcoin
  payout.
  Documentation, journal, forum-process, and receipt-only hygiene work is
  `credit_class` by default, even when it carries good evidence. It should not be
  size/depth-scaled by the code hygiene payout formula. If the owner wants paid
  docs/process work, create a separate funded labor contract with its own budget,
  verifier, and settlement authority.
- `payable_class`: accepted work against an owner-approved funded receipt or
  batch. The receipt has a budget cap, verifier, and settlement authority, but
  the worker still must not call it paid.
- `payable_pending_settlement`: settlement authority has approved release or
  escrow/payout processing, and settlement refs are expected but not complete.
- `settled`: settlement refs exist. Only this state may be described as paid
  or settled.

Funding may be per receipt, or a named batch such as
`#5335 hygiene batch N`. A batch still needs an approved target list, budget
cap, verifier command, per-PR acceptance evidence, and one settlement closeout.

## Typed Fingerprint Keys

Invariant #6 of the debt-receipt invariants (dup/novelty fingerprint) is made
implementable with two typed keys
(`apps/openagents.com/workers/api/src/debt-receipt-key.ts`):

```text
DebtReceiptKey  = sha256(debtReceiptRef | repoBaselineRef | scopeDigest | objectiveDigest)
PatchNoveltyKey = sha256(DebtReceiptKey | normalizedPatchDigest | behaviorReceiptDigest)
```

- `debtReceiptRef` — the issue / EPIC / funded work-request receipt;
- `repoBaselineRef` — the commit/tree the debt was measured against;
- `scopeDigest` — sorted normalized path prefixes (+ optional symbol/contract refs);
- `objectiveDigest` — baseline metric + target metric + stop condition + verifier refs;
- `normalizedPatchDigest` — git patch-id (+ optional AST/semantic digest);
- `behaviorReceiptDigest` — the dereferenceable proof set (tests, benchmark
  replay, regenerate-and-diff, output parity).

The separator is length-prefixed so distinct field groupings cannot collide.
The settlement rule is enforced in the projection:

> **Exactly one accepted settlement per `DebtReceiptKey`, then retired.** A
> near-duplicate patch (a `PatchNoveltyKey` carrying an already-retired
> `DebtReceiptKey`) is a duplicate replay, not payable.

`projectDebtReceiptSettlement` derives the keys from
`debtReceiptKeyInput` / `patchNoveltyKeyInput`, compares the derived
`DebtReceiptKey` against the caller-supplied `retiredDebtReceiptKeys` set, and
collapses to `duplicate_replay` (not payable) on a hit. The earlier loose
`retiredReceiptRefs` + `duplicateFingerprintRefs` path is preserved as
supplementary evidence, but the typed key is the authority.

## Studied-Knowledge Source (SA-3, #5340)

For receipts that require studied-codebase evidence, the receipt carries a
studied-knowledge source:

- study packet ref;
- studied-knowledge graph ref;
- studied-knowledge verification ref;
- `sourceBoundary: public_refs_only`;
- correctness gate state, rejected-claim count, and validator-review refs.

A hygiene pass starts from the SA-1 study graph + verification
(`generateOpenAgentsRepoStudyArtifact` →
`verifyOpenAgentsRepoStudiedKnowledgeClaims`): you cannot safely refactor what
you do not understand. Artanis studying labor
(`apps/openagents.com/workers/api/src/artanis-studying-labor.ts`) is wired as a
recognized hygiene work type through
`studyingVerdictToDebtReceiptStudiedKnowledgeSource`, which maps the S3
verification verdict into the debt-receipt studied-knowledge source.

Passing studied knowledge is evidence, not authority. It can unblock a required
understanding gate for a hygiene receipt, but it does not grant write, spend,
settlement, deployment, or self-review authority.

**Fail closed (reviewer-flagged #5344 fix):** a studied-knowledge source that is
*present but invalid* — correctness gate failed, any rejected claims, or pending
validator review — fails the gate **even when studied knowledge is optional**.
Bad optional evidence may not leave a contribution payable while attaching
blockers. A required source must additionally be present.

**Work-class gate (reviewer-flagged #5387 fix):** `documentation_or_journal`
receipts project to `credit_class`, zero their projected payable sats, and add a
public blocker/caveat explaining that docs/journals are not size-scaled code
hygiene payouts. The durable debt-receipt store accepts only `payable`
projections, so credit-class docs cannot be persisted as payable receipts by
accident.

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
- Replays against a retired `DebtReceiptKey` (or loose retired fingerprint) are
  duplicate work, not payable work.
- Bad optional studied-knowledge evidence fails closed; it cannot leave a
  contribution payable.
- Documentation, journal, and process-discussion work is credit-class by default;
  it does not enter the code hygiene size/depth payout formula.
- After repeated rejected or revision-required attempts, the receipt/scope goes
  human-review-only until the benchmark, budget, or scope changes.

Regression coverage for these invariants lives in
`apps/openagents.com/workers/api/src/debt-receipt-key.test.ts`,
`apps/openagents.com/workers/api/src/debt-receipt-policy.test.ts`,
`apps/openagents.com/workers/api/src/debt-receipt-work-request.test.ts`, and
`apps/openagents.com/workers/api/src/artanis-studying-labor.test.ts`.
