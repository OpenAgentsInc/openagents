# Verification records

These checks ran on coderos with the pinned Rust 1.97.1 toolchain and the
isolated target directory `~/.cache/openagents/target-candidate-evidence`.
They are implementation checks, not benchmark efficacy results.

- [Earlier full gate](20260924T212722Z-20f3c1/run.json), at `4728e91817`:
  feature workspace tests, Clippy, dependency policy, and PostgreSQL passed.
  Formatting needed a test assertion wrap; the default suite also hit an
  unrelated short conformance deadline under concurrent host load.
- [Full gate after those fixes](20260924T213823Z-b61332/run.json), at
  `ac1ae94024`: formatting, default and feature Clippy, the complete default
  suite, tooling, dependency policy, and PostgreSQL passed. The feature
  suite failed only `classify_serial_and_concurrent_batches_are_measured`:
  all eight calls completed correctly, but measured concurrent wall time
  exceeded serial wall time on the shared host. The record remains failed.
- [Isolated retry with the full feature set](microluna-candidate-gateway-exact-retry.log):
  that gateway test passed, with concurrent time below serial time. No
  gateway code or assertion was weakened.
- [Integration gate](20260924T220129Z-0a1c8c/run.json), at `809fbe2a7b`:
  formatting, strict Clippy, and tests for `coder-one` and `microluna` passed
  with default and selected feature coverage. It is correctly recorded as
  partial because it scopes the packages.
- [Final v15 integration gate](20260924T220910Z-e43145/run.json), at
  `daf7af3b6e`: formatting, strict Clippy, and all `coder-one` tests passed
  after merging the latest concurrent v15 and issue-gate changes. This is
  also a scoped, partial gate.
- [Python benchmark checks after v15](microluna-candidate-python-v15.log): 39 tests
  passed for agent profiles, network policy, retention, and replay.

The full gate found two pre-existing test-environment problems before these
runs: headless tests loaded the operator's Jev credential file, and a program
routing test still expected an ungranted program to run. Issue #9612 tracks
the isolated test home and corrected grant expectation. Production credential
lookup and program authorization are unchanged.

Some run records have `dirty: true` with the empty tracked-diff digest because
the test tooling creates an untracked Python cache. No product edit was
uncommitted in those recorded runs. Optional Apple Metal checks and the
long relay soak were not run on this Linux host. These records must not be
summarized as one wholly green full-workspace invocation.
