# 246. Judges are components

> **Status: unimplemented — design draft.** Nothing here runs yet.

A search loop often needs a model to judge or route candidates. This chapter
treats Elephant or Mammoth as a pinned component inside a search. A judge is
measured against real outcomes, and it never grades its own work.

## What this chapter covers

- A pinned judge or router: Elephant or Mammoth, with its model identity
  recorded in every judgment.
- Agreement with real outcomes, measured on joined outcomes from [Chapter 217](ch217-record-readings-join-outcomes-watch-drift.md).
- Separate identities. The grader's identity differs from the student's and
  the reflector's.
- Cost per judgment, recorded.
- No teacher labels. A judge's opinion is never stored as a training label.

## Dependencies

- [Chapter 14](ch014-execute-elephant.md), [Chapter 15](ch015-execute-mammoth.md) and [Chapter 16](ch016-are-the-probabilities-any-good.md): Elephant, Mammoth and decision metrics.
- [Chapter 217](ch217-record-readings-join-outcomes-watch-drift.md): outcome joins.
- [Chapter 244](ch244-search-a-policy-cheapest-first.md): a search that uses judges. [Chapter 245](ch245-reflective-prompt-evolution.md) uses this chapter's judges in turn.
- [Chapter 247](ch247-outcomes-become-training-rows.md): the row admission rule that keeps opinions out of labels.

## Done means

- The chapter reports each judge's agreement with real outcomes, with an
  interval, on a held-out set.
- A configuration where the grader shares an identity with the student or the
  reflector is refused.
- A row whose label comes from a judge's opinion is refused by [Chapter 247](ch247-outcomes-become-training-rows.md).
- Cost per judgment is reported beside agreement.

## Sources

- [Chapter plan](PLAN.md), Part XXIV.
- [Measure, then improve](../docs/self-improvement.md).
