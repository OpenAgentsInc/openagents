# Run Coder One on a mini-task

A mini-task is a small local task with its own grader. Run an episode on
one with the scripted executor to test how Coder One's components compose,
in about a second and with no model or container. Run the same task with
Claude Code or Codex to see whether a real executor uses what the
components give it.

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
covers it. The grader runs last. The exit code is 0 when the grader passed.

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
