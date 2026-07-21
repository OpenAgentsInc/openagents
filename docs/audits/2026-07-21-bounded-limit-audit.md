# Bounded-Limit Audit — 2026-07-21

This audit checks every bounded mechanism in the verification packages against
one rule: a limit must fail closed and state which failure it buys, and no
limit may silently strengthen a result. The rule and its companions are in
[`INVARIANTS.md`](../../INVARIANTS.md) under "Fail-Closed Limits And Printed
Trust". The source discipline is the Freerange teardown
([`docs/teardowns/2026-07-21-freerange-teardown.md`](../teardowns/2026-07-21-freerange-teardown.md),
§4 and §7.3). The owning issue is FREERANGE-02 (#9125).

## Scope and method

Audited packages, in order:

1. `packages/assure-repo/src`
2. `packages/assurance-spec/src`
3. `packages/behavior-contracts/src`
4. `packages/agent-harness-contract/src`

A bounded mechanism is any cap, truncation, depth limit, iteration budget,
timeout, retry ceiling, or queue bound that can cut off a loop, a collection,
or a process. For each one, the audit records the limit, the failure it buys,
and a verdict: **fails-closed** (records a stop, a degradation, or an
`unknown`, and cannot strengthen a result) or **finding** (could silently make
an incomplete or false result read as complete or true). Trivial cosmetic
slices that do not affect a verdict or evidence value are excluded.

## Result

18 bounded mechanisms. 16 fail closed. 2 were findings. Both findings are
fixed in the same change as this audit, each with a regression test.

| Package                   | Mechanisms | Fails-closed | Findings |
| ------------------------- | ---------- | ------------ | -------- |
| `assure-repo`             | 6          | 5            | 1        |
| `assurance-spec`          | 9          | 8            | 1        |
| `behavior-contracts`      | 0          | 0            | 0        |
| `agent-harness-contract`  | 3          | 3            | 0        |

`behavior-contracts` is declarative registry data. Every cap-like word in it
appears inside a contract description string, not in executable code, so it has
no real bounded mechanism.

## The two findings, now fixed

### Finding A — a mutation timeout counted as a kill

`packages/assure-repo/src/mutation-runner.ts` ran the oracle with a 10-minute
timeout. On timeout the process is killed by a signal and `execFileSync`
throws. The old code mapped any thrown error to `passed: false`, and
`runMutation` mapped `passed: false` to `result: "killed"`. So a mutant that
caused an infinite loop or a slow hang was recorded as **killed** — as if the
oracle had caught it. This inflated the kill rate and could make a weak oracle
read as sound. Because this runner exists to demonstrate false greens, a
timeout silently exonerated a candidate when the true state was inconclusive.
The signal was visible only in a free-text detail string, never in the
structured verdict.

Fix: `runTest` now separates a passing run (exit 0), a failing run (a numeric
non-zero exit code), and a terminated run (a signal, such as a timeout
`SIGTERM`, an out-of-memory `SIGKILL`, or a crash, or any run with no exit
code). Only a numeric non-zero exit code is a failing verdict. A terminated
mutant becomes a new `inconclusive` outcome and is never counted as killed.
Regression test: `packages/assure-repo/test/mutation-runner.test.ts` runs a
mutant whose test kills itself with a signal and asserts the result is
`inconclusive`, not `killed`. The existing strong-oracle test is the positive
control through the same path: a real non-zero exit code is still a kill.

### Finding B — an oversized manifest dropped without a record

`packages/assurance-spec/src/repository-inventory.ts` skipped a `package.json`
that was larger than the 512 KB cap or that could not be read. Unlike every
other cap in the file, the skip did not set `truncated = true` and added no
diagnostic. So an inventory with a dropped manifest read as complete
(`truncated: false`) while that manifest's scripts were silently absent.

Fix: the skip now records the drop the same way the count caps do. An
unreadable manifest sets `truncated = true` and adds
`package_manifest_unreadable`. An oversized manifest sets `truncated = true`
and adds `package_manifest_too_large`. Regression test:
`packages/assurance-spec/test/repository-inventory.test.ts` commits a
`package.json` over the byte cap and asserts `truncated` is true, the diagnostic
is present, and the dropped script is absent.

## The fails-closed mechanisms

These 16 mechanisms already fail closed. They are recorded so a future change
that weakens one is visible.

`assure-repo`:

- `readiness.ts` sweep-receipt freshness window (24 hours). An over-age receipt
  renders `unknown` with a reason. No receipt means no light.
- `readiness.ts` commit-binding gate. A receipt from a different commit renders
  `unknown`.
- `workspace.ts` `git ls-files` output buffer (64 MiB). An overflow throws
  loudly rather than truncating a file list into a shorter one.
- `false-green.ts` skip-context window (3 lines for a `#NNNN` tracking ref).
  This only changes which candidate leads are flagged. A lead becomes a finding
  only after a surviving mutation, and the sweep freshness oracle is a byte
  check, so this window cannot turn a red verdict green.
- `false-green.ts` block-body brace match. On an unmatched body it returns the
  rest of the file, which degrades toward under-flagging leads. It cannot
  strengthen a verdict.

`assurance-spec`:

- `repository-inventory.ts` candidate-ref cap (400), package-manifest cap
  (250), and declared-script cap (2000). Each sets `truncated = true` and adds
  a diagnostic.
- `repository-inventory.ts` git output buffer (16 MiB). An overflow makes the
  git call fail, which returns an unavailable inventory.
- `mutation-adapter.ts` mutants-per-plan cap (16), target and replacement byte
  cap (4096), and subject-source byte cap (1 MiB). Each rejects an oversized
  input with a typed `fail(...)`.
- `vite-plus-test-adapter.ts` test output buffer (16 MiB). The verdict comes
  from the JUnit output file, not the captured output. An overflow makes the
  run `INCONCLUSIVE`.

`agent-harness-contract`:

- `slice-runner.ts` intra-turn event budget (`Stream.take(maxEvents)`). A turn
  that does not finish within the budget is cut mid-stream and suspends with a
  continuation. It does not fabricate a result. Note: the completed branch
  defers the result to the adapter's `done` contract, so completion truth rests
  on that contract, not on the count.
- `slice-runner.ts` slice-count safety bound (1000 default). Exhaustion fails
  with an error.
- `event-log.ts` live fan-out buffer (`PubSub.unbounded`). A slow follower
  back-pressures rather than dropping events.

## Boundary

This audit is a point-in-time record over four packages. It is not a standing
gate. A future change that adds a bounded mechanism should add its row here or
prove it fails closed. The fixed findings are guarded by their regression tests
in the normal test sweep.
