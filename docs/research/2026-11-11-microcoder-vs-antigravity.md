# Microcoder against Antigravity on Gemini

**Round 4, Argon, three runs per task (2026-11-12): not proven.** Microcoder
passed 34 of 42 runs to Antigravity's 35 and cost $27.37 to $38.19. Its median
wall time tied at 155 s. On H5–H8 it passed 8 of 12 to 9, cost less ($8.79 to
$10.61) and was slower (179 s to 157 s). See "Round 4 verdict".

**Carbon arm:** see "Carbon".

**Round 3 (2026-11-12): stopped at the cheap check, no verdict run.** One
more look at the final diff and new files before a finish is taken
(a08147fa7f) changed no outcome in four runs and cost 42% more on T1. The
round 2 verdict stands.

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

Round 4 (added 2026-11-12, before its runs): three runs per task in round 4.

Carbon (added 2026-11-12, before its runs): a Carbon arm with 2 runs per task
(`gemini-4-carbon`, `us`, both harnesses at their low default; Microcoder at
the Argon round 4 build).

Carbon arm: 3 runs per task (added 2026-11-12, before any Carbon run ended).
The run count rose from 2 to 3 because the owner raised the budget to $200
for round 4 and Carbon together. Three runs match the Argon round 4 verdict.

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


## Round 4

After three rounds of better observations, AGENTS.md's rule meant changing
approach: the same model (`gemini-4-argon`), with the harness choosing the
thinking level by step. The budget was $50 more.

### The changes

- **0df590f5e7 reverts round 3's final look.** It had shown no gain and cost
  42% more on T1.
- **c2e1e45103 adds thinking by step.** At `--effort low`:
  - **The first step**, which plans the work, goes out at medium.
  - **Every other step** goes out at low.
  - **A finish answered at low** is asked again, once, with the same prompt,
    at medium, and that answer decides.
  - **Medium calls** carry a whole-call limit of 90 s (`GwRequest.limit_ms`),
    which the plan cuts with no retry. If one fails, the first step goes out
    at low, or the low finish is taken.
  - **Only a medium call's reply streams.**
  - **Why medium, not high:** round 1 saw medium already think for minutes on
    some calls, and high only longer.
- **Checks first:**
  - which steps go out at which level and limit;
  - the re-ask happens once per finish, with the same prompt (same length and
    hash);
  - both fallbacks;
  - nothing changes when thinking by step is off;
  - on loopback, a call past the whole limit is cut once, with no retry.
- **Same commit:** the definition's one added line ("three runs per task in
  round 4") went in before any run.

### The cheap check, one run each (Microcoder at c2e1e45103)

| Task | Before (78ab78f450, round 2) | Thinking by step | Levels, call by call |
| --- | --- | --- | --- |
| T3 | 1 of 2 passed; $0.21, $0.36 | failed (no "warn" again); $0.42; 93 s | m l l l l m |
| T4 | 1 of 2 passed; $0.64, $0.63 | passed; $0.41; 87 s | m l l l l m |
| H4 | 0 of 2 passed (0 of 1 in round 3); $1.09, $1.62 | passed; $1.24; 255 s | m, then 14 × l, then m |
| T1 | 2 of 2 passed; $0.12, $0.12; 19 s, 30 s | passed; $0.17; 44 s | m l l m |

- **Medium calls stayed short:** no call hit its 90 s limit, and none failed.
- **H4 went from fail to pass.** H4 is the task this change most plausibly
  helps. But it was one run, and H4 passed 2 of 2 at an older build. T4
  passed too, as it did in 3 of its 4 earlier runs.
- **T3 still failed** the same way.
- **The re-ask's cost is built in.** On T1, the plan call and the re-ask each
  add a full call to a run that otherwise takes 3: cost rose 42% ($0.17
  against $0.12) and time about 80% (44 s against 19 s and 30 s). That passes
  the agreed 30% limit, and every short task will pay the same.
- **It stopped there at first**, as agreed. The coordinator then overrode
  the rule and asked for the verdict (next section).

c2e1e45103 stayed on `main` for the verdict. The recommendation after it is
below.

### Round 4 verdict (Argon, three runs per task)

**The stop rule was overridden.** The cheap check above stopped on a rule set
as a percentage: T1's cost or time must not rise more than 30%. The
coordinator overrode it because the absolute amounts were small. On T1 the
42% was $0.05 and the time rise about 25 s. The definition's measure is the
total cost and median time across all tasks.

