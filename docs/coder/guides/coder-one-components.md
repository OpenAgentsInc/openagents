# Run a Coder One component alone

A Terminal-Bench trial takes about five minutes and answers one question.
To tune one step of Coder One, run that step alone on fixtures instead:
each run takes milliseconds, costs nothing with recorded Jev answers, and
records the same invocation records an episode writes, so the Gym shows
isolated runs and episodes side by side.

The design is in [Coder as a tunable system](../../optimization/coder-components.md#test-each-component-in-isolation).

## The components

| ID | What it decides | Jev |
| --- | --- | --- |
| `evidence.setup` | Which setup commands the task names should run first. The isolated run judges only; it never runs a command. | One request |
| `evidence.probes.planner` | Which typed, read-only operations the host runs before the work. The isolated run plans only; it never runs an operation. | None |
| `evidence.probes.selector` | Which finished probe outputs the briefing carries. | One request |
| `evidence.select` | Which candidate files the survey reads into the briefing. | One request per 20 files |
| `evidence.pack` | The briefing within its character budget, with every omission named. | None |
| `verify.close` | Whether the delegate's report and the changes show the task done. | One request |

Each component is a function from a serializable input to an output and
named metrics. To add one, implement `coder_one::component::Component`
and add it to `registry()`. Its fixture file is `<id>.json` in each
fixture directory.

## Run a component

```sh
coder-one component list
coder-one component run evidence.pack \
  --fixture crates/coder-one/fixtures/components/extended--coder-one-jevprobe3-luna--log-summary-date-ranges
coder-one component suite evidence.probes.selector
coder-one component suite verify.close --json
```

`suite` runs every fixture directory under `crates/coder-one/fixtures/components`
that has an input for the component; `--fixtures DIR` names another root.
Every run records a suite invocation with one child per fixture in an ATIF
session log under `~/.openagents/coder-one/components/`. Use `--out DIR` to
record somewhere else, or `--no-record` to record nothing. The exit code is
1 when a fixture failed.

## Observe without changing the workspace

The host runs only typed operations on its own behalf: list a directory,
read the head of a file, ask Git a read-only question, report the Python
version and packages, clone a repository, or install packages. Each
operation declares its effect class before it runs: `observe`, `write`, or
`install`. No task-derived text reaches a shell. A path argument must
resolve inside the operation's scope, the working directory plus the paths
the task names for reading, and the working directory alone for writing; a
`..` that climbs out, a symbolic link that points out, a credential store,
or an argument that reads as an option is refused. A setup command the task
names becomes a typed clone or install, or a refusal when it uses shell
syntax or an option the host does not pass through.

Git runs with `--no-optional-locks` and `diff.autoRefreshIndex=false`, so a
status or a diff never rewrites the index. The episode collects its changes
the same way: tracked differences with `git diff --binary`, and each
untracked file, within bounds, with `git diff --no-index`, instead of
`git add -N .`. `artifacts/collection.json` names each untracked file that
was left out and why, and the workspace revision before and after the read;
a revision that changed means something wrote during collection.

A test runs every planned probe, every Git query, and change collection on
a fixture repository and checks that a descriptor-level snapshot of the
workspace, `.git` included, is unchanged.

## Choose where Jev's answers come from

`--jev` takes one of three modes:

- `recorded`, the default, replays answers from the fixture's
  `jev-recorded.json`. The key is the digest of the request's state and
  question set, so a change to either misses the cache, and a recorded run
  never passes off an old answer for a new question. A miss reads as
  unknown, and the run reports it under `jev`.
- `live` calls Jev with the key in `TYPESAFE_API_KEY` or
  `~/.openagents/jev.json`. Add `--save-jev` to record the new answers in
  each fixture, so the next recorded run replays them.
- `off` returns unknowns, to test the no-Jev fallback.

A recorded suite reruns byte for byte: the `result` object of
`--json` output holds no times or paths.

## Make fixtures from retained trials

```sh
coder-one component extract \
  --traces bench/terminal-bench/traces --arm coder-one-jevprobe3-luna --out /tmp/fixtures
coder-one component suite evidence.pack --fixtures /tmp/fixtures
```

The extractor reads each retained ATIF trajectory. Every decision call
carries its full request, so each Jev state, question set, and answer comes
back exactly. The delegate call carries the briefing text, which the
extractor reads back into the packer's inputs; the rebuilt briefing's digest
must match the retained one, and `source.json` says whether it does. What a
trajectory doesn't hold stays out:

- A probe output is what Jev read, clipped to 3,000 characters, so the keep
  budget counts at most that much per probe.
- The planner's facts are inferred from the retained battery: a named path
  the battery neither listed nor read reads as missing. Its
  `matches_retained` metric checks that every retained probe command has an
  equivalent typed operation, or was dropped on purpose because another
  operation covers it, as the one-level listing is by the three-level one.
- A file the briefing omitted keeps its name and size, and its content is
  placeholder text of that size.

The checked-in fixtures are eight of the 24 retained v3 Luna trials, one per
task, extracted from `bench/terminal-bench/traces` and scanned for
credentials. A test extracts all 24 and reruns the packer and the probe keep
question on them with recorded Jev.

## See the runs in the Gym

```sh
gym coder components
gym coder components --component evidence.pack --json
gym-terminal --terminal-bench        # press 7
```

The episode timeline (`gym terminal-bench attempt JOB TRIAL --timeline`)
shows every host operation as a `host.operation` invocation with its effect
class, such as `[observe] host.operation · git status`, under the
`evidence.probes.planner` invocation that planned it.

Each component shows its latest isolated suite (Jev mode, fixtures, errors,
latency, Jev cost, and metric summary), each fixture's output digest and
metrics, and its invocations across Terminal-Bench episodes, whether read
from invocation logs or derived from retained trajectories.
