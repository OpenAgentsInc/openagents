# 14. Forks, subagents and jobs

> **Status: partly implemented.** Two of the four kinds run in Coder: a
> fork of a session at a turn boundary (`coder fork ID "TASK" [--at
> TURN]`), and a background job (`coder run "TASK" --detach`, then `coder
> status` and `coder stop`). The offline smoke checks both. Still to come:
> a fork that reuses the parent's cached prefix on the server instead of
> resending the turns as text, subagents with their own grant and budget,
> a job's result returned to the session that started it, deadlines on
> jobs, and goals.

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

## What runs

### Forks

`coder fork ID "TASK"` starts a new session from a parent's log
([session.coax](../../coder/tui/session.coax), `cs_turns_text`). It reads
the parent's `user` and `reply` lines up to a turn boundary: all of them,
or the first `TURN` with `--at`. Each message and reply is cut to 4,000
bytes. The new session's first message carries them ahead of the task:

```text
Earlier in this conversation:

User: remember heliotrope
You: …

Now: what word?
```

The fork runs on the parent's engine unless `--engine` names another, so a
conversation that started on Claude Code can continue on Codex. Its header
names the parent and the turns it kept, so the tree of sessions is in the
files:

```json
{"kind":"session","id":"20261102-071324-728","version":"0.1.0","engine":"echo","cwd":"…","started_us":1793603604728037,"parent":"20261102-071323-161","at":1}
```

The log records only the new task as the `user` line. The turns it
carried are in the parent's file.

Resending the turns as text costs prefill every time. The fork the chapter
asks for reuses the parent's cached prefix state on the server (Psionic
chapter 176). That needs an engine whose server we run, the psionic engine
of chapter 12, and isn't built yet.

### Background jobs

`coder run "TASK" --detach` (or `coder fork … --detach`) gives the run a
session ID up front, starts the same command as a process of its own with
`--session ID`, prints the ID and returns. The job holds none of the
caller's pipes; its session file is the record, and `ID.pid` holds its
process ID while it runs.

```sh
id=$(coder run "run the test suite and fix what fails" --detach)
coder status $id        # state running, then finished, failed or stopped
coder log $id
coder stop $id
```

`coder stop ID` writes `ID.stop`. The running session looks for that file
every 200 milliseconds and stops as if Esc was pressed: the engine's turn is
interrupted, and the log ends with `stopped`. If no end line appears within
10 seconds, `coder stop` ends the job's process and writes the end line
itself. `coder stop` works on any running session, detached or not.

## Checks

```sh
coax test coder/tui/coder.coax    # cs_turns_text: a fork's turns up to a boundary
python3 coder/tui/smoke.py        # fork carries the turns; --detach runs; stop ends it as stopped
```

## Dependencies

- [Chapter2](ch002-model-visible-means-logged.md): forks start from the log.
- [Psionic chapter 176](../../psionic/book/ch176-paged-state-and-a-tiered-prefix-cache.md): prefix-state reuse on the server.
- [Psionic chapter 174](../../psionic/book/ch174-clients-streams-and-background-jobs.md): asynchronous jobs in the client contract.
- [Chapter5](ch005-a-sandbox-and-approvals.md): grants and budgets.

## Done means

- [x] A fork starts from the parent's log at a turn boundary, and its
  header names the parent.
- [x] A background job runs outside the turn that started it, and `coder
  stop` ends it as stopped.
- [ ] A fork's first request reuses the parent's prefix, measured as prefill tokens
  saved on the server.
- [ ] A subagent cannot exceed its parent's grant or budget.
- [ ] A background job that finishes after its turn appends its result, and a job
  past its deadline is reported as expired.
- [ ] Each addition shows a measured gain in [Chapter21](ch021-measure-the-harness.md) on tasks it was not built
  from, or it is removed.

## Sources

- `coder/tui/session.coax` (`cs_turns_text`, `cs_sibling`), `coder/tui/coder.coax`
  (`cd_detach`, `cd_cmd_stop`, the fork in `tool_main`), `coder/tui/smoke.py`.
- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
