# Microcoder against Antigravity on Gemini

**Round 2 (2026-11-12, below): still not proven.** Microcoder met every
condition but the first: it passed 22 of 28 runs against Antigravity's 24.
It cost $14.63 against $24.76 and took a median 68 s against 161 s. On the
fresh hold-out tasks H5–H8 it matched Antigravity's passes (6 of 8 each),
cost less ($4.24 against $6.92) and finished faster (median 67 s against
166 s). Round 1 follows the round 2 section.

Written 2026-11-11. GitHub #13231. Related: #13193, #13227,
[the Antigravity task-fit experiments](2026-11-11-antigravity-task-fit.md).

The owner's goal: make Microcoder work well on Gemini, bench it against
Antigravity (`agy`) and beat it, using only Microcoder's own small tool set.
Both harnesses run the same Gemini models on Vertex AI (project
`openagentsgemini`, location `us`), so this compares harness with harness.

- **Microcoder:** OpenAgents' own loop (`coax/lib/microcoder`): one structured
  model call a step, then the step's edits, shell commands and file views.
  Run by Coder: `coder run --engine microcoder --model M --effort low`.
- **Antigravity:** Google's `agy` CLI with about 60 tools, driven by Coder
  through ACP: `coder run --engine antigravity --model M-low`.

## The verdict, defined before the final runs

"Provably better" means: on the hold-out tasks plus T1–T6, with the same model,
thinking level and location, and at least two runs per task per harness,
Microcoder:

1. passes at least as many runs as Antigravity, and
2. has lower total cost and lower median wall time,

and the difference in cost and wall time holds on the hold-out tasks alone.

The final runs are at Argon low in `us`. A second setting (Flash low or
Carbon low) runs if the budget allows. With two runs a task, a difference of
one or two passes is within noise; the report gives counts, not rates.

## Tasks

- **T1–T6:** the six tasks of the task-fit experiments, each judged by its
  commit's own checks (`tasks.md` there): a rename, a bug fix with a failing
  check, a feature in one unit, a reading task, a five-file change, and a doc
  edit.
- **H1–H4 (hold-out):** built from this repository's history by a separate
  agent the same way (the parent of a real commit, its intent as the prompt,
  its own checks as the judge): a bug fix, a feature in one unit, a
  three-file change and a reading task. Nobody tuning Microcoder looked at
  them before the final runs.


## Round 2

The coordinator asked for a second round with $40 more: fix the cache churn
as one change measured on its own, explain T3, build fresh hold-out tasks
H5–H8, and judge the same verdict with one Microcoder build for every one of
its runs. H4 is no longer a hold-out: its traces were studied after round 1.
The hold-out-only conditions are judged on H5–H8.

### The change: compaction into an append-only `earlier` (78ab78f450)

- **How it works:** each step is written twice as it runs. One copy is
  whole. The other is compacted: its edits, its commands with exit codes and
  the first and last 160 bytes of each output, and the files it viewed, by
  size. When the recent steps would fill their 144 KB, all but the last
  24 KB of them move into `earlier` in one go.
- **Why the prefix stays cached:** the facts, task and route come first and
  never change. `earlier` only grows at its end. The recent steps only grow at
  theirs until the next compaction. So the prompt through `earlier` never
  changes except at its end. A 30-step check tests exactly that.
- **How it fits Vertex's cache:** the cache needs a prefix of at least 4,096
  tokens. On Gemini 4, it caches the whole previous request only when the new
  one adds at least 2,050 tokens; Google's docs give no block size. A
  compaction frees 72 KB or more, which is several steps' growth, so the
  cached prefix breaks rarely, never every step.
- **What it doesn't change:** `AF_REQUEST` and every other `#authority` file
  are untouched.

Measured on H4 and T5, the two long runs, against the build before
(ee6c2215d0 and 941212f68f, two runs each):

