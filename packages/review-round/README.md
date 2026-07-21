# @openagentsinc/review-round

The tested authority for the anti-laundering rules that make an adversarial
review round's failure distinguishable from its success.

A review round runs several finder "lenses" over a frozen commit. The failure
mode this package removes is a review whose failure reads as a pass: a dead
lens whose empty output looks clean, or a lens that reports zero findings
without proving it looked at anything. Both launder an unrun review into a
green.

## The four rules

`aggregateRound` folds a round's lens outcomes into a fail-closed
`RoundResult`. The status can only be `clean` when at least one lens proved a
sweep and no failure row exists.

1. **Positive control.** A lens that reports zero findings must also report
   `probesRun >= 1`. An empty findings list only means "clean" when the lens
   proved a sweep happened. A lens with no findings and zero probes is
   `lens-unproven`.
2. **Died lens surfaces.** A lens whose runner died (its report is missing,
   modelled as `null`) becomes an explicit `agent-died` failure, never a
   dropped row and never a clean pass.
3. **Reproduced contradiction.** A finding must carry an `observed`
   contradiction (a failing command, a crash, or counterexample output). A
   finding with an empty observation is `unsubstantiated-finding`, not a
   confirmed finding and not a clean pass.
4. **Fail-closed round status.** Any `agent-died`, `lens-unproven`,
   `unsubstantiated-finding`, `malformed-report`, or `no-sweep` row makes the
   round `failed` (rerun), a state distinct from a round that produced real
   `findings` and from a `clean` round. An empty or fully-inconclusive round is
   `failed`, never vacuously clean.

Use `roundIsClean(result)` rather than reading `status` by hand so a `failed`
round can never be mistaken for a passing one.

## Relationship to the workflow

The workflow runs one round: N finder lenses over a frozen commit, optional
per-finding adversarial verification, then the same fold. Its canonical file is
`.agents/workflows/review-round.js`, mirrored at `.claude/workflows/review-round.js`
for tool discovery (the same tracked-symlink pattern the `.agents/skills`
directory uses). Workflow scripts must be self-contained plain JavaScript, so
the workflow inlines these rules rather than importing this package. **This package
is the tested authority for the semantics. The workflow must not diverge from
`aggregateRound`.** When the rules change, change them here first, keep the
tests green, then mirror the change into the workflow.

## Provenance

The rules were adapted from the `chenglou/freerange` review workflow and
recorded in [`docs/teardowns/2026-07-21-freerange-teardown.md`](../../docs/teardowns/2026-07-21-freerange-teardown.md)
(§5, §7.4). The verification discipline they serve is
[`docs/verification/REVIEW_ROUNDS.md`](../../docs/verification/REVIEW_ROUNDS.md).
