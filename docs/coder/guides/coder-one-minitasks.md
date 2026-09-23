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
the closing check asks Jev when you pass `--jev live`. The grader runs last.
The exit code is 0 when the grader passed.

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

Each adapter has a capability matrix. The scripted executor demonstrates
all five capabilities. The Claude Code and Codex adapters demonstrate start
only: the host parses their streams after they end, and the supervisor's
deadline ends them without a cleanup acknowledgement.

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

Runs record under `~/.openagents/coder-one/minitasks/`, or `--out DIR`:

```text
minitask-<task>-<executor>-<ms>/
  manifest.json             kind "mini-task": task, executor, outcome, grade
  episode.atif.jsonl        every step and invocation, synced as it happens
  work/                     the scratch directory
  artifacts/                the briefing and the executor's native stream
  verification/grade.json   the grader's verdict
```

```sh
gym coder minitasks
gym coder minitasks --run latest
gym coder minitasks --run latest --json
gym-terminal --terminal-bench        # press 9
```

The Gym labels these runs as mini-tasks, not Terminal-Bench attempts. Each
run shows its grade, its session-control actions, its executor events by
kind, and its invocation timeline.
