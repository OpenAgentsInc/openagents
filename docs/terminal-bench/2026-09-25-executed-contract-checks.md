# Executed contract checks: the task's own contract, run

2026-09-25. Issue
[#9628](https://github.com/OpenAgentsInc/openagents/issues/9628). This is
the first check kind that the
[2026-09-25 assessment](../coder/design/2026-09-25-assessment.md#1-checks-are-executions-never-opinions)
names: every command, example, path, and expected output that a task
states, extracted by code, run by the host, and compared by code. No model
judges a candidate.

**Completion update:** the [sixteen-candidate supplement](2026-09-25-executed-contract-supplement.md)
finishes the eight-task measurement that originally waited for #9584's labels.
It finds two missing required outputs, two environment-caused command failures,
and three false pass calls. All 16 outcomes reproduce. The component remains
outside runtime policies; the original study and split below remain unchanged.

## Result

**Built, measured, and not useful as a done check or a selector yet.** The
executed contract is honest when it speaks: a difference it finds is a
real difference. But it rarely speaks, and when it does it mostly reports
facts every candidate of a task shares:

- **On the held-out tasks, it made 5 calls on 31 graded candidates.** It
  called 2 failures, both right (95% Wilson interval 34–100%), but one of
  them was right by accident (see [What went wrong](#what-went-wrong)).
  It called 3 passes, all wrong (0–56%). It caught 2 of 28 failures
  (2–23%) and had nothing to say about the other 26 candidates.
- **It never separated a task's passing candidates from its failing ones
  on held-out tasks.** Two held-out tasks have both passes and failures,
  and the extractor found no executable item in either. Mean within-task
  concordance is 0.50, which is chance.
- **On the development tasks, which the rules were written against,** it
  called 12 failures, all right (76–100%), and caught 12 of 59 failures
  (12–32%). Its 26 pass calls were right 4 times (6–34%). Within-task
  concordance was 0.52 over 4 tasks (task bootstrap 0.50–0.54); only
  `sound-change-cascade` showed any variation between candidates.
- **"Matched" doesn't mean pass.** Across both halves, a candidate whose
  output paths all existed passed 2 of 27 times (2–23%), and one whose
  stated commands all succeeded passed 2 of 9 times (6–55%).
- **A fail call was right every time,** 14 of 14 across both halves
  (78–100%), but mostly on tasks where every candidate failed anyway, so
  it says little about which candidate to keep.

This measurement made no Luna session and no Terminal-Bench trial. Jev
settled 6 ambiguous spans in 6 requests for 5,718 input tokens, $0.00024.
The component isn't wired into any policy, and these numbers give no
reason to wire it into one.

## What was built

`checks.contract` is in
[`crates/coder-one/src/checks/contract/`](../../crates/coder-one/src/checks/contract/).

- **Extraction, by code** (`extract.rs`). The instruction is split into
  sentences, list items, and fenced blocks. Each code span is read by its
  form (a path, a command, an identifier) and by the words around it:
  the nearest cue before a path says whether it's an output; words after
  an output path give its format (JSON with its stated top level and the
  shape of a nearby example, a table's columns and whether their order is
  stated, one entry per line and whether sorted, a binary signature); a
  usage line with a placeholder becomes an example run on the task file
  the placeholder names, plus stated exit statuses ("non-zero when the
  argument is missing") and stated time bounds. Files the instruction
  points to (named text files, a named directory's `README`) contribute
  their shell blocks, `$` lines, and `Usage:` lines. An output that the
  untouched workspace already has isn't a check.
- **A narrow Jev Noul** settles only two kinds of span that code finds
  but can't read: whether the task states that a command should succeed,
  and whether a named file holds an example's expected output. The state
  is the task's words only; Jev never sees a candidate or its output.
  Answers are recorded and replayed. With `--jev off`, those items are
  `not_executable`.
- **Running** (`host.rs`). `Local` runs commands through `supervise`
  inside a `coder-boundary` writing boundary on the workspace. `Container`
  runs them with `docker exec` in a networkless container, with the
  client under `supervise`. Files are read first, then commands run, each
  command once however many items read it.
- **Comparing, by code** (`mod.rs`). Each item ends in one typed outcome:
  `matched`, `differed` with a diff bounded to 1,200 characters (and, for
  an expected output, the share of characters that agree), `could_not_run`
  with the reason (command not found, timeout with no stated bound, no
  file to read), or `not_executable` with the reason.
- **Offline replay** (`offline.rs`). `coder-one checks contract offline`
  makes each task's plan in a fresh container of its image, runs it on the
  untouched workspace as a baseline, then restores every retained
  workspace that `accept offline` reads into its own container and runs
  the plan. Rewards go to a separate `labels.json`.

Fixtures and tests are in
[`crates/coder-one/fixtures/contract/`](../../crates/coder-one/fixtures/contract/)
and `contract/tests.rs`: a synthetic task (not a benchmark task) whose
every stated item is checked, and in-memory candidates that exercise each
outcome.

## Protocol

The [protocol](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/protocol.md)
was pushed as `e23004bb5c` before the component existed. It fixed:

- **The population:** every retained graded workspace that `accept
  offline` reads (Coder One snapshots, Microluna finals, retained
  lean-loop candidates, and the v12 reconstruction), excluding all eight
  tasks of #9584's prospective cohort, every trial of them, and every
  `truth-confirmation` or `truth-control` job. Those outcomes are sealed;
  nothing here ran on them.
- **The split, by a hash of the task name:** 12 development tasks and 11
  held-out tasks. Rules were written while reading only development
  instructions, candidates, and rewards.
- **The rules and the analysis:** item outcomes from the component alone;
  a candidate is called failed when any item differed, passed when at
  least one matched and none differed; its score is matched over matched
  plus differed; Wilson intervals, a task-grouped bootstrap (10,000
  resamples, seed 9628), and within-task concordance over every (pass,
  fail) pair.

The extractor, the comparisons, the timeouts, and the Jev question were
frozen in `e37c9a5097`, pushed before any held-out task ran. Three
amendments were made before that freeze and are listed in the protocol:
a second grade directory, six images built from public `environment/`
directories, and treating an unrestorable workspace as an error.

The primary set is the workspaces the verifier graded. Snapshots that
later rounds changed are a secondary set, because their reward belongs to
a different candidate.

One disclosure: while listing the population, before the exclusions were
in place, an inventory script printed per-task pass and fail counts that
included the sealed cohort's snapshots. No per-trial outcome was read,
nothing from it entered this work, and it was reported on #9584.

## Results

### Coverage

The extractor found at least one check on 16 of 23 tasks. On 7 it found
none: `embedding-drift-monitor`, `legacy-utility-triage`, `uefi-bootkit`,
`cad-model`, `heat-pump-warranty`, `html-js-filter`, and
`session-window-debug`. Those instructions describe a fix or a result in
prose and state no command, example, or output path. They hold 81 of the
163 retained workspaces, including 39 of the 45 `embedding-drift-monitor` ones and
every `session-window-debug` one.

What the checks were, by kind, over the 23 plans:

| Kind | Checks | Not executable | Where |
| --- | ---: | ---: | --- |
| Path | 17 | 1 | Output files the instruction names |
| Format | 12 | 0 | JSON shapes, table headers, sorted lines, zip and safetensors signatures |
| Command | 6 | 2 | `make test`, `make repro`, a release build, a Coq compile, a dispatch run, a C++ compile |
| Interface | 3 | 0 | Names a module must provide |
| Exit code | 2 | 0 | Non-zero on a missing argument or file |
| Example | 2 | 1 | An example run whose expected output is a task file, and its stated output length |

### Held out (the claim)

31 graded workspaces with a known reward on 7 tasks, 28 failures and 3
passes; 3 more workspaces have no reward.

| Signal | Result, 95% Wilson interval |
| --- | --- |
| Fail calls that were failures | 2 of 2 (34–100%) |
| Failures caught | 2 of 28 (2–23%) |
| Pass calls that were passes | 0 of 3 (0–56%) |
| No call | 26 of 31 |
| Within-task concordance | 0.50 over 2 tasks, no item on either |

By kind: a missing output path went with a failure 1 of 1 time (21–100%);
existing output paths went with a pass 0 of 2 times (0–66%); a stated
command that differed went with a failure 1 of 1 time, and ones that
succeeded went with a pass 0 of 2 times. The three wrong pass calls were
both `coq-block-bound` finals, whose proof compiles and fails the
verifier, and the `photonic-waveguide-routing` final, whose JSON parses
and fails the verifier.

### Development (fitted, not evidence)

81 graded workspaces with a reward on 10 tasks, 59 failures and 22
passes.

| Kind | Differed, and the verifier failed | Matched, and the verifier passed |
| --- | --- | --- |
| Path | 4 of 4 (51–100%) | 2 of 25 (2–25%) |
| Format | never differed | 2 of 16 (4–36%) |
| Interface | never differed | 0 of 2 (0–66%) |
| Example | 10 of 10 (72–100%) | 0 of 1 (0–79%) |
| Exit code | never differed | 0 of 11 (0–26%) |
| Command | never differed | 2 of 7 (8–64%) |

Candidate calls: 12 of 12 fail calls right (76–100%), 12 of 59 failures
caught (12–32%), 4 of 26 pass calls right (6–34%), 43 with no call.

Within-task discrimination on the 4 development tasks with both labels:

| Task | Passes, failures | Concordance | Why |
| --- | --- | ---: | --- |
| `embedding-drift-monitor` | 17, 22 | 0.50 | No item |
| `legacy-utility-triage` | 1, 2 | 0.50 | No item |
| `mvcc-lsm-compaction` | 2, 5 | 0.50 | Every graded candidate passes `make test` and `make repro` |
| `sound-change-cascade` | 2, 12 | 0.58 | Two failures lack the output files |

The task-bootstrap interval of the mean is 0.50–0.54.

### Secondary: every Coder One snapshot

Snapshots that the verifier didn't grade (a later round changed the
workspace) carry a reward for another candidate. With them, held out: 6 of
7 fail calls right, 6 of 20 failures caught, 5 of 16 pass calls right,
and within-task concordance 0.56 over 4 tasks (0.50–0.63). This set is
reported for completeness; its labels don't describe the checked files.

## What went wrong

- **A truncated command.** `ks-solver-cpp` states its compile line as
  `` `g++ -O3 -std=c++17 -DKS_SOLVER_LIBRARY -I/app` `` with the source
  file in a separate span. The extractor ran the span alone, `g++` said
  "no input files", and the item differed on the untouched workspace and
  every candidate. It produced one of the two held-out fail calls, which
  is right only because every `ks-solver-cpp` candidate failed. The
  baseline run on the untouched workspace exposed it; the frozen rule
  didn't use that baseline.
- **Checks that the untouched workspace already satisfies.** `make -C
  /app test` succeeds before any change on `mvcc-lsm-compaction`, and so
  does the missing-argument exit status on `interleaved-vigenere` (a
  missing script also exits non-zero). They can't tell anything apart.
- **A visible example is satisfiable by fitting it.** One
  `interleaved-vigenere` final reproduces the development sample's
  plaintext exactly and fails the verifier, which uses fresh ciphertexts.
  Nine other finals print the right length and differ from the sample
  (21–37% of characters agree); one has no script. A stated example the candidate can see is a floor, not a
  contract.
- **Necessary, not sufficient.** Every stated command the extractor
  found is a precondition: a build, a reproducer, a compile. Five
  `mvcc-lsm-compaction` Microluna finals pass both `make` targets and fail
  the verifier's hidden cases. The deciding facts that the
  [task anatomy](2026-09-24-task-anatomy.md) calls verifier-only are,
  by construction, not in the instruction.
- **One restore failed.** A `telecom-entity-resolution` final couldn't be
  restored: `docker cp` couldn't read the image's read-only data
  directory. It's listed as an error and left out.

## What this means

The task's own contract, run, is a cheap guard and a poor judge. It
catches a missing or malformed deliverable, and its differences are
trustworthy once each item is checked against the untouched workspace.
It doesn't reach what decides these tasks: on 7 of 23 there's nothing to
run, and where there is, the stated items are preconditions every serious
candidate meets.

Two changes follow, neither measured here:

- **Use the baseline.** An item that already matches on the untouched
  workspace is uninformative, and one that differs on the untouched
  workspace in the same way on every candidate is likely an extraction
  error. Both are visible before any candidate runs.
- **Treat it as a gate, not a verdict.** A missing output or a failing
  stated command is worth a repair before submission; a clean run is not
  evidence of a pass. Independent recomputation and differential tests,
  the assessment's next check kinds, are where discrimination has to come
  from.

## Records and reproduction

Everything is under
[`bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/`](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/):

- `protocol.md`: the frozen population, split, rules, analysis, and
  amendments.
- `records/<task>/plan.json`: each frozen plan with its digest and Jev
  answers; `contract.json`: every workspace's items, outcomes, diffs,
  call, and score, and the untouched baseline; `labels.json`: the
  verifier's rewards, kept apart.
- `records/jev-recorded.json`: the six recorded Jev answers.
- `records/summary.json`, from `measure.py`: every table above, with
  Wilson intervals, task-bootstrap intervals, per-task concordance, and
  per-task coverage.
- `records/replay.txt`: the output of the replay below, run at
  `e37c9a5097`: all 163 workspaces on 23 tasks gave identical item
  outcomes, calls, and scores.

To rerun the measurement with no model call:

```sh
cargo build -p coder-one --bin coder-one
python3 bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/replay.py \
  --coder-one target/debug/coder-one --scratch /tmp/contract-replay
python3 bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/measure.py \
  bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/records
```

`replay.py` reuses each frozen plan, replays Jev from the recorded
answers, restores every retained workspace from
`~/.openagents/terminal-bench/jobs` into its own networkless container,
and compares every item outcome, call, and score with the retained
records. It needs the task images the protocol lists.

To make a plan for a new task inside its container, or to run a plan
there:

```sh
coder-one checks contract plan --instruction instruction.md --workdir /app --out plan.json
coder-one checks contract run --plan plan.json
```
