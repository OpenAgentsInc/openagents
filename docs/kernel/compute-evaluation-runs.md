# Compute Evaluation Runs

This document defines the first kernel-owned evaluation-run lifecycle landed for
the compute expansion.

## Purpose

Evaluation runs turn environment-backed scoring into canonical kernel objects
instead of loose JSON folders.

The retained owner split is:

- `openagents-kernel-core`: evaluation-run object model and wire contracts
- `apps/nexus-control`: canonical mutation/read-model authority
- future environment/eval services: run orchestration, harness execution, and
  artifact production

## Object Model

The authority now manages:

- `ComputeEvaluationRun`
  - identity: `eval_run_id`
  - environment truth: `environment_binding`
  - compute linkage: optional `product_id`, `capacity_lot_id`, `instrument_id`,
    `delivery_proof_id`
  - runtime linkage: optional `model_ref`, `source_ref`
  - lifecycle: `created_at_ms`, `started_at_ms`, `finalized_at_ms`, `status`
  - expectations: optional `expected_sample_count`
  - terminal summary: `summary`
  - aggregate artifacts: `run_artifacts`
  - extension surface: `metadata`
- `ComputeEvaluationSample`
  - identity: `eval_run_id`, `sample_id`, optional `ordinal`
  - lifecycle: `status`, `recorded_at_ms`
  - sample refs: `input_ref`, `output_ref`, `expected_output_ref`
  - scoring: optional `score_bps`, `metrics[]`
  - sample artifacts: `artifacts[]`
  - error state: optional `error_reason`
  - extension surface: `metadata`
- `ComputeEvaluationSummary`
  - totals: `total_samples`, `scored_samples`, `passed_samples`,
    `failed_samples`, `errored_samples`
  - aggregate scores: `average_score_bps`, `pass_rate_bps`
  - rollups: `aggregate_metrics[]`
  - aggregate artifacts: `artifacts[]`

## Authority Lifecycle

Nexus now exposes:

- `POST /v1/kernel/compute/evals`
- `GET /v1/kernel/compute/evals?environment_ref=&product_id=&status=`
- `GET /v1/kernel/compute/evals/{eval_run_id}`
- `POST /v1/kernel/compute/evals/{eval_run_id}/samples`
- `GET /v1/kernel/compute/evals/{eval_run_id}/samples`
- `POST /v1/kernel/compute/evals/{eval_run_id}/finalize`

Receipt types:

- `kernel.compute.eval_run.create`
- `kernel.compute.eval_run.samples.append`
- `kernel.compute.eval_run.finalize`

## Binding Rules

- Eval runs are environment-backed objects, so create flow resolves
  `environment_binding` against the environment registry and locks
  `environment_version`.
- When the environment package has a single dataset, rubric, or evaluator policy
  and the caller omitted that field, Nexus hydrates the binding from the
  package.
- Eval runs may link to compute objects through `product_id`,
  `capacity_lot_id`, `instrument_id`, and `delivery_proof_id`.
- If a delivery proof is linked, Nexus enforces environment consistency and, on
  finalize, writes `verification_evidence.eval_run_ref` back into the delivery
  proof.

## Sample Ingestion And Finalize Semantics

- Sample ids are unique per eval run.
- Terminal eval runs reject further sample ingestion.
- If `expected_sample_count` is set, append cannot exceed it and finalize
  requires the run to be complete.
- Finalize computes aggregate summary state from stored samples:
  - average score from `score_bps`
  - pass/fail counts from explicit sample status or the environment rubric
    threshold
  - aggregate metric rollups by metric id
- Aggregate artifacts supplied at finalize are stored on the run and surfaced in
  the summary.

## Read Model Guarantees

- Eval runs and samples persist in canonical kernel state and survive restart.
- HTTP client and generated proto contracts roundtrip the full lifecycle.
- Delivery proofs can now point to a canonical `eval_run_ref` instead of only to
  raw environment refs.

## Next Integration Path

- `#3519`: synthetic-data pipelines should emit directly into these eval-run
  objects
- `#3520`: benchmark adapters should terminate in this lifecycle rather than
  inventing parallel result stores