**The runs:**

- **Microcoder:** all 42 runs at one build, c2e1e45103 (thinking by step).
  The four cheap-check runs at that build count as run 1 of T1, T3, T4 and
  H4.
- **Antigravity:** its 28 earlier runs plus one new run of each of the 14
  tasks at c2e1e45103. Its engine is unchanged since round 1, at its
  uniformly low default.
- **Thinking levels differ:** Microcoder's differ by step (the plan and each
  finish at medium, the rest at low). Antigravity's are uniformly low.
- **Stopped early:** I stopped the arm when Microcoder had 8 fails and so
  could no longer reach Antigravity's 35. The three Microcoder runs already
  going finished, so all 42 have results.
- **Wall times are confounded:** Microcoder's round 4 runs ran alongside the
  Carbon arm, up to 8 runs at once on one machine. Most of Antigravity's ran
  in earlier rounds under lighter load. Seconds per call show the load:
  Microcoder 21.4 here against 14.8 in round 2.

| Task | Harness | Runs | Passed | Wall s (each) | Cost $ (each) | Calls | s/call | Input tok/call | Cached % |
| --- | --- | ---: | ---: | --- | --- | --- | ---: | ---: | ---: |
| H1 | mc-argon | 3 | 3 | 74, 84, 74 | 0.28, 0.28, 0.27 | 5, 5, 5 | 15.5 | 11,794 | 0 |
| H1 | agy-argon | 3 | 3 | 139, 84, 127 | 0.42, 0.34, 0.45 | 11, 9, 12 | 10.9 | 25,691 | 72 |
| H2 | mc-argon | 3 | 3 | 190, 248, 68 | 0.44, 0.51, 0.40 | 7, 6, 6 | 26.6 | 23,428 | 37 |
| H2 | agy-argon | 3 | 3 | 246, 134, 372 | 0.94, 0.67, 0.86 | 26, 16, 25 | 11.2 | 40,030 | 85 |
| H3 | mc-argon | 3 | 3 | 134, 391, 212 | 0.59, 0.98, 0.99 | 8, 15, 11 | 21.7 | 22,595 | 32 |
| H3 | agy-argon | 3 | 3 | 194, 191, 176 | 1.28, 0.86, 1.31 | 16, 17, 18 | 11.0 | 72,754 | 83 |
| H4 | mc-argon | 3 | 3 | 255, 204, 212 | 1.24, 1.29, 0.44 | 16, 14, 6 | 18.6 | 27,723 | 35 |
| H4 | agy-argon | 3 | 2 | 203, 275, 404 | 1.94, 1.40, 2.77 | 20, 16, 30 | 13.4 | 100,264 | 84 |
| H5 | mc-argon | 3 | 2 | 99, 188, 308 | 0.37, 0.38, 0.60 | 6, 8, 5 | 31.3 | 23,467 | 34 |
| H5 | agy-argon | 3 | 3 | 63, 55, 61 | 0.53, 0.67, 0.57 | 4, 4, 4 | 14.9 | 50,234 | 37 |
| H6 | mc-argon | 3 | 3 | 771, 900, 315 | 1.36, 1.77, 1.73 | 23, 25, 26 | 26.8 | 30,342 | 54 |
| H6 | agy-argon | 3 | 3 | 184, 159, 155 | 1.08, 1.16, 1.12 | 12, 17, 13 | 11.9 | 80,954 | 82 |
| H7 | mc-argon | 3 | 3 | 92, 171, 60 | 0.35, 0.26, 0.28 | 6, 5, 5 | 20.2 | 11,731 | 7 |
| H7 | agy-argon | 3 | 3 | 238, 288, 384 | 0.78, 0.95, 1.05 | 20, 22, 19 | 14.9 | 35,506 | 84 |
| H8 | mc-argon | 3 | 0 | 89, 289, 81 | 0.44, 0.82, 0.44 | 4, 11, 4 | 24.1 | 22,486 | 21 |
| H8 | agy-argon | 3 | 0 | 172, 154, 106 | 1.15, 0.60, 0.94 | 10, 6, 6 | 19.6 | 76,809 | 69 |
| T1 | mc-argon | 3 | 3 | 44, 65, 37 | 0.17, 0.17, 0.18 | 4, 4, 4 | 12.2 | 9,879 | 0 |
| T1 | agy-argon | 3 | 3 | 36, 23, 30 | 0.15, 0.23, 0.25 | 4, 3, 4 | 8.2 | 23,971 | 45 |
| T2 | mc-argon | 3 | 3 | 354, 165, 146 | 1.58, 0.49, 0.71 | 25, 6, 9 | 16.6 | 31,505 | 52 |
| T2 | agy-argon | 3 | 3 | 74, 182, 93 | 0.71, 1.28, 0.64 | 10, 11, 10 | 11.3 | 62,079 | 73 |
| T3 | mc-argon | 3 | 1 | 93, 167, 143 | 0.42, 0.34, 0.21 | 6, 4, 4 | 28.8 | 16,017 | 10 |
| T3 | agy-argon | 3 | 2 | 71, 78, 101 | 0.45, 0.48, 0.66 | 7, 9, 7 | 10.9 | 41,234 | 66 |
| T4 | mc-argon | 3 | 3 | 87, 215, 136 | 0.41, 0.68, 1.08 | 6, 9, 10 | 17.5 | 24,556 | 23 |
| T4 | agy-argon | 3 | 3 | 165, 164, 105 | 1.30, 1.23, 1.11 | 8, 8, 8 | 18.1 | 110,682 | 73 |
| T5 | mc-argon | 3 | 3 | 246, 288, 235 | 1.65, 1.51, 0.88 | 16, 17, 10 | 17.9 | 26,943 | 30 |
| T5 | agy-argon | 3 | 3 | 249, 284, 301 | 2.42, 1.49, 1.58 | 22, 16, 17 | 15.2 | 90,246 | 79 |
| T6 | mc-argon | 3 | 1 | 87, 53, 33 | 0.16, 0.12, 0.12 | 5, 4, 4 | 13.4 | 5,992 | 0 |
| T6 | agy-argon | 3 | 1 | 31, 27, 20 | 0.09, 0.17, 0.09 | 3, 4, 3 | 7.8 | 15,174 | 50 |

