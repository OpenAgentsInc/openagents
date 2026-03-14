# Compute Benchmark Adapters

This document defines the first benchmark-adapter layer landed for the compute
expansion.

## Purpose

Benchmark suites should feed the canonical eval lifecycle without forcing the
environment registry or eval object model to become benchmark-specific.

The retained owner split is:

- `openagents-kernel-core`: benchmark-import contract and adapter logic
- `apps/nexus-control`: unchanged canonical eval authority
- future environment/eval services: benchmark execution, artifact production,
  and import orchestration

## Landed Contract

The new adapter layer lives in
`crates/openagents-kernel-core/src/compute_benchmarks.rs`.

It defines:

- `ComputeBenchmarkAdapterKind`
  - currently `mmlu_multiple_choice_v1`
- `ComputeBenchmarkImportRequest`
  - generic adapter input carrying benchmark family, suite ref, environment
    binding, compute linkage, timestamps, cases, run artifacts, and metadata
- `ComputeBenchmarkCaseImport`
  - generic per-case import payload with refs, artifacts, recorded time, and
    adapter-specific metadata
- `ComputeBenchmarkAdaptedRun`
  - generated `CreateComputeEvaluationRunRequest`
  - generated `AppendComputeEvaluationSamplesRequest`
  - generated `FinalizeComputeEvaluationRunRequest`

## Concrete Adapter

The first concrete adapter is `mmlu_multiple_choice_v1`.

Per-case metadata is captured as `MmluMultipleChoiceCaseMetadata`:

- `subject`
- `choices[]`
- `correct_choice_index`
- `predicted_choice_index`
- optional `prompt_id`

Adapter behavior:

- converts each benchmark case into a canonical `ComputeEvaluationSample`
- maps exact-match correctness to `Passed` or `Failed`
- emits an `accuracy` metric per sample
- preserves benchmark-specific metadata inside sample metadata under
  `benchmark_case`
- stamps run-level metadata with `benchmark_adapter_kind`,
  `benchmark_family`, and `benchmark_suite_ref`

## Integration Path

The adapter layer does not add a second authority store.

Instead, callers:

1. build `ComputeBenchmarkImportRequest`
2. run `adapt_compute_benchmark_import(...)`
3. submit the returned eval-run create, sample append, and finalize requests to
   Nexus through the existing eval endpoints

This keeps benchmark breadth operationally useful while preserving one canonical
eval truth path.

## Verification

The landed tests cover:

- adapter-unit validation in `openagents-kernel-core`
- end-to-end MMLU import through `HttpKernelAuthorityClient` into Nexus eval
  endpoints in `apps/nexus-control`

## Next Integration Path

- `#3521`: training-class execution should be able to consume the same
  environment and eval truth without a benchmark-only side channel
