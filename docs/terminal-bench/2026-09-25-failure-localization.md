# Failure localization offline: error context, first mismatch, and phase timing

2026-09-25. Issue
[#9658](https://github.com/OpenAgentsInc/openagents/issues/9658), item 3
of the [Fable pattern map's build list](2026-09-25-fable-pattern-map.md#the-build-list).
No Luna session, no Terminal-Bench trial, and no Jev request was made.

## Result

**Built, measured offline, and left off. Luna already localizes the
failures a component could localize, and there are few of them.**

- **The error context would rarely have anything to print.** Of 582
  failing commands in 553 retained Microluna sessions on 45 tasks outside
  the mapped eleven, 86 printed a file and line the parser reads (14.8%,
  95% Wilson 12.1–17.9%), and 41 of those named a file of the workspace
  (7.0%, 5.2–9.4%). The protocol's bar for a component worth running was
  30%. This is a negative result.
- **When an error names a workspace line, the next edit usually goes
  there.** Of the 21 localized failures whose next edit changed the named
  file, 16 changed a line within 6 lines of the named one (76%, 55–89%),
  against a chance rate of 34% for the same edits around a random line.
  Counting every next edit, other files included, it's 16 of 29 (55%,
  38–72%). This is an association, not a causal claim: the session read
  the same output the component would have printed from.
- **There is little re-reading to remove.** After a localized failure and
  before the next edit, 10 turns only re-read the named file (7 of them
  showed the named line): 0.15% of all 6,591 turns, and 1.2% of the 860
  read-only turns. The protocol's bar was 2% of all turns. This is a
  negative result.
- **Plain output rarely shows a failing case.** The mismatch parser found a
  case in 3 of 582 failing outputs (0.5%), all `unittest`-style, none with
  stages. The mismatch trace's value depends on structured results from
  `checks.oracle` (#9656), which it now reads first.
- **Timeouts are rare and mostly not profilable in depth.** 28 of 582
  failing commands timed out (4.8%). `evidence.phase_timing` would profile
  3 of them with `cProfile` and 1 with a shell trace; the other 24 would
  get only `perf stat`, `/usr/bin/time`, or the wall clock.

The three components, their policy switch, the in-session delivery, the
offline replay, and the tests are in the repository. The switch,
`executor.microluna.lean.localize`, is absent from every manifest, and
nothing here justifies a matched live run. Time to a passing check can't
be measured offline, because no retained session ran with a component on.

## What was built

All three are code operations in `crates/coder-one/src/localize/`. None
asks a model.

- **`evidence.error_context`** (`localize::context` and
  `localize::parse`). One table of rules (`parse::RULES`) reads file and
  line references: Python tracebacks and Coq, Rocq, and OCaml errors
  (`File "p", line n`), rustc diagnostics and Rust panics, GCC, Clang, Go,
  javac, scalac, Lean, and pytest (`p:n:` and `p:n:c:`), JVM stack frames,
  Node stack frames, Go goroutine stacks, and TypeScript and MSVC
  diagnostics (`p(n,c):`). Within one output, the innermost traceback frame
  comes first. References resolve to workspace files, by the workspace
  root or the longest trailing path that is a file; the standard library,
  installed packages, and toolchains never resolve. Each region is printed
  with 6 lines on each side, regions in one file merge, repeats are
  dropped, and the most recent failure comes first, within 6 regions and
  6,000 characters.
- **`evidence.mismatch_trace`** (`localize::mismatch`). The first failing
  case of a shared acceptance result (`checks::acceptance`, #9656) comes
  first: its case, the input or stated parameter value it covers, and
  observed against expected. Otherwise the first case in the output: an
  oracle's JSON lines, pytest's `assert a == b`, `unittest`'s `a != b`,
  Rust's `left:` and `right:`, Go's `got x, want y`, Jest's `Expected:` and
  `Received:`, or labeled `expected:` and `got:` lines. A format that
  doesn't say which side is expected reports left and right, not observed
  and expected. A diff names the first differing character or line. When
  the check exposes stages, they're compared in order and the first stage
  that differs is named, with the agreeing stage before it.
- **`evidence.phase_timing`** (`localize::timing`). A command that timed out
  or used at least half its bound is run once more on a scratch copy of the
  workspace, through `verify.executed`'s runner. A Python program runs under
  `cProfile`, with `faulthandler` dumping the stack if the bound passes
  first. A shell script runs under `bash -x` with a timestamp on each
  top-level command. Anything else runs under `perf stat` or
  `/usr/bin/time -v` when the host has one, and with the wall clock
  otherwise. The report lists functions by cumulative time, the slowest
  top-level commands with their share, or CPU against wall time.

**The switch.** `executor.microluna.lean.localize` takes `error_context`,
`mismatch_trace`, and `phase_timing`, each off by default, plus `window`
and `timing_sec`. After every work session, the loop reads the session's
log and the host's own results (the frozen score's output and the commands
`verify.executed` reran) and puts what the components find in the next
brief. With `in_session`, the error context also reaches the running
session after the turn whose command failed, through the host's watch
(the path #9627's stall check uses), at most 3 times a session. The switch
is absent from every manifest, so no digest changes.

## How it was measured

The [protocol](../../bench/terminal-bench/experiments/2026-09-25-failure-localization/protocol.md),
the parser table, and the measurement code were committed and pushed
(`58e33de5de` and `d1c71387b3`) before any retained command output was read.

- **Data.** Every retained Microluna session log under
  `~/.openagents/terminal-bench/jobs`,
  `~/.openagents/terminal-bench/microluna-jobs-9585`, and
  `bench/terminal-bench/traces`: 226 trials, 553 sessions, 6,591 turns, and
  4,842 commands on 45 tasks, 43 of them with a failing command.
  `sources.json` pins every file by SHA-256.
- **Excluded tasks,** before their records were read: the eleven tasks
  the pattern map read (61 retained trial directories).
- **Failing command:** exit status not 0, or timed out. **Resolved:** a
  reference names a file the trial's briefs carried or its sessions read,
  wrote, or patched.
- **Next edit:** the session's files are followed call by call from the
  brief, with each patch applied by Microluna's own patch code; the lines
  an edit changed come from a line diff. The chance rate is the share of
  the file's lines around which a 13-line window would contain a changed
  line.

## Results

| Measure | Count | Rate (95% Wilson) |
| --- | --- | --- |
| Failing commands | 582 | |
| Output names a file and line | 86 | 14.8% (12.1–17.9%) |
| A reference resolves to a workspace file | 41 | 7.0% (5.2–9.4%) |
| Resolved, of parsable | 41 of 86 | 47.7% (37.4–58.1%) |

References by rule, counted once per failing command: Python-style
`File "p", line n` 59, JVM frames 19, `p:n:` 8, and `p(n,c):` 2.

What the next edit did after the 41 localized failures:

| Next edit | Every localized failure | Last failure before each edit |
| --- | --- | --- |
| Changed the named region | 16 | 14 |
| Changed the file, elsewhere | 5 | 5 |
| Changed other files only | 8 | 8 |
| No edit followed | 12 | 11 |
| Undetermined | 0 | 0 |
| Region, of same-file edits | 16 of 21, 76% (55–89%) | 14 of 19, 74% (51–88%) |
| Chance for the same edits | 34% | 27% |
| Region, of all edits | 16 of 29, 55% (38–72%) | 14 of 27, 52% (34–69%) |

The 16 region hits come from 11 tasks. Of the 12 localized failures no edit
followed, 8 are one task's (`regex-chess`).

Re-reads after a localized failure, before the next edit: 10 turns, 7 of
which showed the named line. That's 0.15% of all turns and 1.2% of
read-only turns; 9 of the 41 localized failures (22%, 12–37%) were followed
by at least one.

## Why so few failures localize

This breakdown was computed after the frozen results were read. It changes
no number above.

- **Many failing commands aren't program failures.** 148 of the 582 exited
  127 (command not found), and 42 exited 100 (`apt-get`). `cat`, `ls`,
  `sed`, and `mkdir` together account for 128.
- **Parsable references that don't resolve** are mostly correct misses:
  frames of a shipped Java tool with no source in the workspace
  (`gsea-proteomics`, 17 commands), and the standard library, `numpy`, and
  `subprocess`. The rest are scripts the session itself wrote outside the
  workspace or piped in (`/tmp/*.py`, `<stdin>`), which the live component,
  reading only the workspace, would miss too.

## What this means for the components

- **`evidence.error_context`** duplicates what Luna already does with the
  output it reads: it goes to the named line, and it rarely re-reads. A
  live run wouldn't show much. Its one untested use is output the session
  never saw, such as the frozen score's or `verify.executed`'s reruns,
  which the between-session note reads; the retained logs can't measure
  that.
- **`evidence.mismatch_trace`** can only help once a structured acceptance
  result exists. It reads `checks::acceptance` first, and the lean loop
  passes it one when `checks.oracle` is wired in.
- **`evidence.phase_timing`** fires rarely, and mostly where it can only
  report CPU against wall time. The metric-target work (#9657) is the
  better place to test it, since a stated performance target makes timing
  the task.

## Replay

```sh
cargo build -p coder-one
python3 bench/terminal-bench/experiments/2026-09-25-failure-localization/replay.py \
    --coder-one "$CARGO_TARGET_DIR/debug/coder-one"
```

It reruns `coder-one component replay evidence.localize` over the same
roots with the same exclusions and checks that the rows, the summary, and
the pinned sources match `records/`. It runs no command from the logs and
asks no model. It needs the retained logs on this machine.

## Files

- `crates/coder-one/src/localize/`: the components, the log reader, the
  offline measurement, and the tests.
- `crates/coder-one/src/micro/localize_hook.rs`: the lean loop's hooks and
  the in-session watch.
- `bench/terminal-bench/experiments/2026-09-25-failure-localization/`: the
  protocol, the replay, and `records/` (`rows.jsonl`, `summary.json`, and
  `sources.json`).
