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

## Read recent runs

`gym runs` lists recent Terminal-Bench runs in plain words, newest first,
and `gym runs show` prints one run's summary. Both read what the Runs pane
reads and print the same words.

```sh
gym runs                                  # the 40 newest runs
gym runs --agent coder-one --outcome failed
gym runs --search tb4 --limit 100 --json
gym runs show tb4--coder-one-tunable-v6--coq-block-bound
gym runs show coq-block-bound --transcript           # steps closed
gym runs show coq-block-bound --expand               # every step open
gym runs show coq-block-bound --json
```

`show` takes a job name, `job/trial`, a trial name, a task name, or a piece
of a job name that only one job has. The summary is deterministic: it is
built from the run's records by fixed rules, so the same records always
print the same words, and a missing record prints as missing rather than as
zero. `--jobs-dir PATH` and `--traces-dir PATH` read other directories, and
`--no-jobs` and `--no-traces` skip one.

## Rank runs by what's worth learning from

`gym runs rank` asks Jev, once per finished run, whether the run's records
show something worth reading: low-hanging fruit (a near miss, an output
slip, a check that should have fired), flagrant misbehavior (ignoring the
task, looping, stopping early, claiming success it didn't earn, repairing
correct work, extra rounds for nothing, wasted money, a harness fault),
evidence against a design choice (that the briefing gives the executor what
it needs, that checks catch failures, that effort and persistence help, that
routing picks well, that Jev's judgments are right, that the controller adds
value), or a surprise against the leaderboard and this host's other runs of
the task. Each is its own Noul, and a Score rates the run's overall learning
value. The question set is `runs-learning-v1`.

```sh
gym runs rank                          # ask about every run that has no answer yet
gym runs rank --json                   # the same report as JSON
gym runs --order learning              # the list, most worth learning from first
gym runs --order learning --json       # with every judgment's probability
gym runs show roy-polymorph-cn         # the summary ends with each judgment
gym runs show roy-polymorph-cn --evidence   # the exact state Jev reads
```

The state is built from the run's records by fixed rules: the task and its
instruction, the outcome and the failing tests as the verifier printed
them, the run's summary, Coder One's sessions, checks, support, repair, and
persistence rounds, the transcript's activity, the leaderboard's pass rate
on the task, and this host's other runs of it. It's clipped to 6,000
characters. Answers are kept in `~/.openagents/gym/learning/answers/`, one
file per digest of the state and the question set, with the state itself,
so an answer can always be explained. `index.json` maps a cheap fingerprint
of each run's record files to its answer, so a second `gym runs rank` on
unchanged evidence makes no requests. A run is asked again only when its
records, the other runs of its task, the leaderboard, or the questions
change. Running trials are ranked once they finish.

The order is a fixed rule over the stored probabilities: half Jev's Score,
half the strongest reason, each reason weighed by one minus the share of
ranked runs that give it. A reason nearly every failure gives, such as an
agent reporting success it didn't earn, says less about one run than a
reason few runs give. Changing the rule needs no new request.

`gym runs rank` reads the TypeSafe key from `TYPESAFE_API_KEY` or `api_key`
in `~/.openagents/jev.json` and never prints it. It reports how many runs it
asked about, how many came from the cache, and the cost at Jev's $0.042 per
million input tokens: ranking all 588 finished runs retained on
2026-09-23 took 588 requests and $0.083, about $0.00014 a request.
`--no-jev` asks nothing, `--recorded FILE` replays recorded answers,
`--record FILE` writes the answers it used, `--learning-dir PATH` keeps the
answers elsewhere, `--no-reference` leaves the leaderboard out, and
`--no-tasks` skips reading task definitions.

## Filter and group runs by judgment

The stored answers also answer questions across runs. `--reason ID[=P]`
keeps the runs whose judgment `ID` is at or above `P`, or 0.5 when you
leave `P` out. Repeat it to require several judgments. `gym runs group
--by` counts the runs the filters keep per reason, task, agent, policy, or
outcome. It lists each group's members, strongest first, with each
judgment's mean probability over them. Code computes the groups from the
stored answers, so neither command asks Jev anything.

```sh
gym runs --reason unearned_success --json            # every run Jev judged claimed success it didn't earn
gym runs --reason near_miss=0.8 --agent coder-one    # strong near misses by Coder One
gym runs --reason unearned_success --reason near_miss
gym runs group --by reason                           # how many runs give each reason
gym runs group --by policy --reason output_slip --json
gym runs show wal-recovery-ordering --json           # learning.every_judgment has all 18
```

