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
| 12 | Learning loop | misses logged per trial for #11210 | logged |
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

### Exploratory main round (before #11229, no judge)

Stopped at 101 of 189 trials; 12 issues. Fix tests passing: A 10/34, B0
12/34, Bbash 13/33; total $39.43, $19.62, $15.82; median s 230, 128, 188.
Briefed with Bash cost no more than briefed with `verify` here, so the
tools doc's guess that verify is the largest single gain is not supported
yet; it needs the frozen B0-vs-Bbash run.

## Next

- Extend S2 prospectively (new frozen plan, more issues) to settle quality.
- Tier 1 per [briefed-agent-tools.md](briefed-agent-tools.md), one at a
  time on B0 under the same rules: Bbash, `related`, `outline`/`read_symbol`,
  `finish`. Arms and harness are ready (`ab.py` arms Bbash, Brelated,
  Boutline, Bfinish).
- One-lever ablations (arms Bsmall, Blarge, Bterse, Blow, Bsonnet, Bcold).
- "Guesses to check": none is settled by S2, which compares only A and B0.
