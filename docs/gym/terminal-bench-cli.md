# Use Terminal-Bench from the Gym CLI

Run Terminal-Bench evidence commands from the repository root:

```sh
CARGO_TARGET_DIR=~/.cache/openagents/gym-target \
  cargo run -p gym --bin gym -- terminal-bench overview
```

`gym terminal-bench --help` lists every command. The read-only commands
use the same local evidence reader and comparison identities as the
[Gym TUI](terminal-bench-tui.md). They need no provider credential or Docker
daemon. The examples below use `gym` as the executable name; with Cargo,
replace it with `cargo run -p gym --bin gym --`.

| Command | Result |
| --- | --- |
| `overview` | Sources, status and usage counts, controls, latest time, report warnings, and each task and arm group. |
| `compare [--task ID] [--arm ID]` | Rewards, statuses, denominators, timing, usage, cost source, evidence health, and member identities for comparable groups. Setup time is reported by cache state (`cold`, `warm`, `none`, or `unknown`) beside agent and total time, with setup failures counted beside the graded attempts and each time boundary named. |
| `attempt JOB TRIAL` | One attempt's pins, model, reward, status, timing, usage, component costs, call counts, and notes. |
| `attempt JOB TRIAL --timeline` | The episode timeline: every component invocation in start order with its component, name, parent, duration, outcome, cost, and the spend accumulated so far. |
| `evidence JOB TRIAL` | Each retained path and its digest or resolution state. |
| `evidence --missing` | Every attempt with a missing stream, artifact, or other referenced file, and why each is missing. |
| `history` | Every attempt, newest first, including failures and unknown outcomes. |
| `runbooks` | Paths to the operating and evidence documents. |

Add `--json` to any read-only command for a JSON document with schema
`openagents.gym.terminal-bench-cli.v1`, a `view`, `data`, and `read_errors`.
An unmeasured reward or cost is JSON `null`; a measured zero is `0`.
The comparison output keeps member job and trial identities and shows a
Wilson interval only after three fresh, graded binary attempts with a
complete comparison identity. These are development observations, not
promotion results.

For example:

```sh
gym terminal-bench compare --task terminal-bench/fix-git --json
gym terminal-bench attempt smoke--oracle fix-git__7TEC9XV --json
gym terminal-bench evidence smoke--oracle fix-git__7TEC9XV
gym terminal-bench attempt panel--coder-one-jevprobe3-luna--build-cython-ext \
  build-cython-ext__jFQbtoW --timeline
```

The timeline reads the attempt's `episode.atif.jsonl` when one was retained,
then the invocation events in its trajectory. An attempt recorded before
Coder One wrote invocation events gets a timeline derived from its
trajectory steps, labeled as derived: each entry ends when its answer
arrived. A log without an end record, or an invocation with no end event,
reads as **INCOMPLETE**, and the invocations that never ended are named.
With `--json`, the view is `timeline` and `data` has schema
`openagents.gym.coder-timeline.v1`.

The commands read local jobs from `~/.openagents/terminal-bench/jobs/`,
retained traces from `bench/terminal-bench/traces/`, and checked samples
from `bench/terminal-bench/samples/`. Use `--jobs-dir`, `--traces-dir`, or
`--samples-dir` to change one source. Use `--no-jobs`, `--no-traces`, or
`--no-samples` to omit one. Nested resilience samples and resumed trials
are included. Read errors stay visible in text and JSON output.

## Read a Terminal-Bench 4.0 suite

Attempts of the `tb4` profile ran Terminal-Bench 4.0 at tag `v4.0.0`. Some
of its task names repeat the panel's tasks at another commit, so read them
under their profile:

```sh
gym terminal-bench overview --profile tb4
gym terminal-bench compare --profile tb4 --task terminal-bench/cad-model
gym coder matrix --profile tb4 --reference-rows 5
```

