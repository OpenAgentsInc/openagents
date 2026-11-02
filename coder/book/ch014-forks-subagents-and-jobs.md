# 14. Forks, subagents and jobs

> **Status: unimplemented — design draft.** Nothing here runs yet.

An agent sometimes needs a second line of work. This chapter adds four kinds:
a fork of a session at a turn boundary, a subagent with its own scope and
budget, a background job that outlives a turn, and a goal that a session works
toward across turns.

## What this chapter covers

- Forks. A new session starts from the parent's log up to a turn boundary. The
  server reuses the parent's cached prefix state.
- Subagents. Each one has its own tool grant, budget and deadline, no wider
  than the parent's.
- Background jobs. A job is acknowledged at once and completes later under its
  own deadline. Its result returns to the session as a log event.
- Goals. A goal is a recorded objective with a check. The session works
  toward it across turns, and the check decides when it is met.

Chapter20 builds the asynchronous loop on these jobs: long tools start and
stop, and their results return as events while the session keeps answering.
Chapter17 runs many Workers, each of which may use this chapter's subagents.

## Dependencies

- [Chapter2](ch002-model-visible-means-logged.md): forks start from the log.
- [Psionic chapter 176](../../psionic/book/ch176-paged-state-and-a-tiered-prefix-cache.md): prefix-state reuse on the server.
- [Psionic chapter 174](../../psionic/book/ch174-clients-streams-and-background-jobs.md): asynchronous jobs in the client contract.
- [Chapter5](ch005-a-sandbox-and-approvals.md): grants and budgets.

## Done means

- A fork's first request reuses the parent's prefix, measured as prefill tokens
  saved on the server.
- A subagent cannot exceed its parent's grant or budget.
- A background job that finishes after its turn appends its result, and a job
  past its deadline is reported as expired.
- Each addition shows a measured gain in [Chapter21](ch021-measure-the-harness.md) on tasks it was not built
  from, or it is removed.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
