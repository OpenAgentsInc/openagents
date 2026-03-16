# 2026-03-16 Tassadar Phase 17 Compiled-Executor Audit

This note records the bounded proof-oriented / compile-to-weights executor lane
requested by `#3817`.

It is intentionally a truthfulness note, not a parity note.

## Scope

Phase 17 was allowed to do one narrower thing than the learned-lane work:

- take one real workload family
- bind compiled weights to exact program artifacts
- prove exactness against CPU reference on that bounded family
- refuse mismatched artifacts exactly
- persist the resulting bundle and lineage

Phase 17 was not allowed to:

- claim arbitrary-program closure
- claim learned-lane success
- close the Phase 14 learned 4x4 gate
- unblock Phase 16 by rhetoric
- claim article parity

The canonical artifact root is:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0`

The decisive machine-readable artifacts are:

- `compiled_executor_exactness_report.json`
- `compiled_executor_compatibility_report.json`
- `run_bundle.json`

## What Landed

Phase 17 makes the bounded compiled lane real in-repo:

- `psionic-models`
  - `TassadarCompiledProgramExecutor` now preserves compile-evidence bundles as
    first-class deployment artifacts rather than only transient compile output
  - descriptor-contract failure is now typed through
    `TassadarExecutorContractError`
- `psionic-eval`
  - `build_tassadar_sudoku_v0_compiled_executor_corpus(...)`
  - `build_tassadar_compiled_executor_exactness_report(...)`
  - `build_tassadar_compiled_executor_compatibility_report(...)`
  - stable workload family id:
    `tassadar.wasm.sudoku_v0_search.v1.compiled_executor`
- `psionic-research`
  - canonical bundle writer:
    `run_tassadar_compiled_executor_bundle(...)`
  - canonical example:
    `tassadar_compiled_executor_bundle.rs`
- persisted bundle layout
  - top-level exactness, compatibility, suite-artifact, and run-bundle files
  - per-case deployment directories for all eight Sudoku-v0 cases
  - per-case persisted artifacts:
    - `program_artifact.json`
    - `compiled_weight_artifact.json`
    - `runtime_contract.json`
    - `compiled_weight_bundle.json`
    - `compile_evidence_bundle.json`
    - `model_descriptor.json`

That means the compiled lane is no longer just a library surface or a unit
test. It now has a committed artifact bundle with exactness, refusal, and
lineage evidence.

## Canonical Bundle

The canonical bundle was materialized directly in the repo with:

- `cargo run -p psionic-research --example tassadar_compiled_executor_bundle`

The resulting root is:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0`

Top-level bundle facts from `run_bundle.json`:

- `run_id = tassadar-sudoku-v0-compiled-executor-v0`
- `requested_decode_mode = reference_linear`
- `serve_posture = eval_only`
- deployment count: `8`
- bundle digest:
  `2a9fcad99dd164fc34f283f28594085990b1e66b7f5ca37d652e92fd13b2d5dd`

The committed claim boundary in that bundle is:

> bounded compiled/proof-backed Sudoku-v0 executor lane exact on the matched
> corpus; not arbitrary-program closure and not exposed in serving by default

That wording is correct and should stay narrow.

## Exactness Result

From `compiled_executor_exactness_report.json`:

- workload family:
  `tassadar.wasm.sudoku_v0_search.v1.compiled_executor`
- total case count: `8`
- exact trace case count: `8`
- exact trace rate: `10000` bps
- final-output match case count: `8`
- halt-match case count: `8`
- report digest:
  `3dbf95e3f02d7ce75db501f702c137770f9c87abe90347abfeb5747eff83c01a`

What that proves:

- every persisted compiled deployment exactly reproduces the CPU-reference
  append-only trace on the real bounded Sudoku-v0 corpus
- the compiled lane is not just output-correct; its full trace and halt posture
  match CPU reference on all committed cases

What that does not prove:

- arbitrary Sudoku programs
- arbitrary WebAssembly programs
- learned executor correctness
- article-grade millions-of-step generality

## Compatibility And Refusal Result

From `compiled_executor_compatibility_report.json`:

- total refusal checks: `32`
- matched refusal checks: `32`
- matched refusal rate: `10000` bps
- report digest:
  `a871e7027f08a90e4643c7f98c70bb9185055a15de1f0404e12c0f65b59bb5c2`

Each compiled deployment is tested against four mismatch classes:

- wrong program artifact digest
- wrong Wasm profile id
- wrong trace ABI version
- internally inconsistent artifact digest binding

The important point is not only that the lane refuses. It refuses for the
right typed reason, and the report records exact expected-vs-observed matches.

That matters because without strict refusal, a compiled lane can quietly drift
from “bounded exact deployment” into “vague runtime that happened to work on
one benchmark.”

## What This Means

Phase 17 succeeds on its narrow objective:

- there is now one bounded compiled/proof-backed executor lane with persisted
  artifacts
- that lane is exact on the matched Sudoku-v0 corpus
- that lane refuses mismatched artifacts exactly
- proof/runtime lineage is committed per deployment

That is enough to support the weaker umbrella success condition:

- “bounded proof/compiled executor exists”

It is not enough to support the stronger article-shaped claims:

- “the learned model is now an exact executor”
- “9x9 is now honestly promoted”
- “Hungarian is reproduced”
- “arbitrary programs can now be executed inside weights”

## Relationship To The Other Open Phases

Phase 17 does not replace the learned-lane work.

`#3814` remains open because the learned 4x4 promotion gate is still red:

- `first_target_exactness_bps = 10000`
- `first_32_token_exactness_bps = 6875`
- `exact_trace_case_count = 0`

So this compiled lane does not make the learned lane green by association.

`#3816` also remains blocked on the learned-lane rules for honest 9x9
promotion. Phase 17 was allowed to land out of that order only because it is a
separate bounded compiled/proof-backed lane with its own explicit claim
boundary.

And `#3818` should still wait for either:

- an honest learned Sudoku lane that really works, or
- a similarly bounded Hungarian compiled/proof-backed lane with its own exact
  artifacts

## Recommended Next Step

The repo should use Phase 17 as a strong bounded truth surface, not as a
marketing shortcut.

The next honest move is:

- keep Phase 14 open until the learned lane clears its own gate
- keep Phase 16 blocked on the learned-lane promotion rule
- treat Hungarian as the next compiled/proof-backed candidate only if it lands
  with the same class of exactness and refusal artifacts

Until then, the correct repo-level statement is:

> Psionic now has one real bounded compiled/proof-backed Sudoku-v0 executor
> lane with exact CPU-reference evidence and exact artifact-compatibility
> refusal, but the learned lane is still not exact and broader article-fidelity
> claims are still unearned.
