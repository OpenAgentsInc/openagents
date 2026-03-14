# Compute Synthetic Data

This document defines the first kernel-owned synthetic-data lifecycle landed for
the compute expansion.

## Purpose

Synthetic generation now terminates in canonical kernel objects instead of loose
artifact folders. The retained model keeps generation, provenance, and
verification under one authority-owned object graph:

- `openagents-kernel-core`: synthetic-data object model and wire contracts
- `apps/nexus-control`: canonical mutation/read-model authority
- future environment/eval services: teacher prompting, generation execution,
  and asynchronous verification orchestration

## Object Model

The authority now manages:

- `ComputeSyntheticDataJob`
  - identity: `synthetic_job_id`
  - environment truth: `environment_binding`
  - generation provenance: `teacher_model_ref`, optional
    `generation_product_id`, optional `generation_delivery_proof_id`
  - output lineage: optional `output_artifact_ref`
  - lifecycle: `created_at_ms`, `generated_at_ms`, `verified_at_ms`, `status`
  - expectations: optional `target_sample_count`
  - verification linkage: optional `verification_eval_run_id`,
    `verification_summary`
  - extension surface: `metadata`
- `ComputeSyntheticDataSample`
  - identity: `synthetic_job_id`, `sample_id`, optional `ordinal`
  - generation refs: `prompt_ref`, `output_ref`, optional
    `generation_config_ref`, optional `generator_machine_ref`
  - verification linkage: optional `verification_eval_sample_id`,
    optional `verification_status`, optional `verification_score_bps`
  - lifecycle: `status`, `recorded_at_ms`
  - extension surface: `metadata`

## Authority Lifecycle

Nexus now exposes:

- `POST /v1/kernel/compute/synthetic`
- `GET /v1/kernel/compute/synthetic?environment_ref=&generation_product_id=&status=`
- `GET /v1/kernel/compute/synthetic/{synthetic_job_id}`
- `POST /v1/kernel/compute/synthetic/{synthetic_job_id}/samples`
- `GET /v1/kernel/compute/synthetic/{synthetic_job_id}/samples`
- `POST /v1/kernel/compute/synthetic/{synthetic_job_id}/finalize_generation`
- `POST /v1/kernel/compute/synthetic/{synthetic_job_id}/record_verification`

Receipt types:

- `kernel.compute.synthetic.create`
- `kernel.compute.synthetic.samples.append`
- `kernel.compute.synthetic.generation.finalize`
- `kernel.compute.synthetic.verification.record`

## Binding And Verification Rules

- Synthetic jobs are environment-backed objects, so create resolves
  `environment_binding` against the environment registry and locks
  `environment_version`.
- When the environment package has a single dataset, rubric, or evaluator
  policy and the caller omitted that field, Nexus hydrates the binding from the
  package.
- Generation may optionally point at a compute product and delivery proof. When
  present, Nexus enforces product and environment consistency instead of letting
  synthetic corpora float outside compute truth.
- Sample ids are unique per synthetic job.
- If `target_sample_count` is set, append cannot exceed it and generation
  finalize requires the job to be complete.
- Verification requires a finalized eval run on the same environment binding.
- Verification maps eval samples onto synthetic samples by `sample_id` and
  copies sample-level verification status and score into the synthetic records.

## Read Model Guarantees

- Synthetic jobs and samples persist in canonical kernel state and survive
  restart.
- HTTP client and generated proto contracts roundtrip the full create, append,
  finalize-generation, and record-verification lifecycle.
- The verification edge is explicit: synthetic jobs point to a canonical
  `verification_eval_run_id`, and synthetic samples point to their
  `verification_eval_sample_id`.

## Next Integration Path

- `#3520`: benchmark adapters should consume these environment/eval/synthetic
  objects rather than inventing a second benchmark result store
  - landed in `docs/kernel/compute-benchmark-adapters.md`
- later training issues should consume `output_artifact_ref` plus sample-level
  provenance as dataset inputs rather than re-scraping raw generation logs
