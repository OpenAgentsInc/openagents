# Second replacement eligibility screen

Status: two candidates await qualification. This screen is closed; a failed
candidate will not trigger another search or a weaker gate. No model calls,
Cargo checks, or preparation ranking measurements were run for this screen.
The [original unsealed proposal](protocol.md) and its
[C/D eligibility failures](eligibility-cd.md) remain retained.

## Scope and denominator

The screen sought two small Rust library bugs outside the large `coder`
package and previous development, held-out, and standing tasks. It first
covered September 26–October 2, then the coordinator extended it to September
19–25. These additional candidates are older than the original recent-week
window. Dates use America/Chicago. Selection used public task scope, source
provenance, dependencies, and ordinary-test feasibility. It did not use
preparation rankings or executor outcomes.

A reconstructed Git inventory at `6705d736620e73af1cc7c1d02fa74bf245967dd5`
provides the bounded search denominator:

| Window | Reachable commits | Commits changing Rust | Metadata filter matches |
| --- | ---: | ---: | ---: |
| September 26–October 2 | 1,461 | 969 | 20 |
| September 19–25 | 1,435 | 863 | 62 |

The [screen record](selection-screen-2.json) retains the filter, inventory
digest, candidate bindings, and ten explicit exclusions, including one
duplicate. These counts describe metadata; they do not imply that every task
body or test suite was examined. The initial exploratory title scan was not
exhaustively logged. The earlier closed-issue lookup was capped at 250 results.
The SDK candidate came from following the issue for a deadline fix, beyond
the title-word filter. This is a bounded screen, not an exhaustive task census.

## Candidates

| Candidate | Public task | Proposed trial scope | Ordinary gate |
| --- | --- | --- | --- |
| Beta | [#9425, audit A13](https://github.com/OpenAgentsInc/openagents/issues/9425) | Recover torn ATIF traces with visible damage and preserve strict grading integrity. Include the original CoderBench fixture adjustments. | Full `atif` and `coderbench` package tests, offline |
| Gamma | [#9424, audit A12](https://github.com/OpenAgentsInc/openagents/issues/9424) | Validate numeric answers and request-aware response coverage. The issue's separate A22 deadline fix is already in the selected base. | Full `jev` package tests with `blocking`, offline |

Both public issues name audit source
`1843fa6c18a05537bf2b022f69361a9ba3ef12a1`. The proposed native trials use the
exact fix parents instead:

| Candidate | Pre-fix source | Historical reference |
| --- | --- | --- |
| Beta | `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6` | `fc385a13a40cef956636284e809f154acddf373c` |
| Gamma | `c427943a5c84ba5938a3549f24b27de551812a37` | `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6` |

These are new trials defined by public issues. No matching fresh Claude launch
was established; the references are Devin-authored fixes. The issue audit
source and fix parents differ, as the JSON records. The retrieved issue bodies
are retained by digest, but their original edit history was not reconstructed.
Gamma's assignment must retain the full issue and explicitly state its A12
scope. Neither candidate is an observed clean Claude session replay.

The independent source review verified manifest hashes and exact commit
parents. Beta's direct dependency manifests support a small data/filesystem
crate plus a local shell/Git consumer gate. Gamma uses an HTTP SDK with local
loopback mocks. These source observations do not establish Linux test success.

## Remaining qualification

At screening time, the generic seed and acceptance commands lack the explicit
`blocking` feature needed for Gamma. The later
[feature amendment](cargo-feature-amendment.json) adds that binding as
`jev/blocking`; actual feature-bearing seeds and checks still need qualification.
`--all-features` enables credentialed live provider tests and is not the offline
gate. The namespace must support private loopback for SDK mocks.

Before registration, each unchanged base and historical reference must pass
the declared ordinary gates on isolated Linux. Behavior-focused independent
checks and two negative controls must then be frozen and calibrated. No test
skips or product repairs may rescue an ineligible reference. Reference patches,
checker contents, and private transcripts are excluded from this public record.
