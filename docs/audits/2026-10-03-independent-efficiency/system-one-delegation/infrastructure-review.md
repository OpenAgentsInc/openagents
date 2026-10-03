# Infrastructure review

This review finds three defects in the prospective delegation study's candidate
capture and result retention. The fixes preserve candidate identity and known
accounting when an attempt fails. They do not establish coding quality or a
benefit from System One. The [study protocol](protocol.md) remains the authority
for scored execution.

## Findings and fixes

### Candidate identity omitted deletions

The original payload archive contains changed files that still exist. Deleting
different files can therefore produce the same empty archive and digest. Binding
acceptance to that digest alone does not identify the complete candidate.

**Fixed.** A canonical [candidate manifest](../../../../bench/delegation-study/candidate.py)
binds the source commit, source archive digest, every before-and-after change,
and the payload digest. Changes include deletions, file modes, content hashes,
and symlink targets. Validation checks the change sidecar and the actual archive
members, modes, and contents. The [reporter](../../../../bench/delegation-study/report.py)
requires native, check, endpoint, and review records to bind that manifest.

Regression evidence:

- `CandidateTests.test_deletion_identity_binds_which_file_was_deleted` in
  [test_candidate.py](../../../../bench/delegation-study/test_candidate.py)
  distinguishes deletion-only candidates with identical payloads.
- The candidate tests in
  [test_report.py](../../../../bench/delegation-study/test_report.py)
  reject a changed sidecar, the wrong frozen source, a mismatched acceptance
  digest, and invalid archive contents even after envelope hashes are updated.

### Artifact capture outlived the executor timeout without bounds

The executor timeout originally ends before workspace hashing and archive
creation. An accidental large output or build directory can keep capture busy
and prevent the final result from being written.

**Fixed.**
[Capture limits](../../../../bench/delegation-study/capture_limits.py) bound
entry count, individual file size, aggregate bytes read, and elapsed capture
time. Hashing and archive reads check limits between chunks. Exceeding a limit
becomes an infrastructure error; finalization still attempts to retain provider
accounting and native results. Manifest hashing and streamed payload validation
use the same capture deadline. These are cooperative checks between operations,
not an operating-system guarantee that every filesystem call returns on time.

The `CaptureBounds` tests in
[test_infrastructure.py](../../../../bench/delegation-study/test_infrastructure.py)
exercise a sparse oversized file, aggregate size, growth during reads, and an
expired deadline. They use small synthetic limits and make no model calls.
`CandidateTests.test_validation_respects_shared_capture_deadline` also checks
that payload validation refuses an expired capture deadline.

### Malformed native events could suppress the result artifact

The original finalizer assumes every parsed JSON event is an object and
`modelUsage` is a map. A valid JSON `null` event or list-valued `modelUsage`
can raise an exception before `result.json` is written, losing the summary of
an attempt that may already have incurred cost.

**Fixed.** `native_summary` in
[run_remote.py](../../../../bench/delegation-study/run_remote.py) validates event
and summary shapes, retains valid known cost, reports parse errors, and marks
model completion false when the stream is incomplete or malformed. Provider
accounting is retained independently. A malformed native summary is not a
zero-cost attempt.

`Retention.test_malformed_stream_never_discards_known_native_cost` in
[test_infrastructure.py](../../../../bench/delegation-study/test_infrastructure.py)
covers both a `null` event followed by a valid cost and malformed `modelUsage`.

## Acceptance integration follow-up

The [actual acceptance preflight](acceptance-preflight.md) records two additional
harness corrections: cancellation could bypass child-process cleanup, and the
isolated toolchain initially omitted `rustdoc`. Signal handlers now preserve
worker checkpoints while stopping descendants, and the shared seed environment
binds the exact `rustdoc` executable and version. Git snapshot initialization
also disables automatic maintenance after cleanup encountered a disappearing
`gc.pid`. Focused regressions and actual Linux probes cover those changes.

The first failed setup remains retained. Corrected A acceptance distinguishes
base and reference. B's independent checker does too, but the historical
ordinary suite fails on both versions, so B's reference is not accepted. The
preflight documents that incompatibility without skipping tests or modifying
historical source. C/D full acceptance remains pending.

## Verification and limits

All six focused regressions pass locally on 2026-10-03:

```sh
cd bench/delegation-study
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest \
  test_candidate test_infrastructure.CaptureBounds test_infrastructure.Retention
```

The static review covers `run_remote.py`, `run_inner.py`, `bridge.py`, and
`broker.py`, followed by the candidate-manifest and reporter integration.
The reviewed design exports only the pinned source, creates a single-commit
Git snapshot, mounts a scratch home, and keeps the real provider credential
outside the executor namespace. The broker restricts routes and models and
records provider admissions before forwarding requests. No direct path to
later source, private checkers, or owner credentials is identified in this
review.

The earlier [capability preflight](../../../../bench/delegation-study/infrastructure-preflight.json)
records actual namespace visibility and network checks. Separate
[namespace probes](../../../../bench/delegation-study/namespace-preflight.json)
make no model calls and run no Cargo commands:

- A detached delayed writer cannot create its late file after the namespace
  exits. The namespace exits successfully in 0.0172 seconds; observation occurs
  1.3 seconds later. This tests one descendant-cleanup case.
- An inventory examines 184,706 entries under the broadly mounted
  `/usr/local` tree. Among the checked names, it finds no Git directories,
  OpenAgents product or study artifacts, or credential files. The retained
  matches are Ruby documentation and Anthropic SDK source directories.
  The inventory records names, types, sizes, and symlink targets; it does
  not inspect every file's contents.

This is a bounded infrastructure review, not exhaustive isolation verification.
Trusted image contents, dependency mounts, the operating system, and the
acceptance implementation remain part of the study's trust assumptions.
Synthetic tests do not prove resistance to every hostile filesystem or process
behavior. No scored executor outcomes or private acceptance details are used
in this review.
