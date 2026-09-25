# Measure an issue-flow change on past issues

The issue flow (`crates/coder-one/src/issue_turn.rs`) turns a GitHub issue
into a draft pull request: a worktree, the Microluna loop, a review session,
the pre-pull-request gate, a push, and the pull request. Every change to it
through 2026-09-24 was fitted on one issue, #9597, over 14 attempts (pull
requests #9600 to #9623). Those attempts are in-sample development history,
not evidence that the changes help on other issues.

The issue-flow evaluation set measures a change on more than one issue. It
holds eight past issues from this repository, each pinned to the commit
before its fix and graded like a [mini-task](coder-one-minitasks.md): a
grader the flow never sees runs after the flow ends and checks what it left.
The set was built for
[#9625](https://github.com/OpenAgentsInc/openagents/issues/9625).

## The set

The set lives in `crates/coder-one/issues-eval/`:

```text
manifest.json            the split and every file's SHA-256
entries/<id>.json        one entry: the frozen issue, the commits, the checks
hidden/<file>            test code a check places in the candidate
```

Each entry pins three things:

- **The issue text as it was before the fix.** GitHub records no body edits
  and no title renames for any of the eight issues, so the title and body
  are the ones the issue was opened with.
- **The base commit.** The fix's parent. The flow starts there.
- **A grader.** The tests the real fix added or changed, run against the
  candidate (the `fix_tests` group), and checks for the deliverables the
  issue states: files that must change, text a page or a view must show,
  and files that must not change (the `deliverables` group). An entry
  passes when every check passes.

| Entry | Part | Category | The grader checks |
| --- | --- | --- | --- |
| `9597-minitask-explanation` | Development | Docs and Gym view | `gym coder minitasks` names Terminal-Bench 4.0, a cost, a time, and how grading works; the guide changes and names Terminal-Bench 4.0; the Gym's mini-task tests pass. |
| `9450-delegate-stale-bullet` | Development | Docs | Only `docs/coder/delegate.md` changes; the stale bullet is gone; the page says to find a retained worktree in `git worktree list` and merge or remove it; its links resolve. |
| `9451-delegate-answer-channel` | Development | Rust behavior | The fix's test: a narrating delegate whose answer is `done` passes and keeps its narration. The trace records `transcript`, and the delegate tests pass. |
| `9446-coderdev-launcher` | Development | CLI | The fix's `scripts/test-coderdev.sh`; a launcher script exists; the README documents it. |
| `9449-worker-executor-troubleshooting` | Held out | Docs | Only `docs/coder/worker-executor.md` changes; it names both refusals and links #9448; no home-directory paths; its links resolve. |
| `9448-preflight-configured-origin` | Held out | Rust behavior | The fix's test: under an `insteadOf` rewrite a matching checkout passes preflight and a different one refuses. `cargo test -p coderbench` and strict Clippy pass. |
| `9452-scratch-git-seed` | Held out | Rust behavior | The fix's test: a fresh delegation worktree's `.coder-git` holds the base, so a delegate's commit holds only what it changed. The burn-down briefing stops saying `git init`, `docs/coder/delegate.md` changes, and the worktree tests pass. |
| `9579-install-coder` | Held out | CLI | The fix's `scripts/test-install-coder.sh`; `scripts/install-coder.sh` exists, parses, and takes `--rollback`; a page under `docs/coder/` and the README cover the install. |

`coder-one issue-eval show ID` prints an entry's issue text and every check.

### The split

The set was split into a development part and a held-out part on
2026-09-24, before any issue-flow change was measured on it. The manifest
records the split. Each part holds one docs entry, one Rust behavior entry,
and one CLI entry. The development part also holds #9597, the only Gym
view entry, because the flow was tuned on it. Issues filed together as
twins, #9449 with #9450 and #9451 with #9452, are split across the parts.

Work on an issue-flow change against the development part. Run the held-out
part only to confirm a change once it's chosen, and don't change the change
because of what the held-out part shows. `coder-one issue-eval run` refuses a
held-out entry unless you pass `--held-out`.

The manifest digests every entry and hidden file. The commands refuse a set
whose files don't match, so an entry can't drift unnoticed. After you change
an entry on purpose, run `coder-one issue-eval seal` to record the new
digests. Don't reseal to fit a grader to a result.

## Verify the graders

A grader that passes the base, or fails the real fix, measures nothing.
`verify` grades each entry's base commit and its fix in scratch clones:

```bash
cargo run -q -p coder-one -- issue-eval verify
```

On 2026-09-24 every entry discriminated: the base failed and the fix
passed.

| Entry | Base | Fix |
| --- | --- | --- |
| `9597-minitask-explanation` | Failed, 2 of 8 checks | Passed, 8 of 8 |
| `9450-delegate-stale-bullet` | Failed, 1 of 4 | Passed, 4 of 4 |
| `9451-delegate-answer-channel` | Failed, 1 of 3 | Passed, 3 of 3 |
| `9446-coderdev-launcher` | Failed, 0 of 3 | Passed, 3 of 3 |
| `9449-worker-executor-troubleshooting` | Failed, 2 of 6 | Passed, 6 of 6 |
| `9448-preflight-configured-origin` | Failed, 2 of 3 | Passed, 3 of 3 |
| `9452-scratch-git-seed` | Failed, 1 of 4 | Passed, 4 of 4 |
| `9579-install-coder` | Failed, 0 of 6 | Passed, 6 of 6 |

The checks that pass at the base are the ones that guard against a
regression: the existing tests, Clippy, links that already resolve, and
the absence of home-directory paths.

`grade ID --base`, `--fix`, `--commit REV`, or `--dir PATH` grades one
checkout. The grader's Cargo builds go to `--target-dir`, `CARGO_TARGET_DIR`,
or `~/.openagents/coder-one/issue-evals/target`.

## Run the issue flow on an entry

```bash
cargo run -q -p coder-one -- issue-eval run 9450
```

The run does the following:

1. Makes a scratch clone at the entry's base commit, fetched one commit
   deep, so the clone holds no later history and `git log` can't show the
   fix.
2. Works the issue through `issue_turn::work`, the same loop, review
   session, and pre-pull-request gate a real issue gets. The request is the
   issue's title and body without its URL, so the loop isn't pointed at the
   closed issue.
3. Publishes nothing: the changes stay staged in the clone, with no commit,
   push, or pull request.
4. Grades the clone and records the run.

Jev is live unless you pass `--jev off`. `--model` names Luna's model.
`--policy` names the Microluna manifest the flow runs under, a reference
file name such as `issue-flow-lean.json` or a path; without it, the flow
takes its own, as `CODER_ISSUE_POLICY` picks it. The run records the
manifest's name and digest, so two runs are comparable only when those
match.
`--script NAME=FILE` plays scripted Luna replies instead of the Codex login,
for a run that costs nothing; the file is a JSON array whose items are
`{"call": TOOL, "arguments": {…}}` or `{"say": TEXT}`.

The loop can still run `gh` and reach GitHub, where the issue is closed and
links its fix. Read a run's session streams before you trust a pass.

## Read the results

Each run is recorded under `~/.openagents/coder-one/issue-evals/`, or
`--out`, in the mini-task run's shape:

```text
issue-eval-<entry>-<executor>-<ms>/
  manifest.json             kind "issue-eval": entry, part, outcome, grade, time, cost
  episode.atif.jsonl        every step the issue flow recorded
  repo/                     the scratch clone, changes staged
  artifacts/                briefings, session streams, reply.md, candidate.diff
  verification/grade.json   every check's result
```

The manifest's `schema` is `openagents.coder-one.minitask-run.v1` and its
`kind` is `issue-eval`. It records:

- `task`: the entry, its part and category, the issue number, the base and
  fix commits, and the entry's and the set's digests.
- `policy`: the Microluna manifest the flow ran under, with its digest.
- `outcome`: `finished`, `unfinished`, or `stuck`.
- `grade`: the verdict, each check's result, the `fix_tests` and
  `deliverables` tallies, and the changed paths.
- `milliseconds`, `flow_milliseconds`, and `grading_milliseconds`.
- `cost`: `luna_usd`, `jev_usd`, `total_usd`, and `lower_bound_usd`, from
  the steps the flow recorded. A null cost is unknown, not zero.

The Gym's mini-task view reads the runs:

```bash
cargo run -q -p gym --bin gym -- coder minitasks --runs-dir ~/.openagents/coder-one/issue-evals
```

## What the grader can't tell you

- The fix's tests use the names the fix chose. For #9451, #9446, and
  #9579 the issue leaves those names open, so a candidate can meet the issue
  and still fail the fix's test. For #9452, the issue allows two designs and
  the test checks one. Each entry's `notes` say so. Read the failing checks
  before you count a failure.
- A pass shows the candidate met the pinned checks, not that a reviewer
  would merge it.
- Eight issues are few. Report results per entry, with the part, and don't
  pool development and held-out results.
