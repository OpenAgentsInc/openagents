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
| `task.profile` | Jev's task features for the router: five Nouls, a difficulty Score, and, with the executors' measured behavior in state, a Choice of executor. | One request |
| `task.requirements` | The requirement map: each span of the instruction as a deliverable, behavior, constraint, check, or context. | One request per 20 spans |
| `evidence.setup` | Which setup commands the task names should run first. The isolated run judges only; it never runs a command. | One request |
| `evidence.probes.planner` | Which typed, read-only operations the host runs before the work. The isolated run plans only; it never runs an operation. | None |
| `evidence.probes.selector` | Which finished probe outputs the briefing carries. | One request |
| `evidence.select` | Which candidate files the survey reads into the briefing. | One request per 20 files |
| `evidence.pack` | The briefing within its character budget, packed by requirement coverage, with every omission named. The suite measures the first packer beside it. | One request per item with Jev coverage judgments; none without |
| `exec.scripted` | How the host starts, observes, steers, stops, and resumes a scripted session. | None |
| `control.monitor` | Whether a running session is making progress, repeating a failed approach, re-reading briefed evidence, or claiming it is done. Shadow mode: it records, never acts. | One request per trigger |
| `exec.system` | The executor's system prompt: the manifest's sections, plus the optional sections the task needs. | One request |
| `verify.close` | Whether the delegate's report and the changes show the task done. | One request |
| `verify.checks` | Which requirements admitted scenarios observe, contradict, or can't verify against the candidate. | None |
| `verify.support` | Whether the evidence supports each requirement, and separately whether it contradicts it; each requirement's state for the candidate revision. | One request per requirement |
| `verify.repair` | Whether one fresh session from the diagnostic packet repairs a contradicted requirement, and what the recheck finds. The suite runs one arm on a preserved mini-task candidate. | None |
| `task.mini` | A whole mini-task episode with the scripted executor, graded. | None |

`exec.scripted` and `task.mini` are covered in
[Run Coder One on a mini-task](coder-one-minitasks.md), `verify.checks` in
[Check claimed behavior with admitted scenarios](coder-one-checks.md), and
`verify.support` in
[Judge requirement support with paired Jev questions](coder-one-support.md),
and `verify.repair` in
[Repair once from a diagnostic packet](coder-one-repair.md).

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

## Extract requirements from the task

`task.requirements` cuts the instruction into identified spans: sentences,
list items, checkbox lines, short lines such as the items of an unmarked
list, code blocks, and blocks of format rows. The spans cover every
non-space character of the instruction, and the map's `coverage` says so.
Code reads each span's exact paths, commands, formats, and constants. Jev
then reads each span in the context of the whole instruction and says
whether it states a deliverable, a behavior, a constraint, a check, or
context, and, for a span that looks like an example, whether the example is
exhaustive.

A span Jev reads as binding (one minus its context probability at 0.5 or
more) becomes a requirement. A span between 0.2 and 0.5 stays a requirement
marked `uncertain`, and a span below 0.2 stays in the map as context. No
instruction text is dropped. Without Jev, a rule places each span and marks
the requirements it keeps `unjudged`. Every requirement starts
`unobserved`.

The episode writes the map to `artifacts/requirements.json`, and the step
and closing checks ask about its requirements instead of only the
instruction's checkbox lines.

The labeled fixtures are `labeled--<task>` for the eight development tasks,
from the instruction each retained episode saw, and two synthetic
mini-tasks, `synthetic--checkbox-issue` and `synthetic--mini-log`. Each
label is text that must appear in the span that states it, and a kind. The
suite reports recall (labels a kept requirement states) and precision (kept
requirements that state a label), over all kept requirements and over the
binding ones alone.

```sh
coder-one component suite task.requirements            # recorded Jev
coder-one component suite task.requirements --jev off  # the rule alone
gym coder requirements log-summary                     # spans, kinds, coverage
```

## Pack the briefing by requirement coverage

The first packer took probe outputs first, then files, each section whole
or not at all, until the 12,000-character cap. Overlapping directory
listings then filled most of a briefing while the log excerpts Jev selected
were dropped, a selected file larger than the cap never went in, and the
directions still called the evidence complete.

The coverage packer (`coder_one::pack`) works from the requirement map:

1. It ranks probe outputs, files, output spans, and commands together, by
   Jev's relevance and by the requirements each one informs. An item
   informs a requirement when it names one of the requirement's exact paths
   or constants, or, with `packer: coverage-jev`, when Jev's coverage
   judgment says so.
2. It removes the lines of a listing that an earlier listing already holds,
   and names a listing that adds nothing as a duplicate.
3. It reserves the task text, gives each requirement's best item and each
   Jev-selected item a first slice (an equal share when the room is
   short), then fills in rank order. A data file's slice is representative
   records: as many opening lines as fit, plus a line for each constant the
   requirements name. An item is trimmed at a line boundary, not dropped
   whole.