| Task | Build | Runs | Passed | Cost $ | Wall s | Calls | Input tokens | Cached |
| --- | --- | ---: | ---: | --- | --- | --- | --- | --- |
| H4 | before | 2 | 2 | 3.04, 4.47 | 251, 321 | 21, 32 | 42,620 a call | 21% |
| H4 | compaction | 3 | 1 | 1.26, 1.09, 1.62 | 162, 126, 201 | 15, 11, 16 | 29,700 a call | 35% |
| T5 | before | 2 | 2 | 1.00, 2.38 | 192, 347 | 8, 20 | 38,265 a call | 34% |
| T5 | compaction | 3 | 3 | 0.38, 0.95, 0.43 | 101, 130, 85 | 5, 11, 4 | 28,700 a call | 39% |

- **Cost and time:** H4's cost fell by two thirds and T5's by more than half.
  Both got faster.
- **Cache:** the cached share rose less than expected, from 21% to 35% on H4.
  Vertex's implicit cache still misses on many calls whose prefix is
  unchanged, and that is outside Microcoder's control.
- **Passes:** H4 passed once in three runs, against twice in two before. Each
  failure missed one of the five required facts. A passing run also had two
  compactions, so these runs don't show whether compaction lost the facts.

### T3: why the warning never said "warn"

- **What happened:** in both failed runs (round 1, ee6c2215d0 and
  941212f68f), Microcoder edited `wasm-size.coax`, ran the task's own check
  (`coax/bin/coax test coax/web/wasm-size.coax`, which passed) and read
  `git diff`. Then it finished.
- **Why the check failed:** for the warning line, it reused the existing
  "over its budget" message, which never says it is a warning. The file's own
  tests check the numbers, not the words.
- **What it didn't do:** it never ran `wasm-size budget` itself to see what
  the command prints. No tool result hid any output.
- **Verdict:** the model's judgment, not a general fault in the loop. No
  change was made.
- **At the final build:** T3 passed once and failed once.

### H5–H8

A separate agent built them the same way as H1–H4, before any tuning:

- H5: reading and explaining.
- H6: a bug fix with a failing check.
- H7: a small feature in one unit.
- H8: a two-file change.

Their traces weren't opened until every verdict run had ended. In H8 both
harnesses failed both runs, and the same way:

- the hidden test passed;
- a structural part of the check didn't find the two constants it expects
  (`ENV_GLADE_DAY_ID`, `ENV_GLADE_DAY_KEYS`).

Four runs from two harnesses failing in the same place suggests that the
check asks for more than the prompt says. It counts against both harnesses
alike. It wasn't changed after the runs.

### The verdict runs

- **Microcoder:** every run at 78ab78f450, two runs of each of H1–H8 and
  T1–T6.
- **Antigravity:**
  - the 20 runs of round 1 for H1–H4 and T1–T6;
  - two new runs of each of H5–H8 at 78ab78f450, whose Antigravity engine is
    the same as round 1's (no change under `coax/lib/agent`, `coder/tui` or
    the gateway since ee6c2215d0).
- **Setup otherwise as round 1:** Argon low, `us`.
- **Warm-up:** H5–H7's start trees have no published compiler, so each of
  those runs first bootstrapped one, for 3 to 6 minutes. That time came before
  the timed run, but it loaded the machine while other runs were going.

