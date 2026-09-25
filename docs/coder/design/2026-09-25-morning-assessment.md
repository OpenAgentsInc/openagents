# Where Coder stands: morning, 2026-09-25

> **Infrastructure update:** [the retention and budget repairs](../../terminal-bench/2026-09-25-retention-budget-repairs.md)
> complete #9649 and #9650. The audit recovers no missing v18 state, so oracle
> coverage stays at 6/18. No new policy or family run is admitted by these repairs.

Status: assessment, written at 09:15 local on 2026-09-25 from everything
pushed since the [midnight assessment](2026-09-25-assessment.md): about 110
commits from three sessions (Claude, a Claude Fable 5.1 session, and the
Codex agent), 23 closed issues, and 20 new result documents under
[`terminal-bench/`](../../terminal-bench/). It updates the
[2026-09-24 assessment](2026-09-24-assessment.md) and the midnight one, and
doesn't replace their tables.

## Summary

- **Every test of the cheaper-work claim on a task we didn't tune on has
  come back without a pass.** The held-out test set was 0 of 4
  (`microluna-v15`), the first family run graded 0 of 3 before an
  environment failure stopped it, and overnight `microluna-v18` ran the
  whole pinned Luna-sized family: **0 of 18** (confirmation 0 of 9,
  development 0 of 9). Fable 5.1 low passes those six tasks 4 or 5 times in
  5. The only repeated Microluna pass is still `embedding-drift-monitor`,
  in-sample.
- **Luna's self-score is a false assurance.** On v18, all nine submissions
  with a known full self-score failed the official verifier, and twelve
  failed trials reached a full score somewhere in the session. Across
  every cohort measured so far, a green self-score has never been a
  reliable sign of a pass.
- **No check separates passing from failing work on held-out tasks yet.**
  Overnight added the executed task contract (a guard: right when it
  speaks, silent on 26 of 31 held-out candidates), tiered acceptance tests
  (no tier promotable: no held-out task had both passing and failing graded
  work), a verified baseline rerun, a finish rule, check-line grades,
  departure miners, and a review-on-disagreement rule. Each was measured,
  most were not admitted, and the admitted ones did not change v18's
  outcome. The newest candidate, deterministic literal artifact checks,
  caught 3 real failures with 3 calls on already-opened development data,
  and a reserved 90-attempt confirmation of it is running now.