4. Each item's heading says whether it is complete or trimmed, how much of
   it is shown, and what it informs; a trimmed or left-out item names how
   to read the rest, such as `sed -n '25,128p' logs/2025-08-10_db.log`.
   The directions' blanket "complete and current" sentence becomes that
   per-item account.

The policy manifest selects the packer with `brief.packer`: `sections` (the
first packer, the default), `coverage`, or `coverage-jev`.
`policies/pack-luna.json` is `jevprobe3-luna.json` with `coverage-jev`, and
the `coder-one-pack-luna` arm runs it. An episode with the coverage packer
writes `artifacts/briefing-pack.json`: the pack record, the Jev coverage
judgments, the requirement map, and the inputs.

`brief.pack` sets the coverage packer's `slice` (each owed item's first
slice, and the fill's step), `item_max` (the most one item delivers), and
`instruction_share` (the most of the cap the task text takes). A manifest
without it keeps the packer's defaults and its digest. Each of the three
has a canary, so a study can search it; the data-file parameters stay the
packer's defaults because no canary reaches a data file.

Replay the packer over every retained briefing:

```sh
coder-one component replay evidence.pack     # --json, --out FILE, --traces DIR
coder-one component suite evidence.pack      # both packers per fixture, with recorded Jev
gym coder briefing                           # totals before and after
gym coder briefing log-summary-date-ranges   # per attempt, and each item's fate
```

The replay reads each retained briefing back into the first packer's
inputs, checks that they rebuild the retained digest, restores each omitted
item's relevance and text from the episode's `state.json`, and packs the
same evidence with the coverage packer. "Needed" evidence recall still needs
independent labels; the replay measures selected against delivered items,
duplicate bytes, and omissions.

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

## Run a study

A study proposes candidate manifests, screens them cheapest first, and
records every candidate, its spend, and whether the winner beats the
baseline on held-out evidence. The records follow
[NIP-OPT](../../../nips/openagents/NIP-OPT.md)'s shapes. The one study
this build defines tunes `evidence.pack`:

```sh
coder-one study run evidence.pack                 # tier 0: replay only
coder-one study run evidence.pack --through mini  # and tier 1: mini-tasks
coder-one study run evidence.pack --through mini --retain
coder-one study list
gym coder study                                   # the latest study
gym coder study evidence-pack --all --json
```

- **Cases**: every retained briefing, read back and restored as the
  replay reads it. Three of the eight tasks, chosen by the SHA-256 of the
  seed and the task name, are held out; each development task's briefings
  alternate between the search and the selection partitions.
- **Objective**: J = ½·Jev-selected items delivered + ½·labeled
  requirements covered by delivered evidence − duplicate listing bytes /
  12,000 − 0.1·briefing characters / 12,000, with tasks weighted equally.
  The labels are the `task.requirements` fixtures' hand-authored ones.
- **Operators**: `swap` (the first packer in place of the coverage
  packer), `grid` (every combination of the budget, slice, span, and
  task-text reserve values), and `climb` (coordinate ascent from the
  baseline) run by default. `random`, `reflect` (text edits a person or a
  model wrote, read from `--reflection FILE`, never called GEPA), and
  `router-refit` (refused until the manifest has a route slot) run when
  `--operators` names them. A proposal becomes a complete manifest,
  validated like any other, and only fields with canaries may change.
- **Tiers**: every candidate replays on the search partition; the best
  third moves to selection, and the best third of those is promoted. With
  `--through mini`, each promoted candidate runs every mini-task's
  known-good script under its briefing policy. The scripted executor
  doesn't read the briefing, so this tier catches only a candidate that
  breaks the episode or overruns its cap. The selection is written to
  `selection.json` before the held-out briefings are read.
- **Confirmation**: the selected candidate and the baseline pack the
  held-out briefings once. The winner beats the baseline only when the
  mean paired difference in J is at least 0.01 and the 2.5th percentile of
  a task-clustered bootstrap is above zero.

The Terminal-Bench tiers (`screen`, `measure`, and `confirm`) are defined
and runnable, but only with `--allow-terminal-bench`, `--artifact`, and
`--artifact-sha256`; without them, the result lists the harness commands
each tier would run. A study records under
`~/.openagents/coder-one/studies/<id>/`, and `--retain` copies all but
`trials.jsonl` to `bench/terminal-bench/studies/`.
[The first study](../../terminal-bench/2026-09-22-pack-study.md) has the
results.

## See the runs in the Gym

```sh
gym coder components
gym coder components --component evidence.pack --json
gym-terminal --terminal-bench        # 7 components, 8 requirements, b briefing, s study
```

The episode timeline (`gym terminal-bench attempt JOB TRIAL --timeline`)
shows every host operation as a `host.operation` invocation with its effect
class, such as `[observe] host.operation · git status`, under the
`evidence.probes.planner` invocation that planned it.

