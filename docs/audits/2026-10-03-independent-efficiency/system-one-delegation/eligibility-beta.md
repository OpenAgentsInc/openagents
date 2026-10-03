# Beta eligibility: torn-trace recovery and grading

**Beta qualifies as a prospective replacement.** Both the historical base and the
scoped reference pass formatting and the ordinary `atif` and `coderbench` tests.
The final independent check fails the base, passes the reference, and rejects two
compiled negative controls. The original panel and schedule remain unchanged.
There were no model, provider, or scored executor calls.

The [machine-readable record](eligibility-beta.json) retains every attempt,
phase timing, artifact identity, and cleanup measurement. The
[second bounded screen](selection-screen-2.md) explains the selection and its
provenance limits.

## Scope and provenance

This is a fresh native task reconstructed from
[public issue 9425](https://github.com/OpenAgentsInc/openagents/issues/9425),
covering finding A13. Its source is
`7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`, the exact parent of the historical
reference. A clean Claude-session launch at that revision was not observed.
The public reference is Devin AI authored. The task is older than the initial
recent-week search and was admitted through the authorized second bounded
screen, before any scored executor outcomes or preparation rankings.

The retrieved issue body was retained on 2026-10-03. Its edit history was not
reconstructed, so original-assignment byte identity is not claimed. The issue's
original audit snapshot differs from this task's source; those identities remain
separate.

The task allows the whole `crates/atif/` and `crates/coderbench/` packages. This
keeps alternative valid consumer fixes in scope. The historical documentation
edit outside those packages is excluded. The retained transcript archive and
valid ATIF fixtures remain protected by the public requirement.

## Gate and results

The unchanged 240-second outer gate checks candidate identity and scope,
formatting without rewriting, ordinary package tests, and the separately injected
independent check. Checks run in an isolated Linux namespace, without provider
credentials or external network. A verified library-only baseline seed is copied
into a separate target. Changed candidate sources and the injected check receive
fresh timestamps. Unchanged source retains its verified baseline cache identity.
The common Rust 1.97.1 profile disables debug information for dev and test builds;
it preserves optimization and debug assertions.

All listed attempts passed formatting. Both negative controls compiled in both
ordinary and independent phases.

| Attempt | Ordinary tests | Independent checks | Total seconds |
|---|---|---|---:|
| Base, ordinary gate | Pass | Not injected | 17.645 |
| Reference, ordinary gate | Pass | Not injected | 17.280 |
| Base, initial checker | Pass | 0/3; invalid checker contract | 18.250 |
| Reference, initial checker | Pass | 1/3; invalid checker contract | 17.848 |
| Base, corrected checker | Pass | 0/3 | 17.750 |
| Reference, corrected checker | Pass | 3/3 | 17.900 |
| Control 1 | Fail | 2/3 | 12.141 |
| Control 2 | Fail | 2/3 | 11.687 |

The independent check uses the pre-task public reader, writer, and grader APIs.
It covers clean successful grading, writer closure, recovery at every byte of a
non-ASCII final record, preservation of prefix content and order, visible damage,
and refusal to grade damaged or incomplete evidence successfully. It does not
require a particular new reader API, error variant, or implementation symbol.

The two controls remove writer-closure enforcement and break recovery of a final
partial UTF-8 record. Both fail the expected independent behavior assertion;
both also fail ordinary tests. These controls show sensitivity to those defects,
without demonstrating additional coverage beyond the ordinary suite.

## Retained checker correction

The first calibrated checker required `Unverifiable` for incomplete evidence.
The reference returned `Failed` for an unfinished trace. The pinned pre-task
[consumer mapping](https://github.com/OpenAgentsInc/openagents/blob/7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6/crates/coderbench/src/lib.rs#L541)
already maps `Unfinished` to `Failed`; its torn-trace path maps to
`Unverifiable`. Both refuse successful grading.

This was a checker contract error. Independent review also missed the distinction.
Both initial calibration attempts remain retained and excluded from the final
eligibility verdict. The correction accepts a strict-consumer error or a
non-`Passed` verdict. It still requires successful grading for clean evidence and
leaves the prefix, recovery, damage, and lifecycle assertions unchanged. The
correction follows the pre-task public mapping and the issue's prohibition on
successful grading; it does not adopt a new API from the reference.

The corrected checker was frozen before its base/reference calibration and both
negative controls. No scored executor outcomes existed during this correction.
The final checker SHA-256 is
`ee58a16d8975ee63b1f7a6911ee63e990a1e0355d0456c9b83da7febfea1ef01`.

## Preparation, retention, and limits

The baseline seed succeeded on its first attempt: 7.600 seconds total, including
6.527 seconds of Cargo work. It contains 331,204,258 bytes in 874 files. Fifteen
workspace units reported a fresh compilation during seed construction. Seed
copies during the eight check attempts took about 0.105–0.112 seconds each.

The eight check attempts total **130.501 seconds**. Their cleanup adds **0.649
seconds**, outside those timers. Seed cleanup adds 0.034 seconds outside its build
timer. Transfers, research, review, and evidence collection are excluded from
these measurements. There is no dollar ledger or engineering-return estimate.

The collector verified 152 retained attempt files against their hashes and
sizes, retained all logs and failed calibration receipts privately, and confirmed
that closed attempts' reconstructed workspaces and targets were removed. Source
archives, baseline seeds, and the shared build target remain available. Private
checker and reference contents are not published in this record.

This is one calibration per final variant on one Linux environment. It does not
estimate flakiness or exhaustive correctness. It establishes feasibility and
selected defect sensitivity, not a briefing or delegation benefit. The complete
native-to-acceptance endpoint remains unmeasured for this task.
