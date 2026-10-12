# Briefer arms: decision doors vs the constrained Haiku agent (#11389)

Status, 2026-10-11: one measured comparison of the Briefer's **judgments**
(plan `ba1`). Downstream pass rate is not measured yet (see "What is not
measured"). Jev is unmeasured: TypeSafe refused every call for credit.

## The split

The Briefer does two things: **judgments** (which candidate files the fix
needs; what kind of change the issue asks for) and the **generative** part
(the analysis text the Coder reads). A Haiku Briefer session does both, so
its cost is not comparable to a door's batch of Nouls. This measurement
takes the judgments alone: every arm sees the same candidate files for an
issue, answers keep/drop per file, and is scored against the fix commit's
own files. The analysis text is written afterward either way (the Haiku
briefer's `likely_change`, or the Briefer hand in the solver).

## Setup (frozen: `scripts/bench/briefed-ab/plans/briefer-arms.json`)

- **Issues:** the 21 S2 issues of the Haiku briefer's recall table
  (`docs/inference/haiku-briefer.md`), chosen before any arm ran.
- **Candidates:** `filefind query --no-cards --k 12` at the fix's parent
  (deterministic stages, Vertex embeddings), cached per issue so every arm
  reads the identical list.
- **Door arms** ask `crates/coder-new/src/issue_run/decide.rs`'s exact
  questions: a Noul per candidate, "Does the fix for this issue need to
  change this file?", over the issue (first 4000 characters) and the file's
  first 80 lines at the base; and the issue-kind Choice (five options).
  Keep at p >= 0.35, at least 3 kept (decide.rs `KEEP_AT`, `MIN_FILES`).
  - `clef`: Clef-Flash on this Mac's Ollama (MLX).
  - `hosted`: the OpenAgents decision API (`openagents.com/api`); every
    answer this run came from `pylon:coderos-4080-clef`, i.e. Clef-Flash
    Q4_K_M GGUF on CUDA. Same model as `clef`, another runtime.
  - `jev`: TypeSafe `jev-latest`. **HTTP 402 on every call** ("no available
    TypeSafe API credits"), so 0 of 21 answered.
- **Haiku arm:** `haiku_briefer.ask` with the same candidates and its
  default levers (claude-haiku-5-5, 14 turns, 120 s, $0.40 cap, read-only
  tools). A candidate it names with role change or test is keep; the rest
  are drop. Files it names outside the candidates count toward fix recall.
- One rep. Labels: `task.json` source_files + test_files.

## Results

All 21 issues (Haiku answered 15: the other 6 hit the subscription's
session limit, recorded as failures, not zeros):

| Arm | Issues answered | Judgments | Accuracy | Precision (keep) | Recall in candidates | Fix recall | Top 1 is a fix file | Kept (mean) | $ per issue | Seconds p50 / p95 |
|---|---|---|---|---|---|---|---|---|---|---|
| clef | 21/21 | 245 | 0.735 | 0.288 | 0.543 | 0.334 | 0.381 | 3.14 | 0 (local) | 11.75 / 13.23 |
| hosted | 21/21 | 252 | 0.766 | 0.338 | 0.681 | 0.429 | 0.571 | 3.24 | unpriced | 11.73 / 37.77 |
| jev | 0/21 | 0 | - | - | - | - | - | - | - | - |
| haiku | 15/21 | 180 | **0.889** | **0.600** | **0.841** | **0.749** | **0.933** | 2.33 | 0.0062 | 29.74 / 50.53 |

Like for like, the 15 issues every answering arm answered
(`briefer_arms.py report --common`):

| Arm | Judgments | Accuracy | Precision (keep) | Recall in candidates | Fix recall | Top 1 is a fix file | $ per issue | Seconds p50 / p95 |
|---|---|---|---|---|---|---|---|---|
| clef | 176 | 0.727 | 0.277 | 0.521 | 0.343 | 0.467 | 0 (local) | 11.66 / 16.29 |
| hosted | 180 | 0.756 | 0.327 | 0.636 | 0.410 | 0.533 | unpriced | 11.38 / 40.09 |
| haiku | 180 | **0.889** | **0.600** | **0.841** | **0.749** | **0.933** | 0.0062 | 29.74 / 50.53 |

Per-judgment agreement (same issue, same file):

| Pair | Judgments | Agree |
|---|---|---|
| clef ~ hosted | 245 | 0.894 |
| clef ~ haiku | 176 | 0.727 |
| hosted ~ haiku | 180 | 0.733 |

Issue kind (doors only; Haiku's answer has no kind field): clef and hosted
agree on 20 of 21; the one divergence is #10893 (clef `docs`, hosted `bug`).

Every disagreement is kept, not resolved: 122 rows in
`scripts/bench/briefed-ab/results/briefer-arms-ba1/summary.json`
(`divergences`: issue, path, each arm's decision and probability, and
whether the fix changed the file). Per-judgment rows:
`results/briefer-arms-ba1/judgments.csv`.

### What the door numbers say

The doors rank better than they decide. Over all candidates, the mean
probability for a fix file is 0.29 (clef) / 0.35 (hosted) against 0.15 /
0.14 for the others (AUC 0.70 / 0.78), but at the 0.35 threshold only 13 /
10 of the 37 fix files clear the bar, so the keep set is mostly the
`MIN_FILES` top-up. The door's view is the file's first 80 lines; Haiku
greps and reads around the matches. Same weights on two runtimes (MLX vs
GGUF Q4 CUDA) disagree on 11% of files.

### Noise

One rep. The doors are deterministic per state; the Haiku agent is not, and
its rerun spread is unmeasured. With about 180 judgments per arm on the
like-for-like set, an accuracy gap under about 0.06 would be inside the
noise; the measured gap (0.889 vs 0.756) is about twice that. Haiku's 6
lost issues were not chosen by difficulty (they were the last in its run
order when the session limit hit).

### Haiku rerun of the limited issues (2026-10-12)

The 6 issues Haiku lost to the session limit were asked again over the same
frozen candidate lists (rebuilt from `judgments.csv` by
`briefer_arms_restore.py`; a fresh filefind run differs by one or two files
on most issues, so the lists were not regenerated). 5 of 6 answered; #10181
ran out of turns (`error_max_turns`, 14 turns), a real failure, not a limit.
The rerun stopped at the limit again after those, so the other 15 keep
their ba1 rows. Rows: `results/briefer-arms-ba1/haiku-rerun-2026-10-12/`.

On the 5 newly answered issues (#10167, #10179, #10201, #10228, #10248):

| Arm | Judgments | Accuracy | Precision (keep) | Recall in candidates | Fix recall | Top 1 is a fix file | $ per issue | Seconds p50 |
|---|---|---|---|---|---|---|---|---|
| clef | 57 | 0.754 | 0.375 | 0.600 | 0.373 | 0.200 | 0 (local) | 11.98 |
| hosted | 60 | 0.800 | 0.438 | 0.800 | 0.573 | 0.800 | unpriced | 16.90 |
| haiku | 60 | **0.883** | **0.714** | 0.633 | **0.723** | 0.800 | 0.0055 | 29.53 |

Agreement: clef ~ hosted 0.93, hosted ~ haiku 0.78, clef ~ haiku 0.70.
With these added, Haiku answered 20 of 21 issues; pooled over its 240
judgments its accuracy is 0.888 (213/240), the same as the 15-issue figure,
against clef 0.735 and hosted 0.766 over all 21. One repeat (#10074, asked
in both runs) gave the same keep/drop on all 12 candidates: a first sign
that the Haiku agent's judgment spread is small, one issue is not a
measurement of it. On this set the hosted door's top-1 matched Haiku's
(0.8 each) and its recall inside the candidates was higher (0.80 vs 0.63):
Haiku keeps fewer files (precise, but it misses some candidate fix files
that it makes up for with files outside the list). The decision stands.

## Decision

- **Default arm for the Briefer's file judgments: Haiku** (constrained,
  with filefind's candidates), set from this measurement: +0.13 judgment
  accuracy, +0.34 fix recall and 0.93 vs 0.53 top-1 over the best door, for
  $0.006 and about 18 s more per issue.
- **The door arms stay behind the flag:** the bench lever `finder=clef-arm`
  (`hosted-arm`, `jev-arm`) and ab.py arms `BC` / `BJ`; the solver's
  `--decisions relevance=clef|jev` (#11387).
- **Re-measure when:** Jev has credit (`--arms jev`), or the door question
  changes (more of the file than its head, or a threshold set from
  calibration rather than 0.35).

## What is not measured

- **Downstream pass rate.** The acceptance's "briefing quality" needs agent
  trials: an `ab.py batch` plan with arms `B0`, `BH`, `BC` over this issue
  set (cloud dev environment, as `plans/haiku-briefer.json`). The lever and
  arms exist; no trial ran here.
- **Jev.** 402 from TypeSafe on every call.
- **SIZE and checks.** The doors have no SIZE or checks question in
  decide.rs today; only file relevance and issue kind are compared.

## Reproduce

```sh
cd scripts/bench/briefed-ab
export FILEFIND_EMBEDDINGS=vertex GOOGLE_APPLICATION_CREDENTIALS=...   # candidates
python3 briefer_arms.py run --plan plans/briefer-arms.json             # all arms; skips written rows, retries usage-limited ones
python3 briefer_arms.py run --plan plans/briefer-arms.json --arms haiku   # finish Haiku's 6 after the limit resets
python3 briefer_arms.py report --plan plans/briefer-arms.json --md [--common]
python3 -m unittest test_briefer_arms.py
```