Each component shows its latest isolated suite (Jev mode, fixtures, errors,
latency, Jev cost, and metric summary), each fixture's output digest and
metrics, and its invocations across Terminal-Bench episodes, whether read
from invocation logs or derived from retained trajectories.

## Tune the executor's system prompt

`exec.system` treats each executor's system prompt as a library of
sections. `coder-one prompt list` shows them: Claude Code's and Codex's
defaults split verbatim, each with a status (`keep`, `tune`, `replace`, or
`remove` for a headless run), and the headless library:

- `security`, **protected**: Claude Code's own security policy, byte for
  byte. Every variant must carry it, and manifest validation refuses one
  that doesn't. Codex's default has no security policy, so a Codex variant
  always adds it.
- The **core**: `role`, `security`, `authority` (replaces the rule to
  confirm hard-to-reverse actions), `verify` (replaces Codex's rule against
  running tests unasked), `report`, and `code-style`. It's 1,542
  characters, against 5,453 for Claude Code's default and 18,037 for
  Codex's.
- Six **optional** sections, each with one Jev Noul that selects it:
  `long-builds`, `packages`, `data-parsing`, `git-recovery`,
  `concurrency`, and `services`.

A policy manifest names a variant in `executor.system`:

```json
"system": {
  "mode": "replace",
  "sections": ["role", "security", "authority", "verify", "report", "code-style"],
  "select": ["long-builds", "packages", "data-parsing", "git-recovery", "concurrency", "services"]
}
```

`replace` reaches Claude Code as `--system-prompt-file` and Codex as
`model_instructions_file`; `append` reaches them as
`--append-system-prompt-file` and `developer_instructions`. Before the
dispatch, one Jev request asks about every section in `select`, and each
section at p ≥ 0.5 is added. Without `executor.system`, the executor runs
its own default, and the manifest's digest is unchanged.
`CODER_ONE_SYSTEM=core` or `core-select` sets it from the environment.
`policy.executor.system` is searchable: a canary shows each mode reaching
both executors.

```sh
coder-one prompt show codex core-select --select long-builds
coder-one component suite exec.system            # recorded Jev, 8 fixtures
coder-one prompt capture --out /tmp/prompt-captures
gym coder prompt                                 # every variant
gym coder prompt log-summary-date-ranges__XWSKgz5
gym-terminal --terminal-bench                    # press 0
```

`prompt capture` measures each variant's first request and cache markers
through a local server, with no inference;
[the measurements](../../terminal-bench/delegate-prompts/README.md) are
checked in. The Gym's prompt view shows the selected attempt's sections,
their sizes, the variant digest, and Jev's answers; an attempt that
recorded no variant shows its executor's default, marked inferred. The
Components view compares the variants on the captured first request, pass
rate, mean cost, and delegate turns.

## Route each task

`task.profile` gives the router its input: one Jev request per task asks
five Nouls (builds code, installs packages, parses data, recovers Git
history, concurrency) and a difficulty Score. When the fixture carries each
executor's measured behavior, the same request asks a Choice of executor;
without that behavior, the request refuses the Choice, because task text
alone says nothing about which executor is cheapest and still reliable.
Each fixture's measured behavior leaves its own task out.

The Gym reads the features from an export that a test keeps current:

```sh
coder-one component suite task.profile --no-record \
  --export bench/terminal-bench/profiles/task-features.json
gym coder matrix       # task by policy: Wilson intervals, frontiers, oracles
gym coder router       # features, picks, and leave-one-task-out regret
```

[Route each task](../../terminal-bench/2026-09-22-routing.md) has the
measured matrix, the router's regret against fixed Luna, fixed Opus, and a
hand-written rule, and the frozen larger task pool.

## Watch a session with `control.monitor`

Every Jev request in an episode happens before the executor starts or
after it ends. `control.monitor` watches the session while it runs. At
each trigger it answers four questions, once by rule and, when Jev is on,
once by Jev:

| Question | The rule | Jev's Noul |
| --- | --- | --- |
| Stalled: no progress on an open requirement | Four completed commands since the last artifact change | Is the agent making progress on at least one open requirement? (flags below 0.5) |
| Repeating a failing approach | The last failing command has failed twice since the last change | Is it repeating an approach that already failed? |
| Re-reading briefed evidence | A command reads a path, or reruns a listing, the briefing holds | Is it re-reading evidence the briefing already holds? |
| Claiming completion | A new claim uses a completion word, such as `done` or `tests pass` | Does its latest message claim the task is done? |

A trigger is a completed command, an artifact change, an assistant claim,
or a long silence (two minutes by default), never every event. A Jev
request carries the task, up to 10 open requirements, what the briefing
holds, the last eight events clipped to 300 characters each, and counts
since the last judgment, never the whole transcript, so a request stays
near 4,400 characters however long the session runs.