- **Half the evidence the next step needs is missing.** Oracle headroom,
  whether any retained candidate passes even when the submitted one
  doesn't, is known for only 6 of v18's 18 trials (all zero). The other 12
  lost their candidates to snapshot and collection gaps (#9649). Without
  it, nobody can say whether better selection could help or whether Luna
  never produced a right answer at all.
- **The infrastructure is far better than it was.** Trials start faster
  and more safely, multi-service tasks run under the network allowlist,
  evaluation runs are sealed, keep-best works on real repositories, the
  product reads policy manifests, the issue flow has an evaluation set,
  and every result is pre-registered, replayable, and costed with Jev and
  failed attempts included.

## What ran overnight, and what it showed

| Work | Issue | Result |
| --- | --- | --- |
| `microluna-v18` on the pinned Luna-sized family, 18 attempts | #9640 (tracking, closed) | 0 of 18. Numerically a loss; strictly inconclusive, because a harness change and a driver restart happened mid-run. About $0.96 counted. [Report](../../terminal-bench/2026-09-25-microluna-v18-family.md) |
| Executed task contract, run by code | #9628 (closed) | Guard only: 2 of 2 failure calls right, 0 of 3 pass calls right, 26 of 31 held-out candidates with no call; within-task discrimination at chance. [Report](../../terminal-bench/2026-09-25-executed-contract-checks.md), [supplement](../../terminal-bench/2026-09-25-executed-contract-supplement.md) |
| Tiered authority for acceptance tests | #9629 (closed) | Built, off. No tier promotable; "independently supported" tests are the one promising tier, in-sample only. [Report](../../terminal-bench/2026-09-25-tiered-acceptance.md) |
| Host runs the task's own program first, and reruns it after every session | #9633, #9636 (closed) | Built. On v18 no family task exposed an entry point, so it never ran. [Baseline](../../terminal-bench/2026-09-25-baseline-offline.md), [executed rerun](../../terminal-bench/2026-09-25-verify-executed-offline.md) |
| Finish rule, review-on-disagreement, check-line grades, departure miners, environment facts | #9632, #9634, #9635, #9637, #9638 (closed) | Each measured offline or on mini-tasks; admitted pieces went into v18; none changed its outcome. See the `2026-09-25-*-offline.md` reports. |
| Literal artifact checks (required output paths and byte limits, no model) | #9646, #9648 (closed) | 3 real failures caught with 3 calls on opened development data; a reserved 90-attempt confirmation is running now. [Report](../../terminal-bench/2026-09-25-literal-artifact-checks.md) |
| Archive confirmation of executed checks, 72 sealed predictions | #9584 | Missed its declared bar. [Report](../../terminal-bench/2026-09-25-archive-check-confirmation.md) |
| Evidence plugins as program steps (repo map, code search, test-report parser) | #9630 (closed) | Built and off; measurement planned on the issue-flow evaluation set. |
| Trial setup time | #9631 (closed) | The agent is 85% of trial time; setup was cut from 33 to 71 s to 10 to 36 s per trial. |
| Network allowlist with multi-service tasks | #9607 | Fixed; the three blocked family tasks now start. |
| Program `module` steps: limits, snapshots, cancellation | (roadmap phase 3) | Fixed; a scoped snapshot-read guest can read the workspace. |
| Run cards, the Gym file viewer, and confirmation views | #9639, #9647, #9594 (closed) | Every trial can be characterized from its retained records. |

## The thesis this morning

The determinism thesis says a cheap model reaches a program-defined
"done" if the contract is faithful. Two parts have now been tested apart:

1. **A contract Luna writes is not faithful.** Suites (v6 to v8) and
   self-scores (v9 to v18) are green on wrong answers. Tiered authority can
   keep a bad test from doing harm, but it can't turn one into a good test.
2. **A contract the host executes from the task is faithful but nearly
   silent.** Tasks rarely state their deciding facts; what they state, every
   serious attempt already meets. The executed contract and the literal
   checks work as guards, not verdicts.

What remains untested is the one piece both assessments point to and
nobody has built at scale: **independent support for an expected value**,
a figure recomputed by a separate route, or a defect reproduced in a
read-only container. The Codex agent's reviewer is the nearest attempt.

The pivot's premise, that Luna can do most coding work with the right
structure, has **no positive evidence outside one tuned task**. That is a
statement about the evidence so far, not a proof that it's false: v18 gave
Luna a 20-minute loop, and six trials ran out of time mid-work, and 12 of 18
never had their candidates graded.

## What's needed before the next family run

In order, and none of it requires a new policy:

1. **Measure oracle headroom (#9649).** Retain every sequential candidate
   so it can be restored and graded, including collect-hook outputs, and
   report the share of trials where any candidate passes. If it's zero,
   selection and checks can't help on this family, and the gap is
   capability.
2. **Fix budget accounting (#9650).** Count unknown costs by the declared
   rule during the run, not only in the report.
3. **Read the literal-check confirmation** when the Codex agent opens it,
   and admit the check only on its pre-registered bar.
4. **Then decide the family question on evidence:** another pre-registered
   run only if headroom shows something to select, or a check passes its
   bar.

## Product work that doesn't wait on the benchmark

The issue flow runs on this repository, where tasks are smaller and better
specified than Terminal-Bench's. It has a sealed evaluation set of eight
past issues (#9625) and the lean loop behind a manifest (#9624). Measuring
the two issue-flow policies on the development split costs about a dollar
and answers a question users care about directly: does Coder resolve a
real issue in this repository. It's also where the evidence plugins get
their first measurement. It waits only for the machine: the Codex agent's
cohort is running, and cohorts shouldn't overlap.

## Open issues

| Issue | State | Next |
| --- | --- | --- |
| #9649 Retain complete sequential candidates and report oracle coverage | New | First. It decides whether selection can help at all. |
| #9650 Enforce cohort budgets with unknown costs | New | Second; small. |
| #9584 Truthful checks | Open, Codex agent | Its 90-attempt literal-check confirmation is running; don't overlap it. |
| #9624 Lean loop in the product | Built; default undecided | The issue-flow evaluation run decides it. |
| #9607 Repeatable wins against Fable | Open | No win yet; waits on #9649 and #9584. |
| #9587 Best-of-N | Arms ready | Waits on headroom (#9649) and a selection signal (#9584). |
| #9577 Human marks in the Gym | Blocked on the operator | Unchanged. |

## Housekeeping

- Several verifier and environment containers from runs 25 to 31 hours old
  are still up on this host (`verify__*`, `gsea-proteomics__*`,
  `sound-change-cascade__*`, `wdm-design__*`). They should be checked and
  removed by whoever owns them, after the running cohort finishes.
- Disk is at about 110 GB free after last night's cache prune; the literal
  confirmation protocol asks for 20 GB before launch.