| Tasks | Harness | Runs | Passed | Total cost $ | Median wall s | Calls | s/call | Input tok/call | Output tok/call |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| all | mc-argon | 42 | 34 | 27.37 | 155 | 379 | 21.4 | 24,137 | 513 |
| all | agy-argon | 42 | 35 | 38.19 | 155 | 507 | 13.0 | 64,297 | 480 |
| hold | mc-argon | 24 | 20 | 16.49 | 189 | 232 | 23.7 | 24,545 | 509 |
| hold | agy-argon | 24 | 20 | 23.84 | 174 | 353 | 12.9 | 61,446 | 501 |
| seen | mc-argon | 18 | 14 | 10.88 | 139 | 147 | 17.7 | 23,492 | 520 |
| seen | agy-argon | 18 | 15 | 14.35 | 85 | 154 | 13.2 | 70,832 | 433 |

| Condition | Microcoder | Antigravity | Met |
| --- | --- | --- | --- |
| Passes, all 14 tasks (42 runs each) | 34 | 35 | **no** |
| Total cost, all tasks | $27.37 | $38.19 | yes |
| Median wall time, all tasks | 155 s | 155 s | **no** (tied) |
| Passes, H5–H8 (12 runs each) | 8 | 9 | **no** |
| Total cost, H5–H8 | $8.79 | $10.61 | yes |
| Median wall time, H5–H8 | 179 s | 157 s | **no** |

**Verdict on Argon: not proven.** H8 fails every run of both harnesses (see
H5–H8). Without it the passes are 34 to 35 of 39.

**What thinking by step adds per run:** averaged over the 14 tasks, against
the round 2 build (78ab78f450, 2 runs a task):

- **Cost:** +$0.13 a run ($0.52 to $0.65, about +25%).
- **Wall time:** +82 s a run (111 s to 193 s). Most of that is likely the
  heavier machine load described above, not the thinking itself.
- **Passes:** 22 of 28 (79%) at the round 2 build, 34 of 42 (81%) at this
  one.
- **By task:** H4 went from 0 of 2 to 3 of 3. T6 went from 2 of 2 to 1 of 3,
  and T3 from 1 of 2 to 1 of 3.

**Recommendation: revert c2e1e45103.** Its pass change is within the noise of
these run counts, and it costs about a quarter more per run. By the owner's
rule, an addition without a measured gain comes out. The Carbon arm runs at
the same build, so its result is noted in "Carbon" before the revert.