The monitor runs in **shadow mode**. Each judgment becomes a versioned
proposal, a steer for a flag that asks for intervention or a certify for a
completion claim, that the session's controller records with whether it
would have admitted it, and never acts on. A Jev answer arrives after its
latency, and the judgment is **stale** when the workspace revision or the
process generation moved before it arrived. Each judgment is a
`monitor_judgment` step in the episode log.

Turn it on for a mini-task with `--monitor`, or for an episode with
`policy.control.monitor` in the policy manifest (`{}` takes the defaults;
absent, the manifest's digest is unchanged):

```sh
coder-one minitask run log-severity --script bad --monitor
coder-one component suite control.monitor           # five scripted streams, recorded Jev
coder-one component replay control.monitor          # every retained stream
gym coder monitor                                    # precision, stale answers, and cost
```

### Measure it on scripted and retained streams

The five scripted fixtures, `monitor--scripted-*`, write in their stalls
and loops: a test rerun five times after one edit, reads of briefed files
followed by silence, a clean run, an early completion claim, and a
five-second silence. Each fixture names when each flag becomes true, and a
judgment is labeled by when its trigger arrived. With recorded answers,
the 28 judgments score:

| Question | Labels | Rules: flagged, precision, recall | Jev: flagged, precision, recall |
| --- | --- | --- | --- |
| Any intervention | 12 | 11, 1.00, 0.92 | 23, 0.52, 1.00 |
| Stalled | 9 | 6, 1.00, 0.67 | 23, 0.39, 1.00 |
| Repeating | 5 | 5, 1.00, 1.00 | 6, 0.83, 1.00 |
| Re-reading | 3 | 3, 1.00, 1.00 | 5, 0.60, 1.00 |
| Claims done | 2 | 2, 1.00, 1.00 | 2, 1.00, 1.00 |

The scripts were written beside the rules, so the rules' precision there
is an upper bound. Jev catches the silence the rules miss, and flags a
stall at the first events of a session, before anything could progress.
With the recorded latencies (median 200 ms) and events 100 ms apart, 12
of the 28 answers arrive stale.

The replay feeds each of the 234 retained native streams under
`bench/terminal-bench/traces` through the same monitor one event at a
time, so a judgment sees only the prefix up to its trigger, and labels it
by hindsight: stalled when the attempt failed and nothing changed over at
least two more commands, repeating when the same failing command failed
again before any change, and a completion claim when it was the session's
last. Re-reading is a fact of the prefix, so the rule defines its label.
The streams carry no per-line times, so lines are paced evenly over the
dispatch's duration, and silences never trigger.

The 234 streams give 1,782 triggers: 1,062 completed commands, 612
claims, and 108 artifact changes. The rules alone, over every stream:

| Question | Labels | Flagged | Precision | Recall |
| --- | --- | --- | --- | --- |
| Any intervention | 92 | 566 | 0.12 | 0.76 |
| Stalled | 38 | 533 | 0.04 | 0.55 |
| Repeating | 20 | 18 | 0.17 | 0.15 |
| Claims done | 234 | 112 | 0.83 | 0.40 |

Jev answered 111 of those judgments live, from the first trial of each
development task under the v3 Luna, v2 Luna, and v2 Opus arms, and the
answers are recorded in `bench/terminal-bench/monitor/jev-recorded.json`.
On those 111, Jev's stall flag was never right (22 flagged, 0 correct,
against the rules' 13 flagged and 3 correct); Jev found half the repeated
failures the rules missed (recall 0.50 against 0.00) and every final
completion claim (recall 1.00 against 0.28). None of the 111 answers
arrived stale at the retained streams' pace.

Priced at Jev's published input rate, a judgment at every trigger of
every stream costs $0.086, 0.6% of the $14.05 the executors spent on
those attempts. By that measure the monitor is cheap; whether it repays
itself depends on whether acting on it prevents failures or saves time,
which only a live run measures. Deterministic triggers with the rules
alone cost nothing and over-flag stalls; Jev at the same triggers adds
completion claims and loops but not stalls.

```sh
coder-one component replay control.monitor --out bench/terminal-bench/monitor/replay.json
coder-one component replay control.monitor --jev live --live-limit 40 --save-jev
```

A test replays every stream with the recorded answers and checks that the
result is byte for byte the checked-in report.

### See the monitor in the Gym

The episode and mini-task timelines overlay each judgment, marked `◆`,
between the executor events it followed: its trigger, the rules' flags,
Jev's flags, the proposal it would have made, `STALE` when it arrived
late, and the version it was asked at. The Components view and `gym coder
monitor` report the replay's trigger precision, stale answers, and cost
per question, for the rules alone and beside Jev; `--json` prints the
same as versioned JSON.

