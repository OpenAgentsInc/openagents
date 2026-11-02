# 12. A Psionic-served model as an engine

> **Status: unimplemented — design draft.** The chapter text and its checks aren't written yet.

Psionic serves models on this machine and on Pylons. This chapter runs
Microcoder on one and measures what owning both the harness and the server
buys.

## What this chapter covers

- Microcoder's gateway for a local Psionic server and for a Pylon.
- The stable prefix (chapter 3) against Psionic's prefix cache: hit rate,
  prefill saved and time to first token across a long session.
- Tool arguments constrained during generation, so a malformed call is never
  produced.
- The same tasks on a hosted model, for comparison.

## Dependencies

- [Chapter 3](PLAN.md#i-the-loop) and [Chapter 9](ch009-microcoder.md).
- Psionic [chapter 53](../../psionic/book/ch053-choosing-the-next-token.md),
  [chapter 174](../../psionic/book/ch174-clients-streams-and-background-jobs.md) and
  [chapter 176](../../psionic/book/ch176-paged-state-and-a-tiered-prefix-cache.md).

## Done means

- `openagents coder run --engine psionic "TASK"` runs against a local server.
- Cache hit rate, prefill saved, time to first token, task success and cost
  are reported for the local model and a hosted one on the same tasks.

## Sources

- [Chapter plan](PLAN.md), Part II.