| Task | Harness | Runs | Passed | Wall s (each) | Cost $ (each) | Calls | s/call | Input tok/call | Cached % |
| --- | --- | ---: | ---: | --- | --- | --- | ---: | ---: | ---: |
| H1 | mc-argon | 2 | 2 | 48, 67 | 0.21, 0.22 | 4, 4 | 14.4 | 10,877 | 0 |
| H1 | agy-argon | 2 | 2 | 139, 84 | 0.42, 0.34 | 11, 9 | 11.1 | 24,284 | 71 |
| H2 | mc-argon | 2 | 2 | 88, 59 | 0.46, 0.34 | 7, 5 | 12.2 | 23,619 | 41 |
| H2 | agy-argon | 2 | 2 | 246, 134 | 0.94, 0.67 | 26, 16 | 9.1 | 41,919 | 85 |
| H3 | mc-argon | 2 | 2 | 191, 180 | 0.71, 1.11 | 11, 14 | 14.8 | 26,590 | 43 |
| H3 | agy-argon | 2 | 2 | 194, 191 | 1.28, 0.86 | 16, 17 | 11.7 | 67,743 | 82 |
| H4 | mc-argon | 2 | 0 | 126, 201 | 1.09, 1.62 | 11, 16 | 12.1 | 34,012 | 34 |
| H4 | agy-argon | 2 | 1 | 203, 275 | 1.94, 1.40 | 20, 16 | 13.3 | 96,330 | 83 |
| H5 | mc-argon | 2 | 2 | 68, 69 | 0.28, 0.23 | 5, 6 | 12.4 | 15,117 | 42 |
| H5 | agy-argon | 2 | 2 | 63, 55 | 0.53, 0.67 | 4, 4 | 14.7 | 53,786 | 40 |
| H6 | mc-argon | 2 | 2 | 337, 676 | 1.04, 1.63 | 23, 29 | 19.5 | 29,217 | 67 |
| H6 | agy-argon | 2 | 2 | 184, 159 | 1.08, 1.16 | 12, 17 | 11.8 | 80,786 | 82 |
| H7 | mc-argon | 2 | 2 | 39, 45 | 0.15, 0.16 | 3, 3 | 14.0 | 9,089 | 0 |
| H7 | agy-argon | 2 | 2 | 238, 288 | 0.78, 0.95 | 20, 22 | 12.5 | 35,098 | 84 |
| H8 | mc-argon | 2 | 0 | 66, 66 | 0.37, 0.38 | 3, 3 | 22.0 | 24,325 | 0 |
| H8 | agy-argon | 2 | 0 | 172, 154 | 1.15, 0.60 | 10, 6 | 20.4 | 77,276 | 74 |
| T1 | mc-argon | 2 | 2 | 19, 30 | 0.12, 0.12 | 3, 3 | 8.2 | 8,810 | 0 |
| T1 | agy-argon | 2 | 2 | 36, 23 | 0.15, 0.23 | 4, 3 | 8.5 | 23,801 | 48 |
| T2 | mc-argon | 2 | 2 | 86, 103 | 0.51, 0.51 | 7, 9 | 11.8 | 31,751 | 59 |
| T2 | agy-argon | 2 | 2 | 74, 182 | 0.71, 1.28 | 10, 11 | 12.2 | 68,981 | 73 |
| T3 | mc-argon | 2 | 1 | 44, 81 | 0.21, 0.36 | 4, 5 | 14.0 | 17,540 | 30 |
| T3 | agy-argon | 2 | 2 | 71, 78 | 0.45, 0.48 | 7, 9 | 9.3 | 35,014 | 67 |
| T4 | mc-argon | 2 | 1 | 81, 57 | 0.64, 0.63 | 5, 6 | 12.5 | 26,920 | 0 |
| T4 | agy-argon | 2 | 2 | 165, 164 | 1.30, 1.23 | 8, 8 | 20.5 | 110,973 | 72 |
| T5 | mc-argon | 2 | 2 | 130, 85 | 0.95, 0.43 | 11, 4 | 14.3 | 29,928 | 37 |
| T5 | agy-argon | 2 | 2 | 249, 284 | 2.42, 1.49 | 22, 16 | 14.0 | 88,748 | 77 |
| T6 | mc-argon | 2 | 2 | 24, 37 | 0.08, 0.08 | 3, 3 | 10.0 | 5,217 | 0 |
| T6 | agy-argon | 2 | 1 | 31, 27 | 0.09, 0.17 | 3, 4 | 8.3 | 14,858 | 45 |

| Tasks | Harness | Runs | Passed | Total cost $ | Median wall s | Calls | s/call | Input tok/call | Output tok/call |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| all | mc-argon | 28 | 22 | 14.63 | 68 | 210 | 14.8 | 25,403 | 499 |
| all | agy-argon | 28 | 24 | 24.76 | 161 | 331 | 12.6 | 63,023 | 466 |
| hold | mc-argon | 16 | 12 | 9.99 | 68 | 147 | 15.8 | 26,120 | 505 |
| hold | agy-argon | 16 | 13 | 14.76 | 178 | 226 | 12.3 | 59,439 | 461 |
| seen | mc-argon | 12 | 10 | 4.64 | 69 | 63 | 12.3 | 23,732 | 485 |
| seen | agy-argon | 12 | 11 | 10.01 | 76 | 105 | 13.2 | 70,738 | 476 |

