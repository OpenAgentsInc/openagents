# The finish rule for Microluna, measured offline and on mini-tasks

2026-09-25. Issue
[#9638](https://github.com/OpenAgentsInc/openagents/issues/9638), change 7
of [Microluna v18](../coder/design/microluna-v18.md), part of
[#9640](https://github.com/OpenAgentsInc/openagents/issues/9640).

**The rule costs almost nothing where Luna already tests after its last
edit, and it rarely fires on the retained logs.** Across every retained
Microluna session log on this host, 0 of 31 `done` finishes would have been
refused (95% Wilson interval 0–11%). Across the retained Luna Codex
streams, 11 of 180 would have been refused (6%, 3–11%), and 2 of those 11
were on trials the verifier failed (18%, 5–48%). On the Luna TB4 baseline's
23 graded failures with a retained stream, 1 would have been refused
(4%, 1–21%). On 24 matched mini-task runs, the rule refused 4 finishes in
54 sessions, turns per session stayed level (8.8 against 8.7), and passes
were 8 of 12 with the rule against 7 of 12 without it. The rule is
admitted to v18 as a rule that costs nothing, not as a predictor of
failure: a refusal on the retained logs didn't mark a failing trial. It
stays off in every manifest until the #9640 close-out writes v18.

This makes no live Terminal-Bench claim.

## What was built

- **The rule** (`crates/microluna/src/finish.rs`). The session loop folds
  every call into a `Ledger`. An applied `apply_patch` or `write_file` is
  an edit, and so is a command that changed a workspace file: while the
  rule is on, the host lists the workspace (size and modification time,
  hidden directories, `__pycache__`, `node_modules`, and `target` left
  out, at most 20,000 files) before and after each command that doesn't
  only read, and records the changed files under `changed` in the
  command's step. A command that names `score.sh` other than as a
  redirection target, and doesn't only read, is a score run. A command
  that contains a baseline command, spaces collapsed, is a baseline run. A
  score or baseline run's own file changes are its outputs, not edits.
- **The verdict.** A `finish` with status `done` is refused unless the
  score and a baseline command both ran after the last edit. The refusal
  is the call's result, not a host turn-back, so it doesn't count against
  `persist.max_returns`. It names the file edited last: "The host refused
  this finish: you edited `headless_terminal.py` after your last run of
  the score. Run the score (`score.sh`) and read the result, then call
  finish again." After three refusals, the next `done` finish stands and
  the session's record says `unverified`. `blocked` and `failed` finishes
  aren't gated. A session that never edited meets the rule.
- **Baseline commands are an input.** `FinishRule::baseline` holds them.
  With none, only the score is required. The lean loop passes an empty set
  today, from `baseline_commands()` in `crates/coder-one/src/micro/lean.rs`;
  `evidence.baseline` ([#9633](https://github.com/OpenAgentsInc/openagents/issues/9633))
  fills it.
- **The switch.** `executor.microluna.lean.finish_rule`, with
  `max_refusals` (default 3), turns it on for the lean loop's sessions and
  lanes. It needs `keep_best`, whose evaluation script is the score. It is
  absent from every manifest, so no manifest's digest changes.
- **The record.** Each session in the loop record carries
  `finish_refusals` and `unverified`, and its invocation summary carries
  `finish_rule: {refusals, verified}`.
- **Fixtures.** `control.finish` runs the rule alone on five fixtures under
  `crates/coder-one/fixtures/components/finish--*`: finish after the
  score and a baseline command (allowed), an edit after the score
  (refused, naming `statistical_tests.py`), no baseline command available
  (the score alone is required), the fourth `done` finish after three
  refusals (accepted as `unverified`), and a `blocked` finish (not gated).
  `crates/microluna/tests/finish.rs` runs the same cases as whole sessions
  on the fake transport, plus a shell edit caught by the listing and a
  refusal that leaves the turn-back budget untouched.

## Offline measurement

`coder-one component replay control.finish --out DIR` reads every trial
under `bench/terminal-bench/traces/`, finds each `done` finish, and judges
it on the session's history up to that finish. It asks no model. The rows
and the summary are in
[`records/`](../../bench/terminal-bench/experiments/2026-09-25-finish-rule/records/),
from implementation `finish-rule-v1` (digest `f405f2b8…0190`).

**Population.** Every Microluna session log (`microluna-*.atif.jsonl`, 36
logs in 18 trials of the v12, v13-retained, and evidence-v1 arms) and
every Luna Codex stream (`native/codex.txt` for `codex-gpt-6-luna`, and
the Codex-format `delegate-*.stream.jsonl` of the Coder One Luna arms and
`luna-jev`; 185 streams). Excluded by rule: the eight tasks of the #9584
cohort and jobs named `truth-confirmation` or `truth-control`; none of
these appeared under the traces. Eighteen Luna trials have no retained
stream (16 older `codex-gpt-6-luna` trials and two `luna-jev` trials) and
aren't counted; `summary.json` lists them. Every counted finish has a
verifier reward.

Task exclusions are measurement inputs, supplied by `--exclude-task` in the
reproduction command below. They are no longer hardcoded in product source:
that made the strict contamination guard reject unrelated live trials before
setup ([#9642](https://github.com/OpenAgentsInc/openagents/issues/9642)). The guard
remains unchanged. The replay records the supplied list and keeps the original
`excluded_sealed` count field; its finish rule and historical labels are unchanged.

**Two rules.** Microluna sessions ran the lean loop, so the rule is the
shipped one: score `score.sh`, no baseline commands. Codex sessions had no
host score, so the replay uses a proxy: a command that runs code (an
interpreter, a test runner, a build, or a script; not reading, moving, or
writing a file through a here-document) counts as the score run. A Codex
session has no typed finish, so one that ends with a final message and no
error counts as one `done` finish. A looser proxy, where any command that
doesn't only read counts, is reported beside it.

**Edits the logs can see.** Tool edits, here-document writes, and `sed -i`
or `perl -i` edits. A Python program that rewrites a source file isn't
visible, and 78 of the 180 Codex finishes show no edit at all, so they
pass the rule by default. The counts are lower bounds on what the live
rule, which lists the workspace, would refuse.

**No threshold was chosen.** The rule's only parameter, three refusals,
comes from the issue. Nothing was tuned on any task, so the frozen split
of the executed-contract protocol isn't needed; the counts are
descriptive.

| Population | `done` finishes | Refused | Verifier failed, of refused | Refused, of verifier-failed |
| --- | ---: | ---: | ---: | ---: |
| Microluna, shipped rule | 31 (18 trials) | 0 (0–11%) | none refused | 0 of 21 (0–15%) |
| Luna Codex, proxy | 180 (180 trials) | 11 (6%, 3–11%) | 2 of 11 (18%, 5–48%) | 2 of 48 (4%, 1–14%) |
| Luna Codex, loose proxy | 180 | 1 (1%, 0.1–3%) | 1 of 1 (21–100%) | 1 of 48 |
| Luna TB4 baseline (#9583), proxy | 23 (23 trials) | 1 (4%, 1–21%) | 1 of 1 (21–100%) | 1 of 23 (1–21%) |
| All, shipped rule and proxy | 211 (198 trials) | 11 (5%, 3–9%) | 2 of 11 (18%, 5–48%) | 2 of 69 (3%, 1–10%) |

Intervals are 95% Wilson intervals. Finishes within a trial aren't
independent, but only Microluna has more than one per trial, and at the
trial level the Microluna counts are 0 of 18 refused and 0 of 13 failing
trials refused.

What the refusals were:

- Ten of the eleven Codex refusals are `fix-git` runs: Luna resolved the
  merge conflict in `_includes/about.md` or `_layouts/default.html` and
  finished after `git` commands only. Nine of those ten passed. The task
  has no code to run, so the proxy's refusal is a false alarm there; in the
  lean loop the task would have a `score.sh`, and running it would meet
  the rule.
- The eleventh is the `codex-gpt-6-luna` baseline run on
  `embedding-drift-monitor`, which edited `drift_monitor/normalize.py`
  after its last run and failed.

On the Luna TB4 baseline, 17 of 23 failing finals claimed success or said
Luna hadn't tested
([baseline](2026-09-24-luna-tb4-baseline.md)). The rule would have caught
one of the 23 by its command history: most of those sessions ran code
after their last visible edit, even when their final message said
otherwise, or edited only through programs the log doesn't show as edits.

## Mini-tasks, matched against `microluna-v15`

Each of the four mini-tasks ran three times under
`crates/coder-one/policies/microluna-v15.json` (SHA-256 `161f51c7…ad0b`)
and three times under the same manifest with `finish_rule` on
([`microluna-v15-finish-rule.json`](../../bench/terminal-bench/experiments/2026-09-25-finish-rule/microluna-v15-finish-rule.json),
SHA-256 `8cc2bb57…0765`), on Luna through the Codex login, 900 seconds
each, with `SUPERVISE_MEMORY_MAX=off` because `setrlimit(RLIMIT_DATA)`
fails on this host. Per-run rows are in
[`records/minitasks.jsonl`](../../bench/terminal-bench/experiments/2026-09-25-finish-rule/records/minitasks.jsonl),
made by
[`minitasks.py`](../../bench/terminal-bench/experiments/2026-09-25-finish-rule/minitasks.py).

| Task | v15 passes | With the rule | v15 turns per session | With the rule | Refusals |
| --- | ---: | ---: | ---: | ---: | ---: |
| `cancel-cleanup` | 2 of 3 | 1 of 3 | 8.0 | 8.7 | 1 |
| `git-recovery` | 3 of 3 | 3 of 3 | 6.8 | 5.3 | 0 |
| `interactive-terminal` | 2 of 3 | 3 of 3 | 10.1 | 10.5 | 3 |
| `log-severity` | 0 of 3 | 1 of 3 | 10.2 | 9.4 | 0 |
| All | 7 of 12 | 8 of 12 | 8.7 (58 sessions) | 8.8 (54 sessions) | 4 |

Spend was $0.139 for v15 and $0.124 with the rule. No finish was accepted
as unverified. Each refusal named the file the session had edited after
its last score run (`headless_terminal.py` three times, `run.py` once).
After three of the four, the session ran `score.sh` and finished again;
after one, it ran a `python3 -c` check instead, was refused a second time,
and then ran the score. At three attempts a task, the pass difference is
noise.

## Not done

- **Baseline commands.** The rule requires only the score until #9633
  supplies the task's baseline commands. The replay can't measure the
  baseline half: no retained log knows which commands those are.
- **Shell edits in the retained logs.** Edits through programs are
  invisible offline; the live rule sees them through the workspace
  listing, which no retained trial had.
- **No Terminal-Bench run.** The v18 manifest and any live claim belong to
  the #9640 close-out.

## Reproduce

```sh
cargo build -p coder-one
target/debug/coder-one component replay control.finish \
  --exclude-task distributed-dedup --exclude-task formal-crypto \
  --exclude-task freecad-impeller --exclude-task freecad-spring-clip \
  --exclude-task math-eval-grader --exclude-task pretrain-shard-corruption \
  --exclude-task shadow-relay --exclude-task vpp-loss-divergence \
  --out bench/terminal-bench/experiments/2026-09-25-finish-rule/records
target/debug/coder-one component suite control.finish --no-record
```
