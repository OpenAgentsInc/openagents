# Briefed agent vs bare Claude Code (#11211)

Status: S2 complete, 2026-10-10. Tier-1 tool and one-lever ablations are
not yet run (see Next).

## Headline (pre-registered S2: 21 closed issues x 3 runs, A vs B0)

| Arm | Accepted (fix tests + judge) | Median $ (sd) | Median s (sd) | $ per accepted | Same outcome in all 3 runs |
|---|---|---|---|---|---|
| A bare Claude Code | 19/63 | 1.19 (0.56) | 284 (233) | 4.00 | 16/21 issues |
| B0 briefed + verify | 19/63 | 0.52 (0.29) | 121 (96) | 1.84 | 17/21 issues |

- **Success:** equal point estimates (0.302 each, issue-weighted); difference
  +0.000, 95% CI [-0.048, +0.048]. Under the frozen rule a CI spanning 0 is
  **inconclusive on "equal or better"**. Secondary: the fix's tests pass
  0.333 vs 0.302 (CI [-0.079, 0.000]); the judge accepts 0.889 vs 0.794
  (CI [-0.254, +0.048]), so a quality cost for B0 is not ruled out.
- **Cost:** 53.9% lower per accepted change (95% CI [44.3%, 60.4%]): the
  30% gate is met with its lower bound. Total list price $75.98 (A) and
  $35.03 (B0); no unknown costs; one timeout.
- **Time:** median 121 s vs 284 s, p90 289 s vs 657 s: the latency gate is
  met. Spread is lower too.
- **Recommendation:** use B0 as the default path for small, well-scoped Rust
  issues: it halves the cost and time per accepted change at the same
  observed success. Do not claim it is equal or better in quality yet:
  extend S2 prospectively (a new frozen plan with more issues) before that
  claim, and watch the judge-acceptance gap.

The question: is a Claude agent with a prepared **briefing**, a custom system
prompt and a minimal tool set a cheaper, faster or more reliable way to turn
an issue into an accepted change than bare Claude Code told "complete this
issue"? The objective is **cost per accepted PR**, then time to PR and
variance. Tool candidates and the experiment order follow
[briefed-agent-tools.md](briefed-agent-tools.md).

## Setup

Code: `scripts/bench/briefed-ab/` and `crates/briefed-agent`.

- **Tasks** (`prepare.py`): closed issues fixed by one small commit in one or
  two crates whose diff adds tests. Each is replayed at the fix's parent. The
  fix's own test changes (`tests.patch`) and the names of the tests it added
  are the hidden check. A task is usable when those tests pass on the fix and
  fail on the parent (`--validate`); 21 of 29 candidates are.
- **Worktrees**: a shallow clone holding only the parent and 50 commits of
  history, sparse (no `assets/`, no `bench/terminal-bench/`), so no arm can
  see the fix. `gh` and every `openagents` command but `lease ... -- CMD`
  refuse; WebFetch and WebSearch are off for arm A.
- **Builds**: every `cargo` command from any arm runs on coderos-4080 in one
  build checkout and one persistent target dir (`remote/run.sh`), with the
  trial's working copy applied and any file the command wrote brought back.
  Trials of one issue run two at a time and share the warm build. A disk guard
  keeps the host above 50 GB free. The Mac holds only the small worktrees.
