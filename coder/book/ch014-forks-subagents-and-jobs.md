# 14. Forks, subagents and jobs

> **Status: partly implemented.** Three of the four kinds run in Coder: a
> fork of a session at a turn boundary (`coder fork ID "TASK" [--at
> TURN]`), a background job (`coder run "TASK" --detach`, with `--deadline`
> and `--report-to`), and a subagent that a Claude Code session starts with
> `--coder-tools`, held to its parent's policy and time. The offline smoke
> checks all three. A goal (`--goal CHECK`) keeps a session working, turn
> after turn, until its check passes. A fork of a whole Claude Code
> conversation resumes the parent's Claude Code session, so the server's
> cache holds the earlier turns instead of Coder resending them as text.
> Still to come: the same for the psionic engine, and the measured gains
> chapter 21 asks for.

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
asks for reuses the parent's cached prefix state on the server.

Claude Code keeps its own sessions, and Coder logs each one's ID when Claude
Code starts:

```json
{"kind":"engine_session","engine":"claude","id":"1341c45f-a27b-466f-af3e-413b341f8c10"}
```

A fork of a Claude Code parent that keeps every turn (no `--at`, or
`--at` the last turn) and stays on Claude Code starts Claude Code with
`--resume ID --fork-session`. Claude Code continues the parent's
conversation in a new session of its own, and the fork's first message is
only the new task. The fork's header records `"resume":ID`, and `coder
replay` rebuilds that message without the earlier turns. Coder does this
only when Claude Code's session file is on this machine
(`~/.claude/projects/FOLDER/ID.jsonl`, the folder with each `/` and `.` as
`-`). Otherwise it sends the turns as text, as before.

In a live run on 2026-11-02, the parent was "Remember the word heliotrope.
Reply with just OK." The fork asked "What word did I ask you to remember?"
in a 63-byte message and got "heliotrope". Its first request read 13,537
tokens from the cache against the parent's 11,268, so the parent's turn
came from the server's cache instead of being resent.

The psionic engine (chapter 12) runs on a server we run, so its fork has
to come from Psionic chapter 176's prefix cache. Psionic's GPUs are off,
so that isn't built yet.

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

## Deadlines and reports

`--deadline SECONDS` bounds a run or job. Past it, Coder stops the work and
the turn ends as `expired`, with exit code 124. `coder ls` and `coder
status` show the state.

`--report-to ID` names the session that started the job. When the job is
over, Coder:

- appends a `job` line to that session's log:
  `{"kind":"job","id":JOB,"ending":"finished","text":LAST_REPLY}`, shown by
  `coder log` as `◆ job JOB finished: …`;
- leaves a message in that session's inbox: `Job JOB ended finished: …`.

A session that is still working takes the message as its next turn: a
detached job between turns, or the terminal UI when its composer is empty.

## Subagents

With `--coder-tools` ([chapter 10](ch010-drive-claude-code.md#coders-tools-in-claude-code)),
a Claude Code session can start another Coder session with `coder_run` or
`coder_fork`. Coder adds three things to every session it starts this way:

- `--report-to` the parent, so the result comes back.
- The parent's policy: the same `--allow`, `--ceiling` and `--root`. The
  tool's schema has no field for a policy, so the model can't widen it.
- `--deadline` set to the time the parent has left, when it has a deadline.

The child's session header records `report_to`, `allow`, `ceiling` and
`deadline_s`.

## Goals

`coder run "TASK" --goal CHECK` records an objective as a shell command that
decides when it is met. After each finished turn, Coder runs CHECK through
`/bin/sh` in the session's folder (`cd_goal_check` in
[coder.coax](../../coder/tui/coder.coax)), for at most 15 minutes. Exit 0 is
met, and the session ends. Otherwise the next turn's message is the check's
failure:

```text
The goal is not met yet: its check `grep -qx 'hello, goal' hello.txt` failed.
Keep working until the check passes.
```

with the last 2,000 bytes of the check's output when it printed any. Each
check is a `goal` line in the session log, with `check`, `turn`, `met` and
`text`. There is no turn limit unless the person sets one: with
`--goal-turns N`, a goal still unmet after N turns ends the session with
"goal not met after N turns" and exit 1. A turn that fails or is stopped
ends the session without a check. `--detach` passes the goal to the job.

In the live check, Claude Code got "Create hello.txt with a greeting in it."
and the goal `grep -qx 'hello, goal' hello.txt`. Its first turn wrote
"Hello, world!", the check failed, and the second turn rewrote the file and
ran the check itself; the goal was met on turn 2.

## Checks

```sh
coax test coder/tui/coder.coax    # cs_turns_text: a fork's turns up to a boundary
python3 coder/tui/smoke.py        # fork carries the turns; a Claude Code fork resumes (--resume, --fork-session),
                                  # replays, and falls back to text without the session file; --detach runs; stop ends it as stopped;
                                  # a goal met on its second turn, and one capped unmet at two
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
  saved on the server. Done for Claude Code (13,537 cached tokens read on
  the live fork above); the psionic engine remains.
- [x] A subagent cannot exceed its parent's grant or budget: it gets the
  parent's policy and remaining deadline, and the smoke checks its header.
- [x] A background job that finishes after its turn appends its result, and a job
  past its deadline is reported as expired.
- [x] A goal's check decides when the session's work is done; the smoke and
  a live Claude Code run check it.
- [ ] Each addition shows a measured gain in [Chapter21](ch021-measure-the-harness.md) on tasks it was not built
  from, or it is removed.

## Sources

- `coder/tui/session.coax` (`cs_turns_text`, `cs_sibling`), `coder/tui/coder.coax`
  (`cd_detach`, `cd_cmd_stop`, `cd_report_job`, the fork in `tool_main`),
  `coder/tui/mcp.coax` (`cd_claude_tool_call`), `coder/tui/smoke.py`.
- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
