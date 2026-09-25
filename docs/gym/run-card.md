# Characterize a run with its card

`gym runs characterize` computes one trial's run card from its retained
records: one Markdown page and one JSON record. The card is what the
[reconstruction of the three v13 trials](../terminal-bench/2026-09-25-microluna-v13-embedding-trials.md)
worked out by hand over a day, computed in under a second. Part two of the
[Microluna v18 design](../coder/design/microluna-v18.md) specifies it, and
issue [#9639](https://github.com/OpenAgentsInc/openagents/issues/9639)
holds the work.

Every number is arithmetic over retained files. A number whose record is
missing is `unknown`, and a section whose record no policy writes yet is
`not recorded`. The card never estimates a number.

## Run it

```sh
gym runs characterize embedding-drift-monitor__6zRjd9n
gym runs characterize RUN --json               # the JSON record
gym runs characterize RUN --out DIR            # write JOB--TRIAL.card.md and .card.json
gym runs characterize --all                    # every Coder One run, one line each
gym runs characterize diff RUN_A RUN_B         # compare two cards on one task
gym runs characterize diff a.card.json b.card.json --changed
```

`RUN` is a job name, `job/trial`, a trial name, or a piece of a job name
that only one job has, as for `gym runs show`. `--all` exits 1 when any run
can't be characterized. `--jobs-dir PATH` and `--traces-dir PATH` read
other directories, `--no-jobs` and `--no-traces` skip one, and
`--sites PATH` reads another defect-site record.

The card also shows in three other places:

- `gym runs show RUN` prints a short `Run card` section, and
  `gym runs show RUN --json` carries the whole record under `card`.
- The Runs pane in `gym-terminal` shows the card under `C`.
- `coder-one ask` puts each opened run's known card rows in its briefing,
  and an answer cites them in `card_rows`. Code checks that each cited row
  exists on that run's card with a known value.

## What it reads

The trial directory: Harbor's result, the attempt record, the episode log,
each Microluna session log, the lean loop's `selection.json` and retained
candidate workspaces, the briefing pack, and the verifier's records. The
task's checkout is never read, so the untouched source is only what the
first session's briefing carried under `## The current PATH` or what a
session read in full before its first edit to that file.

Beside the trial it reads three task-level records:

- the experiments' `bench/terminal-bench/experiments/*/records/pins.json`,
  for whether the task is in the policy's development set;
- `bench/terminal-bench/reference/defect-sites.json`, the defect sites a
  written task analysis found, when the task has any;
- the public-attempt manifest, for the reference trajectory.

Jev isn't asked. A session command the phase rules can't place takes a
cached answer from the fingerprint store when one exists and stays unplaced
otherwise; `--no-jev` skips the store. Whether an edit addresses a suspect
whose line it didn't touch is never asked.

## What the card holds

**Identity.** Task and revision, policy name and digest, binary, arm,
attempt, reward and verifier tests, cost, trial time, agent time, and
whether the task is in the policy's development set, from the first pins
file that names the policy.

**Phase timeline.** Environment setup, agent setup, host before session 1,
each session, host after each session, close, agent exit, gap to verifier,
and verifier, each with its start from the trial's start, its duration, and
its share of trial time. A session runs from its log's `session` record to
its `end` record. Host time after the last session ends when the delegation
that ran the sessions returns or the closing check starts, whichever is
first. Close runs from there to the end of the episode, and agent exit from
the episode's end to the end of Harbor's agent phase.

**Session anatomy.** Per session: role, turns, calls, model latency and its
share, command time and its share, tool overhead (time neither the model
nor a tool accounts for), tokens with the cached share, and cost. Moments
count from the session's start and sit at the model turn that made the
call: first read, first command, first edit, last edit, and finish. An edit
is any change to a file inside the session's repository, whether by an
edit tool or by a shell command the phase rules name as a write. The
verification tail runs from the last edit's turn to the finish turn, with
the turns after the last edit and the program runs among them. An edit
round is a run of edits with no program run between them, and it counts as
checked when a program run follows it.

**Evidence provenance.** Each suspect from the briefing's likely-defects
list with whether a session edited its file and whether the submitted
workspace changed its line; each edited file with the briefing's file item
for it and the suspects that named it. The numbers:

| Row | Meaning |
| --- | --- |
| `provenance.suspect_line_hits` | Suspects whose line the submitted workspace changed, over suspects with a known answer |
| `provenance.suspect_file_hits` | Suspects that named an edited file, over suspects |
| `provenance.pointer_coverage` | Edited files a suspect named, over edited files |
| `provenance.briefing_coverage` | Edited files any briefing item named, over edited files |
| `provenance.defect_sites_named` | The task's defect sites a suspect named, over sites |
| `provenance.defect_sites_edited` | The task's defect sites a session edited, over sites |

**Check lineage.** The session-written check's path from the briefing, each
version (the session, the turn, how it was written, whether a code edit or a
failing score came before it), each version's score on the untouched
workspace when it ran before any edit, every score run inside a session,
and the host's score, snapshot, and hard-coding answer after each session.
Check-line grades come from the grades record.

**Executed evidence.** Every host operation from the episode log with its
exit code and output digest, the host-executed command records, and the
session steps by phase with where the phases came from.

**Waste.** Turns lost to a program the shell couldn't find, refused tool
calls with their cause, reads of files the session's own briefing carried
in full, turns with no call, program runs while the session's last score
was full, and time in commands over 5 seconds, by first line.

**Reversals.** Files whose digest after a session returned to an earlier
session's digest, from the host's snapshots; patch-level reversals from
[the run analysis](run-analysis.md). No record keeps per-edit digests inside
a session, so that row is `not recorded`.

**Review delta.** For each self-check or audit session, the files whose
digest changed from the snapshot before it, and for each file whether the
change touched code or only docstrings and comments, from the two retained
workspaces. Scores before and after, and the host-executed records of the
two candidates.

**Claims against outcomes.** Each session's finish status and summary, the
verifier's reward and tests, the host's final score and whether a full
score agrees with the reward, Jev's `verify.close` probability when the
policy asked it, and each graded check line beside the reward.

**Against the reference.** The cheapest passing public trajectory on the
task, with its cost, time, and steps, and, when its body is on this
computer, its first edit, edit rounds, and phase sequence from the
[fingerprint code](head-to-head.md).

## Rows

Every number is a row with a stable ID, such as `session.1.model_share`,
`phase.verifier.s`, or `waste.not_found.python`. The JSON record carries
the rows beside the typed sections; each row has `id`, `label`, `value` (a
number, a pair `[hits, total]`, a string, a Boolean, or `null`), and
`text`, the value as the page prints it. `null` always comes with a `text`
that says why: `unknown`, `not recorded`, or a reason. The record's schema
is `openagents.gym.run-card.v1` and its rules version `run-card-v1`.

`diff` lists every row of either card with both values and `B − A` for
numbers; for a pair it compares the ratios. Two cards on different tasks
are refused.

## Records the card reads that other components write

The card reads two records that the v18 components write. Both live in the
episode's `artifacts/` directory or a lean group's directory under it, such
as `artifacts/lean-1/`. When neither exists, the rows that depend on them
say `not recorded`.

### Host-executed commands

`artifacts/executed-commands.jsonl`, written by `evidence.baseline`
([#9633](https://github.com/OpenAgentsInc/openagents/issues/9633)) and
`verify.executed`
([#9636](https://github.com/OpenAgentsInc/openagents/issues/9636)). One
JSON object a line; lines with another schema are skipped.

```json
{
  "schema": "openagents.coder-one.executed-command.v1",
  "at": 1790289480000,
  "stage": "baseline",
  "session": null,
  "candidate": null,
  "kind": "module",
  "command": "python3 -m drift_monitor data/reference_embeddings.npy data/current_stable.npy",
  "cwd": "/app",
  "exit": 0,
  "timed_out": false,
  "ms": 3382,
  "stdout_digest": "sha256 hex",
  "stderr_digest": "sha256 hex",
  "stdout_head": "the first 16 KiB, or less",
  "stderr_head": "the first 16 KiB, or less",
  "requirements": ["R3"],
  "verdict": "ok",
  "rule": "exited 0 on the untouched workspace and on the candidate"
}
```

| Field | Meaning |
| --- | --- |
| `at` | When the command started, in milliseconds since the epoch |
| `stage` | `baseline` before session 1, `after_session` after a session, or `probe` |
| `session` | The session whose candidate ran, for `after_session`; `null` otherwise |
| `candidate` | The candidate's retained directory under `artifacts/`, such as `lean-1/session-1`, or `null` |
| `kind` | `named` (the instruction names it), `module`, `make`, `script`, `compile`, or `score` |
| `exit` | The exit code, or `null` when it didn't finish |
| `timed_out` | Whether the bound stopped it |
| `requirements` | The requirement IDs it bears on, when known; may be empty |
| `verdict` | `ok`, `regressed`, `not_a_regression`, `unknown`, or `null` before a verdict applies |
| `rule` | Why the verdict holds, in words; may be `null` |

The review delta pairs records by `session`: the records of the session
before a review and of the review itself.

`evidence.baseline` writes its records to `artifacts/lean-<n>/` and adds
five fields the card doesn't read: `stated` (the instruction's command when
the host ran it differently, such as `python3` for a stated `python`),
`failed` (why it never ran), `confine` (`boundary` or `container`), and
`stdout_bytes` and `stderr_bytes` (what each stream produced, before the
16 KiB cap). Its digests are of the kept heads.
[`crates/coder-one/src/baseline/`](../../crates/coder-one/src/baseline/)
writes them, and `baseline::read_commands` reads its commands back.

### Check-line grades

`artifacts/check-grades.json`, written at the freeze by `accept.grade`
([#9635](https://github.com/OpenAgentsInc/openagents/issues/9635)).

```json
{
  "schema": "openagents.coder-one.check-grades.v1",
  "check": "lean-1/evaluator/score.sh",
  "check_digest": "sha256 hex",
  "frozen_after_session": 1,
  "split": "lines",
  "lines": [
    {
      "id": "c8",
      "line": 41,
      "text": "check(mmd(x, x) == 0 and mmd(x, far) >= 0)",
      "grade": "advisory",
      "basis": null,
      "p": 0.12,
      "jev": {"how": "live", "error": null},
      "results": [{"session": 1, "passed": false}, {"session": 2, "passed": true}]
    }
  ]
}
```

| Field | Meaning |
| --- | --- |
| `check` | The frozen check's path under `artifacts/` |
| `split` | `lines`, or `one_unit` when the script didn't split and was graded whole |
| `lines[].grade` | `follows`, `advisory`, or `unknown` |
| `lines[].basis` | `task`, `baseline`, or `standard` for a line that follows; `null` otherwise |
| `lines[].p` | Jev's probability that the expectation follows |
| `lines[].jev.how` | `live`, `recorded`, or `skipped` |
| `lines[].results` | Optional: whether the line passed on each session's candidate, in order |

The card counts lines and advisory lines, and beside the reward it puts
whether each line's last result agrees: a passing line agrees with reward 1
and a failing line with reward below 1.

### Defect sites

`bench/terminal-bench/reference/defect-sites.json` is task-level and
written by hand from a task analysis. It names files, never verifier test
names, and no policy reads it.

```json
{
  "schema": "openagents.gym.defect-sites.v1",
  "tasks": {
    "embedding-drift-monitor": {
      "source": "where the sites come from",
      "sites": [{"id": "bug1", "file": "drift_monitor/normalize.py", "what": "zero-vector normalization"}]
    }
  }
}
```

## Verify the card

The acceptance test reproduces the published v13 reconstruction from the
three retained trials:

```sh
cargo test -p gym --lib runs_card
```

[The reproduction record](../terminal-bench/2026-09-25-run-card-v13-reproduction.md)
lists each number beside the page's, and the three places the page and
the records disagree.