- **Login**: the Mac's Claude Code login for every arm and the judge
  (coderos-4080's login was at its weekly limit). Same model for every arm,
  `claude-opus-5-5`, default effort, 20-minute limit. A trial that hits the
  login's usage limit is discarded and rerun when the login answers again.
- **Interface** (from the main round on): most fix tests call a helper the fix
  introduced (on #10228 every arm, oracle included, compiled and touched the
  fix's file, and every one failed the fix's test on the helper's name). Every
  arm is now told the signatures of the new items the fix's tests call, as
  SWE-bench Pro does (`prepare.interface`).

### Arms

| Arm | Briefing | Tools |
|---|---|---|
| **A** bare | none: "Complete this issue." and the issue's title and body | Claude Code's defaults, permissions skipped |
| **B0** | from #11210's `filefind` (top 8 files) | Read, Edit, Write, Grep, Glob, `verify` |
| **Bbash** | same | Read, Edit, Write, Grep, Glob, Bash (cargo through the build host) |
| **C** oracle | the fix's real files (an upper bound, never a result) | as B0 |
| round 0 **B** | same | Read, Edit, Write, Grep, Glob, `run_check` (a narrow command tool) |

The **briefing** (`briefing.py`, about 1-7 s, deterministic, read from git
objects at the base commit): the issue; a short change plan (goal, the
issue's own bullet points, where to change, where the test goes, the checks);
each file with line-numbered excerpts around what the issue names, or an
item outline when nothing matches, and why it is listed (the finder's
confidence and reasons); one or two recent small changes to the top files as
diffs; the exact checks; AGENTS.md's core rules and the crate's own entry.

**`verify`** (in-process, `SdkMcpServer`): one call runs the touched crates'
tests (with the agent's filter) and `cargo fmt` on the build host and returns
`status`, `compile_errors {file,line,msg}` (new against the base's own),
`failing_tests {name,file,line,assert}`, `fmt` (applied),
`untouched_but_implicated` (error and failure sites, and history co-change),
and `done_when` (the issue's bullet points mapped to that check). `fast`
compiles only; an unchanged working copy returns the cached result.

### What a trial records

Wall time; dollars and tokens as the CLI reports them; turns; every tool call
with tokens in and out, seconds, and whether the next edit touched a file the
result named; files read (Read, and files named in Bash commands); files the
agent opened outside the briefing (misses, for #11210); whether the change
compiles (no new errors against the parent's own); whether the fix's tests
pass on it; overlap with the fix's files; and a blind judge's score (Opus,
given the issue, the merged change as reference and the candidate, 1-5,
accept at 4+). **Accepted** = the fix's tests pass and the judge accepts.
Every trial stores its full lever settings.

## Levers

Every lever is a knob in `common.LEVERS` (or an arm's override) and is saved
with each trial.

| # | Lever | Knob | Tried |
|---|---|---|---|
| 1 | Briefing | `finder` (filefind, lite, oracle), `briefing_files`, `excerpt`, `excerpt_lines`, `briefing_tokens`, `history`, `plan` | filefind vs oracle (C); size ablation |
| 2 | Instructions | `template` (v1, v2, v3 tool-aware) | v1 (round 0), v3 |
| 3 | Tools | `tools`: `verify`, `bash`, `checks`, `verify+related`, `verify+outline`, `verify+finish` | B0 vs Bbash vs round-0 run_check; tier 1 |
| 4 | Plugins / MCP | `mcp`: none but the bench's own | fixed |
| 5 | Model routing | `model`, `effort` | ablation |
| 6 | Loop control | `max_turns`, `timeout_secs` | fixed (no turn limit, 20 min) |
| 7 | Token economics | `briefing_in` (system prompt, cached, or first message) | system |
| 8 | Execution env | `build_cache` (warm prewarm, cold), `build_host` | warm vs cold ablation |
| 9 | Post-processing | `post_fmt`; verify applies fmt | verify's fmt |
| 10 | Decomposition | `decomposition` | single |
| 11 | Triage | `triage` | none |
| 12 | Learning loop | misses logged per trial; `scripts/work/learn.py` retrains filefind through the gate weekly (#11312) | see "Learning loop" |
| - | Task text | `interface` | off in the pilot, on after |

## Pilot

Round 0 (#10074; A, B with `run_check` and template v1, C; 3 runs each) and
round 1 (#10228, #10301, #11118; A, B0, Bbash, C; 3 runs each).

What changed after the pilot, and why:

- **Hidden tests were naming tests.** See Interface above.
- **The grader's test overlay** first used `git apply --3way` with fuzz, which
  broke a file when the agent had also added a test at the end of the same
  module (an unclosed brace). The overlay now applies the fix's test hunks
  cleanly or, when that fails, inserts the lines the fix added before the
  test module's closing brace (`remote/overlay.py`). All trials were
  regraded.
- **Compile status against the parent.** Some parents already fail to build an
  unrelated test target (`openagents-cli`'s `customer` test); "compiles" now
  means no error the parent does not have, from `cargo check --keep-going`,
  and the fix's tests run only in their own target (`--lib`, `--bins`,
  `--test NAME`).
- **verify was slower than Bash** on #10228 (one call, 41-188 s, against
  Bash's 23-39 s `cargo test`): it ran `cargo check`, then `cargo test`, then
  `cargo fmt` as three build-host calls, compiling twice. It now runs the
  tests and fmt in one call, parses compile errors from the test build,
  caches an unchanged working copy, and has a compile-only `fast` mode. The
  prompt says to call it when done, not after every edit.
- **finish** ran the whole crate's tests (no filter) and failed on unrelated
  slow tests; it now reuses the agent's last filter.
- **Arm A followed AGENTS.md** into `openagents lease build -- cargo ...`,
  which would have queued behind the Mac's other builds; in trials `lease`
  runs its command directly (the build host queues).
- **#11210's finder** replaced the bench's first keyword finder: recall of the
  fix's files in the top 6 went from 0.35 to 0.58 (0.68 in the top 10) on the
  29 candidates.

Pilot round 1 (3 issues, interface off, before #11229):

| Arm | Trials | Pass fix tests | Judge accepts | Total $ | $ / judge-accepted | Median $ (sd) | Median s (sd) |
|---|---|---|---|---|---|---|---|
| A | 9 | 1 | 7 | 10.17 | 1.45 | 1.08 (0.41) | 282 (545) |
| B0 | 9 | 3 | 8 | 4.77 | 0.60 | 0.57 (0.26) | 220 (106) |
| Bbash | 9 | 1 | 7 | 4.93 | 0.70 | 0.61 (0.32) | 249 (509) |
| C | 9 | 3 | 9 | 3.19 | 0.35 | 0.35 (0.22) | 127 (126) |

## The verify defect (#11229) and what it touched

The self-improving-codebases audit (RUN-01, RUN-02) found that `verify` and
`finish` could report `pass` after a nonzero exit, a checker that did not
start, zero tests or a failed fmt, and that the cache keyed on `git diff` plus
file names, so new bytes in an untracked file or staged content could return
a stale verdict. Fixed in c34036f3a5 with a test per case: `pass` now needs
the tests' exit 0 (read from exit markers, not the log), at least one test
run, and fmt's exit 0; `finish` runs a declared plan (compile, every test the
change adds, fmt), uncached and independent of the agent's filter; the cache
key is the git tree of the whole candidate plus mode, filter, crates and the
checker's identity.

What it could have affected: only what the B arms' agents saw, and so when
they stopped. **No outcome in this document came from verify or from an
agent's own report.** Acceptance is graded by `remote/eval.sh`, which applies
the change, lays the fix's tests over it and requires cargo's real exit 0 and
every named test passing, and by the judge, which sees only the diff. Of
verify's 67 `pass` verdicts in earlier rounds, none showed zero tests or a
failed fmt; a nonzero exit without a named failure cannot be ruled out from
those logs. Earlier rounds therefore need no re-scoring, but they ran the
defective agent and stay exploratory.

The exploratory main round (`m1`: 21 issues, A / B0 / Bbash, interface on)
was stopped at 101 of 189 trials when the defect was found, and is not used
for the S2 claim.

## Pre-registered S2 run

Frozen before its first trial in `scripts/bench/briefed-ab/plans/s2.json`,
committed with this section; `ab.py batch --plan plans/s2.json` refuses to
run with a different agent binary or Claude Code version.

- **Question:** does B0 cost at least 30% less per accepted change than bare
  Claude Code (A), with equal or better success and no worse median time?
- **Issues:** all 21 that passed validation before any trial result was
  seen; none is dropped after results.
- **Arms:** A and B0, 3 runs each per issue, `claude-opus-5-5` at default
  effort, 20-minute limit, interface on. Agent binary sha256
  `0480700f...07aa1` (c34036f3a5), Claude Code 2.1.296 with auto-update
  off in trials.
- **Order:** issues in a seeded random order; per issue each run rotates the
  arm order from a seeded starting arm, so each arm goes first equally often.
- **Budget:** $200 of CLI-reported list price for agent trials; reaching it
  stops the run, reported as incomplete.
- **Outcome:** accepted = the fix's own tests pass on the change and the
  blind judge (score 4+) accepts. Never the agent's report.
- **Analysis:** the issue is the unit and its runs a cluster. Success
  difference first, with a 95% CI from 10,000 issue-level bootstrap resamples;
  "equal or better" is shown only if the lower bound is at least 0, a CI
  spanning 0 is inconclusive. Cost per accepted from known costs only; a
  timed-out trial is a failure with unknown cost (an estimate from its tokens
  is shown separately and labeled). The 30% cost gate counts only if the
  CI's lower bound reaches 30%. Median and p90 time. Judge, grading and
  briefing costs are reported apart; subscription billing is not observable.
- **Reruns:** only trials that hit the login's usage limit (no outcome was
  observed). Nothing else is rerun or excluded.

The first start of S2 was stopped before any trial finished: a stale process
from the stopped exploratory round (a `coder host serve` a test had left
running) held the build lock, and the per-issue prewarm was a no-op since the
disconnect watcher (it saw the prewarm's closed stdin and stopped it). Both
are fixed (commands run without the lock's descriptor; prewarm skips the
watcher; incremental builds are off to fit the build host's 50 GB floor), and
S2 restarted from scratch under the same plan. The exploratory rounds ran
without a working prewarm, so their first trial per issue built cold.

### S2 results

Issues: 21; trials: 126; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (B0 vs A) | A | B0 | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.302 | 0.302 | +0.000 | [-0.048, +0.048] |
| fix tests pass | 0.333 | 0.302 | -0.032 | [-0.079, +0.000] |
| judge accepts | 0.889 | 0.794 | -0.095 | [-0.254, +0.048] |

| Cost (list price, as the CLI reports) | A | B0 |
|---|---|---|
| $ per accepted (known costs) | 4.00 | 1.84 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | +53.9% (95% CI [+44.3%, +60.4%]) |

| Time (s) | A | B0 |
|---|---|---|
| median | 284.2 | 121.1 |
| p90 | 656.5 | 289.0 |

| Issue | A accepted | B0 accepted | A mean $ | B0 mean $ | A mean s | B0 mean s |
|---|---|---|---|---|---|---|
| #10074 | 2/3 | 2/3 | 0.70 | 0.17 | 193 | 67 |
| #10167 | 0/3 | 0/3 | 0.47 | 0.27 | 66 | 64 |
| #10179 | 0/3 | 0/3 | 1.99 | 0.94 | 369 | 198 |
| #10181 | 1/3 | 2/3 | 1.38 | 0.85 | 310 | 163 |
| #10201 | 3/3 | 3/3 | 1.05 | 0.54 | 202 | 137 |
| #10228 | 3/3 | 3/3 | 0.61 | 0.16 | 156 | 67 |
| #10248 | 0/3 | 0/3 | 1.60 | 0.51 | 470 | 99 |
| #10258 | 0/3 | 0/3 | 1.08 | 0.56 | 170 | 109 |
| #10273 | 3/3 | 3/3 | 1.32 | 0.69 | 501 | 138 |
| #10283 | 0/3 | 0/3 | 1.88 | 0.91 | 237 | 176 |
| #10289 | 2/3 | 2/3 | 0.67 | 0.26 | 363 | 93 |
| #10295 | 0/3 | 0/3 | 0.76 | 0.39 | 89 | 113 |
| #10298 | 0/3 | 0/3 | 1.32 | 0.58 | 364 | 153 |
| #10299 | 0/3 | 0/3 | 2.29 | 0.72 | 555 | 107 |
| #10301 | 1/3 | 0/3 | 1.64 | 0.55 | 348 | 199 |
| #10349 | 0/3 | 0/3 | 1.94 | 0.86 | 664 | 287 |
| #10369 | 0/3 | 0/3 | 1.36 | 0.99 | 315 | 304 |
| #10370 | 0/3 | 0/3 | 0.70 | 0.44 | 373 | 176 |
| #10893 | 3/3 | 3/3 | 0.48 | 0.20 | 257 | 354 |
| #10986 | 1/3 | 1/3 | 0.79 | 0.25 | 660 | 120 |
| #11118 | 0/3 | 0/3 | 1.31 | 0.83 | 649 | 163 |


Judge cost (not in either arm): $34.18. Build-lock wait per trial: median
9.8 s. Per-trial rows: `scripts/bench/briefed-ab/results/s2-trials.csv`.

Where B0's saving comes from: A's median run read 1.02M cached tokens against
B0's 0.46M (Claude Code's own prompt and tool definitions on every turn, and
Bash output: 23 Bash calls a run at about 650 tokens each). B0 takes more
turns (23 vs 15) but cheaper ones: about 6.9 Edit, 6.7 Grep, 6.4 Read and 1.5
`verify` calls a run; a `verify` returns about 200 tokens in a median 39 s.
Output tokens are the same (about 7.3k).

Tool use in S2 (calls per run, share of runs, tokens in and out per call,
median seconds, and how often the next edit touched a file the result named):

| Arm | Tool | Calls/run | Share | In/call | Out/call | Median s | Acted on |
|---|---|---|---|---|---|---|---|
| A | Bash | 22.6 | 1.00 | 136 | 648 | 0.1 | 0.02 |
| A | Edit | 0.3 | 0.13 | 366 | 58 | 0.0 | 0.29 |
| B0 | Edit | 6.9 | 1.00 | 286 | 41 | 0.0 | 0.57 |
| B0 | Grep | 6.7 | 0.98 | 33 | 653 | 0.0 | 0.35 |
| B0 | Read | 6.4 | 0.98 | 39 | 1458 | 0.0 | 0.07 |
| B0 | verify | 1.5 | 1.00 | 16 | 200 | 39.1 | 0.18 |

### Kinds run (#11310): checks per package kind, multi-package briefings

`plans/kinds.json` (tag `ab-kinds`, frozen 2026-10-11 with the pins it names:
agent sha `a88372f5...`, Claude Code 2.1.291), run on `oa-dev-env-1` from
main at 616c6f79d0. A against B0 on six closed issues that are not
single-crate Rust fixes: docs only (#10065), shell with a Python test
(#10257), a Python script with tests, a shell gate and docs (#11284), two
multi-crate Rust fixes (#11190, #11281) and Kotlin glue (#11283), 2 reps
each. Report: `scripts/bench/briefed-ab/results/ab-kinds.md`; rows:
`results/ab-kinds-trials.csv`.

| Success (B0 vs A) | A | B0 | Difference | 95% CI |
|---|---|---|---|---|
| accepted (kinds grade and judge) | 0.250 | 0.167 | -0.083 | [-0.250, +0.000] |
| kinds grade passes | 0.333 | 0.333 | +0.000 | [+0.000, +0.000] |
| judge accepts | 0.750 | 0.667 | -0.083 | [-0.333, +0.167] |

| Cost and time | A | B0 |
|---|---|---|
| total $ | 15.42 | 11.32 |
| $ per accepted | 5.14 | 5.66 (-10.1%, CI [-33.8%, +28.8%]) |
| median s | 169.5 | 195.5 |
| p90 s | 531.2 | 448.7 |

| Issue | A accepted | B0 accepted | A mean $ | B0 mean $ | A mean s | B0 mean s |
|---|---|---|---|---|---|---|
| #10065 | 1/2 | 0/2 | 3.01 | 0.74 | 248 | 132 |
| #10257 | 0/2 | 0/2 | 0.44 | 0.35 | 46 | 68 |
| #11190 | 0/2 | 0/2 | 1.85 | 1.78 | 571 | 483 |
| #11281 | 0/2 | 0/2 | 1.22 | 0.95 | 350 | 315 |
| #11283 | 2/2 | 2/2 | 0.37 | 0.51 | 30 | 181 |
| #11284 | 0/2 | 0/2 | 0.82 | 1.34 | 77 | 167 |

Answer to the plan's question: no. Off single-crate Rust, B0 does not beat
bare Claude Code on success (one accepted change fewer, on the docs issue),
costs 27% less in total but about the same per accepted change, and is
slower at the median. Both arms fail the kinds grade on the same four
issues, so the grade, not the arm, decides most outcomes here; #11283
(Kotlin, judge only) is accepted 2/2 by both. Six issues x 2 reps: the CIs
are wide and this is reported as measured. B0's single-crate S2 saving
(about 54% per accepted change) does not carry over to these kinds yet.

### Exploratory main round (before #11229, no judge)

Stopped at 101 of 189 trials; 12 issues. Fix tests passing: A 10/34, B0
12/34, Bbash 13/33; total $39.43, $19.62, $15.82; median s 230, 128, 188.
Briefed with Bash cost no more than briefed with `verify` here, so the
tools doc's guess that verify is the largest single gain is not supported
yet; it needs the frozen B0-vs-Bbash run.

## Learning loop

Lever 12 is now a loop (#11312). `scripts/work/learn.py` reads back what
the briefed runs logged and feeds the finder and the levers:

- `collect`: production runs (`scripts/work/work_issue.py` run folders on a
  work host, which is also where the website's `POST /v1/work` runs execute,
  uploaded daily by the timer from `scripts/work/host-setup.sh` through
  `scripts/work/learn-job/upload-ledger.sh`), A/B trials and uploaded ledgers
  go into one `openagents.work.learn.run.v1` ledger.
- `examples`: accepted runs become filefind training rows. The files the
  accepted change touched are positives, and the briefing's misses are
  half-weight extra positives. Issues in the held-out window are never
  trained on.
- `levers`: briefing size against misses and cost, by engine, as advice only.
  The production levers are set by #11309.
- `cycle`: `scripts/filefind/retrain.sh` under the embedding spend cap
  (Vertex embeddings only), then the gate's comparison on the held-out
  window, then the goldens guard. `--promote` runs `ranker_gate.py promote`
  only when both pass, and `record` adds a line to
  [filefind-promotions.md](filefind-promotions.md).

The weekly run uses `oa-learn-1` (e2-highmem-8, us-central1-b). That VM is
stopped between runs. Cloud Scheduler job `oa-learn-weekly` starts it every
Monday at 06:00 UTC. At boot the VM runs `scripts/work/learn-job/run.sh`,
which writes the report to `gs://openagentsgemini-filefind-learn/reports/DAY/`,
posts it as a comment on #11313, pushes a promotion if one passed, and powers
the VM off. Set up and operate it with `scripts/work/learn-job/setup.sh vm|install|schedule|run|pause|resume`.
Instance metadata `oa-learn-hold=1` keeps the VM up and skips the cycle.

### First cycle (2026-10-12 00:52 UTC, oa-learn-1)

The first cycle ran end to end on the VM. Its report is in
`gs://openagentsgemini-filefind-learn/reports/2026-10-11/` and on #11313.

- Runs read: 323 (8 production, 315 bench). Embedding spend: $0.03 of the
  $5 cap.
- Training rows: 26 labels on 4 issues already in the dataset, and 0 new
  production cases. All 4 accepted production issues (#11257, #11260-#11262)
  fall in the held-out window (#10819..#11359). They will train once they
  age out of it.
- Gate, 100 held-out cases: **kept the active model** (`sha256:b11f9b6b1b74`).
  The candidate (`sha256:1c952f2ccdde`) gained recall@50 +0.0034 (SE
  0.0120), short of the 2 SE needed. Its other numbers were recall@20
  −0.0057 (SE 0.0098), recall@100 −0.0125 (SE 0.0104) and Brier on the top
  100 −0.0116 (SE 0.0012, better). The active model was trained on 63 of
  the 100 evaluation cases, so its numbers are optimistic and the gate is
  harder to pass than it reads.
- Levers, production (8 runs): briefed 4/4 accepted at $0.26 per accepted,
  bare 3/4 at $0.62. Briefing recall of the accepted changes was 1.0 with 0
  misses. Bench (315 runs): briefing recall 0.498, 3.17 misses per briefed
  run. The most missed files are in `crates/coder/src/task/` (`local.rs` 61,
  `issue_run.rs` 54, `owner.rs` 39).

The gate decides on recall only. A ranker cannot be scored on cost per
accepted change offline, so cost per accepted is reported by the levers
section of each week's report and checked by #11309's bench, not by the
ranker gate. A promotion will come when production runs on issues past the
held-out window add signal, or when the window moves past the active model's
training range. Until then the loop correctly keeps the active model.

## Next

Frozen 2026-10-10 (3e57b96c4b), running on `oa-dev-env-2` (a GCE
`c3-standard-22` spot VM, agents, worktrees, builds and grading on one
machine; the Mac and coderos-4080 are no longer used) by
`scripts/bench/briefed-ab/run-env.sh`, which publishes each plan's report to
`scripts/bench/briefed-ab/results/` and powers the VM off when done:

1. `plans/ablate-tools.json`: B0 against Bbash, Brelated, Boutline, Bfinish
   (tier 1 of [briefed-agent-tools.md](briefed-agent-tools.md)).
2. `plans/ablate-levers.json`: B0 against Bsmall, Blarge, Bterse, Blow,
   Bsonnet.
3. `plans/ablate-cold.json`: B0 against a cold build cache, 7 issues.
4. `plans/s2b.json`: the quality study, A against B0 on 99 fresh issues
   chosen by `select_issues.py` (every single-commit Rust fix up to 1,200
   lines in this repository, S2's excluded). Judge acceptance is primary; a
   loss of 10 points or more is ruled out if the CI's lower bound is above
   −0.10 (about 80% power if the true difference is 0). That margin is a
   proposed amendment to the spec's equal-or-better gate, declared before
   results.

Each ablation reports every arm against B0 run in the same plan: the
success difference with its issue-level CI first, then the change in cost
per accepted change.

### Ablation results (#11309)

Each table is in `scripts/bench/briefed-ab/results/`, written by
`write_report.sh PLAN B0` (per-arm table, the frozen `--s2` analysis of every
arm against B0, and the same with judge 2, Gemini 3.1 Pro on Vertex via
`judge2.py`, in place of the frozen Claude judge). Keep rule, frozen in each
plan: success CI lower bound above -0.05 and cost per accepted below B0's with
the reduction CI's lower bound above 0. **No arm passed, so the production
levers in `common.LEVERS` stay at B0**, and the comment there cites these
arms.

| Plan | Arm | Accepted vs B0 | Cost per accepted vs B0 | Kept |
|---|---|---|---|---|
| ablate-tools | Bbash | +0.000 [-0.063, +0.063] | -9.1% [-31.4%, +14.0%] | no |
| ablate-tools | Brelated | -0.032 [-0.095, +0.000] | +10.8% | no |
| ablate-tools | Boutline | +0.000 [-0.063, +0.063] | -7.4% [-29.6%, +18.7%] | no |
| ablate-tools | Bfinish | -0.032 [-0.111, +0.032] | +10.8% | no |
| ablate-levers | Bsmall | +0.048 [+0.000, +0.127] | -15.4% [-44.8%, +4.1%] | no (cost CI crosses 0) |
| ablate-levers | Blarge | +0.032 [-0.032, +0.095] | -3.9% [-27.6%, +24.7%] | no |
| ablate-levers | Bterse | +0.000 [-0.079, +0.095] | -9.0% | no (success bound) |
| ablate-levers | Blow | -0.079 [-0.175, +0.000] | -21.0% | no (judge -30 points) |
| ablate-levers | Bsonnet | -0.095 [-0.238, +0.032] | -44.1% | no (judge -38 points) |
| ablate-cold | Bcold | -0.048 [-0.190, +0.095] | +1.1% | no |

ablate-levers ran in 7 shards on dev envs (`AB_SHARD=i/n`,
`remote/run-shard2.sh`) after the briefing trim loop fix (0d50f9e25c);
`merge_shards.py` unioned the archives (378 trials, one duplicate run kept
first). Under judge 2 no arm saves money either; Bsmall and Blarge tie B0 on
accepted at +0.000 [-0.048, +0.048].

## Acceptance on real issues (#11313)

The epic is done when a production run of the briefed flow on an open issue,
on Claude or Codex, is accepted at a lower cost per accepted change than bare
Claude Code on the same issue, three issues in a row. Three prepared issues,
small real defects of different kinds verified in the code:

| Issue | Kind | Defect | Check |
|---|---|---|---|
| #11416 | Rust | `town-clock` `Mode::parse("wall:-2147483648")` panics (`i32::MIN.abs()`) | `cargo test -p town-clock` |
| #11417 | Python | `scripts/report-kev-candidate.py` divides by zero on an empty group | a new `scripts/test_report_kev_candidate.py` |
| #11418 | Docs | `docs/verse/workshop-agent.md` links a moved `studio.rs` | every relative link resolves |

(The handoff's first docs defect, `docs/verse/zones.md` → `water.rs`, was
filed as #11338 and fixed before the run, so #11418 replaces it.)

`scripts/work/acceptance.py` runs both arms of each issue with
`scripts/work/work_issue.py`: bare Claude Code first with `--land none`, then
the briefed flow (`--engine briefed` or `briefed:codex`, landing a PR), both
from `origin/main`. An arm is accepted when its change passes the replayed
checks; the briefed flow wins an issue when it is accepted without falling
back to bare and costs less per accepted change (a rejected bare arm counts as
infinite cost). It stops at the first loss, writes `acceptance.json`, and
prints the table below. It needs `briefed-agent` installed and a Claude
sign-in, so it runs on a cloud dev env, not the Mac:

    python3 scripts/work/acceptance.py --issues 11416 11417 11418 \
        --out ~/.openagents/work/acceptance/run1
    python3 scripts/work/acceptance.py --report ~/.openagents/work/acceptance/run1

Other blitz agents fixed all three by hand on 2026-10-12 (#11422, #11421,
#11420), so the three are closed. The run now replays them: `--replay` finds
the commit on `origin/main` that names `(#N)` and starts both arms from its
first parent, where the defect is still present; `work_issue.py --base <sha>
--allow-closed` works a closed issue from that commit and lands nothing. The
hand fix is recorded beside each arm for comparison. Bases:

| Issue | Base (fix's parent) | Hand fix |
|---|---|---|
| #11416 | f413f408ee | e4699af065 |
| #11417 | 73943d3992 | f413f408ee |
| #11418 | 610e829e1d | 73943d3992 |

    python3 scripts/work/acceptance.py --replay --issues 11416 11417 11418 \
        --out ~/.openagents/work/acceptance/replay1

A replay cannot show the landing step; three new open issues are still needed
for a run that lands. Results: not run yet (waiting on #11308, #11309 and
#11311 on main, and a cloud dev env with `briefed-agent` and a Claude
sign-in).
