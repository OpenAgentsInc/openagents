# Adversarial Review Rounds

A review round runs several finder agents ("lenses") over one frozen commit.
Each lens looks for a different class of defect. This document states the
rules that keep a review round honest. The rules make a failed review
different from a passing review.

The rules come from the `chenglou/freerange` review workflow. The teardown
[`docs/teardowns/2026-07-21-freerange-teardown.md`](../teardowns/2026-07-21-freerange-teardown.md)
records the source. The tested authority for the rules is the
`@openagentsinc/review-round` package
([`packages/review-round`](../../packages/review-round/README.md)). The
workflow is `.agents/workflows/review-round.js` (mirrored at
`.claude/workflows/review-round.js` for tool discovery).

## The problem

A review process can fail in a way that looks like success. Two failures are
common:

- A lens agent dies. Its result is empty. An empty result can read as "no
  defects found".
- A lens reports zero findings, but it did not look at anything. A zero count
  can read as "the code is clean".

Both failures turn an unrun review into a green result. The rules below make
each failure visible.

## The four rules

`aggregateRound` folds the lens results into one `RoundResult`. The status is
`clean` only when at least one lens proved a sweep and no failure row exists.

1. **Positive control.** A lens that reports zero findings must also report
   `probesRun` of 1 or more. `probesRun` is the count of concrete probes the
   lens ran. An empty findings list is trustworthy only when the lens proved a
   sweep. A lens with zero findings and zero probes is `lens-unproven`.
2. **A died lens is visible.** A lens whose runner died has no report. The
   round records an `agent-died` failure. The round does not drop the row and
   does not read it as clean. The workflow does not filter dead agents out of
   the result set.
3. **A finding needs a reproduced contradiction.** A finding must include an
   `observed` field with a reproduced contradiction: a failing command, a
   crash, or a counterexample. A finding with an empty observation is
   `unsubstantiated-finding`. It does not count as a finding and it does not
   pass as clean.
4. **The status fails closed.** Any failure row makes the round `failed`. A
   `failed` round is different from a round that found real defects
   (`findings`) and from a clean round (`clean`). A round that swept nothing is
   `failed`, not clean.

Read the result with `roundIsClean(result)`. Do not read `status` by hand. A
`failed` round must never read as a passing round.

## Test design rule

Pair every required rejection or `unknown` path with a positive control
through the same path. A test that only shows a rejection does not prove the
tool can accept a valid input on that path. This rule comes from the Freerange
`goal-prompt.md` and is the reason `probesRun` exists: an empty findings list
needs a positive control to prove the sweep ran.

## How to run one round

The workflow takes a frozen commit and an optional lens set:

```
Workflow({
  scriptPath: ".agents/workflows/review-round.js",
  args: {
    commit: "<sha>",
    context: "<what changed and why>",
    lenses: [{ key: "correctness", prompt: "..." }],
    verify: true
  }
})
```

Without a `lenses` argument, the workflow uses a default set of three lenses:
correctness, boundary, and contract. With `verify: true`, each confirmed
finding gets an independent agent that tries to refute it. A refuted finding is
removed and recorded as a failure. A died verifier cannot clear a finding.

Repeat the round with fresh lenses until a round returns `clean`. A single
clean round is not proof that no defect exists. It is proof that the lenses ran
and found nothing.

## Boundary

A review round is producer-side evidence. A clean round is not admission, not
acceptance, and not a release decision. The round runs under the standing
independence rule: no agent accepts its own work. A review round supports a
decision. It does not make one.
