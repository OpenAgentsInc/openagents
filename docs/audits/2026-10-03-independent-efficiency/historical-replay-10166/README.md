# Historical replay: ignored worktree files

Recorded October 3, 2026 UTC (October 2 in America/Chicago).
Audit issue: [#10280](https://github.com/OpenAgentsInc/openagents/issues/10280).

**The briefing did not beat the historical implementation on correctness.
It also did not show a consistent cost or speed advantage over a fresh
control.** All four fresh candidates passed their own tests but failed the
same independent case. The historical implementation passed all seven cases.

This is an actual coding experiment: four fresh Claude sessions produced
patches against a historical repository export. It measures the first
implementation attempt with file tools and subsequent external verification.
The original agent also had command execution and repair feedback, so this
experiment does not establish an end-to-end agent ranking.

## Task and reconstruction

[#10166](https://github.com/OpenAgentsInc/openagents/issues/10166) asked the
background cleanup service to preserve ignored user files when removing an
ended task's worktree. A worktree with `.env` or private data should remain;
one containing only recognized build caches should still be removable.

The original Claude child began with a single explicit task on October 2 at
15:51:08 UTC. There was no intervening human correction before the fix. This
was a clean task boundary, although its instructions included coordination,
an investigation of an earlier cleanup, and deployment. The replay uses the
implementation, regression-test, and documentation portion of that request.

| Input | Pin |
| --- | --- |
| Original checkout and replay source | `4705102273140a5f381fb75e17a29965662629c8` |
| Historical fix | `d7f0ab8c6c7f795363ced901b8e08b35db204893` |
| Frozen briefing implementation | `45dffd63ff35585e6b4273ae7b45196b3694c7a6` |
| Briefing binary SHA-256 | `f895afd212856bfe140c58b2b9f624aa3eeaa4b9a59467556321411bd9ffaec9` |
| Executor | Claude Code 2.1.287, `claude-opus-5-5`, medium effort |
| Per-run bound | 600 seconds, $10 CLI budget |

The historical issue title and body match the retained launch-time output;
the comparison removes only the CLI's trailing newline. Another agent landed
#10167 before the original fix, so the fix's parent is a different commit.
Using that parent would reconstruct the wrong starting state.

Each executor received a separate full `git archive` export of the original
checkout, without Git objects, remotes, or later files. Restricted mode
confined file tools to its working directory. Available tools were exactly
`Read`, `Edit`, `Write`, `Glob`, and `Grep`; the emitted initialization records
confirm this. Safe mode disabled personal customizations, and MCP servers,
web access, command execution, delegation, and session persistence were
disabled. Existing local authentication stayed on the Mac. Cargo ran only
on Boat, with a temporary home and the retained agent target directory.

Both conditions received the same [task](task.txt). The treatment additionally
received the unedited [generated briefing](briefing.md): default lexical,
symbol, and ancestor-history evidence plus execution facts for
`crates/background/Cargo.toml`. There were no Tree-sitter spans, prior
attempts, Jev calls, or hand-selected solution hints. Its command suggestions
were explicitly reserved for the external verifier. This tests the new
briefing prototype with Claude; it does not test the production Microcoder
controller.

## Results

| Run order | Condition | Agent seconds | CLI list-price estimate | Own tests | Independent cases | Formatting |
| ---: | --- | ---: | ---: | ---: | ---: | --- |
| 1 | Control 1 | 87.49 | $0.780317 | 22/22 | 6/7 | Fail |
| 2 | Briefing 1 | 77.59 | $0.632174 | 22/22 | 6/7 | Fail |
| 3 | Briefing 2 | 97.57 | $0.699192 | 22/22 | 6/7 | Pass |
| 4 | Control 2 | 95.60 | $0.663235 | 22/22 | 6/7 | Fail |

The first pair favored the briefing. The second pair favored the control on
both time and cost. Across two runs each, briefing means were **87.58 seconds
and $0.6657**, versus **91.55 seconds and $0.7218** for the control. Those
aggregate differences, 4.3% and 7.8%, are descriptive; they do not establish
an improvement. Each condition produced **zero accepted results**, so no
finite cost per accepted result can be estimated from these runs.

The reverse-order pair was added after viewing the first pair's usage and
before its acceptance results were available. All four outputs are retained.
Model, effort, tools, task, briefing, checker, and limits stayed fixed. This
reduces dependence on a single favorable run but is not a randomized study.
Provider cache state was not reset or controlled.

The total reported model cost was **$2.7749**. Claude reports `costBasis: list`;
these are usage estimates, not verified subscription charges. They exclude
benchmark engineering, orchestration, and Boat charges. Each candidate's
external tests, formatting check, and independent check took about 12 seconds
combined, including warm compilation. Those times are recorded separately
from the agent turn. None of these wall times is time to an accepted fix.

Briefing assembly took **0.674 seconds** externally, including writing its
outputs. The separate index build took **5.506 seconds**. Compilation of the
already-built prototype and source export are excluded from those figures.
This was one timing sample, not a latency distribution. See
[measurements](measurements.json), [protocol](protocol.json), and
[preparation timing](briefing-timing.json).

## What failed

The independent [checker](ignored_worktrees.rs) tests ignored `.env`, private
data, mixed caches and private data, cache-only worktrees, clean worktrees,
ordinary unsaved changes, and unpushed commits. It uses temporary repositories
and a local bare remote. Its source was frozen before inspecting candidate
patches and remained outside every executor directory. Calibration followed
the first model launch: the original source failed the three ignored-data
checks, while the historical `git.rs` transplanted onto that same base passed
all seven. The transplant excludes the concurrent trigger change in #10167.

All fresh candidates failed the cache-only case. Their command,
`git ls-files --others --ignored --exclude-standard --directory`, reports
`web/` when that directory contains only ignored `web/node_modules/`. They
classify the returned parent name as unknown user data and refuse cleanup:

```text
Recognized ignored caches must not block cleanup: "holds ignored files: web/"
```

This refusal preserves data, but it does not satisfy the requested cache
cleanup behavior. Their new tests cover root-level caches and miss the nested
case. Passing a candidate's own regression test therefore would have produced
a false completion signal in all four runs. No candidate was repaired by the
researcher or applied to product code.

The historical agent hit a related test failure, inspected actual Git output,
and changed to `git status --ignored=matching`. The retained conversation
records that probe and correction. That feedback opportunity is absent from
this first-attempt replay and is a material limitation of the comparison.

Exploratory code review also found that the first briefing candidate treats
cache-like regular filenames as disposable, and both first-pair candidates
treat a `.DS_Store` directory as disposable. These findings were not added to
the frozen seven-case score. They motivate filesystem-type tests; seven
passing fixtures alone would not prove cleanup safe for every path.

## Observed reading differences

In the first pair, Read and Grep output fell from **129,129 to 52,848 bytes**.
The briefing added 43,296 prompt bytes; prompt plus reading output still fell
from 131,907 to 98,922 bytes. These byte counts include tool formatting and
exclude other messages, so they are not token counts.

The control read all of `AGENTS.md` and `plan.rs`. The briefing agent read only
60 lines of `plan.rs` and did not separately read `AGENTS.md`. Both reread the
entire `git.rs`; source-search calls were eight versus nine. Smaller reads,
rather than fewer searches, explain the observed reduction in returned text.
Recorded cache creation fell from 62,056 to 49,348 tokens, cache reads from
533,365 to 367,950, and output from 8,855 to 8,186.

Skipping full instructions cannot automatically count as useful efficiency.
The brief supplied only the first 64 lines of `AGENTS.md`; it did not certify
coverage of every applicable obligation. The next experiment must preserve
mandatory instructions identically before comparing optional source context.

Retrieval itself was weak. The brief chose `git.rs` but included lines 4–67,
ending before the faulty `removable` implementation. It omitted the test file,
the named background spec, and `plan.rs`. Seven other ranked source files
concerned Coder execution or delegation. The issue already named the faulty
path and lines, yet the packer spent its budget elsewhere. This does not
demonstrate successful fault localization.

## Historical comparison

The original took **178.069 seconds from initial task to successful push**,
with 19 Bash calls. First edit to push was 95.909 seconds. The full first
completion took about 8 minutes 24 seconds because it also included host
deployment and the cleanup investigation; a later stale-monitor notification
must not be counted as implementation time.

The historical implementation passes the seven-case checker, and its transcript
records successful crate tests after rebasing. No structured monetary cost was
recorded for that child, so this report does not invent one. Historical usage
is deduplicated by assistant message ID, taking the maximum of each counter
across streaming records, and tool calls by tool-use ID. Repeated cached input
is not unique context. See [historical aggregates](historical-metrics.json).

The useful conclusion is that the original's small runtime investigation
found a fact all four fresh file-only agents missed. Comparing 178 seconds
directly with the fresh 78–98-second turns would mix different tasks, tools,
feedback, environments, and completion criteria.

## A concrete preparation component

The standalone [Git probe](probe_git.py) now reproduces the missing fact in a
temporary repository. It creates fixtures, stages two ordinary files, and
reads Git output; it never removes a worktree or accesses owner repositories.

| Probe | Mac, Git 2.50.1 | Boat, Git 2.43.0 |
| --- | ---: | ---: |
| Fixture construction and both command observations | 41.6 ms | 26.2 ms |
| `ls-files --directory` result | `web/` | `web/` |
| `status --ignored=matching` ignored result | `!! web/node_modules/` | `!! web/node_modules/` |

These are single samples of the script's internal elapsed time, excluding
interpreter startup and transport. The probe was written after the failure
and was not given to any executor. Its speed and diagnostic value are
measured; whether including it improves agent acceptance still needs a new
controlled experiment.

## Next experiments

| Component | Isolated experiment | Success criterion |
| --- | --- | --- |
| Explicit source anchors | Resolve issue paths and line ranges before lexical ranking; use a Rust AST or Tree-sitter to include the containing function. | Include the complete `removable` body under a fixed byte budget; label stale anchors. |
| Tests, callers, and specifications | Retrieve named docs, the affected caller, and existing temp-repository helpers as separate requirements. | Cover each required role without adding unrelated execution files. |
| Cheap executable facts | Run bounded Git fixtures for nested caches, mixed data, unusual filenames, and empty directories. | Record exact outputs, versions, duration, and failures within the preparation budget. |
| Filesystem facts | Separate directory names, regular files, and symlinks before judging cache eligibility. | Unknown user data remains protected; a filename alone cannot authorize deletion. |
| Instruction coverage | Build the applicable instruction set separately from optional evidence. | Every required obligation remains available in both conditions. |
| Test and repair feedback | Give each condition the same external test interface and one bounded repair opportunity. | Compare total cost and wall time through acceptance, retaining failed attempts. |
| System One selection | Give a deterministic selector and a typed relevance judgment the same candidate pool, including correct spans and matched distractors. | Improve requirement coverage per byte without overriding facts or permissions; abstain when uncertain. |

Run the next paid comparison on a frozen panel of additional historical
tasks. Keep this case as a development regression after using its result to
design changes. Test source packing, executable probes, and repair feedback
as separate additions so an improvement can be attributed to a component.
System One could help select evidence or nominate an unresolved question for
a probe. Deterministic code should establish filesystem types, command
results, and cleanup permissions.

## Reproduce and inspect

- [Control 1 patch](baseline/candidate.patch),
  [briefing 1 patch](briefing/candidate.patch),
  [briefing 2 patch](briefing-2/candidate.patch), and
  [control 2 patch](baseline-2/candidate.patch) are unmodified candidate outputs.
  Each directory also retains sanitized metrics and verification logs.
- [run_arm.py](run_arm.py) takes `--workspace`, `--prompt`, and a new `--output`
  directory. Export the source pin into a fresh directory for every invocation.
  Verify `claude --version` is 2.1.287; the script uses the CLI on `PATH`.
  A run with another CLI version is a replication with a changed executor.
  Use `task.txt` for the control. For treatment, append the literal preamble in
  `protocol.json` and `briefing.md`. Keep the checker and other runs outside the
  executor directory. The script pins the model, effort, tools, and limits.
- To regenerate preparation, use the pinned `briefing-lab` binary: `index`
  at the source pin, then `preview --execution --manifest
  crates/background/Cargo.toml --environment-id
  historical-replay-boat-verification` with the retained `issue.json`.
  Artifacts must be outside the inspected repository. Its environment
  fingerprint will differ on another machine.
- Apply each candidate patch to a fresh historical export on Boat. Run
  `cargo test -p background`, `cargo fmt -p background -- --check`, and
  `python3 check.py --repo EXPORT --run-dir NEW_DIR --target-dir
  LONG_LIVED_AGENT_TARGET`. The independent runner uses the pinned Rust 1.97.1
  toolchain and an offline external harness; dependencies must be cached.
  Give each check process a temporary `HOME`, retaining the existing
  `CARGO_HOME` and `RUSTUP_HOME` so the pinned compiler and dependency cache
  remain available. The experiment saved their existing values, falling back
  to the original home's `.cargo` and `.rustup`, then changed `HOME` only in
  the child process environment. Keep `CARGO_TARGET_DIR` at the external
  long-lived agent target and set `CARGO_INCREMENTAL=0`. The checker inherits
  this environment; it does not create an isolated home itself.
- `python3 probe_git.py` runs the small Git observation on its own.

Original conversations, raw model event streams, model reasoning, account
identifiers, and authentication remain private. Published event-stream
digests bind the summaries to those retained records without publishing them.
