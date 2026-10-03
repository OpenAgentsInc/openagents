# Executed order-independence diagnostics

Five of the eight retained final patches pass both previously published
[counterexample tests](posthoc_equal_rank.rs). The unchanged base fails both;
the historical reference passes both. These observations confirm the gap
identified in the [earlier source review](heldout-audit.json).

Three of four controls and two of four briefing candidates pass both tests.
These are descriptive post hoc counts from one task and a small panel, not
a causal quality estimate. The original five-case acceptance results and
**no clear win** efficiency verdict remain unchanged. No model calls were made.

## Results

Each row runs the unchanged fixture once. “Equal-rank copies” reverses two
manifest-listed copies with different rewards. “Conflicting marks” reverses
manifests with conflicting attribution and mixed-run reasons. Each test
exercises both the direct reader and its catalog consumer, stopping at its
first failed assertion.

| Variant | Equal-rank copies | Conflicting marks | Output |
| --- | --- | --- | --- |
| Unchanged base | Fail | Fail | [Log](posthoc-execution-logs/base-test.log) |
| Historical reference | Pass | Pass | [Log](posthoc-execution-logs/reference-test.log) |
| 1 — control | Pass | Pass | [Log](posthoc-execution-logs/1-control-test.log) |
| 2 — briefing | Pass | Pass | [Log](posthoc-execution-logs/2-treatment-test.log) |
| 3 — briefing | Pass | Pass | [Log](posthoc-execution-logs/3-treatment-test.log) |
| 4 — control | Fail | Fail | [Log](posthoc-execution-logs/4-control-test.log) |
| 5 — control | Pass | Pass | [Log](posthoc-execution-logs/5-control-test.log) |
| 6 — briefing | Fail | Pass | [Log](posthoc-execution-logs/6-treatment-test.log) |
| 7 — briefing | Fail | Fail | [Log](posthoc-execution-logs/7-treatment-test.log) |
| 8 — control | Pass | Pass | [Log](posthoc-execution-logs/8-control-test.log) |

Control 4 and briefing 7 fail both checks: reversing the directories changes
the selected reward from `0.0` to `1.0` and changes the selected manifest
commit. Briefing 6 fails the equal-rank reward check and passes the conflicting
marks check. The base also fails both, but its marks failure stops earlier:
it does not preserve the mixed-run flags in both input orders.

## Source and execution binding

- Base: `f29611f211747884b4f508db80634d3f6df2a5e9`.
- Reference: `47ad4d73dae2d927f919ff1ef6d880c9df4bcedc`. Its exact retained five-file
  overlay is applied to the base. The relevant source and lockfile are unchanged
  between that base and the reference’s original parent.
- Each published final candidate patch was applied independently to the base.
  All five resulting files match the retained final normalized payload bytes.
  This diagnostic does not run a formatter or modify a candidate.
- Fixture SHA-256:
  `7fe37bbb5d06c251f5f938000917e485ac15021fe2f5cda84f953fae470b6293`.
- Rust and Cargo: `1.97.1`, on an isolated Linux sandbox, with offline
  dependencies and a separate scratch `HOME` for each variant.

The driver exports the pinned archive once and restores the base files before
each overlay. It refreshes Gym source and fixture modification times, then
requires Cargo to report `fresh=false` for both the library and test artifact.
All ten compiles satisfy those checks. The driver hashes and executes the
exact test binary returned by each compile. The shared Cargo target is held
under an exclusive lock throughout the batch.

The standalone harness depends on `gym` with default features disabled,
`serde_json`, and `tempfile`. It starts with the historical workspace lockfile;
offline resolution retains registry and Git package identities from that
lockfile. Every variant produces the same resolved harness lock.

Commands, with local paths replaced by placeholders:

```sh
<toolchain>/cargo test --manifest-path <harness>/Cargo.toml --offline \
  --no-run --message-format=json --test posthoc_equal_rank
<compiled-test-executable> --test-threads=1
```

The build uses three jobs, disables incremental compilation, and disables
debug information in dev and test profiles to share the current sandbox cache
policy. These settings do not change optimization or assertions. Compilation
has a 300-second cap per variant; test execution has a 30-second cap. There
are no setup failures, timeouts, retries, or source or fixture repairs.

The complete diagnostic takes 99.4 seconds after driver start: 4.1 seconds
for source export and 95.0 seconds for compilation. The archive upload takes
252.9 seconds separately. These setup and diagnostic timings are not a new
agent efficiency result.

## Retention and limits

The [machine-readable report](posthoc-execution.json) binds the source archive,
candidate payloads and public patches, fixture, driver, lockfiles, compiled
executables, and logs by SHA-256. Public logs redact temporary paths and
thread IDs. Full raw compiler output, receipts, and exact inputs remain
retained locally. All 40 raw stdout and stderr files were downloaded and
verified before sandbox shutdown. The driver exited successfully, and a
separate closure check reacquired the shared target lock.

This fixture was prepared after reviewing the candidates. It provides observed
counterexamples to complete correctness, but it does not measure the frequency
of unseen defects or establish a treatment effect. Each variant runs once.
The original ordinary tests and frozen acceptance are not rerun or rescored.