| Condition | Microcoder | Antigravity | Met |
| --- | --- | --- | --- |
| Passes, all 14 tasks (28 runs each) | 22 | 24 | **no** |
| Total cost, all tasks | $14.63 | $24.76 | yes |
| Median wall time, all tasks | 68 s | 161 s | yes |
| Passes, H5–H8 (8 runs each) | 6 | 6 | yes |
| Total cost, H5–H8 | $4.24 | $6.92 | yes |
| Median wall time, H5–H8 | 67 s | 166 s | yes |
| Total cost, H1–H8 | $9.99 | $14.76 | yes |
| Median wall time, H1–H8 | 68 s | 178 s | yes |

H1–H8 together: Microcoder passed 12 of 16 runs, Antigravity 13.

**Verdict: not proven.** Every condition holds but the first: Microcoder
passed 22 runs to Antigravity's 24. The runs that made the difference:

- H4: Microcoder 0 of 2, Antigravity 1 of 2. A fact was missing.
- T3: 1 of 2 against 2 of 2. The warning didn't say "warn".
- T4: 1 of 2 against 2 of 2. In one run Microcoder wrote a 22-word
  EXPLAIN.md and gave the full answer in its reply instead.

T6 went Microcoder's way (2 against 1). With two runs a task, a two-run
difference is within noise: with the same build, H4 went 1 of 3 and T3 and
T4 each 1 of 2.

### What blocks it, and the next lever

- **The blocker is passes, not cost or time.** The misses are the model
  leaving things out of answers it writes: a fact in a reading task, a whole
  answer kept in its reply, a word in a message. They aren't tool or loop
  faults found in the traces.
- **The next lever:** a general check before `finished`. The loop would show
  the model the task's numbered requirements beside its final diff and new
  files (`git diff` shows no untracked file such as EXPLAIN.md). That is a
  better observation inside the one loop, not a new component.
  - It needs one more run of all 28 Microcoder runs, about $15. Round 2 has
    spent $23.19 of its $40. Measuring the change on its own as well wouldn't
    fit what is left, so round 2 stops here.

### Round 2 spend and gates

- **Spend:** $23.19 of the $40, for 39 runs (Microcoder 31, Antigravity 8).
  Both rounds together: $68.84.
- **Gates at 78ab78f450:** `coax test` passes for every `coax/lib/microcoder`
  unit, `lab.coax`, `engine.coax` (274/274), `run.coax`,
  `coder/tui/mcworker.coax` and `coder/tui/coder.coax` (673/673).
  `tools_smoke.sh` and `tools-check` (0 new without a reason) pass too.

## Round 1

### Verdict

**Not proven.** Microcoder passed as many runs as Antigravity (18 of 20 each),
cost less in total ($16.29 against $17.84) and finished faster (median 71 s
against 151 s). On the hold-out tasks alone it was still faster (median 84 s
against 192 s) but cost more ($9.51 against $7.83). Condition 3 fails on cost,
and all of the gap is H4, the reading task: Microcoder spent $3.04 and $4.47 on
it, Antigravity $1.94 and $1.40. These are two runs a task, so a single run
can move any of these sums.

| Condition | Microcoder | Antigravity | Met |
| --- | --- | --- | --- |
| Passes, all 10 tasks (20 runs each) | 18 | 18 | yes |
| Total cost, all tasks | $16.29 | $17.84 | yes |
| Median wall time, all tasks | 71 s | 151 s | yes |
| Total cost, hold-out only | $9.51 | $7.83 | **no** |
| Median wall time, hold-out only | 84 s | 192 s | yes |

Where the runs differed:

- **Passes.** Microcoder failed T3 both times: its warning reused the
  existing "over its budget" line, which doesn't say it's a warning, and the
  check asks for the word. Antigravity failed T6 once ("one commit per issue"
  broken across a line, which the check's regex doesn't match) and H4 once.
