# Compute Benchmark Adapters

This document defines the retained benchmark-adapter layer for the compute
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
  - `apple_adapter_eval_v1`
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

## Concrete Adapters

### `mmlu_multiple_choice_v1`

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

### `apple_adapter_eval_v1`

The Apple lane uses one mixed benchmark adapter rather than a separate import
path per sample family.

Per-case metadata is captured as `AppleAdapterBenchmarkCaseMetadata`:

- `sample_kind`
  - `supervised_fine_tune`
  - `schema_free_guided_generation`
  - `guided_generation_with_schema`
  - `tool_calling`
- `expected_output_digest`
- `expected_output_text`
- `observed_output_text`
- optional `expected_structured_output`
- optional `observed_structured_output`
- optional `required_tool_names[]`
- optional `observed_tool_calls[]`

Adapter behavior:

- converts each Apple benchmark case into a canonical
  `ComputeEvaluationSample`
- always emits `apple_adapter.text_match`
- emits `apple_adapter.structured_output_match` for structured-generation
  cases
- emits `apple_adapter.tool_call_coverage` for tool-calling cases
- averages emitted metrics into canonical `score_bps`
- preserves the typed Apple case payload inside sample metadata under
  `benchmark_case`
- rejects malformed Apple imports before authority submission when required
  structured output or required tool names are missing

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
- mixed Apple adapter import cases in `openagents-kernel-core`, including
  structured-output and tool-call scoring plus malformed-request rejection

## Next Integration Path

- `#3568`: `psionic-eval` now executes held-out and benchmark-class runs
  locally with the same environment-bound scoring shape, and it can simulate
  validator execution against the same packaged benchmark contract that later
  imports into this kernel eval lifecycle
