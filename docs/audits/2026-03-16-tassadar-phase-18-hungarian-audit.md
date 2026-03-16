# 2026-03-16 Tassadar Phase 18 Hungarian Audit

This note records the Phase 18 closeout requested by `#3818`.

It is intentionally a truthfulness note, not a parity note.

## Scope

Phase 18 was allowed to do one narrower thing than the article rhetoric:

- land a real Hungarian-class workload family in-repo
- package it as a benchmark/environment contract
- prove one bounded exact lane against CPU reference
- keep learned and compiled claims separated
- persist the resulting bundle in machine-readable form

Phase 18 was not allowed to:

- claim learned Hungarian success
- imply that the open Sudoku learned-lane gate is now green
- claim arbitrary matrix size or arbitrary-program closure
- claim that the article's Hungarian-algorithm result is reproduced
- use speed or hull numbers as a substitute for correctness

The canonical artifact root is:

- `crates/psionic/fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0`

The decisive machine-readable artifacts are:

- `benchmark_package.json`
- `compiled_executor_exactness_report.json`
- `compiled_executor_compatibility_report.json`
- `hungarian_lane_status_report.json`
- `run_bundle.json`

## What Landed

Phase 18 makes the Hungarian lane real in bounded form:

- `psionic-runtime`
  - new bounded Wasm profile:
    `tassadar.wasm.hungarian_v0_matching.v1`
  - real bounded min-cost perfect-matching workload over `4x4` cost matrices
  - explicit `i32.lt` opcode support in the runtime and trace vocabulary
  - real split-aware Hungarian-v0 corpus with stable `train`, `validation`,
    and `test` cases
  - exact CPU-reference execution traces and outputs for every committed case
- `psionic-models`
  - matching bounded compiled deployment fixture for the Hungarian-v0 profile
  - tokenizer support for the new instruction and binary-op event
- `psionic-eval`
  - real Hungarian-v0 benchmark package and environment bundle
  - bounded compiled exactness report
  - bounded compiled compatibility/refusal report
  - explicit learned-vs-compiled lane status report
- `psionic-research`
  - canonical bundle writer:
    `run_tassadar_hungarian_compiled_executor_bundle(...)`
  - canonical example:
    `tassadar_hungarian_compiled_executor_bundle.rs`
  - persisted top-level benchmark, environment, exactness, compatibility,
    lane-status, suite-artifact, and run-bundle files
  - per-case deployment directories for all eight Hungarian-v0 cases

That means Phase 18 is no longer a label on an article-class benchmark slot.
It is now a committed workload contract with committed exact artifacts.

## What Makes It Real

The current lane is real in the bounded sense that matters for truthfulness:

- it solves an actual min-cost perfect-matching problem family
- it uses explicit `4x4` cost matrices, not a fake branch-choice proxy
- it has stable train/validation/test case ids and corpus metadata
- it has a validator-owned benchmark package, not only ad hoc tests
- it has CPU-reference traces for every committed case
- it has a compiled lane that is bound to the matched programs and refuses
  mismatched artifacts exactly

The current lane is still bounded:

- matrix size is fixed at `4x4`
- the committed program family is matched to the bundled corpus
- the exact result is currently on the compiled/proof-backed lane, not the
  learned lane
- the program builder currently uses bounded exhaustive assignment search over
  the `4x4` family, so this is an honest Hungarian-class benchmark/result, not
  a claim that the repo now reproduces the article's exact algorithmic path

That distinction matters and should remain explicit.

## Canonical Bundle

The canonical bundle was materialized directly in the repo with:

- `cargo run -p psionic-research --example tassadar_hungarian_compiled_executor_bundle`

The resulting root is:

- `crates/psionic/fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0`

Top-level bundle facts from `run_bundle.json`:

- `run_id = tassadar-hungarian-v0-compiled-executor-v0`
- `requested_decode_mode = reference_linear`
- `serve_posture = eval_only`
- deployment count: `8`
- benchmark package digest:
  `7ada3dab5f979ca3d789cc8d12ab68cc5b0c7647051eafc5e7598a8ffebf8d65`
- environment bundle digest:
  `7f3c87ecedbb1acaf313b49e259831bd7bdaf894baa10f4b222f3e065fff1031`
- exactness report digest:
  `f072f2f74c6856dd076be4ce301f36a48e2e7d6a3b799864fc577f52731a1584`
- compatibility report digest:
  `fd49bf4dd82362798eab9c86696d37f16cf4cdf7b2e0f848681a6b4c29a7829a`
- lane-status report digest:
  `55e7ca9f4d1ca9c26095f3988410f111c72b1a9084779781311bbbdb7011bd8a`
- compiled suite artifact digest:
  `dfe16aca902fcdcd153c4b3a9743a4ce7f4f6fbb933f33f1c3b890a038dc1cbf`
