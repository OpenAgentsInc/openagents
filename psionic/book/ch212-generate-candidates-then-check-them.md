# 212. Generate candidates, then check them

> **Status: unimplemented — design draft.** Nothing here runs yet.

A model can produce several answers to a coding task, and a check can decide
which one is right. This chapter compares three ways to spend one fixed budget
on independently testable Coax tasks, and reports where each one succeeds and
fails.

## What this chapter covers

- Three strategies under one total budget:
  1. One candidate.
  2. Several independent candidates and a selection.
  3. Generate, run, read the failures and repair.
- Executable checks first. A learned verifier is added only once its labels
  are trustworthy.
- Separate tests: visible tests guide repair, and held-out checks score the
  result.
- Compilation is recorded apart from correctness.
- Three separate results:
  - whether any candidate was correct;
  - whether the selection chose a correct one;
  - whether a verifier accepted a wrong one.
- Cost: tokens, wall-clock time and execution cost per task.

## Dependencies

- [Chapter 200](ch200-the-simplest-loop.md), [Chapter 203](ch203-tools-as-typed-capabilities.md) and [Chapter 204](ch204-a-sandbox-and-approvals.md): the loop, tools and sandbox that run
  candidates.
- [Chapter 242](ch242-environments-with-checkers.md): environments with checkers.
- [Chapter 239](ch239-a-suite-is-three-digests.md) and [Chapter 241](ch241-gates-that-can-say-unverifiable.md): suites and verdicts.

## Done means

- All three strategies run on the same task set, model identity, seeds and
  budget.
- The three results above are reported separately for each strategy, with
  cost.
- Held-out checks are never visible to the repair step.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
