# 20. An agent that stays online

> **Status: partly implemented.** `coder router` runs as a long-lived loop
> that keeps its state in files and resumes from them after a restart. Its
> watchdog ends Workers whose process died as `lost`, redirects a Worker
> that repeats one step or, by System One's judgment, isn't making progress,
> and stops it if that happens again. Replies follow per-channel rules: at
> most 100 words for chat, structured for issues. Long work runs as
> background jobs (chapter14). `coder daemon` attaches a machine as an
> execution environment: it starts queued jobs only within the machine's
> grant, and a job for a machine that is offline waits and is sent when the
> machine answers. Still to come: credential handles, rules for pull
> requests, mail and documents, and the restart and fault-injection
> measurements.

Chapter17's agent is only useful if it keeps working while a laptop sleeps,
answers while a long build runs, talks the way each channel expects and
recovers from failures without a person watching. This chapter covers the
asynchronous loop, channel rules, the split between compute, state and
execution environments, and a watchdog. It follows the second half of the
DeepSeek Harness team's Super-agent article
([notes](../../psionic/docs/harness.md#one-agent-across-many-tasks)).

## What this chapter covers

- **Full duplex.** Long tools become `start` and `stop`. `start` returns a
  `started` notice with a job ID at once, and the loop goes on; the result
  arrives later as a new event in the log (chapter14's background jobs). The
  model can `stop` a job that runs too long. New input is handled when the
  current inference step ends, not when the tools finish.
- **Channel rules.** The Router writes every reply, so verbose reasoning and
  tool logs stay inside Workers. Each channel has written rules:
  - Chat: short, interrupt only for a decision or a sign-off, ask direct
    questions, give conclusions. A hard cap of 100 words per reply.
  - Issues, pull requests, mail and documents: structured and complete, such
    as background, change and verification for a pull request, following the
    project's conventions.
- **Compute apart from state.** The loop runs in stateless containers that can
  restart, scale out or move. Sessions, Threads, subscriptions and jobs live
  in a durable store as chapter2's log, and a replacement container
  resumes from the log.
- **Compute apart from execution environments.** Repositories with
  uncommitted work, local credentials, intranet services and specific
  hardware stay where they are. Each machine runs a small daemon that holds no
  session state; it runs commands and returns results under chapter5's
  sandbox:
  - The person's own machine, a cloud machine or a sandbox made on demand all
    attach through the same interface.
  - A task that needs an offline machine waits, and starts when that machine
    reconnects.
- **A watchdog.** A separate session, outside the main system and woken on a
  schedule:
  - Handles infrastructure faults: API errors, failed state recovery, stuck
    jobs. It rechecks transient external failures until they clear.
  - Judges with a model whether each Worker is making real progress or has
    drifted from its goal (chapter14's goal check), and stops or redirects
    it.
  - Answers maintainers' questions about failures and can change the main
    system's prompts and trigger rules while it runs. Every change it makes is
    a logged, revertible event.
- **Trust boundaries.** The loop takes in untrusted events, and the machines it
  reaches hold private code and credentials:
  - A daemon admits only commands within its machine's grant, so a
    compromised loop cannot take every attached machine.
  - Credentials are used through capability handles the daemon resolves. The
    secret never enters model context or the log.
  - No session content leaves the person's machines without chapter18's
    opt-in.
- **Open problems kept open.** When to shard the Router and how finely, when
  Workers should talk to each other directly, memory over months (chapter
  214), how tasks appear to a person once a conversation is no longer one
  session, and agent-to-agent work between instances that share no context
  and hold different permissions. The chapter records these as unanswered
  rather than guessing.

## What runs

### The loop

`coder router` ([router.coax](../../coder/tui/router.coax),
`cd_cmd_router`) makes a pass over the inbox every two seconds, then runs
the watchdog. `--once` makes one pass and exits, for scripts and tests.
`--json` writes each fate as a JSON line.

The Router holds no state in memory that it can't rebuild. Everything it
needs is in files under `~/.openagents/coder`:

| File | What it holds |
| --- | --- |
| `inbox/*.json` | Events not yet decided |
| `inbox/done/*.json` | Events already decided |
| `router/sources.json` | Source policies |
| `router/threads` | Each Thread's Worker session |
| `router/log.jsonl` | Each event's fate, and the watchdog's findings |
| `sessions/ID.jsonl`, `ID.pid` | Each Worker's log and process |

A Router that is stopped and started again reads `router/threads` and
continues. Events that arrived while it was down wait in the inbox. Workers
are separate processes, so they keep running while the Router restarts.

Long work doesn't block the loop: each Worker is a background job
(chapter14), and the Router only starts it and records the session ID.

### The watchdog

On each pass, the watchdog (`rt_tend`) reads every subscribed Worker whose
log has no end line:

- **Lost.** If the Worker's process is gone, the watchdog appends an end
  line with the ending `lost` and logs `worker lost: ended`. On the next
  pass the Thread gets a reply saying so.
- **Looping.** If the last turn ran the same step five times in a row, the
  watchdog redirects the Worker (`worker redirected: looping`). This
  compares the logged tool labels for equality; it doesn't read intent.
- **Quiet.** If the Worker's log hasn't grown for `--stuck` seconds (1,800
  by default), System One reads the task and the end of the log and answers
  whether the Worker is still making real progress. If it isn't, the
  watchdog redirects it. If it is, the watchdog logs that once and leaves
  it. With no System One key, it logs `worker stuck` with the reason.

A redirect gives the Worker one more chance:

1. The watchdog leaves a message in the Worker's inbox: for a loop, "you ran
   the same step 5 times in a row. Stop repeating it. Try a different
   approach, or say what blocks you and end the turn"; for no progress, the
   task again.
2. It writes `ID.turnstop`. The job ends its current turn as `redirected`
   and takes the message as its next turn. The conversation goes on.
3. It adds the ID to `router/redirected`.

A Worker that loops or stalls again after a redirect is stopped, the same
way as with `coder stop`: the watchdog writes the session's stop file, and
the Worker ends its turn as `stopped`.

### Channel rules

The Router writes every reply; Workers never do. Each source has a channel,
and the reply follows its rules (`rt_reply`):

| Channel | Rules |
| --- | --- |
| `chat` | At most 100 words, cut with " …". Says how a Worker stopped if it didn't finish. |
| `issue` | Structured: the ending, steps and session first, then the full result. |

### Machine daemons

`coder daemon` ([daemon.coax](../../coder/tui/daemon.coax)) attaches the
machine it runs on. It holds no session state. Every two seconds it:

1. Writes a heartbeat, `~/.openagents/coder/daemon/daemon.json`, with its
   name, host, grant and the time of the pass.
2. Takes each job in `daemon/jobs/` and checks it against the grant:
   `--engines E,F` (any engine when left out) and `--root DIR,DIR` (the
   directories a job may run in, `$HOME` by default; a path that steps up
   with `..` is refused). An admitted job starts as an ordinary detached
   session, `coder run TASK --engine E --cwd DIR --detach`. The job then
   moves to `daemon/done/` with its session, or with why it was refused.
3. Every 30 seconds, tries the jobs held for machines that did not answer.

`coder queue "TASK" [--engine E] [--cwd DIR]` adds a job and prints its ID.
With `--on ssh:DEST` the job goes into DEST's queue. When ssh can't reach
DEST (exit 255), the job is held here in `daemon/held/` with its route, and
this machine's daemon sends it once DEST answers. `coder jobs` shows the
daemon's heartbeat (`up` when its last pass was within 10 seconds) and every
job: `queued`, `held`, `started` with its session, `refused` with why,
`sent` with the job ID it got on the other machine, or `failed` with the
exit status when the other machine answered but did not take it.

In the live check, a job queued for coderos-4080 while `ssh` failed was
held here. The next daemon pass sent it, and coderos-4080's own daemon,
granted `/tmp` and `/home`, started it as a session that printed the host
name and finished.

## Checks

- `python3 coder/tui/smoke.py`, offline: each check runs `coder router
  --once` as a fresh process, so every pass resumes from the files. The
  hand-off of a Thread to its Worker works across those restarts, and a
  Worker whose process is killed ends as `lost`. A Worker that repeats one
  step is stopped, and a quiet Worker with no System One key is reported as
  stuck.
- The smoke also queues three jobs for a daemon granted `echo,micro`: the one
  inside the grant starts a session, and the ones with another engine or
  another directory are refused with why. A job queued `--on ssh:box` while
  a stand-in `ssh` fails is held, sent on the next daemon pass after `ssh`
  works, and started by the other machine's daemon.
- Unit checks: a job's admission against the grant, the chat cap of 100 words and the issue format.
- Live: two quiet Workers with the same task, "fix the failing parser test".
  System One judged the one that read, edited and tested the parser as still
  making progress, and the one browsing news sites as not, and the watchdog
  stopped only the second.

## Dependencies

- [Chapter2](ch002-model-visible-means-logged.md): the durable log the stateless loop resumes from.
- [Chapter5](ch005-a-sandbox-and-approvals.md): grants and confinement on each daemon.
- [Chapter6](ch006-a-document-tries-to-become-the-system-prompt.md): hostile events and tool results.
- [Chapter14](ch014-forks-subagents-and-jobs.md): background jobs and goals.
- [Chapter18](ch018-consent-before-sharing.md) and [Chapter19](ch019-remember-a-fact-then-revise-it.md): consent and long-term memory.
- [Chapter17](ch017-one-agent-many-tasks.md): the Router and Workers this chapter keeps online.

## Done means

- **Duplex.** With a ten-minute job running, a new message gets a reply within
  one inference step, and the job's result arrives later as an event. A
  stopped job reports as stopped, and an expired one as expired.
- **Channels.** Every chat reply in a replayed stream is 100 words or fewer,
  and every pull request body has its required sections.
- **Restart.** Killing the loop's container mid-task and starting a new one
  resumes every open task from the log with no lost or repeated tool effects.
- [x] **Intermittent machines.** A task bound to a machine that goes offline
  waits, then completes when the machine reconnects. Checked by the smoke
  with a stand-in `ssh`, and live against coderos-4080.
- **Watchdog.** In fault-injection runs (API errors, a Worker looping on one
  failing call, a Worker drifting off its goal) the watchdog detects each
  fault and the time to detection is reported.
- **Containment.** In chapter6's lab, extended to Triggers, a hostile event
  produces no admitted command outside a daemon's grant and no secret in
  model context, the log or outbound messages.
- Each mechanism passes chapter21 against the simpler design on tasks it was
  not built from, or it is removed.

## Sources

- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
- DeepSeek Harness team, "Exploring Super-agent Design: Towards a More
  Autonomous Agent System" (Yujia Qiao et al.).
- Anthropic, "Decoupling the brain from the hands", on a stateless harness
  with external session logs.
