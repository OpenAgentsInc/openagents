# Briefed agent vs bare Claude Code (#11211)

Status: in progress, 2026-10-10. Pilot and an exploratory main round done;
the pre-registered S2 run (below) is running. Its result replaces the
exploratory numbers.

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