`--profile` keeps only that profile's attempts. The overview lists each
profile's arms with their passes over graded trials, and for `tb4` every
row of the public leaderboard. A `tb4` comparison group carries
`reference`: each leaderboard row's successes, trials, cost, and mean agent
time on that task, best rank first. Text shows the top six; `--json` has
all of them. `gym coder matrix --profile tb4` lists all 66 tasks, marks the
ones no arm has run, and prints leaderboard rows (`▷`) under each task's
cells; its JSON adds `reference` with every row per task. Without
`--profile`, the matrix leaves `tb4` attempts out. `--no-reference` drops
the leaderboard from the overview and comparison. The leaderboard comes
from `bench/terminal-bench/reference/tb4-leaderboard.json`; the
[runbook](../terminal-bench/runbook.md#run-the-terminal-bench-40-suite)
says how to refresh it.

## Compare Coder One components

`gym coder components` lists each Coder One component with its isolated
fixture runs beside its invocations across episodes. Isolated runs come from
the logs `coder-one component suite` records under
`~/.openagents/coder-one/components/`; episodes come from the same local
jobs and retained traces as the commands above.

```sh
gym coder components
gym coder components --component evidence.pack --json
```

For each component, it shows the latest suite's Jev mode, fixture count,
errors, latency per fixture, Jev cost, and metric summary, each fixture's
output digest and metrics, and the episode invocation count, latency, cost,
and whether each came from an invocation log or was derived from a
trajectory. `--runs-dir PATH` reads runs from elsewhere, and `--no-runs`,
`--no-jobs`, or `--no-traces` omits one source. With `--json`, the schema is
`openagents.gym.coder-components.v1`. The
[component guide](../coder/guides/coder-one-components.md) covers the
runner, fixtures, and Jev modes.

## Read Coder One mini-task runs

`gym coder minitasks` lists the runs `coder-one minitask run` records under
`~/.openagents/coder-one/minitasks/`, newest first: task, executor, how the
episode ended, and the grader's verdict. It labels them as mini-tasks, not
Terminal-Bench attempts.

```sh
gym coder minitasks
gym coder minitasks --run latest
gym coder minitasks --task cancel-cleanup --json
```

`--run ID` or `--run latest` shows one run's grade, its session-control
actions, its executor events by kind, and its invocation timeline.
`--runs-dir PATH` reads runs from elsewhere. With `--json`, the schema is
`openagents.gym.coder-minitasks.v1`. The
[mini-task guide](../coder/guides/coder-one-minitasks.md) covers recording
runs.

## Read requirement coverage

`gym coder coverage` lists the requirement coverage Coder One's
`verify.checks` recorded: each retained attempt `coder-one checks recover`
checked, under `~/.openagents/coder-one/checks/`, and each mini-task run
that ran checks. A row shows the verifier reward, the scenario verdicts,
and how many requirements were observed, contradicted, or unverifiable.

```sh
gym coder coverage
gym coder coverage --attempt JOB/TRIAL
gym coder coverage --run latest --json
```

`--attempt` or `--run` shows one report: each requirement with its
scenarios, verdicts, and coverage limits, then each diagnostic packet's
expected relation and hypotheses. `--dir PATH` and `--minitasks-dir PATH`
read from elsewhere. With `--json`, the schema is
`openagents.gym.coder-coverage.v1`. The
[checks guide](../coder/guides/coder-one-checks.md) covers the scenarios.

## Compare Coder One policy manifests

`gym coder policy` reads the policy manifest every Coder One episode
records, and the manifest files in `crates/coder-one/policies/`. Attempts
that record a manifest group in `compare` by its digest, so a group's label
is `policy <digest prefix>` and it lists the arms that ran it.

```sh
gym coder policy list
gym coder policy show coder-one-jevprobe3-luna
gym coder policy diff bdefda51a03c coder-one-jevprobe2-opus-lean-low-5m --json
```

A query is a digest or a prefix of six or more hex digits, a manifest name,
an arm, or a manifest file name. `--policies-dir` adds another manifest
directory, and the source options above apply.

## Run the pinned harness

The commands below call `uv run tbench` in `bench/terminal-bench/`. The
existing harness checks the pinned profile, tasks, agent, artifact, and
credentials. The Gym CLI passes arguments as process arguments without a
shell and returns the harness exit code. It does not print credential
values. Set credentials in the environment as the
[harness runbook](../coder/terminal-bench.md) describes.

```sh
gym terminal-bench doctor
gym terminal-bench tasks list --profile smoke
gym terminal-bench profiles
gym terminal-bench materialize --profile smoke --agent oracle
gym terminal-bench run --profile smoke --agent oracle --task fix-git
gym terminal-bench resume --profile smoke --agent oracle --task fix-git
gym terminal-bench inspect-job smoke--oracle--fix-git
gym terminal-bench collect smoke--oracle--fix-git
gym terminal-bench report
```

`doctor` reads the local environment; `doctor --smoke` also reaches the
registry and network. `tasks checkout` fetches the pinned upstream task
repository. `materialize` writes the job config. `run` and `resume` can
start containers and reach providers. `collect` rewrites attempt records;
`report` writes `tbench-report.json`. Use `--uv PATH` to select an installed
`uv` executable or `--harness-dir PATH` to select the local harness
package. Both options are for harness commands only.

Run controls before agent arms, then inspect the attempt and its evidence.
The [operating runbook](../terminal-bench/runbook.md) covers host setup,
prices, retention, and comparison rules. The
[resilience record](../terminal-bench/resilience.md) shows cancellations,
timeouts, resume, and missing evidence.