- **Calls.** Microcoder used 164 model calls against 236. Its prompts were
  half the size (31K input tokens a call against 65K), but they were cached
  far less often (see "Cost").

### Setup

- **Models and location:** `gemini-4-argon` at thinking level low, location
  `us` (`aiplatform.us.rep.googleapis.com`), project `openagentsgemini`, the
  service account that `~/.openagents/coder/antigravity.env` names. Both
  harnesses used the same credentials.
- **Harness:** the task-fit experiments' scratch harness, copied. For each
  run: a fresh worktree at the task's start commit with its own compiler,
  one `coder run ... --stream-json` with a 15-minute limit, then the task's
  check (no model). Each line of Coder's stream was timestamped as it arrived.
  Up to four runs ran at once.
- **Builds:**
  - Microcoder's counted runs: the first run of each task was Coder at
    ee6c2215d0 and the second at 941212f68f. 941212f68f fixes a crash and
    shrinks the step log from 192 KB to 160 KB (see below).
  - Antigravity: 15 runs at ee6c2215d0 and 5 (T2-T6, second run) at
    941212f68f. The Antigravity engine is the same in both: neither commit
    touches `coax/lib/agent`, `coder/tui` or the gateway. Both include
    2620f8fe9c (low thinking by default, the stall watchdog) and e7e07ba706
    (a cancelled part's usage is counted).
- **Cost:**
  - Microcoder's cost is the rates card's price for each attempt's usage
    (`usd_micros`). Antigravity's is its usage lines, priced the same way.
    Both use Vertex on-demand rates for Argon: $4.00 per million input
    tokens, $0.20 cached, $20.00 output (thinking included).
  - Antigravity's own safety-monitor calls to Flash-Lite are not counted, as
    in the task-fit doc.
- **Columns:**
  - "s/call" is wall time divided by model calls. It includes the time tools
    took.
  - Antigravity's "calls" are the `streamGenerateContent` lines in its ACP
    log.

### Per-task results (Argon low, us, two runs a task)

| Task | Harness | Runs | Passed | Wall s (each) | Cost $ (each) | Calls | s/call | Input tok/call | Cached % |
| --- | --- | ---: | ---: | --- | --- | --- | ---: | ---: | ---: |
| H1 | mc-argon | 2 | 2 | 54, 46 | 0.22, 0.21 | 4, 4 | 12.6 | 10,852 | 0 |
| H1 | agy-argon | 2 | 2 | 139, 84 | 0.42, 0.34 | 11, 9 | 11.1 | 24,284 | 71 |
| H2 | mc-argon | 2 | 2 | 61, 80 | 0.28, 0.25 | 6, 5 | 12.8 | 14,197 | 35 |
| H2 | agy-argon | 2 | 2 | 246, 134 | 0.94, 0.67 | 26, 16 | 9.1 | 41,919 | 85 |
| H3 | mc-argon | 2 | 2 | 88, 107 | 0.48, 0.55 | 7, 7 | 13.9 | 28,356 | 46 |
| H3 | agy-argon | 2 | 2 | 194, 191 | 1.28, 0.86 | 16, 17 | 11.7 | 67,743 | 82 |
| H4 | mc-argon | 2 | 2 | 251, 321 | 3.04, 4.47 | 21, 32 | 10.8 | 42,620 | 21 |
| H4 | agy-argon | 2 | 1 | 203, 275 | 1.94, 1.40 | 20, 16 | 13.3 | 96,330 | 83 |
| T1 | mc-argon | 2 | 2 | 20, 20 | 0.12, 0.12 | 3, 3 | 6.7 | 8,769 | 0 |
| T1 | agy-argon | 2 | 2 | 36, 23 | 0.15, 0.23 | 4, 3 | 8.5 | 23,801 | 48 |
| T2 | mc-argon | 2 | 2 | 104, 44 | 1.00, 0.26 | 11, 4 | 9.9 | 38,440 | 53 |
| T2 | agy-argon | 2 | 2 | 74, 182 | 0.71, 1.28 | 10, 11 | 12.2 | 68,981 | 73 |
| T3 | mc-argon | 2 | 0 | 54, 50 | 0.20, 0.30 | 4, 4 | 13.0 | 13,110 | 0 |
| T3 | agy-argon | 2 | 2 | 71, 78 | 0.45, 0.48 | 7, 9 | 9.3 | 35,014 | 67 |
| T4 | mc-argon | 2 | 2 | 91, 90 | 0.59, 0.67 | 9, 6 | 12.1 | 24,305 | 23 |
| T4 | agy-argon | 2 | 2 | 165, 164 | 1.30, 1.23 | 8, 8 | 20.5 | 110,973 | 72 |
| T5 | mc-argon | 2 | 2 | 192, 347 | 1.00, 2.38 | 8, 20 | 19.2 | 38,265 | 34 |
| T5 | agy-argon | 2 | 2 | 249, 284 | 2.42, 1.49 | 22, 16 | 14.0 | 88,748 | 77 |
| T6 | mc-argon | 2 | 2 | 18, 28 | 0.08, 0.08 | 3, 3 | 7.7 | 5,185 | 0 |
| T6 | agy-argon | 2 | 1 | 31, 27 | 0.09, 0.17 | 3, 4 | 8.3 | 14,858 | 45 |

| Tasks | Harness | Runs | Passed | Total cost $ | Median wall s | Calls | s/call | Input tok/call | Output tok/call |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| all | mc-argon | 20 | 18 | 16.29 | 71 | 164 | 12.6 | 31,098 | 447 |
| all | agy-argon | 20 | 18 | 17.84 | 151 | 236 | 12.1 | 65,157 | 412 |
| hold | mc-argon | 8 | 8 | 9.51 | 84 | 86 | 11.7 | 33,708 | 374 |
| hold | agy-argon | 8 | 7 | 7.83 | 192 | 131 | 11.2 | 60,684 | 360 |
| seen | mc-argon | 12 | 10 | 6.78 | 52 | 78 | 13.6 | 28,221 | 529 |
| seen | agy-argon | 12 | 11 | 10.01 | 76 | 105 | 13.2 | 70,738 | 476 |

Extra Microcoder runs at ee6c2215d0 that aren't in the counted set (a second
queue, stopped when the crash below turned up): H1, H2, H3, T1 and T2 passed.
H4 crashed with "slice out of bounds" after 63 s and $0.96.

