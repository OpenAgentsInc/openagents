# Run the tunable composition on Terminal-Bench

Each Coder One component runs alone on fixtures, in the Gym, and in a
mini-task episode. A Terminal-Bench episode runs them together when its
policy manifest turns them on: `control.route` picks the first executor,
`control.monitor` watches it, `verify.checks` and `verify.support` judge
what it left, `control.handoff` escalates or splits the work, and
`verify.repair` runs once, all within one deadline sized from the task's
own timeout.

The design is in
[Coder as a tunable system](../../optimization/coder-components.md).

## What an episode runs

```text
task.requirements   the requirement map, as before
control.route       task.profile's features pick the first executor
evidence.*          probes, the survey, and the briefing, as before
control.handoff     planner-worker only: a plan from a scratch copy
exec.session        the first executor, under control.monitor
verify.checks       scenarios on the live workspace
verify.support      Jev's paired judgments per requirement
control.handoff     escalate: the second executor from a handoff brief
verify.repair       one fresh session from the diagnostic packets
```

An episode runs this path when its manifest sets any of `control.route`,
`control.handoff` (escalate or planner-worker), `control.horizon`, or
`verify`. A manifest without them runs one executor, as before, and keeps
its digest. Steer and race still run only on mini-tasks: steer needs an
adapter that demonstrated steering, and race needs two isolated copies of
the task's state.

### Route the start

`control.route` asks task.profile's fixed battery (five Nouls and a
difficulty Score) about the instruction, then applies the `profile-v1`
rule:

| Start | When |
| --- | --- |
| `strong` | The episode deadline is at least `long_after_sec`: the task's own timeout says it's long. |
| `strong` | Jev returned no difficulty, so the profile can't be read. |
| `strong` | The difficulty is at least `hard_at`. |
| `strong` | A feature in `hard_features` is at least `feature_at`. |
| `cheap` | Otherwise. |

The route writes a `control.route` invocation with the profile, the rule,
and the reason, and the start is the executor every later dispatch builds
on. An escalation to the executor that already started isn't one, so a
strong start skips it and leaves the repair.

### Check any task

The data, interactive, and cancellation scenarios need a task of their
family. The generic scenarios read only what any task states:

| Scenario | Expected relation |
| --- | --- |
| `generic.output` | Each output file a requirement asks for exists and isn't empty. |
| `generic.parse` | A JSON, JSONL, CSV, Python, or shell output parses as its format; a CSV starts with the header the task shows. |
| `generic.public-command` | A test command the instruction names exits 0. |
| `generic.claimed-command` | A test command the executor ran and saw pass still passes on the final state. |

A command runs only when it reads as a test or a check (`pytest`,
`python3 -m pytest`, `make test`, `cargo test`, `go test`, `npm test`, or a
script named `test*`, `check*`, `verify*`, or `run_tests*`), names no
placeholder, installs nothing, reaches no network, and writes nothing
through a redirect. It runs in the task's working directory within the
horizon's per-command bound, without the episode's credentials. The
protected verifier's tests are never looked for, and a command that names
`/tests` is refused.

### Escalate or split

With `control.handoff` set to `escalate`, the first executor runs under
an acting monitor: after `after` judgments in a row flag one of `on`, the
host stops it. The episode escalates to `to` when any of these holds:

- The monitor stopped a stalled session.
- A check failed.
- `verify.support` judged a requirement contradicted.
- The session ended without an answer.

The second executor reads the first briefing plus a handoff brief built by
code: each requirement's state, up to three diagnostic packets, the
requirements judged contradicted, the last failing commands, and the
workspace's Git changes. It runs under a shadow monitor, and the checks
and support run again on what it left.

With `planner-worker`, the planner runs first in a scratch copy of the
workspace, where nothing it writes counts. Its plan joins the first
executor's directions. A workspace over 20,000 files or 256 MiB isn't
copied, and the planner is skipped with that reason.

### Size the work from the timeout

`control.horizon` sizes each dispatch from what is left of the episode
deadline:

| Dispatch | Asks for |
| --- | --- |
| The first, when an escalation can follow | `first_share` of the time left |
| The first or the escalation, when only a repair can follow | `later_share` of the time left |
| The last | All of the time left |

