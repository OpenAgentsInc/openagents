# Alpha eligibility

Alternative alpha qualifies for prospective registration under the unchanged
240-second final-check limit. The historical reference passes scope, formatting,
all ordinary `coder-access` tests, and the independent checker. The original
source passes the ordinary gate and fails the independent checker. Two compiled
negative controls fail both test gates.

The [complete record](eligibility-alpha.json) retains all eight attempts,
including an initial checker fixture error. These Linux checks make no model
calls and produce no scored executor outcomes. The original panel, schedule,
checkers, and source-preparation policy remain separate and unchanged.

## Selection and provenance

The first bounded screen inspects metadata from 595 child assignments in the
September 26–October 2 local window and examines three candidate issues:

| Issue | Decision | Reason |
| --- | --- | --- |
| [#9908](https://github.com/OpenAgentsInc/openagents/issues/9908) | Calibrate as alpha | Bounded grant-retention behavior, a recorded clean source, and public APIs for local signed exchanges. |
| [#9899](https://github.com/OpenAgentsInc/openagents/issues/9899) | Do not calibrate | A near-duplicate grant-retention family; its crate also appears in an earlier holdout. |
| [#9827](https://github.com/OpenAgentsInc/openagents/issues/9827) | Do not calibrate | Five physics behaviors and Verse workaround removal span three crates; no fresh child launch found in the bounded inventory. |

The public task starts at `09f4e0915150f503e332c2b213d24f793c433f3b` and allows
`crates/coder-access/`. A retained child records a fresh worktree before the
implementation on September 28. Calibration uses its original scoped reference,
whose parent is that source. The later rebased commit includes concurrent changes
and is not substituted for it. Private transcript and reference patch contents
are withheld while prospective executors remain unscored.

This is a bounded implementation task derived from the public issue. The original
child also handled #9909 and host cleanup/deployment; those operations and the
issue's separate `INVARIANTS.md` update are outside this package-only trial.
Historical macOS test output supplies provenance, not Linux eligibility. The
[second bounded screen](selection-screen-2.md) has its own candidate decisions.

## Calibration

| Attempt | Ordinary gate | Independent checker | Total time | Interpretation |
| --- | --- | --- | ---: | --- |
| Original, ordinary only | Pass | Not injected | 38.027 s | Baseline ordinary feasibility |
| Reference, ordinary only | Pass | Not injected | 36.207 s | Reference ordinary feasibility |
| Original, checker v1 | Pass | Setup failure | 35.482 s | Invalid checker fixture |
| Reference, checker v1 | Pass | Setup failure | 36.190 s | Invalid checker fixture |
| Original, corrected checker | Pass | Fail | 54.113 s | Valid behavior rejection |
| Reference, corrected checker | Pass | Pass | 55.813 s | Accepted reference |
| Negative control 1 | Fail | Fail | 54.164 s | Both test targets compile; behavior rejected |
| Negative control 2 | Fail | Fail | 55.576 s | Both test targets compile; behavior rejected |

Every attempt passes formatting. The first checker initializes its store in an
already-created empty scratch directory. The public store API treats that path
as an existing store and expects initialized lock state. All four checks fail
before reaching their behavior assertions. The correction initializes a new
child directory; it changes no assertion. Both original attempts and checker
hashes remain in the record. They are not counted as valid behavior evidence.

The corrected checker has four public-API scenarios. It uses scratch stores and
signed local exchanges, without a relay or provider call. It permits either
stable grant IDs or replacement grants and avoids requiring a specific private
helper or refusal code. Fixture times account for a public client method that
also checks the real clock; this is not a wholly injected-clock model.

The corrected reference compiles ordinary tests in 13.344 seconds and runs them
in 4.522 seconds. Its independent target compiles in 0.315 seconds and runs in
19.997 seconds. The original fails all four independent scenarios. The first
negative control fails four; the second fails one and passes three. Both controls
are also caught by ordinary tests, so this experiment does not establish unique
marginal coverage from the independent checker.

## Preparation, timing, and retention

Source archive creation and hashing take 3.274 seconds, excluding earlier tree
admission. The separate baseline seed build takes 35.882 seconds, including
13.341 seconds of Cargo execution. It refreshes 11 workspace units and exports
349,137,657 bytes across 776 cache files under the common library-only policy.
The amended common debug-info settings and pinned Rust toolchain remain in use.

The eight attempt timers total 365.571 seconds. Their cleanup adds 7.326 seconds
outside those timers; seed-builder cleanup adds 0.921 seconds. Seed-copy times
are recorded separately within each attempt. These measurements exclude research,
transfers, and unmeasured local preparation. No dollar ledger is collected.

One local control-preparation command also fails because standalone formatting
tries to follow an absent sibling module. Disabling child-module traversal fixes
that formatting step without changing the mutation. No remote execution or
payload upload occurs before the fix; its exact duration is unknown.

All 152 files named by the eight retained manifests are verified. The private
168,976-byte evidence archive has SHA-256
`e608067df7c9c07ad221f294a83fd7e335cf53faebe1ce262b60a2e01ea63f8b`.
It preserves receipts, logs, inputs, and both checker versions. After process
closure, each attempt's reconstructible workspace and home are removed; seeds,
source archives, and the shared target remain. A final lock acquisition confirms
the slot is free, with 11,922,567,168 disk bytes available.

The acceptance helper used for every attempt is
`f2c40fe470ccd92dbdcf4e9a72cfc03310583026a677114638e8355cdaa498ee`.
Later feature-forwarding changes are prospective and are not relabeled as the
helper that produced these results.

## Limits

Four scenarios and two controls do not establish exhaustive correctness. These
checks do not exercise owner devices, deployment, or live network behavior.
The current native-to-acceptance pipeline, including its later early scratch
release, still needs an end-to-end feasibility check. Qualification establishes
a usable task and checker under this environment;
it measures no executor success rate, briefing benefit, token saving, or
engineering return.
