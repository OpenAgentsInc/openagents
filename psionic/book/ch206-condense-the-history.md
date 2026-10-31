# 206. Condense the history

> **Status: unimplemented — design draft.** Nothing here runs yet.

A long session fills the model's context. This chapter replaces older history
with one marked summary when that happens, and prunes tool results by rule.
Each condensation breaks the stable prefix of [Chapter 202](ch202-keep-the-prefix-stable.md), so the chapter
schedules condensation to limit recomputed prefill and measures that cost.

## What this chapter covers

- A trigger: the next request would exceed the context budget.
- A summary with a stable marker that replaces a range of older history.
- Log brackets. A condensation start event and a condensation end event
  surround the summary, so a crash mid-condensation leaves a visible record.
- Rule-based pruning of tool results, such as keeping only the last N
  results of a tool or truncating large outputs to a stated size.
- Scheduling. Condense at turn boundaries and in large steps, so prefill is
  recomputed less often.

## Dependencies

- [Chapter 201](ch201-model-visible-means-logged.md): the log that holds the brackets.
- [Chapter 202](ch202-keep-the-prefix-stable.md): the prefix measurement that condensation breaks.
- [Chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md): the prefix cache whose misses are counted.

## Done means

- A crash injected between the start and end events leaves a log that opens
  and shows the incomplete condensation.
- Rebuilding a request after a condensation reproduces it byte for byte from
  the log.
- The chapter reports recomputed prefill tokens and time to first token for at
  least two schedules on the same long session.
- Task success on held-out long sessions is reported with and without
  condensation.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