The judgment IDs are the question set's: `near_miss`, `output_slip`,
`missed_check`, `ignored_task`, `looped`, `stopped_early`,
`unearned_success`, `needless_repair`, `wasted_rounds`, `wasted_money`,
`harness_fault`, `h_briefing`, `h_checks`, `h_effort`, `h_routing`,
`h_jev`, `h_controller`, and `surprise`. A run Jev hasn't judged never meets
a reason. A run is in every reason group whose judgment it meets, at 0.5 or
at the threshold a `--reason` names, so reason groups overlap. A policy is
the agent with its variant or model, such as `Coder One · tunable-v6` or
`Claude Code · Opus 5.5`. `--members N` sets how many members the text
lists per group; `--json` lists them all.

`coder-one ask` reads these views to answer a question about runs with
checked citations; [Ask Coder One about runs](../coder/guides/coder-one-ask.md)
covers it, and `gym coder asks` lists what it answered. `gym coder
proposals` lists the changes an ask proposed, records a person's decision
on each, and shows what measuring it found; see
[Turn a finding into a proposal](../coder/guides/coder-one-ask.md#turn-a-finding-into-a-proposal).

`gym runs show RUN --json` carries every judgment's probability in
`learning.every_judgment`, with its tag, category, and whether it's a
reason, not only the reasons above the threshold. Each transcript step
carries its `step` number, which is what a citation of a step names.

## Mark bad runs and steps

When you read a run and see the agent do something wrong, mark it. A mark
says that a run, or one step of its transcript, is bad. It can carry a
one-line note and the judgment IDs that name what went wrong, so the mark
can say "this is `unearned_success`." When you read a run and find nothing
wrong, clear it. The Runs pane does the same with `x`, `v`, and `u`; see
[Mark runs](terminal-bench-tui.md#mark-runs).

```sh
gym runs mark wal-recovery-ordering --tag unearned_success --note "said the tests passed; two failed"
gym runs mark coq-block-bound/12 --tag looped         # step 12 of the transcript
gym runs mark cancel-async-tasks --clear              # read it; nothing wrong
gym runs unmark coq-block-bound/12
gym runs marks                                        # every mark, newest first
gym runs marks --json
gym runs --marked                                     # only the marked runs
```

`RUN` names a run the way `show` does. `/STEP` counts transcript steps from
1, the way `gym runs show RUN --json` numbers them. `--tag` repeats, and
takes the IDs listed in the previous section. A new mark on the same run or
step replaces the old one, and `unmark` removes it. Only a whole run can be
cleared, and a cleared run takes no tags. `--author NAME` names who marked;
it defaults to `$USER`.

Marks are appended to `~/.openagents/gym/marks/marks.jsonl`, one
`openagents.gym.runs-mark.v1` record per line. Each record names the run,
the step, the verdict, the tags, the note, the author, the time, and the
digest of the evidence Jev reads for the run as it stood when you marked
it: the key its answer is stored under. Each record carries its own digest
and the digest of the line before it. Removing a mark appends an `unmark`
record, so no line is rewritten, and a mark never edits a run's records.
`gym runs marks` reports a line whose digest or chain doesn't hold.
`--marks-dir PATH` keeps the marks elsewhere.

The list prints each mark under its run, `gym runs show RUN` prints a
**Marks** section, and `--transcript` prints a step's mark under the step.
`--json` output carries a `marks` array on each run.

## Measure Jev against the marks

`gym runs agreement` builds a suite from the marks and scores Jev's stored
answers against it. It asks Jev nothing.

```sh
gym runs agreement
gym runs agreement --json      # schema openagents.gym.runs-agreement.v1
```

A tag on a bad mark, on the run or on any of its steps, is a positive label
for that judgment on that run. A cleared run is a negative label for every
judgment. A run marked bad is not a negative for the judgments it wasn't
tagged with, because a person marking one fault hasn't said the others are
absent. Jev says yes when a judgment's probability is 0.5 or above. The
`any_reason` row asks whether Jev gives any reason at all: every run marked
bad is a positive, and every cleared run a negative, tagged or not.

For each judgment, the report gives the positive and negative labels,
agreement, precision, and recall, each as a value, its numerator and
denominator, and a 95% Wilson interval. A share with no denominator is
unknown, never zero. A judgment needs 5 positive and 5 negative labels
whose runs have Jev answers before the text gives its numbers; below that,
the row says **too few labels**, and the report's first line says how many
rows lack them. JSON always carries the counts, with `supported` and
`too_few` on each row.

Jev's answer for a marked run is its answer to the evidence you saw when
you marked it, when the store has one. Otherwise it's the run's current
answer, and the report counts those as `changed_evidence`. Labeled runs
with no answer count as `unjudged` and in no denominator; `gym runs rank`
asks about them.

When a judgment's agreement is low, reword its question. A reworded
question is a new question set with its own digest, and the same marks
measure it: the report names the question set and its digest, so two
reports on different wording are comparable.

## Find highlights worth sharing

The learning order ranks runs by what a person improving the agents should
read. `gym runs highlights` answers a different question: which findings
are worth telling other people, with evidence a stranger can check against
the retained runs. Code computes every claim with fixed rules. No model
writes a number, and nothing posts anywhere: a person picks, edits, and
posts.

```sh
gym runs highlights                        # the 20 strongest claims
gym runs highlights --rule leaderboard     # one rule; repeat for several
gym runs highlights --limit 50 --json      # schema openagents.gym.runs-highlights.v1
```

The rules:

| Rule | A claim when |
| --- | --- |
| `cost` | Two agents' arms both passed a task, and one arm spent at least 2 times as much a passing run on average. The cheapest arm is compared with each other agent's arm that has the most runs. |
| `time` | The same, for the agent's own working time. |
| `leaderboard` | An arm here passed a task that the leaderboard's rows ranked 5 or better pass in at most 20% of their trials, from `bench/terminal-bench/reference/tb4-leaderboard.json`. |
| `shared-failure` | A low-hanging-fruit or misbehavior judgment Jev gave runs of at least 2 agents, 3 runs in all, from the same groups as `gym runs group --by reason`. |
| `surprise` | A run Jev judged `surprise` at 0.70 or above; the 8 strongest. |

The reference solution, the do-nothing control, and runs without a grade
never count. Each claim carries:

- **Its runs**, as `job/trial`, up to 6 per arm.
- **Its numbers**, each with a label, a value, and the text the claim
  writes it as. A draft must use these texts; see
  [Ask Coder One about runs](../coder/guides/coder-one-ask.md#draft-highlights).
- **Its sample size**: the fewest runs any side of the claim rests on. A
  claim that rests on one run is labeled `n=1`, an anecdote rather than a
  benchmark result.
- **Caveats generated from the data**, such as one run on an arm, a cost
  from list prices applied by hand to token counts, the Claude Code CLI's
  own list-price figure for a subscription run, passing runs that report
  no cost, arms that ran in different batches, the cheaper arm failing more
  often, the leaderboard's own infrastructure and fetch date, Jev's
  judgment rather than a person's, and a cited run a person marked bad.

The claims come strongest first. A cost or time claim's strength grows with
the ratio, up to 32 times; a leaderboard claim's is the share of trials the
top rows fail; a shared failure's is its mean probability, weighed by how
many agents share it; and a surprise's is its probability. A claim resting
on one run counts half, and one citing a run a person marked bad counts
half again. Each claim has a stable key, such as `cost-0d4bb2f6`, digested
from the rule and what it compares, so the same claim keeps its key as runs
are added. The source flags, `--learning-dir`, `--marks-dir`, and
`--no-reference` work as they do for `gym runs`. The Runs pane lists the
same claims under `h`; see
[Find highlights](terminal-bench-tui.md#find-highlights).

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

## Run and report a targeted experiment

`gym terminal-bench experiment run|plan|status|stop` forwards to `tbench
experiment`, which compares two or more arms on a few tasks, three
interleaved attempts per task per arm by default, on the long-lived Claude
token, with an optional Claude quota budget. After every graded trial, the
scheduler applies the early-stopping rule described in
[Stop losers early](#stop-losers-early); `--no-stop-early` runs every
planned attempt when selected for a new experiment. Restarts preserve the
experiment's stopping policy.

```sh
gym terminal-bench experiment plan --id v7-vs-cc-0924 --profile tb4 \
  --arm claude-code-opus --arm coder-one-tunable-v7 \
  --tasks risk-scorer-replay,wal-recovery-ordering --quota-usd 150
gym terminal-bench experiment run --id v7-vs-cc-0924 --profile tb4 \
  --arm claude-code-opus --arm coder-one-tunable-v7 \
  --tasks risk-scorer-replay,wal-recovery-ordering --quota-usd 150 --detach
gym terminal-bench experiment report v7-vs-cc-0924
gym terminal-bench experiment report v7-vs-cc-0924 --markdown
gym terminal-bench experiment report path/to/status.json --json
```

`experiment report` reads the experiment's `status.json` and needs no
credential. It prints each arm's passes over graded attempts with a 95%
Wilson interval, and compares every arm with the first one listed: an
exact McNemar test on attempts paired by task and attempt number, and an
exact sign test over tasks, by which arm passed a larger share of its
attempts. It lists attempts lost to credentials, quota, or infrastructure,
which ran again and never enter a denominator, and the Claude quota used
against the budget. `--markdown` fills the results sections of the
[targeted experiment template](../terminal-bench/targeted-experiment-template.md),
and `--json` has schema `openagents.gym.terminal-bench-experiment.v1`.
`--experiments-dir PATH` reads experiments from somewhere other than
`~/.openagents/terminal-bench/experiments/`.

## Read an experiment in flight

`gym experiment pulse` reads a running or finished experiment's
`status.json`, each finished trial's verifier reward and harness attempt
record, and Coder One's `composition.json` where a trial ran Coder One. It
makes no model call.

```sh
gym experiment list                        # every experiment, most recently updated first
gym experiment pulse effort-9569           # one experiment
gym experiment pulse effort-9569 escalate-9571b escalate-9571c v10-persist-9570
gym experiment pulse effort-9569 --json    # schema openagents.gym.experiment-pulse.v1
gym experiment replay effort-9569          # the stopping rule, replayed
```

The pulse shows:

- **Arms.** Each arm's passes over graded attempts with a 95% Wilson
  interval, its mean and total cost per graded attempt from the attempt
  records, its mean time, and the Claude quota it drew, then passes per
  task and arm.
- **The stopping verdict.** The rule in [Stop losers early](#stop-losers-early)
  applied to the trials as they stand, each comparison's exact McNemar
  p-value now and at best for each side, and every stop the scheduler
  recorded in the ledger.
- **Signal discrimination.** How the final checks' verdicts (all passed,
  inconclusive, a check failed), Jev's support answers, and the effort
  score line up with the verifier: passes and fails per row with a Wilson
  interval, a two-sided Fisher exact test of the first row against the
  rest, and, for the score, the area under its ROC curve. A signal that
  doesn't separate says `no separation`.
- **Component fire rates.** For escalation (`verify.second`), repair,
  persistence, and handoff escalation: how many trials configured it, how
  many it fired on, and how those trials ended, with each skip reason and
  persistence's stop reasons grouped with their numbers replaced by `N`,
  and persistence's rounds per trial.
- **Notable trials.** A trial whose final checks all passed and that the
  verifier failed, a check that failed a pass, a kept second candidate, a
  near miss (a failure with at least 80% of the verifier's tests passing),
  a task one arm passed at least twice and another never in at least two
  attempts, and the costliest trial.

With several experiments, the pulse prints each one's standing, then the
component health of all their trials pooled. Pooled on 2026-09-24 over
`effort-9569`, `escalate-9571b`, `escalate-9571c`, and `v10-persist-9570`,
the final checks didn't separate passes from failures:

```text
Combined component health of effort-9569, escalate-9571b, escalate-9571c, v10-persist-9570: 83 graded trials ran Coder One with a composition record
  Final checks against the verifier
    all passed                                    19 pass  19 fail    50%  95% 35–65%
    inconclusive                                  17 pass  19 fail    47%  95% 32–63%  (6 ran no scenario: 6 pass, 0 fail)
    a check failed                                 4 pass   5 fail    44%  95% 19–73%
    "all passed" against the rest: Fisher exact p = 0.827, no separation
  ...
  Escalation (verify.second): configured on 39, fired on 17 (44%)
    ...
    ran 17 times with 3 passes; second executors cost $28.40
    skipped on 19 trials (no check failed and the executor reported no failure), and 8 of those failed anyway
```

Two of those experiments were still running, so the counts grow as trials
finish.

### Ask Jev about finished and running trials

`--jev` runs `gym runs rank`'s `runs-learning-v1` set on each finished
trial that has no answer yet, and groups its 18 judgments by arm. It also
asks three experiment questions (`experiment-pulse-v1`), each only where it
applies: whether escalation changed the candidate (when the second
executor ran), whether the effort level plausibly decided the outcome (when
Coder One recorded an effort score), and whether a failure was a near miss.
The state is the trial's outcome and failing tests, its compact
composition record, and every arm's passes on the task. Answers are kept
under `~/.openagents/gym/pulse/answers.json` by the digest of the state and
questions, so a trial is asked once. The pulse reports what it asked, what
came from the cache, and the cost at Jev's $0.042 per million input
tokens. On 2026-09-24, the experiment questions on `effort-9569` took 27
requests and $0.0011.

`--live` asks Jev, advisory only, about each running trial's live tail
(`experiment-live-v1`): whether the agent is looping, stalled on transport,
or done but still spending. The state is the last 30 executor events, the
time since the last event and the last poll, the current component, and
the spend so far; code adds a note when the log has been quiet past the
stale window or the copy stopped polling. Nothing is stopped by these
answers. A trial without a live tail, such as a plain Claude Code or Codex
arm, is listed and not judged.

Both read the TypeSafe key from `TYPESAFE_API_KEY` or `api_key` in
`~/.openagents/jev.json`. `--recorded FILE` answers from a recorded file,
`--pulse-dir PATH` and `--learning-dir PATH` keep answers elsewhere, and
`--no-reference` leaves the leaderboard out of the runs-learning state.

## Stop losers early

`tbench experiment run` applies an early-stopping rule after every graded
trial. It compares every candidate arm with the baseline, the first arm
that isn't a control (`nop` or `oracle`), on attempts paired by task and
attempt number. A candidate stops when one of these holds, in this order:

1. **Dominated.** Even if every open attempt of the arm passes, it ends
   with fewer passes than another arm has now, and its mean cost per
   graded attempt isn't lower. Both arms must have a whole-trial price
   for every graded attempt. A Claude quota subtotal or an average of only
   the priced trials cannot establish cost dominance.
2. **Decided.** The exact McNemar test is below the significance level
   (`--stop-alpha`, 0.05 by default) and stays below it with the same
   winner even if every open pair goes the other way.
3. **Undecidable.** Even if every open pair went one way, the test
   couldn't get below the level. A design too small to get below it even
   if every planned pair went one way, such as one task with three
   attempts, is exploratory, and this test never stops it.
4. **Below the acceptance bar.** Even if every open attempt passes, the
   arm's pass rate stays under `--accept-pass-rate`. There's no bar unless
   you set one.

A stopped arm's pending trials are skipped; its running trials finish. The
experiment ends when every candidate has stopped. Each stop is appended to
the experiment's `ledger.jsonl` as an `event: stop` record with its scope,
its state, why, the graded trials it read, and the jobs it skipped, and
`status.json` carries the settings and the latest verdict under
`stop_early`. A restart keeps the stops. New experiments pin the enabled
flag, alpha, and acceptance bar in `experiment.json`; changing one requires
a new experiment ID. Choose `--no-stop-early` at creation to run every
planned attempt. Older experiments preserve the settings in their last
status file and seal them on restart. Experiments from before early
stopping, with neither record, keep it disabled. The Claude quota budget
can still change independently.

The pulse leaves mean and total cost unknown when any graded trial lacks
a complete price. It shows the priced subtotal and the number of unknown
trials. JSON keeps that subtotal under `priced_total_cost_usd` and returns
`null` for `mean_cost_usd` and `total_cost_usd`. The subtotal excludes even
the known charges of partially priced trials. Claude quota remains a
separate measure.

The significance rule requires the final paired test to hold even under
the worst remaining outcomes; it does not stop on a significant prefix
alone. The rule makes no correction for comparing several arms.

The rule is code: `crates/gym/src/terminal_bench_stop.rs` for the Gym and
`bench/terminal-bench/tbench/stop_rule.py` for the scheduler, both tested
against `bench/terminal-bench/tests/fixtures/stop-rule/cases.json`.

`gym experiment replay ID` runs a recorded experiment's graded trials back
through the rule in the order they finished, reading a trial only if it
would have started, and says where the rule stops each arm and the
experiment, and which trials that ran to the end would not have started,
with their cost. Replayed on 2026-09-24:

| Experiment | Where the rule stops | Trials and cost saved |
| --- | --- | --- |
| `effort-9569` | `coder-one-tunable-v9` undecidable against `coder-one-tunable-v3` after graded trial 32 of 44; `v2` against `v3` stays open | 4 trials that ran, $10.12, and 3 more that never finished |
| `matched-v8-9567` | `coder-one-matched-v8` undecidable, ending the experiment, after graded trial 50 of 60 | 9 trials that ran, $11.13 |
| `escalate-9571b` | Never: its only other arm is `nop`, a control | none |
