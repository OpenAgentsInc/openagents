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
2. Seals the run, as [Seal a run](#seal-a-run) describes: the sessions'
   commands get no GitHub access and, by default, no network.
3. Works the issue through `issue_turn::work`, the same loop, review
   session, and pre-pull-request gate a real issue gets. The request is the
   issue's title and body without its URL, so the loop isn't pointed at the
   closed issue.
4. Publishes nothing: the changes stay staged in the clone, with no commit,
   push, or pull request.
5. Grades the clone, reads every session's commands for attempts to reach
   GitHub or the network, and records the run.

Jev is live unless you pass `--jev off`. `--model` names Luna's model.
`--policy` names the Microluna manifest the flow runs under, a reference
file name such as `issue-flow-lean.json` or a path; without it, the flow
takes its own, as `CODER_ISSUE_POLICY` picks it. The run records the
manifest's name and digest, so two runs are comparable only when those
match.
`--script NAME=FILE` plays scripted Luna replies instead of the Codex login,
for a run that costs nothing; the file is a JSON array whose items are
`{"call": TOOL, "arguments": {…}}` or `{"say": TEXT}`.

`--network on` runs the sessions with the network open; GitHub stays
withheld.

## Seal a run

Every entry is a closed issue whose fix is merged, so a session that reads
the issue on GitHub, or fetches the repository, can copy the answer. The
clone has no remote, but without a seal a model's command could still run
`gh` on the operator's login, or `curl` and `git clone` over an open
network. A run seals every Microluna session it starts, including the
review session and the fix rounds, in two ways.

**GitHub withheld.** Every command a session runs gets the following:

- No `GH_*` or `GITHUB_*` variable, such as `GH_TOKEN`, `GITHUB_TOKEN`,
  or `GH_ENTERPRISE_TOKEN`, and no `SSH_AUTH_SOCK`. Every `*_TOKEN`,
  `*_API_KEY`, and `*_SECRET` is withheld from every session already.
- `GH_CONFIG_DIR` set to an empty directory the run owns, so `gh` can't
  find the operator's login.
- A stub `gh` first on `PATH` that prints `gh: GitHub access is off during
  an evaluation run.` and exits 1.
- An empty `credential.helper`, which clears the helpers the operator's
  Git configuration names, such as `gh auth git-credential`.

Git keeps working on the local clone.

**Network off.** Every command runs inside the `coder-boundary` write
boundary with the network taken away: on Linux, `bwrap --unshare-net`
gives the command a network namespace that holds only loopback; on macOS,
the boundary's profile denies outbound connections to any address but
`localhost`. The offline gate has been exercised on macOS. A host that can't take the
network away refuses the run and says to pass `--network on`.

With the network off, Cargo can't reach its registry. Before the flow
starts, the host runs `cargo fetch --locked` in the clone with the network
open, and the sessions run with `CARGO_NET_OFFLINE=true`, so `cargo build`
and `cargo test` use crates already on disk. On 2026-09-24, a
`cargo check` of this workspace ran to completion inside an offline `bwrap`
boundary from a warm registry. No entry needs the network for anything
else: every entry is this repository, the CLI entries' tests stub `cargo`,
and the docs entries build nothing. The run records what the fetch did,
and a failed fetch leaves the network off, so a session that needs a crate
the registry lacks fails to build rather than reaching out.

**The gate's tests run sealed too.** The pre-pull-request gate runs
`cargo test -p <package> --all-features` for each package the change
touches, and a test the model wrote runs there. Each test command runs as
follows:

- Inside the `coder-boundary` write boundary, which lets it write only to
  the clone, the gate's Cargo target directory
  (`repo/target` inside this evaluation's candidate), a private Cargo
  metadata directory, and scratch space the boundary owns for `TMPDIR`.
- Through `supervise`, with a 1,200-second deadline and 1 MiB kept of each
  output stream.
- With the run's seal: no credential that Microluna withholds from a
  session, no `GH_*` or `GITHUB_*` variable, the stub `gh`, and no Git
  credential helper.
- With the network off when the run's is, after the same
  `cargo fetch --locked` the sessions build from. The gate fetches
  nothing itself, so a manifest the model changed can't pull code in.

A host that can't build the boundary doesn't run the tests. The gate
records itself as incomplete, and no fix round follows, because a fix
round can't give the host a boundary. The manifest's `gate_tests` records
how the tests ran, and `gate_incomplete` says whether they didn't.

**Reads are confined too.** Sessions and the gate can read their candidate,
their own writable scratch and tool state, the system program directories,
installed Rust tools, and prefetched registry sources. Cargo's credentials
and configuration, sibling checkouts, the grader's build output, and the
operator's conversation histories are outside that scope. Cargo metadata
is private to the run; registry sources are read-only. The host supplies
`CARGO_HOME` and `RUSTUP_HOME` explicitly, and commands get a scratch `HOME`.
On macOS, Xcode's installed bundle and license receipts are readable, and
the SDK is selected explicitly. The manifest records these grants. A host
that cannot build the scope refuses the evaluation before inference.

The [read-isolation verification](../verification/2026-09-25-issue-eval-read-isolation.md)
records the private-history exposure that motivated this boundary and the
original study's quarantine.

What the seal doesn't cover:

- The macOS boundary permits file metadata outside the scope, as required
  by the loader. It denies file contents and directory listings there.
- The grader runs on the host after the flow, with the network open. The
  flow never sees what it does.
- `--network on` leaves `curl`, `wget`, and `git clone` of a public URL
  open.

## Read the results

Each run is recorded under `~/.openagents/coder-one/issue-evals/`, or
`--out`, in the mini-task run's shape:

```text
issue-eval-<entry>-<executor>-<ms>/
  manifest.json             kind "issue-eval": entry, part, seal, outcome, grade, time, cost
  episode.atif.jsonl        every step the issue flow recorded
  repo/                     the scratch clone, changes staged
  artifacts/                briefings, session streams, reply.md, candidate.diff
  seal/                     the stub gh and the empty gh configuration
  verification/grade.json   every check's result
```

The manifest's `schema` is `openagents.coder-one.minitask-run.v1` and its
`kind` is `issue-eval`. It records:

- `task`: the entry, its part and category, the issue number, the base and
  fix commits, and the entry's and the set's digests.
- `policy`: the Microluna manifest the flow ran under, with its digest.
- `sealed`: `github_withheld`, `network_off`, `read_isolation`, the readable
  paths and writable tool state, and `prefetch`, what the
  Cargo fetch did (`fetched`, `no Cargo.lock`, `skipped`, or why it
  failed).
- `contamination`: every command a session ran that looked like it reached
  for GitHub or the network, with the trace it's in, whether the seal
  blocked it, and the start of its output; `blocked`, the count the seal
  stopped; and `traces_read`.
- `gate_tests`: how the gate ran its tests, or `null` when the gate never
  ran. `mode` is `confined`, `refused` (no boundary on this host), or
  `none` (no Rust package changed); the record also holds the sandbox
  program, `network`, `reads_confined`, the readable and writable paths, the deadline, the output cap,
  and the packages. `gate_incomplete` is `true` when the mode is
  `refused`.
- `contaminated`: true when an attempt got past the seal, or might have.
  Don't count a contaminated pass.
- `outcome`: `finished`, `unfinished`, or `stuck`.
- `grade`: the verdict, each check's result, the `fix_tests` and
  `deliverables` tallies, and the changed paths.
- `milliseconds`, `flow_milliseconds`, and `grading_milliseconds`.
- `cost`: `luna_usd`, `jev_usd`, `total_usd`, and `lower_bound_usd`, from
  the steps the flow recorded. A null cost is unknown, not zero.

The scan reads every session trace under the run's directory, except the
clone, for `run_command` calls that name `gh`, `curl`, or `wget`, run a Git
`fetch`, `pull`, `clone`, `ls-remote`, or `push` against a URL, or name a
GitHub address. A `gh` attempt is blocked when GitHub was withheld or the
stub answered, and a network attempt is blocked when the network was off.
A blocked attempt is recorded and doesn't contaminate the run: the model
asked, and got nothing. The scan matches command text, so it can flag a
command that only prints a GitHub link; read the attempt before you
discard a run for it.

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