- bundle digest:
  `f97fdff6739d57f5fe4d5e4423c287c00d8fb0fcc74652e7cd9871f0fd5b0e29`

The committed claim boundary in that bundle is:

> bounded compiled/proof-backed Hungarian-v0 lane exact on the matched 4x4
> corpus and benchmark package; not a learned lane, not arbitrary-program
> closure, and not article parity

That wording is correct and should stay narrow.

## Exactness Result

From `compiled_executor_exactness_report.json`:

- workload family:
  `tassadar.wasm.hungarian_v0_matching.v1.compiled_executor`
- total case count: `8`
- exact trace case count: `8`
- exact trace rate: `10000` bps
- final-output match case count: `8`
- halt-match case count: `8`
- report digest:
  `f072f2f74c6856dd076be4ce301f36a48e2e7d6a3b799864fc577f52731a1584`

What that proves:

- every persisted compiled deployment exactly reproduces the CPU-reference
  append-only trace on the committed Hungarian-v0 corpus
- the bounded Hungarian lane is not just output-correct; its full trace and
  halt posture match CPU reference on all committed cases

What that does not prove:

- a learned Hungarian executor
- arbitrary min-cost matching dimensions
- arbitrary WebAssembly programs
- article-grade Hungarian execution inside a learned transformer
- article parity more broadly

## Compatibility And Refusal Result

From `compiled_executor_compatibility_report.json`:

- total refusal checks: `32`
- matched refusal checks: `32`
- matched refusal rate: `10000` bps
- report digest:
  `fd49bf4dd82362798eab9c86696d37f16cf4cdf7b2e0f848681a6b4c29a7829a`

Each compiled deployment is tested against four mismatch classes:

- wrong program artifact digest
- wrong Wasm profile id
- wrong trace ABI version
- internally inconsistent artifact digest binding

And the report records exact expected-vs-observed refusal matches for all
checks.

That matters because a bounded compiled lane is only honest if it refuses exact
out-of-contract requests just as precisely as it executes in-contract ones.

## Learned Vs Compiled Status

From `hungarian_lane_status_report.json`:

- learned lane status: `not_done`
- learned lane detail:
  `no learned Hungarian-v0 executor lane exists yet; Phase 14 remains blocked on Sudoku`
- compiled lane status: `exact`
- compiled lane detail:
  `bounded compiled/proof-backed Hungarian-v0 lane is exact on the matched corpus`
- report digest:
  `55e7ca9f4d1ca9c26095f3988410f111c72b1a9084779781311bbbdb7011bd8a`

This split is important. Phase 18 does not make the learned lane green by
association.

## Benchmark Package Truth

The top-level `benchmark_package.json` and `environment_bundle.json` mean the
Hungarian-v0 lane is now packaged as validator-owned workload truth rather than
only as a research harness.

The package includes:

- all `8` bounded cases
- stable split metadata
- the committed cost matrices
- the committed optimal assignments
- the committed optimal costs

The bundle writer also verifies that the benchmark package and the compiled
lane align on the same validated program digests before it writes the final
bundle. That prevents the repo from silently drifting into "benchmark contract
over here, compiled proof lane over there" ambiguity.

## What This Means

Phase 18 succeeds on its narrow objective:

- there is now one real bounded Hungarian-class workload family in the repo
- that workload has a real benchmark/environment package
- there is now one exact bounded compiled/proof-backed lane on that corpus
- that lane refuses mismatched artifacts exactly
- the learned and compiled claims are separated explicitly

That is enough to support the stronger umbrella success condition:

- `Hungarian exact result exists`

But only in the bounded compiled/proof-backed sense recorded above.

It is not enough to support the stronger article-shaped claims:

- `the learned model now executes Hungarian exactly`
- `the article's Hungarian result is fully reproduced`
- `arbitrary programs can now be executed inside weights`
- `the repo has article parity`

## Relationship To The Other Open Phases

Phase 18 does not replace the learned-lane work.

`#3814` remains open because the learned 4x4 promotion gate is still red:

- `first_target_exactness_bps = 10000`
- `first_32_token_exactness_bps = 6875`
- `exact_trace_case_count = 0`

So this Hungarian compiled lane does not make the learned Sudoku lane exact by
association.

`#3816` also remains blocked on the learned-lane rule for honest 9x9
promotion. Phase 18 was allowed to land because the gate in the issue text was:

- at least one Sudoku lane works end-to-end with exact artifact-backed evidence

Phase 17 already satisfied that prerequisite on the bounded compiled Sudoku-v0
lane, and Phase 18 now does the same on a second bounded workload family.

## Recommended Repo-Level Statement

The honest repo-level statement after Phase 18 is:

> Psionic now has a real bounded Hungarian-v0 benchmark package and a matched
> compiled/proof-backed executor lane that is exact against CPU reference on
> the committed `4x4` corpus, while the learned Sudoku/Hungarian lane remains
> unfinished and article-parity claims remain unearned.
