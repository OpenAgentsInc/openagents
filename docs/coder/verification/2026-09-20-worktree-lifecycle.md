# Worktree lifecycle regression

Issue: [#9442](https://github.com/OpenAgentsInc/openagents/issues/9442).
The source before this correction was `ab79cd85c0`. The reproduction also used
an isolated checkout of `1f78b260e2`, the snapshot named in the delegation brief.
No live model or delegated-agent calls ran in these tests.

## Reproduction

The ordinary parallel `coder` program suite at the older snapshot intermittently
returned fewer answered delegations. Adding the delegation records to assertion
messages exposed the cause:

```text
Status::Harness("cannot make a worktree ...
fatal: failed to read .git/worktrees/<sibling>/commondir: Undefined error: 0")
```

One reproduced run had six delegation records and five answers. Another returned
four answers. Git was creating one worktree while a finished sibling removed
its metadata. This was not a lost output buffer or the already-fixed CoderBench
probe-filename collision. The original report did not include those diagnostics.

Thirty-eight repeated suites on the newer snapshot initially passed, despite
retaining the uncoordinated creation/removal code. Passing repetitions alone
therefore do not establish that this race is absent.

## Correction

`coder::worktree` serializes metadata changes through an advisory file lock in the
canonical Git common directory. Separate Coder processes and linked checkouts
resolve the same lock. Only creation/removal hold it; executors run concurrently.
Lock acquisition and each supervised Git process are bounded to 30 seconds,
with output capped at 64 KiB per stream. A directory is reserved atomically
before Git uses it, so cleanup cannot adopt a stale checkout after PID reuse.

Normal completion awaits removal and surfaces failures. The subprocess
supervisor's `run_holding` retains the checkout through cancellation and reaping.
Cancelled work then schedules cleanup outside the caller's runtime. This does
not guarantee cleanup after abrupt termination of the entire Coder process,
and unrelated Git tools must cooperate with the advisory lock.

## Verification

An independent target directory and Rust 1.97.1 were used:

```sh
cargo +1.97.1 test --locked -p coder
cargo +1.97.1 test --locked -p coder --lib worktree
cargo +1.97.1 test --locked -p supervise
cargo +1.97.1 clippy --locked -p coder -p supervise --all-targets -- -D warnings
```

- The full Coder suite passed with five explicit live-test skips. The focused
  worktree tests also passed after the final path-validation change.
- The supervisor suite passed, including a new cancellation test proving that
  a resource is released only after its child is reaped.
- Lock tests cover a separate process, linked-checkout identity, bounded
  contention, and recovery after release. Another test interleaves 18 creations
  and removals and verifies both filesystem and Git metadata cleanup.
- A symlinked worktree parent is refused before creating directories outside
  the repository.
- The final parallel program suite was repeated 18 times, with three test
  processes running together and 24 test threads per process. All repetitions
  passed. Each ran 22 tests and explicitly skipped the live program test.
- Strict Clippy passed for Coder and the supervisor, including all targets.

The lock protocol and its negative tests establish the coordination boundary;
repeat runs supplement that evidence. These checks do not prove trusted probes,
read-only enforcement, or a safe independence model. Those remain #9427 and the
separate burndown prerequisites.
