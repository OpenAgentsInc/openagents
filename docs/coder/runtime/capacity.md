# The usage-limit book

A provider's usage limit belongs to the login, not to one agent. When the
Claude weekly limit runs out, every agent on that login stops at once. The
usage-limit book is where every agent on this computer records a limit it
hit and reads the limits others hit, so no agent starts work that can't
succeed, and a task a limit stopped resumes after the reset instead of
starting over.

Status: implemented
([#10765](https://github.com/OpenAgentsInc/openagents/issues/10765)). The
book is `crates/microcoder-loop/src/capacity.rs`; the detector is
`crates/coder/src/task/capacity.rs`; resume points are
`crates/coder/src/task/resume.rs`; the commands are
`crates/openagents-cli/src/capacity.rs`.

## The book

The book is `capacity.json` in the task store: `~/.openagents/tasks`, or
the directory `OPENAGENTS_TASKS` names. It holds one entry for each
provider that refused work: the provider (`codex`, `claude`, `vertex`,
`devin`, `opencode`, or `grok`), the kind (`usage_limit` or `rate_limit`),
when the refusal was observed, the reset the provider reported, and
`until`, when the provider has capacity again. A refusal with no reported
reset holds for 30 minutes (5 for the OpenAgents cloud). The file is
`0600` and written under a lock. It holds no prompt, credential, or model
output. A refusal applies only to the login it was observed on: once
another account is signed in, it no longer holds.

## Who writes it

| Writer | What it detects |
| --- | --- |
| The Microcoder loop | Codex's HTTP 429 `usage_limit_reached`, Claude Code's rejected `rate_limit_event`, Vertex's `RESOURCE_EXHAUSTED`, and the cloud worker's `rate_limited`, `busy`, and `quota_exhausted`. See [the delegate door](delegate-door.md#which-provider-generates). |
| Coder's Claude Code, Codex, and OpenCode delegations | The limit a delegated CLI turn ended on: the stream's rejected `rate_limit_event` with its `resetsAt`, or the limit sentence the CLI printed. |
| Repository runs on a Claude Code or Codex session | A limit before any work fails the run over to the next route; a limit after work ends the turn with what it did and records the limit. |
| Repository runs on an ACP agent (Devin, OpenCode, and Grok Build) | An ACP error the agent marked retryable, whose code is 429, or whose message names a usage, session, weekly, or rate limit. |
| `openagents capacity record` | A limit an agent outside Coder hit, such as a Claude Code subagent an orchestrator launched. |

The detector reads these limit messages, with the reset each one states:

- `Claude AI usage limit reached|1790164200`: Unix seconds after the bar.
- `You've hit your session limit · resets 11:50am (UTC)` and `You've hit
  your weekly limit · resets Sep 24, 5pm (UTC)`.
- `You've hit your usage limit. ... try again at 3:04 PM.` from Codex, and
  `try again at Sep 25th, 2026 9:15 AM`.
- `exceeded retry limit, last status: 429 Too Many Requests`: a rate limit.

A time in a named zone other than UTC, such as `resets 3pm
(America/Los_Angeles)`, is not read, because the detector carries no zone
database; that limit holds for 30 minutes. The detector reads error text
only, never what a model wrote, because a model can quote a limit message.

## Who reads it

- Coder's delegate door skips a provider whose limit holds and says why.
- The auto-start policy routes each start to an admitted provider with
  capacity, and ends a task as `no_capacity` when none has it.
- `openagents capacity check` answers an orchestrator before it starts an
  agent.

## Resume after a limit

When the auto-start policy sees a task it started end on a limit, it
records a resume point in `resume.json` beside the book. A run ended on a
limit when it ended `no_capacity`, or when it failed and the book holds a
refusal for one of its grant's providers observed during the run. A task
the policy ends as `no_capacity` before it starts gets a resume point too.
The point keeps:

- The task and the turn the limit stopped.
- The provider and its reset.
- The delegate's session, from the stopped turn's trace (the
  `claude_session`, `codex_session`, `devin_session`, `opencode_session`,
  or `grok_session` note).
- The task's worktree.
- The last checkpoint: the candidate snapshot digest the run left.
- The workspace and the device the turn was started for.

Each sweep, the policy continues every open point once an admitted provider
has capacity: after the reset, or sooner when another admitted provider has
capacity. It continues the task with a resume turn whose message says a
limit stopped the work, names the checkpoint, and repeats the request, and
makes that turn eligible in the same workspace. The turn then starts like
any other, under the policy's workspace, concurrency, and engine bounds. Its
engine resumes the delegate's session from the earlier turn's trace, as
every follow-up does, and works in the same worktree. A point resumes once;
a task that moved on, ran out of turns, or left the policy's workspaces
resolves its point with the reason. A run that started more than a week
before the sweep gets no point, so tasks from before an upgrade don't
resume.

The journal, `autostart.jsonl`, records `resume_point` when a point is made
and `resumed` when the policy continues the task.

Without the auto-start policy, continue a stopped task by hand:

```sh
openagents task resume TASK_ID
```

This continues the task from its open resume point at once, whatever the
book says, and the turn waits queued for a start like any follow-up.

## Commands

```sh
openagents capacity [--json]
openagents capacity check PROVIDER
openagents capacity record PROVIDER [--reset TIME] [--rate]
```

- `openagents capacity` (or `capacity list`) prints each provider and, when
  a limit holds, its kind and until when. With `--json` it prints
  `{"book", "now", "providers": [...]}`; each provider has `capacity`, and a
  limited one has `kind`, `observed_at`, `resets_at`, `until`, and
  `until_utc`.
- `openagents capacity check PROVIDER` exits 0 when the provider has
  capacity and 1 when a recorded limit holds.
- `openagents capacity record PROVIDER --reset TIME` records a usage limit
  (a rate limit with `--rate`) until `TIME`. `TIME` is Unix seconds, an ISO
  8601 UTC time, a duration from now (`90m`, `5h`, or `2d`), or a clock
  time as the CLI printed it (`3pm` or `"resets 11:50am (UTC)"`), read as
  UTC. Without `--reset` the limit holds for 30 minutes.

Every command takes `--store DIR` to read another task store.

### For orchestrators

Before you launch a Claude Code subagent, run `openagents capacity check
claude`, and don't launch it when the command exits 1. When a subagent
stops on a limit, record it with the reset the subagent printed:

```sh
openagents capacity record claude --reset "resets 3pm (UTC)"
```

## Limits

- An orchestrator's own subagents resume only when the orchestrator
  restarts them. The book and resume points cover tasks in the task store.
- A turn that hit a limit in Coder's interactive terminal is recorded in the
  book but has no resume point: the person continues the conversation.
- Studio seats and the workshop agent read the book through their engines.
  A seat's task gets a resume point like any task the policy started.

## Related

- [The delegate door](delegate-door.md)
- [Host auto-start](host-autostart.md)
- [Cloud fallback](cloud-fallback.md)
