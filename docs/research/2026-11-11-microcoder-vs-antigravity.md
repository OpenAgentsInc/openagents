# Microcoder against Antigravity on Gemini

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


## Verdict

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

## Setup

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

## Per-task results (Argon low, us, two runs a task)

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

## What was built, and what each piece bought

| Commit | Change | What it bought, measured |
| --- | --- | --- |
| 0cd61204f6 | Microcoder on Vertex Gemini: `--model` and `--effort` (`thinkingConfig.thinkingLevel`, native `streamGenerateContent` with thought and cached-token counts), the `us`/`eu` hosts, sign-in from `antigravity.env`, card prices, retries on 429/5xx and on silence (first-token and idle limits) | Microcoder runs at all on this machine: before this, its only Vertex key file was missing and it fell through to OpenRouter Flash. Retries never fired in 38 Microcoder runs: no 429, 5xx or stall happened. They are checked against real curl and a loopback server (`coax/web/tool.sh coax/lib/gateway/plan-retry-checks.coax`). |
| c1d6da4b2b | A view finds `head` on NixOS | Before it, every `view` returned "not found" (T1 baseline). |
| ddf0dae6c9 | One append-only step log with views inside their step, exact `edits`, a coding system prompt, the real OS in the facts, 15-minute runs | Measured together on T1–T6, one run each, against the baseline: passes 3 → 4 (T5's check passed, though its run then ended early: next row), cost $5.90 → $2.38, cached share of input 6% → 58%. T4: $1.41 and 213 s → $0.65 and 59 s. T5: out of time at 302 s and $3.46 → passed its check in 147 s for $0.87. The pieces weren't measured one at a time, so this doesn't say which bought what. After it, edits replaced every shell write (12 python, printf or `cat >` writes in the baseline, none after). Of 64 edits in later runs, 62 applied the first time. |
| ee6c2215d0 | A request never passes the journal's 256 KB (`AF_REQUEST`); a view cut at 64 KB says where | T5's v2 run passed its check but ended "run budget refused": its 10th request was about 350 KB. |
| 941212f68f | A fix for a crash when every step leaves the log; the log is 160 KB | Found by a final H4 run. Not measured apart. |

The hold-out tasks weren't looked at until the final runs: a separate agent
built them and reported only their kinds and start commits.

## Failures, classified

| Run | Class | What happened |
| --- | --- | --- |
| Baseline T1–T6 views | tools | `/usr/bin/head` is missing on NixOS, so every view failed (c1d6da4b2b) |
| Baseline T5 | loop | Each view replaced the files viewed before it, so Argon viewed the same three files over and over. It ran out of the 5-minute run budget after 36 calls. |
| Baseline T3, T4 | tools | Shell quoting broke python heredocs and `printf` writes: lost steps, not failures |
| v2 T5 | loop | The request passed the journal's 256 KB, which ends a journaled run |
| Final H4 (extra run) | loop | `mc_log_drop` read a stale offset (941212f68f) |
| T3, every Microcoder run | model | The warning line reuses the error's words and never says "warn" |
| T6, baseline and v2 | model | "one commit per issue" broken across a line. Antigravity did the same once. |

## Cost: why Microcoder's prompts are cached less

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

## Spend

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

## Gates

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

## What was not verified

- **A second setting.** Only Argon low in `us` was run.
- **Statistical confidence.** Two runs a task can't separate 18 passes from
  17 or 19.
- **The retry path against Vertex.** No 429 or stall happened in any
  Microcoder run, so the retries are verified only against curl and a
  loopback server.
- **Each piece of ddf0dae6c9 on its own.** Its parts were measured together.
