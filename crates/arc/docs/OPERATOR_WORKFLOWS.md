# ARC Operator Workflows
Status: canonical operator guide for retained ARC benchmark and compatibility workflows
Date: 2026-03-16

This file is the canonical operator-facing doc for running the retained ARC
subtree without Python-only tooling.

Use it when you need to:

- run a bounded local ARC-AGI-3 flow against the compatibility server
- exercise the remote-compatible path from Rust
- run owned benchmark / replay / checkpoint checks
- find the typed artifacts those flows produce

## Scope

This guide is intentionally bounded to repo-owned Rust entrypoints:

- `cargo test` flows that exercise the retained operator path
- fixture manifests in `crates/arc/*/tests/fixtures`
- typed artifact APIs in `arc-client`, `arc-benchmark`, and `arc-solvers`

It is not a substitute for the upstream hosted ARC Prize service, and it does
not claim full ARC-AGI-3 release equivalence.

## Preconditions

Run commands from the repo root:

```bash
cd /home/christopherdavid/code/openagents
```

The retained ARC subtree needs only the Rust workspace toolchain.

## Fast Checks

Use these first when you just need a quick confidence pass:

```bash
cargo test -p arc-client compatibility_server_supports_local_docs_flow_without_authoritative_scoring -- --nocapture
cargo test -p arc-client compatibility_server_enforces_competition_mode_lifecycle_restrictions -- --nocapture
cargo test -p arc-solvers interactive_runner_executes_a_bounded_local_episode_end_to_end -- --nocapture
cargo test -p arc-solvers interactive_runner_executes_a_bounded_remote_episode_end_to_end -- --nocapture
cargo test -p arc-solvers interactive_runner_parity_manifest_covers_one_shot_and_resume_flows -- --nocapture
cargo test -p arc-benchmark benchmark_parity_manifest_covers_interactive_checkpoint_recording_and_replay -- --nocapture
cargo test -p arc-solvers repeated_interactive_eval_aggregates_replayed_rounds_over_psionic_eval -- --nocapture
```

## Local Compatibility-Server Flow

The retained local docs-flow lives in
`crates/arc/client/tests/compatibility_server.rs`.

Run:

```bash
cargo test -p arc-client compatibility_server_supports_local_docs_flow_without_authoritative_scoring -- --nocapture
```

What this exercises:

1. start an in-process `ArcCompatibilityServer`
2. list games over HTTP
3. open a scorecard
4. `RESET` the game
5. execute at least one action
6. read scorecard state back through the client
7. close the scorecard

Why this matters:

- it is the retained Rust equivalent of the upstream "full play test" docs flow
- it proves the local compatibility server can back the same scorecard / reset /
  action / close lifecycle without a hosted ARC service
- it keeps the operator loop rooted in owned Rust crates and fixtures

Competition-mode restrictions are covered separately:

```bash
cargo test -p arc-client compatibility_server_enforces_competition_mode_lifecycle_restrictions -- --nocapture
```

That test is the canonical check for:

- one scorecard at a time
- one environment interaction in competition mode
- level-reset-only semantics
- no inflight `get_scorecard` reads in competition mode

## Remote-Compatible Flow

In the retained subtree, "remote" means HTTP against a compatibility server,
not the public ARC Prize hosted API.

Use these runner checks:

```bash
cargo test -p arc-solvers interactive_runner_executes_a_bounded_remote_episode_end_to_end -- --nocapture
cargo test -p arc-solvers interactive_runner_parity_manifest_covers_one_shot_and_resume_flows -- --nocapture
```

What they cover:

- remote scorecard open/close behavior
- session reset and typed action execution over HTTP
- local/offline vs remote/online parity for one-shot and resume flows
- expected local-vs-remote differences recorded explicitly instead of treated as
  hidden drift

If you only need transport / cookie-affinity smoke instead of full solver runs,
use:

```bash
cargo test -p arc-client remote_client_keeps_cookie_affinity_across_wrapper_steps -- --nocapture
cargo test -p arc-client remote_client_retries_rate_limits_without_losing_cookie_affinity -- --nocapture
```

## Local Benchmark And Eval Flows

### Static + Interactive Benchmark Parity

Use the benchmark parity manifest as the top-level owned regression surface:

```bash
cargo test -p arc-benchmark benchmark_parity_manifest_covers_exact_match_and_task_checkpoint_surfaces -- --nocapture
cargo test -p arc-benchmark benchmark_parity_manifest_covers_interactive_checkpoint_recording_and_replay -- --nocapture
cargo test -p arc-benchmark benchmark_parity_manifest_keeps_checkpoint_refusal_machine_legible -- --nocapture
```