### What blocked round 4's cheap check, and the lever it named

- **The blocker was still the round 2 verdict:** 22 against 24 of 28 runs.
- **Spending thinking at the finish costs a call on every task**, which
  breaks the cost and time budget on short ones.
- **A cheaper variant to measure next:** keep the medium plan step but re-ask
  only finishes of runs that wrote a file or ran more than a few steps. That
  is a threshold on the harness's own counters, not on the task.
  - Or skip the re-ask and put the plan at medium only. Its cost on T1 is one
    medium call instead of two.
- **More runs per task, needed either way.** The changes being judged are
  within the noise of two runs per task.

### Round 4 spend

- **Round 4:** $2.24 of the $50, plus a few cents for one tiny test turn.
- **All rounds:** $72.91.

## Round 3

The coordinator approved a third round, with $35 more, for the lever round 2
named. It was to be checked cheaply first, on the failures it was meant to
address, then run in full only if it held up.

### The change: one look before a finish is taken (a08147fa7f)

- **What it does:** when a run that has run commands or edits says it is
  finished, the loop gives it one more step of the one loop:
  - `git --no-pager diff`;
  - the new untracked files, at most 20, the first 8 KB of each;
  - a note with the task again and a fixed instruction: check each
    requirement the task states against them, then finish again or keep
    working.
- **Rules:** the next answer is taken as it is. It happens once a run, never
  for a plain answer, and only when the owner asks: Coder's worker does, the
  Lab doesn't. The reply of a finish that will get its look isn't streamed,
  so the user sees only the one taken.
- **Checks first:** in `loop.coax`, one check covers a finish after work
  getting its look and the next answer being taken. Another covers once a
  run, none for a plain answer, and none when not asked for.
- **Where it came from:** it was designed from the failures on T3, T4 and H4.
  H5–H8 were seen after round 2's verdict, but nothing was tuned on them.

### The cheap check, one run each, against round 2's build (78ab78f450)

| Task | Round 2 build | With the look | Steps after the look |
| --- | --- | --- | --- |
| T4 | 1 of 2 passed; $0.64, $0.63; 81 s, 57 s | passed; $0.61; 114 s | none: finished at once |
| T3 | 1 of 2 passed; $0.21, $0.36; 44 s, 81 s | failed (no "warn" again); $0.55; 90 s | none |
| H4 | 0 of 2 passed; $1.09, $1.62; 126 s, 201 s | failed (a fact missing); $0.48; 82 s | none |
| T1 | 2 of 2 passed; $0.12, $0.12; 19 s, 30 s | passed; $0.17; 41 s | none |

- **It changed no outcome.** In all four runs, the model read the diff and
  its new files and finished at once, with nothing more done.
- **T4's pass isn't the look's doing.** The answer the model gave before the
  look was already enough. T4 had also passed in 3 of its 4 runs before this
  round.
- **T3 and H4 failed the same way as before.**
- **It costs more.** On T1 it added 42% to cost ($0.17 against $0.12) and
  about 15 s.
- **So it stopped there.** Both of the coordinator's stop rules were met: it
  fixed none of the failures, and it added more than about 20% to T1's cost.
  The 28-run verdict wasn't spent.

The change stays on `main` for now: the loop's checks cover it, and Coder's
worker is the only place it is turned on. The next agent should take it out
or keep it, but not tune it further without a measurement.

### What blocks the goal, and the next lever

- **The blocker stays the round 2 verdict's:** Microcoder passed 22 of 28
  runs to Antigravity's 24, while costing 41% less and taking less than half
  the time.
- **What the misses are:** Argon at thinking level low leaving something out
  of an answer. The model judges a self-check like this one as passed, so a
  prompt-level look at its own work doesn't move it.
- **Levers not tried:**
  - Run the hard steps at a higher thinking level. For example, use medium
    effort for the first step and for finishing steps only. Round 1's
    research saw medium thinking take minutes on some calls, so this needs a
    measurement of its own.
  - A second setting where the model itself is stronger.
  - Both are changes to how the model is asked, not to the loop.
  - More runs per task, to tell a real two-run gap from noise. At $0.5 a
    Microcoder run and $0.9 an Antigravity run, five runs of each of the 14
    tasks cost about $100.

### Round 3 spend

- **Round 3:** $1.82 of the $35, plus a few cents for one tiny test turn.
- **All rounds:** $70.67.

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
