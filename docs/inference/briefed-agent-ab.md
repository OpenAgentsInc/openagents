# Briefed agent vs bare Claude Code (#11211)

Status: in progress, 2026-10-10. Pilot done; the main round is running. The
numbers below are replaced as rounds finish.

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

Results: see below (filled in as rounds finish).