These tests cover:

- exact-match score parity
- task checkpoint persistence
- interactive recording scoring
- checkpoint bundle layout
- replay from fixtures
- corrupted-checkpoint refusal behavior

### Checkpoint Bundle Round-Trip

Use:

```bash
cargo test -p arc-benchmark interactive_checkpoint_bundle_round_trips_and_validates_digest -- --nocapture
```

This is the canonical retained check for:

- saving `ArcInteractiveCheckpointBundle` to disk
- loading it back
- verifying digest integrity
- refusing corrupted metadata

### Repeated Interactive Eval Over Psionic

Use:

```bash
cargo test -p arc-solvers repeated_interactive_eval_aggregates_replayed_rounds_over_psionic_eval -- --nocapture
cargo test -p arc-solvers repeated_interactive_eval_surfaces_refusal_and_error_coverage -- --nocapture
```

These are the canonical bounded checks for:

- repeated ARC-AGI-3 case execution over `psionic-eval`
- per-case trajectory bundle export through `psionic-environments`
- finalized `EvalRunState` and repeated `BenchmarkExecutionSession` rounds
- completion / refusal / error / replay-coverage summaries

## Replay, Recording, And Checkpoint Artifacts

### Local Recording Round-Trip

Use:

```bash
cargo test -p arc-client jsonl_round_trip_preserves_online_recordings_with_frame_data -- --nocapture
cargo test -p arc-client jsonl_import_refuses_sparse_entries_without_frame_data -- --nocapture
```

What this covers:

- canonical `ArcRecording` to JSONL export
- JSONL import back into a typed recording
- transport-policy split between local canonical recordings and online JSONL

By default the test writes temporary JSONL files under:

```text
${TMPDIR}/arc-client-*.jsonl
```

and deletes them at the end of the test.

### Checkpoint Bundle Paths

The benchmark checkpoint tests write under a temporary directory shaped like:

```text
${TMPDIR}/arc_benchmark_<label>_<pid>_<nanos>/.checkpoint/<checkpoint_id>/
```

and delete the directory when the test exits.

Inside the checkpoint directory, the retained bundle layout is the thing to
inspect:

- `metadata.json`
- `recording.json`
- `scorecard.json`
- `step_summaries.json`
- `costs.json`

The exact expected file set is asserted in
`crates/arc/benchmark/tests/benchmark_parity.rs`.

### Trajectory Bundle Export

Trajectory bundles are ARC-owned typed values produced from interactive run
artifacts:

- API: `arc_solvers::ArcInteractiveTrajectoryExport::from_run_artifacts`
- module: `crates/arc/solvers/src/interactive_receipts.rs`

If you need to persist one manually from a scratch harness, the retained seam
is:

```rust
let export = arc_solvers::ArcInteractiveTrajectoryExport::from_run_artifacts(&artifacts)?;
std::fs::write("trajectory_bundle.json", arc_core::canonical_json_string(&export)?)?;
```

That bundle already contains:

- full ARC run artifacts
- per-turn replay locators
- generalized Psionic turn receipts
- the final Psionic session summary

## Fixtures To Know

The main retained fixture roots are:

- `crates/arc/engine/fixtures/`
- `crates/arc/client/tests/fixtures/`
- `crates/arc/benchmark/tests/fixtures/`
- `crates/arc/solvers/tests/fixtures/`

The most reused demo game fixture is:

- `crates/arc/engine/fixtures/demo_game.json`

The main manifest-driven parity fixture is:

- `crates/arc/solvers/tests/fixtures/interactive_runner_parity_manifest.json`

## Recommended Operator Sequence

When bringing up or reviewing the subtree, run in this order:

1. `cargo test -p arc-client compatibility_server_supports_local_docs_flow_without_authoritative_scoring -- --nocapture`
2. `cargo test -p arc-client compatibility_server_enforces_competition_mode_lifecycle_restrictions -- --nocapture`
3. `cargo test -p arc-solvers interactive_runner_parity_manifest_covers_one_shot_and_resume_flows -- --nocapture`
4. `cargo test -p arc-benchmark benchmark_parity_manifest_covers_interactive_checkpoint_recording_and_replay -- --nocapture`
5. `cargo test -p arc-solvers repeated_interactive_eval_aggregates_replayed_rounds_over_psionic_eval -- --nocapture`

If all five are green, the retained local, remote-compatible, replay,
checkpoint, and repeated-eval surfaces are all alive at the same time.

## Maintenance Rule

When a retained ARC operator flow changes command shape, fixture location, or
artifact path, update this file in the same change.
