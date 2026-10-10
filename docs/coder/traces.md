# Verify-replayed agent traces (#11218)

Every `coder issue-run` ([issue-run.md](issue-run.md)) and every briefed-agent
A/B trial ([briefed-agent-ab.md](../inference/briefed-agent-ab.md)) becomes a
training trace, but only after an independent replay. This is roadmap item
X11 in [the training-system audit](../audits/2026-10-10-training-system-audit/roadmap.md):
the trace, validator replay, digest compare, and accept-or-reject loop from
[the Tassadar revival](../roadmap/2026-09-28-tassadar-revival.md), applied
to coding runs.

Code: `scripts/bench/traces/traces.py`. Tests:
`python3 -m unittest scripts/bench/traces/test_traces.py`.

## Labels come from the diff

An issue-run summary once said a run had changed files that were not in its
worktree. So a trace never takes a label from the agent's reply or from a
summary's account of the change. It takes labels from two things only:

- **The diff**, stored by digest: which files it changes, and the git tree it
  makes when applied to the base commit.
- **The checks the harness ran**: the A/B grader's fields (`applied`,
  `compiles`, `tests_applied`, `tests_compiled`, `tests_pass`, `passed`,
  `failed`) or issue-run's final `check:` commands.

What the run said about itself is kept as `claimed`. When it disagrees with
the diff, the replay receipt notes `claim_mismatch`, and labels still come
from the diff.

## The trace record (`openagents.coder-trace.v1`)

| Field | What it is |
| --- | --- |
| `id`, `source` | `ab:TAG/ISSUE-ARM-REP` or `issue-run:<run-id>`, and the run folder |
| `issue`, `base` | The issue, and the commit the run started from |
| `briefing_digest`, `briefed` | `sha256` of the briefing the agent was given (none for arm A), and its files |
| `diff_digest`, `result_tree` | `sha256` of the diff, and the tree it makes at `base` |
| `files_changed` | Read from the diff's headers |
| `opened_outside_briefing` | From the harness's tool-call accounting, not from the agent's words |
| `checks`, `checks_digest`, `check_spec` | The recorded check results, their digest, and how to run them again |
| `evidence_class` | `recorded` until a replay passes, then `exact_replay` |
| `teacher` | The A/B judge's score. It is a model's opinion, so it is kept only as a teacher field, never as a label |
| `run_id`, `attempt`, `outcome` | Issue-runs: the run's id, its position among the issue's attempts, and its outcome (`passed`, `failed`, `unchecked`, `cancelled`, `setup_failed`, `decision_failed`; `incomplete` for a run killed before its summary; `unknown` for summaries older than #11230) |
| `cost`, `cost_usd` | Issue-runs: each cost component's amount, or `null` with why it is unknown; `cost_usd` is the total only when every component is known, else `null`. Missing cost is never 0 (#11230) |

## Every attempt is kept (#11230)

`capture --issue-runs ROOT` reads every run folder under `ROOT`, not only
those with a diff: a run that stopped in setup or the decision steps, was
cancelled, or was killed before its summary (a `run.json` whose process is
gone) still becomes a trace, unverifiable, with its outcome. A run still
working is skipped until it ends. `capture --issue-run-folders F` captures
single folders (each issue-run calls it on its own folder at the end) and
writes `trace-captured.json` into each, naming the stored diff digest; only
then may a later issue-run clean that run's worktree. `manifest` carries an
`attempts` list with every captured trace, replayed or not, with its
outcome, attempt and cost completeness.

## Replay

`traces.py replay` checks each trace in order and stops at the first
divergence, which the receipt (`openagents.coder-trace-replay.v1`) names:

1. **`diff_digest`.** The stored diff must still have the recorded digest.
2. **`result_tree`.** Applied in a clean index at `base`, the diff must make
   the recorded tree.
3. **`files_changed`.** The files read from the diff must match the trace's
   labels.
4. **`checks.*`.** The same verify checks run again from a clean checkout at
   `base`, and their results must match.

Each receipt ends in one of three verdicts:
- **verified**: verification `passed`, class `exact_replay`.
- **rejected**: verification `failed`, naming the field, the expected value
  and the actual value.
- **unverifiable**: no diff was recorded, or the checks could not run.

Only verified traces are admitted.

Where the checks run (`--on`):
- **`mac`**: a fresh worktree on this computer, with a dedicated
  `CARGO_TARGET_DIR`. Both are deleted afterwards. A build is refused
  (unverifiable) when the disk has less than `TRACES_MIN_FREE_GB` (30) free.
- **A host name**, such as `coderos-4080`: the A/B grader (`~/ab/bin/eval.sh`)
  runs in the bench's second build checkout (`AB_BUILD=v`), which has its own
  target dir, so trial builds are not disturbed. The replay checkout and
  target are separate from the ones that graded the trial.

## Admission and feeds

`traces.py admit` writes `admitted.jsonl`. Each row holds the trace with its
receipt, plus corpus items in the `tenancy::training` `CorpusItem` shape
(`label_source: measurement`, provenance naming the receipt digest):

- one **outcome** item, `accepted` or `rejected` from the replayed checks;
- one **file** item per changed path, `changed`, when the checks passed.

All of an issue's items share the group `issue-N`, and take that group's
partition from the file-relevance-v1 corpus map
(`crates/gym/suites/file-relevance-v1/issues.tsv`, #11215; `--corpus-map`).
A calibration, development or locked group keeps its role; an issue the map
does not hold gets no items. Each row records the map's digest under
`partition` (#11231, LEARN-02). The 41 admitted traces of
`docs/coder/traces/2026-10-10-manifest.json` are issues #10074 and #10228
(calibration) and #10273 (development); none is training.

`filefind.py feedback` reads `~/.openagents/traces/admitted.jsonl` by default.
Only a training-partition trace whose replayed checks passed yields learning
labels; the rest are observations. The issue-run input now reads changed
files from the run's own `change.patch`, never from the summary.

## Hooks

- **`coder issue-run`** writes `change.patch` beside `summary.json`. This is
  the exact diff the summary was computed from. The summary also records
  `base`, `diff_sha256` and `check_commands`. Runs from before this change
  have no diff, so they replay as unverifiable.
- **The A/B harness** (`ab.py`) captures a trace after each trial's
  `result.json`.

The store is `~/.openagents/traces` (`TRACES_STORE`). It holds diffs, so it
stays out of git. `traces.py manifest --out FILE` writes digests and verdicts
only. The first manifest is
[traces/2026-10-10-manifest.json](traces/2026-10-10-manifest.json).

```sh
python3 scripts/bench/traces/traces.py capture \
  --ab scripts/bench/briefed-ab/.work/results --issue-runs ~/.openagents/coder-new/issue-runs
python3 scripts/bench/traces/traces.py replay --on coderos-4080 --issues 10074
python3 scripts/bench/traces/traces.py admit
```
