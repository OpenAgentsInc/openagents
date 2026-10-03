# Gamma eligibility: typed SDK answer validation

**Gamma qualifies as a prospective replacement.** Its historical base and
reference pass formatting and the ordinary `jev` suite with the `blocking`
feature. The independent check fails the base, passes the reference, and rejects
two compiled negative controls. One control passes ordinary tests while failing
the independent check. The original panel and schedule remain unchanged. There
were no model, provider, or scored executor calls.

The [machine-readable record](eligibility-gamma.json) retains all setup attempts,
check timings, dependency receipts, and artifact identities. The
[second bounded screen](selection-screen-2.md) records the candidate's selection.

## Scope and provenance

The task covers the remaining A12 portion of
[public issue 9424](https://github.com/OpenAgentsInc/openagents/issues/9424): typed
numeric validation and request-aware answer coverage, types, and option identity.
The A22 whole-call deadline work already exists at the selected source,
`c427943a5c84ba5938a3549f24b27de551812a37`. This is an isolated SDK task, rather
than a replay of the entire two-finding issue.

The source is the exact parent of the historical reference. A clean Claude-session
launch at that revision was not observed; this is a new issue-defined native task.
The public reference is Devin AI authored. The task is older than the initial
recent-week search and comes from the authorized second bounded screen. Selection
used public requirements, source provenance, and feasibility before preparation
rankings or scored executor outcomes. The issue body was retrieved on 2026-10-03;
its edit history was not reconstructed.

Allowed changes are within `crates/jev/`. Both seed construction and acceptance
bind `cargo_features: ["jev/blocking"]`. The live feature is disabled because it
runs credentialed provider tests. Async and blocking mock tests run over private
loopback without external network. Full Coder, Gym, and CoderBench consumer
regression checks remain outside this small SDK gate and are not claimed passed.

The pinned
[selected-answer contract](https://github.com/OpenAgentsInc/openagents/blob/c427943a5c84ba5938a3549f24b27de551812a37/docs/decision-models/2026-09-20-score-contract.md)
permits calibrated answers whose selected option differs from the largest raw
probability. The checker preserves that contract and fractional scores. It also
includes a valid legacy Score with no probabilities or selected field, as the
pre-task SDK permits. These are compatibility requirements, not new model
thresholds.

## Gate and results

The unchanged 240-second outer gate checks identity and scope, formatting without
rewriting, ordinary package tests, and a separately injected independent check.
It uses the common isolated Linux environment, Rust 1.97.1, and the debug-info-free
dev/test profile. Verified baseline libraries are copied into a separate target;
final executables and build-script state are regenerated. Changed candidate
sources and the injected check receive fresh timestamps. No executor-mutated
cache or provider credential is used.

All attempts passed formatting. Both negative controls compiled in both ordinary
and independent phases.

| Attempt | Ordinary tests | Independent checks | Total seconds |
|---|---|---|---:|
| Base, ordinary gate | Pass | Not injected | 11.253 |
| Reference, ordinary gate | Pass | Not injected | 11.138 |
| Base, independent calibration | Pass | 1/4 | 10.935 |
| Reference, independent calibration | Pass | 4/4 | 11.688 |
| Numeric-validation control | Fail | 3/4 | 12.190 |
| Request-validation control | Pass | 2/4 | 11.387 |

The checker exercises existing public decode, async, blocking, and raw-response
APIs. It requires structured rejection of invalid typed answers while preserving
valid boundary and compatibility cases. The loopback responses are independently
constructed. No assertion depends on a new implementation method or symbol.
Independent review added the legacy Score positive case before the first
calibration. The checker then remained unchanged for base, reference, and controls.

The numeric control omits a range guard; ordinary and independent tests both
reject it. The request control omits typed request-response checking while keeping
numeric decoding. Ordinary tests pass, but the independent async and blocking
checks reject it. **This demonstrates added coverage for that one compiled
omission.** It does not estimate the checker's coverage across arbitrary patches
or show a treatment effect.

The checker SHA-256 is
`575a283ba8d4e09005b75ad8d81977eb532cc6c0bf2d47eaf80bca0bb62fbb9f`.

## Retained setup failures

Three seed attempts stopped during offline dependency setup before compilation.
Each has its own retained output and was followed by a new attempt directory.

| Seed attempt | Result | Total seconds |
|---|---|---:|
| 1 | `nu-ansi-term` 0.50.3 absent from the offline registry | 0.513 |
| 2 | Verified archive present, but unpacking requires a write to the read-only registry | 0.543 |
| 3 | After trusted unpacking, `sharded-slab` 0.1.7 was absent | 0.740 |
| 4 | Complete baseline seed | 5.814 |

Trusted setup downloaded public crate archives and verified their exact pinned
`Cargo.lock` checksums before extraction. The bounded tracing dev-dependency
family was then checked and provisioned. These three recorded provisioning
operations total 0.189 seconds; they ran no Cargo or model calls. Ordinary checks
kept their offline policy and read-only registry. The failures are setup evidence,
not failed product behavior or reasons to relax the gate.

The successful seed includes 296,925,328 bytes in 811 files. Its Cargo work took
4.725 seconds, and 16 workspace units reported fresh compilation. Seed copies
during the six check attempts took about 0.104–0.120 seconds each.

## Accounting, retention, and limits

The six ordinary and full-check attempts total **68.592 seconds**, with **0.561
seconds** of cleanup outside their timers. All four seed attempts total **7.610
seconds**, with a further 0.194 seconds of retained cleanup. These totals include
the three failed seed attempts. Transfers, research, review, and evidence
collection are excluded. No dollar ledger or engineering-return estimate was
collected.

The collector verified 110 retained attempt files against hashes and sizes,
retained logs and setup failures privately, and removed only closed attempts'
reconstructible scratch directories. Source archives, verified baseline seeds,
and the shared target remain available. Private checkers and reference contents
are not published in this record.

One Linux calibration per final variant establishes this task's feasibility and
selected behavior sensitivity. It does not establish a flakiness rate, exhaustive
SDK correctness, or a briefing/delegation benefit. The complete native-to-acceptance endpoint remains unmeasured for this task. The original
240-second deadline remains in effect.
