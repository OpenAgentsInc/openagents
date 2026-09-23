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
| `compare [--task ID] [--arm ID]` | Rewards, statuses, denominators, timing, usage, cost source, evidence health, and member identities for comparable groups. |
| `attempt JOB TRIAL` | One attempt's pins, model, reward, status, timing, usage, component costs, call counts, and notes. |
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
```

The commands read local jobs from `~/.openagents/terminal-bench/jobs/`,
retained traces from `bench/terminal-bench/traces/`, and checked samples
from `bench/terminal-bench/samples/`. Use `--jobs-dir`, `--traces-dir`, or
`--samples-dir` to change one source. Use `--no-jobs`, `--no-traces`, or
`--no-samples` to omit one. Nested resilience samples and resumed trials
are included. Read errors stay visible in text and JSON output.

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
