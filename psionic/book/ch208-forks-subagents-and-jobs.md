# 208. Forks, subagents and jobs

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

## Dependencies

- [Chapter 201](ch201-model-visible-means-logged.md): forks start from the log.
- [Chapter 176](ch176-paged-state-and-a-tiered-prefix-cache.md): prefix-state reuse on the server.
- [Chapter 174](ch174-clients-streams-and-background-jobs.md): asynchronous jobs in the client contract.
- [Chapter 204](ch204-a-sandbox-and-approvals.md): grants and budgets.

## Done means

- A fork's first request reuses the parent's prefix, measured as prefill tokens
  saved on the server.
- A subagent cannot exceed its parent's grant or budget.
- A background job that finishes after its turn appends its result, and a job
  past its deadline is reported as expired.
- Each addition shows a measured gain in [Chapter 211](ch211-measure-the-harness.md) on tasks it was not built
  from, or it is removed.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