### What was built, and what each piece bought

| Commit | Change | What it bought, measured |
| --- | --- | --- |
| 0cd61204f6 | Microcoder on Vertex Gemini: `--model` and `--effort` (`thinkingConfig.thinkingLevel`, native `streamGenerateContent` with thought and cached-token counts), the `us`/`eu` hosts, sign-in from `antigravity.env`, card prices, retries on 429/5xx and on silence (first-token and idle limits) | Microcoder runs at all on this machine: before this, its only Vertex key file was missing and it fell through to OpenRouter Flash. Retries never fired in 38 Microcoder runs: no 429, 5xx or stall happened. They are checked against real curl and a loopback server (`coax/web/tool.sh coax/lib/gateway/plan-retry-checks.coax`). |
| c1d6da4b2b | A view finds `head` on NixOS | Before it, every `view` returned "not found" (T1 baseline). |
| ddf0dae6c9 | One append-only step log with views inside their step, exact `edits`, a coding system prompt, the real OS in the facts, 15-minute runs | Measured together on T1–T6, one run each, against the baseline: passes 3 → 4 (T5's check passed, though its run then ended early: next row), cost $5.90 → $2.38, cached share of input 6% → 58%. T4: $1.41 and 213 s → $0.65 and 59 s. T5: out of time at 302 s and $3.46 → passed its check in 147 s for $0.87. The pieces weren't measured one at a time, so this doesn't say which bought what. After it, edits replaced every shell write (12 python, printf or `cat >` writes in the baseline, none after). Of 64 edits in later runs, 62 applied the first time. |
| ee6c2215d0 | A request never passes the journal's 256 KB (`AF_REQUEST`); a view cut at 64 KB says where | T5's v2 run passed its check but ended "run budget refused": its 10th request was about 350 KB. |
| 941212f68f | A fix for a crash when every step leaves the log; the log is 160 KB | Found by a final H4 run. Not measured apart. |

The hold-out tasks weren't looked at until the final runs: a separate agent
built them and reported only their kinds and start commits.

### Failures, classified

| Run | Class | What happened |
| --- | --- | --- |
| Baseline T1–T6 views | tools | `/usr/bin/head` is missing on NixOS, so every view failed (c1d6da4b2b) |
| Baseline T5 | loop | Each view replaced the files viewed before it, so Argon viewed the same three files over and over. It ran out of the 5-minute run budget after 36 calls. |
| Baseline T3, T4 | tools | Shell quoting broke python heredocs and `printf` writes: lost steps, not failures |
| v2 T5 | loop | The request passed the journal's 256 KB, which ends a journaled run |
| Final H4 (extra run) | loop | `mc_log_drop` read a stale offset (941212f68f) |
| T3, every Microcoder run | model | The warning line reuses the error's words and never says "warn" |
| T6, baseline and v2 | model | "one commit per issue" broken across a line. Antigravity did the same once. |

### Cost: why Microcoder's prompts are cached less

- **Implicit caching only.** Explicit context caching (`cachedContents`) is
  not offered for `gemini-4-argon`: Vertex answers 400 "does not support
  explicit context caching". So cached tokens come only from Vertex's
  implicit cache.
- **Hits are intermittent.** Even when a prompt starts with the whole
  previous prompt, about half of Microcoder's calls get no cache hit. For
  example, in T2 the calls of 54,850, 60,277 and 64,917 tokens were cached
  39,942, none and 58,223.
- **Antigravity's prefix is stable.** It resends a 60–90K-token system prompt
  and tool list that never changes, and 71–85% of its input was cached.
- **The step log is capped.** The journal holds a request to 256 KB
  (`coax/lib/agent/journal-flow.coax`, an `#authority` file), so Microcoder's
  step log is 160 KB. On a reading task that explores many files, the log
  fills, the earliest half leaves, and the cached prefix breaks.
  - H4 ran 21 and 32 calls, cached 21% of its input, and read again parts of
    files that had left the log.

What would close the hold-out cost gap, not yet tried:

- Keep a step's command and its exit code when the step leaves, and drop only
  the output. The model keeps its trail and doesn't read the same thing
  twice.
- A larger request limit for the journal. This is an `#authority` change for
  its owner.

### Spend

About $45.65 of Google credits across 65 recorded runs, plus a few cents of
probes. The runs:

| Runs | Cost |
| --- | --- |
| Microcoder baseline | $5.90 |
| Microcoder v2 | $2.38 |
| Microcoder, final counted | $16.29 |
| Microcoder, extra final | $2.48 |
| Antigravity, final | $17.84 |
| Antigravity, first two | $0.76 |

The second model setting (Flash or Carbon) wasn't run: the budget doesn't
cover it.

### Gates

- `coax test` passes for:
  - every `coax/lib/microcoder` unit;
  - `coax/lib/agent/engine/lab.coax` (which loads `microcoder.coax`),
    `engine.coax` and `run.coax`;
  - the gateway units changed: `gateway.coax`, `plan.coax`, `vertex.coax` and
    `rates.coax`;
  - `coder/tui/mcworker.coax` and `coder/tui/coder.coax`.
- `coax/web/tool.sh tools-check`: no new file without a reason.
- `coax/lib/microcoder/tools_smoke.sh` builds on Linux again and passes.
- `coax/web/tool.sh coax/lib/gateway/plan-retry-checks.coax` passes.
- `coder/tui/smoke.py`: 123 checks pass. The same two fail as before this
  work: `run --json` and the Microcoder gateway replay. This work changes
  neither.

### What was not verified

- **A second setting.** Only Argon low in `us` was run.
- **Statistical confidence.** Two runs a task can't separate 18 passes from
  17 or 19.
- **The retry path against Vertex.** No 429 or stall happened in any
  Microcoder run, so the retries are verified only against curl and a
  loopback server.
- **Each piece of ddf0dae6c9 on its own.** Its parts were measured together.