Each asks for at least `min_dispatch_sec` and never more than is left.
The checks get `check_share` of the whole deadline (between 3 and 30
minutes), and each command they run gets one sixtieth of it (between one
and 15 minutes). A deadline at least `long_after_sec` is a long task:
every executor runs at `long_effort`, and Claude Code's shell commands may
run up to `long_command_sec` (`BASH_MAX_TIMEOUT_MS`) instead of its
ten-minute cap.

Without a deadline, each dispatch asks for `executor.deadline_sec`, as
before.

The Harbor adapter reads the trial's `lock.json` and the task's
`task.toml` for Harbor's agent timeout, runs the episode process 60
seconds inside it, and the episode keeps its deadline 60 seconds inside
that. An eight-hour Terminal-Bench 4.0 task gets an episode deadline of
28,680 seconds. A session that runs for hours isn't cut by the host: no
turn cap is passed to either CLI, the supervisor's wall is the granted
deadline plus five seconds, the stream file keeps its first and last 4
MiB while every line is still read for the summary, and every dispatch,
the planner, the escalation, and the repair included, is a delegate call
in `evaluation/usage.json`.

## The reference manifests and arms

| Manifest | Arm | What it runs |
| --- | --- | --- |
| `crates/coder-one/policies/tunable.json` | `coder-one-tunable` | Routed start (Luna or lean Opus), checks, support, escalation to lean Opus, and one repair. |
| `crates/coder-one/policies/tunable-opus.json` | `coder-one-tunable-opus` | Lean Opus 5.5 at low effort, checks, and one repair. |
| `crates/coder-one/policies/tunable-luna.json` | `coder-one-tunable-luna` | Luna, checks, escalation to lean Opus, and one repair. |

All three run deep Jev with the v2 probes, the headless core system prompt
with its protected sections, the five-minute Claude Code prompt cache, and
a monitor that asks Jev at each trigger. The arms use the
`tbench.coder_one:CoderOneTunable` adapter, which installs Claude Code
2.1.280 and Codex 0.155.1 from prebuilt layers in every arm, places the
Codex `auth.json`, and forwards the Claude token by name.

Run the smoke trial:

```sh
export OPENAGENTS_API_KEY="$(tr -d '\n' < ~/.openagents/bearer)"
export TYPESAFE_API_KEY="$(jq -r .api_key ~/.openagents/jev.json)"
export CLAUDE_CODE_OAUTH_TOKEN="$(jq -r .claudeAiOauth.accessToken ~/.claude/.credentials.json)"
export CODEX_FORCE_AUTH_JSON=1
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN OPENAI_API_KEY
cd bench/terminal-bench
uv run tbench run --profile smoke --task fix-git \
  --agent coder-one-tunable --auth-mode subscription-oauth \
  --agent-kwarg artifact_path="$artifact_path" \
  --agent-kwarg artifact_sha256="$artifact_sha256"
uv run tbench retain smoke--coder-one-tunable--fix-git
```

The first smoke trial, on 2026-09-23 with artifact `6c7379c18c35`
(`smoke--coder-one-tunable--fix-git`, retained under
`bench/terminal-bench/traces/`), passed with reward 1.0. The route started
strong: task.profile read `fix-git` at difficulty 0.77. Lean Opus answered
in 15 seconds of the 560 it asked for, no scenario applied, support left
the one requirement unresolved, and nothing escalated or repaired. It cost
$0.0698: $0.0689 for Claude Code at its list price on a subscription
token, and $0.0009 for 13 Jev requests. Agent time was 19.6 seconds, and
the whole trial 63 seconds.

## See it in the Gym

The episode writes `artifacts/composition.json`
(`openagents.coder-one.composition.v1`): the route and its profile, the
horizon, each dispatch with its tier, status, time, requested deadline,
and cost, each handoff and its trigger, the checks after each dispatch,
support, and the repair. The bundle's manifest names it, and its
`verification` block carries the checks, support, and repair.

```sh
gym coder composition                # one row per composed attempt, then the newest in detail
gym coder composition fix-git        # the detail of each attempt on fix-git
gym coder composition --json         # openagents.gym.coder-composition.v1
```

In `gym-terminal`, the attempt view lists the same detail under the
attempt's calls.

## Test it offline

The composition runs on Terminal-Bench-shaped tasks with scripted
executors, in virtual time, with no model or network:

```sh
cargo test -p coder-one compose::
```

The tests cover a cheap start that escalates to Opus when the task's own
test fails, a strong start repaired once from the packet, an eight-hour
deadline that starts strong at the long effort with hours to work, and a
planner whose writes stay in its scratch copy.
