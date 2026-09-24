# Run Coder One on a mini-task

A mini-task is a small local task with its own grader. Run an episode on
one with the scripted executor to test how Coder One's components compose,
in about a second and with no model or container. Run the same task with
Claude Code or Codex to see whether a real executor uses what the
components give it.

Think of these as fast local screens for a focused failure family, not
Terminal-Bench 4.0 (TB4) results. They borrow the episode path and the
independent, post-episode grading idea, but use four small purpose-built
tasks rather than TB4's curated benchmark suite. A pass here is not a TB4
score and says nothing by itself about benchmark performance. The grader
checks the task's concrete outcome after the agent finishes; it is hidden
from the episode. Scripted runs take about a second and cost $0 in model
calls. The retained four-task comparison models Opus at 22.0 seconds and
$0.05368 mean per episode (`bench/terminal-bench/handoff/minitask-patterns.json`,
summarized below).

This is rung 2 and rung 3 of the ladder in
[Coder as a tunable system](../../optimization/coder-components.md#test-each-component-in-isolation):
components alone come first, in the
[component guide](coder-one-components.md), and a Terminal-Bench trial comes
after.

## The mini-tasks

Each task covers one failure family from the v3 Luna trials, or a recovery:

| ID | Family | The grader checks |
| --- | --- | --- |
| `log-severity` | Field meaning in data | `summary.csv` counts each log line by its severity field, not by severity words in the message. |
| `interactive-terminal` | Interactive behavior | An interactive program started through `HeadlessTerminal` reads a typed line. |
| `cancel-cleanup` | Cancellation lifecycle | After a real interrupt, both started tasks finish their cleanup. |
| `git-recovery` | Recovery | A commit lost to a hard reset is back on `master`, and the tree is clean. |

The grader runs after the episode ends, and the episode never sees it, as
Harbor's verifier is to a Terminal-Bench task. The interactive and
cancellation graders need `python3` on `PATH`; without it, their verdict is
`unavailable`.

## Run an episode

```sh
coder-one minitask list
coder-one minitask run log-severity                  # the good script
coder-one minitask run cancel-cleanup --script bad   # the known-bad script
coder-one minitask run git-recovery --json
```

A run sets the task up in a scratch directory, then runs the same
explore-then-delegate path a Terminal-Bench episode runs, with no explore
steps: the briefing packer builds the briefing, the executor runs it, and
the closing check asks Jev when you pass `--jev live`. Before the grader,
`verify.checks` observes the workspace and writes its report to
`verification/checks.json`; pass `--no-checks` to skip it. The
[checks guide](coder-one-checks.md) covers the scenarios. With `--jev
live`, `verify.support` then judges the checked requirements and writes
`verification/support.json`, as the
[support guide](coder-one-support.md) describes. Pass `--monitor` to watch
the session with `control.monitor` in shadow mode; the
[component guide](coder-one-components.md#watch-a-session-with-controlmonitor)
covers it. With `--repair PROFILE`, `verify.repair` then runs one fresh
session from the checks' diagnostic packets and reruns the checks, as the
[repair guide](coder-one-repair.md) describes. The grader runs last. The
exit code is 0 when the grader passed.

Each task has two scripts. `good` writes a correct solution, and `bad`
writes the known-bad one for its family: a whole-line severity search, a
terminal that runs each line with `bash -c`, a runner that returns before
cleanup finishes, and a retyped file instead of the lost commit. Pass
`--script FILE` to play a script of your own.

## Write a script

A script is JSON in the `openagents.coder-one.executor-script.v1` shape: a
list of timed actions, and the lists played when the host steers or resumes
the session.

```json
{
  "schema": "openagents.coder-one.executor-script.v1",
  "name": "fix-after-steer",
  "events": [
    { "at_ms": 0, "do": "command", "command": "pytest -q", "output": "1 failed", "exit_code": 1 },
    { "at_ms": 10, "do": "hang" }
  ],
  "on_steer": [
    { "at_ms": 30, "do": "write", "path": "fixed.txt", "content": "ok\n" },
    { "at_ms": 40, "do": "claim", "text": "Fixed after the steer." },
    { "at_ms": 40, "do": "end" }
  ]
}
```

| Action | What it does |
| --- | --- |
| `claim` | Emits an assistant message. |
| `command` | Reports a command with its output and exit code; nothing runs. |
| `run` | Runs a real shell command in the scratch directory, bounded to 60 seconds, and reports it. |
| `write`, `remove` | Changes a file under the scratch directory. A path that leaves it is refused. |
| `emit`, `raw` | Emits one native event, as JSON or as a verbatim line. |
| `end` | Ends the turn, completed or, with `"error": true`, failed. |
| `exit` | Exits with a code and no result. |
| `hang` | Stops making progress until the host steers, stops, or the deadline passes. |

The session writes a Codex stream by default, or a Claude Code stream with
`"format": "claude"`, so the host's parsers read it as they read a real one.
`capabilities` limits what the session lets the host do; every capability is
on by default.

## Control the session

`--controls FILE` gives the host rules for the scripted session:

```json
{
  "deadline_ms": 5000,
  "tick_ms": 10,
  "steer": { "when": { "on": "command_failed" }, "message": "A command failed; fix it." },
  "stop_when": { "on": "claim", "contains": "draft" },
  "resume": "Continue from the draft."
}
```

A trigger fires on an assistant claim (`claim`), a failed command
(`command_failed`), a changed file (`artifact`), or a time (`after`). The
host records each action as an `exec.control` invocation: start, observe,
steer, stop with its cleanup acknowledgement, and resume, which names the
session it continues. An action the adapter hasn't demonstrated is recorded
as refused, not attempted.

One controller owns the session's state. Each normalized event is stamped
with a version: the session ID, the process generation (1 for the start,
one more for each resume), the controller's sequence number, and the
workspace revision, which counts the artifact changes observed so far. A
rule that fires submits a proposal carrying the version it saw, and the
controller refuses a stale one: a steer made from the first process can't
reach the resumed one, and a check that saw revision 3 can't accept
revision 4.

### The capability matrix

Each adapter has a capability matrix, and a policy may use only what its
adapter demonstrated:

| Adapter | Start | Observe | Stop | Resume | Steer |
| --- | --- | --- | --- | --- | --- |
| Scripted | Yes | Yes | Yes | Yes | Yes |
| Claude Code 2.1.280 | `--session-id`, chosen by the host | stream-json, read as it arrives | Process group ended, acknowledged when empty | `--resume <id>` | `--input-format stream-json` |
| Codex 0.155.1 | `codex exec --json -` | `--json`, read as it arrives | The same | `codex exec resume <id> -` | Refused |

The CLI adapters live in `coder_one::adapter`. They run the CLI under
`supervise::Live`, which keeps the supervisor's process-group ownership:
the host's stop sends `SIGTERM` to the group, then `SIGKILL`, and
acknowledges the stop only once the group is empty. The stream is read as
it arrives, one bounded record per line (4 MiB at most), and a record past
the cap or bytes the host fell behind on become explicit gaps. The stream
file under `artifacts/` grows as the session runs, up to 8 MiB, and keeps
its first and last halves past that.

A policy manifest asks for session control in `executor.session`, and
`Manifest::validate` refuses a rule that needs a capability the adapter
hasn't demonstrated:

```json
"session": {
  "steer": { "when": { "on": "command_failed" }, "message": "Read the failing test first." },
  "stop_when": { "on": "after", "ms": 900000 },
  "resume": "Finish the task."
}
```

`coder-one capabilities` prints the matrix with the tests behind each cell
and writes it to `~/.openagents/coder-one/capabilities.json`.
`--demonstrate` also drives the installed Claude Code and Codex through
each capability against a local model server that answers every call with
a scripted turn. The CLI runs with a scratch home, a dummy credential, and
a cleared environment, so no real credential is read and no inference
runs. The same demonstration runs as a test when you set
`CODER_ONE_REAL_CLI=1`.

## Replay a retained stream

`coder_one::scripted::Script::from_stream` turns a retained native stream
into a script that replays it line by line and rewrites the files its
here-documents and `Write` calls wrote, rebound from `/app` into the scratch
directory. The `exec.scripted` component suite replays two v3 Luna streams
this way.

## Run with a real executor

```sh
coder-one minitask run cancel-cleanup --executor codex --model gpt-6-luna
coder-one minitask run interactive-terminal --executor claude-code
```

The CLI runs inside a `coder-boundary` filesystem boundary that lets it
write only the run's directories, its own state directories (`~/.claude`,
`~/.codex`, `~/.cache`), and the temporary directory. The boundary needs
`bwrap` at `/usr/bin/bwrap` on Linux or `sandbox-exec` on macOS. Where it
can't be enforced, the run refuses to start rather than run unbounded.

## Suite the adapter and the tasks

Two components in the runner cover this work:

```sh
coder-one component suite exec.scripted   # every capability, and two replays
coder-one component suite task.mini       # every task, good and bad scripts
```

`task.mini` passes when each good script passes its grader and each bad
script fails it, and each episode finishes in under 10 seconds.

## Hand off, escalate, and split work

`control.handoff` moves an episode between executors under one budget. A
policy manifest names the pattern in `policy.control.handoff`, and the
manifest's own executor goes first:

| Pattern | What runs | Manifest |
| --- | --- | --- |
| `escalate` | Luna runs under an acting monitor. When two judgments in a row flag a stall or a repeated failure, the host stops Luna and starts Opus from a handoff brief. | `handoff-escalate.json` |
| `planner-worker` | Opus plans in a scratch copy of the task, where nothing it writes counts, and names the scenarios that would show the task done. Luna implements from the plan. | `handoff-planner-worker.json` |
| `steer` | The session keeps running, and the monitor's steer carries the last errors and the open requirements into it. | `handoff-steer.json` |
| `race` | Two executors run at once, each in its own copy of the task's scratch directory. The first to end with `verify.checks` finding no failed scenario wins; the host stops the other and waits for its cleanup before it copies the winner's state back. | `handoff-race.json` |

The handoff brief is built by code, not by a model. It carries each
requirement's state and the failed scenarios' diagnostic packets, from
`verify.checks` run on the workspace as it is; the files added, changed,
or removed since the episode started; and the last failing commands. A
planner's brief carries its plan instead.

Every branch draws from the episode's one deadline: a later branch gets
what the earlier ones left, and a race charges its wall time once and
both branches' cost. The manifest's `max_handoffs` bounds the handoffs.

A policy may use only what its adapters demonstrated, and
`Manifest::validate` refuses the rest:

- A steer needs an adapter that demonstrated steering. Codex hasn't, so
  `handoff-steer.json` runs Claude Code on Haiku 4.5 instead of Codex on
  Luna.
- A race needs `"isolation": "scratch-copy"`. A Terminal-Bench task's
  state lives in its container, where it can't be copied, and no CLI
  adapter has demonstrated two sessions in lockstep, so a race runs only
  with the scripted adapter.
- A Terminal-Bench episode refuses any pattern but `single` in this build.
  Patterns run on mini-tasks.

```sh
coder-one handoff run cancel-cleanup --policy crates/coder-one/policies/handoff-escalate.json
coder-one handoff run log-severity --policy crates/coder-one/policies/handoff-planner-worker.json --executor real
coder-one handoff compare --out bench/terminal-bench/handoff/minitask-patterns.json
coder-one component suite control.handoff
gym coder handoff                  # the patterns as policies
gym coder matrix                   # the outcome matrix, with the patterns below it
```

`--executor scripted`, the default, replaces each executor with a scripted
one of the same model family. A scripted tier plays modeled behavior, not
a measurement: Opus solves each mini-task in about 22 seconds. The cheap
families solve `git-recovery`, write the whole-line parser for
`log-severity` and call it done, and loop on a failing check on
`interactive-terminal` and `cancel-cleanup` until something stops them. A
steer or a plan leads them to the fix. A branch's cost is its episode
seconds times its family's mean spend per second in the retained arms:
$0.000064 for Luna, $0.00244 for Opus, and $0.00196 for Haiku. The
comparison therefore shows how the patterns compose and what they cost
under those assumptions, not how real executors behave.

### Compare patterns with single-pass policies

`coder-one handoff compare` runs six manifests on the four mini-tasks
with a 600-second deadline. The objective uses the outcome matrix's
prices: J runtime is mean cost plus $0.0001 per second, and J offline adds
$1 times the failure rate.

| Policy | Pattern | Passed | Mean cost | Mean time | J runtime | J offline |
| --- | --- | --- | --- | --- | --- | --- |
| Luna, one pass | single | 1/4 | $0.0201 | 313.8 s | 0.0515 | 0.8015 |
| Opus, one pass | single | 4/4 | $0.0537 | 22.0 s | 0.0559 | 0.0559 |
| Luna, then Opus on a stall | escalate | 3/4 | $0.0293 | 49.8 s | 0.0343 | 0.2843 |
| Opus plans, Luna implements | planner-worker | 4/4 | $0.0312 | 42.0 s | 0.0354 | 0.0354 |
| Haiku, steered in place | steer | 3/4 | $0.0955 | 48.8 s | 0.1004 | 0.3504 |
| Luna and Opus race | race | 4/4 | $0.0551 | 22.0 s | 0.0573 | 0.0573 |

A single Luna pass spends the whole deadline on the two tasks where it
loops. Escalation and steering stop the loop at the third failure, but
neither helps on `log-severity`, where Luna never stalls: it confidently
writes the wrong parser. The plan names the severity field, so
planner-worker passes all four at about 58% of Opus's cost and twice its
time. A race matches Opus's time and pass rate, and pays for the Luna
branch it stops.

Two live runs with real executors checked the same machinery on this host,
inside `coder-boundary`:

| Run | Branches | Result |
| --- | --- | --- |
| `escalate` on `cancel-cleanup` | Codex on Luna, 142.5 s, $0.0036 | Passed. The monitor made eight judgments and flagged nothing, so no handoff happened. |
| `planner-worker` on `log-severity` | Claude Code on Opus planning, 21.1 s, $0.1633; then Codex on Luna, 55.5 s, $0.0024 | Passed. The worker's deadline was the 378 s the planner left. |

Real planning cost more than modeled: $0.1633 against the modeled
$0.029 for 12 seconds of Opus. No Terminal-Bench trial ran a pattern.

The episode timeline marks each handoff `⇢` with its pattern, action,
executors, trigger, and brief size. Its record holds the brief's text and
digest, and each mini-task run's manifest holds the ledger's branches.

## See runs in the Gym

To follow a run while it goes, give the scripted executor real time with
`--speed 1` and read it from another terminal with `gym coder live
--follow`, or open the Gym terminal's live view with `f`. The run's log
grows as each event arrives, and `manifest.json` appears only when the run
ends.

The episode timeline shows the executor's normalized events inline, marked
`▸`, between the invocations they happened during. The runbooks view and
`gym coder capabilities` show the capability matrix.

Runs record under `~/.openagents/coder-one/minitasks/`, or `--out DIR`:

```text
minitask-<task>-<executor>-<ms>/
  manifest.json             kind "mini-task": task, executor, outcome, grade
  episode.atif.jsonl        every step and invocation, synced as it happens
  work/                     the scratch directory
  artifacts/                the briefing and the executor's native stream
  verification/grade.json   the grader's verdict
  verification/checks.json  verify.checks' requirement coverage
  verification/support.json verify.support's requirement states, with --jev live
  verification/repair.json  verify.repair's brief, fresh session, and recheck, with --repair
```

```sh
gym coder minitasks
gym coder minitasks --run latest
gym coder minitasks --run latest --json
gym-terminal --terminal-bench        # press 9
```

The Gym labels these runs as mini-tasks, not Terminal-Bench attempts. Each
run shows its grade, its session-control actions, its executor events by
kind, its requirement coverage, and its invocation timeline.
