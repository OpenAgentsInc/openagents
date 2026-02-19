# DS-Elixir Runtime Contract

Date: 2026-02-19  
Status: Active contract (implemented surface)

This document defines the canonical DS-Elixir runtime contract implemented in `apps/openagents-runtime`.

## 1) Scope and ownership

`openagents-runtime` is the writer/enforcer for DS execution state. Laravel remains control-plane/UI and reads DS outcomes through receipts, events, and projections.

Contract ownership split:

- Runtime owns: signature execution, strategy dispatch, policy evaluation, spend enforcement, receipts/traces, pointer selection, compile/eval persistence.
- Control plane owns: user-facing policy authoring, operator workflow entry points, cohort/feature gating, and run-level UX.

## 2) Non-negotiable invariants

1. Signature IDs are stable and versioned.
2. Signature and artifact compatibility is hash-validated.
3. Compiled artifacts are immutable; behavior changes by pointer mutation only.
4. Predict/tool decisions are receipt-visible with reason codes.
5. Spending is authorization-scoped and reservation-backed before settlement-boundary execution.
6. Runtime event log is replayable and restart-safe; memory state is cache only.

## 3) Canonical schema and reason sources

Primary contract sources:

- `proto/openagents/protocol/v1/receipts.proto`
- `proto/openagents/protocol/v1/reasons.proto`
- `docs/protocol/reasons/runtime-policy-reason-codes.v1.json`
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`

Module-to-contract bindings:

- Signature catalog + hashing: `apps/openagents-runtime/lib/openagents_runtime/ds/signatures/catalog.ex`
- Predict orchestration: `apps/openagents-runtime/lib/openagents_runtime/ds/predict.ex`
- Receipts: `apps/openagents-runtime/lib/openagents_runtime/ds/receipts.ex`
- Policy evaluation and taxonomy: `apps/openagents-runtime/lib/openagents_runtime/ds/policy_evaluator.ex`, `apps/openagents-runtime/lib/openagents_runtime/ds/policy_reason_codes.ex`
- Traces: `apps/openagents-runtime/lib/openagents_runtime/ds/traces.ex`
- Pointer selection + canary rollout: `apps/openagents-runtime/lib/openagents_runtime/ds/policy_registry.ex`
- Pointer audit + promotion/rollback: `apps/openagents-runtime/lib/openagents_runtime/ds/compile/promote_service.ex`
- Proto-derived boundary adapters: `apps/openagents-runtime/lib/openagents_runtime/contracts/layer0_type_adapters.ex`

## 4) Signature contract

Stable ID rule:

- `signature_id = "#{namespace}/#{name}.v#{version}"`

Catalog payload (per signature):

- `signature_id`
- `namespace`, `name`, `version`
- `input_schema`, `output_schema`
- `prompt_template`
- `program_template` (`direct.v1` or `rlm_lite.v1`)

Deterministic hash set:

- `schema_hash`: sha256 over canonicalized `{input_schema, output_schema}`
- `prompt_hash`: sha256 over canonicalized prompt template
- `program_hash`: sha256 over canonicalized program template

Canonicalization/hashing implementation is in:

- `OpenAgentsRuntime.DS.Signatures.Catalog.stable_hash/1`
- `OpenAgentsRuntime.DS.Receipts.stable_hash/1`

Artifact compatibility rule:

- When artifact hash fields are present, runtime must validate hash equality for `schema_hash`, `prompt_hash`, `program_hash`.
- Mismatch is terminal for that predict attempt (`artifact_incompatible`).

## 5) Predict contract

Runtime predict entrypoint:

- `OpenAgentsRuntime.DS.Predict.run/3`

Supported strategies:

- `direct.v1` (`OpenAgentsRuntime.DS.Strategies.DirectV1`)
- `rlm_lite.v1` (`OpenAgentsRuntime.DS.Strategies.RlmLiteV1`)

Required receipt fields (predict):

- `receipt_id`
- `run_id`
- `signature_id`
- `strategy_id`
- `compiled_id`
- `schema_hash`, `prompt_hash`, `program_hash`
- `params_hash`, `output_hash`
- `policy`
- `budget`
- `timing`
- `catalog_version`

Optional trace fields:

- `trace_ref`
- `trace_hash`
- `trace_storage` (`inline` or `external`)
- `trace_artifact_uri`

Policy/authorization linkage (required in runtime decisions):

- `authorization_id`
- `authorization_mode`
- `decision`
- `reason_code`
- `reason_codes_version`
- `evaluation_hash`

## 6) Spend authorization and reservation contract

Authorizations table:

- `runtime.spend_authorizations`

Resolution entrypoint:

- `OpenAgentsRuntime.Spend.Authorizations.resolve_for_run/2`

Authorization modes:

- `interactive`
- `delegated_budget`
- `delegated_budget_with_threshold`
- `deny`
- `system` (runtime/service scope)

Reservation ledger table:

- `runtime.spend_reservations`

Reservation state machine:

- `reserved`
- `committed`
- `released`
- `reconcile_required`

Required spend safety properties:

1. Reserve before settlement-boundary execution.
2. Commit or release deterministically after tool outcome.
3. Use `provider_idempotency_key` and correlation linkage for settlement surfaces.
4. For unknown outcomes, mark `reconcile_required` and reconcile before retry when retry class is `dedupe_reconcile_required`.

Enforcement modules:

- `apps/openagents-runtime/lib/openagents_runtime/spend/authorizations.ex`
- `apps/openagents-runtime/lib/openagents_runtime/spend/reservations.ex`
- `apps/openagents-runtime/lib/openagents_runtime/spend/policy.ex`
- `apps/openagents-runtime/lib/openagents_runtime/tools/tool_runner.ex`

## 7) Artifact pointer contract (immutability + rollout)

Pointer table:

- `runtime.ds_artifact_pointers`

Audit table:

- `runtime.ds_pointer_audits`

Pointer fields:

- `signature_id`
- `primary_artifact`
- `canary_artifact`
- `canary_percent`
- `rollout_seed`
- `metadata`

Selection rules:

1. Determine canary key from run/thread/autopilot/user scope.
2. Compute deterministic rollout bucket (`0..99`) from `signature_id|rollout_seed|canary_key`.
3. Select canary artifact only when `canary_percent > bucket`.
4. Emit `artifact_variant` and `rollout_bucket` in policy/receipt context.

Mutation rules:

- Promote/rollback updates pointers only.
- Every mutation appends a durable pointer audit record.
- Artifact payloads are treated as immutable once referenced by pointer history.

## 8) Trace contract

Trace capture entrypoint:

- `OpenAgentsRuntime.DS.Traces.capture/4`

Storage policy:

- Inline payload when serialized trace size is under threshold (`max_inline_bytes`).
- External pointer when over threshold:
  - `storage = external`
  - immutable `artifact_uri` (default `gcs://openagents-runtime-ds-traces/...`)
  - payload summary retained inline

Sanitization policy:

- Trace payloads are sanitized before storage/reference.
- See `apps/openagents-runtime/docs/SANITIZATION_POLICY.md`.

## 9) Compile/eval contract

Compile modules:

- `apps/openagents-runtime/lib/openagents_runtime/ds/compile/dataset_exporter.ex`
- `apps/openagents-runtime/lib/openagents_runtime/ds/compile/compile_service.ex`
- `apps/openagents-runtime/lib/openagents_runtime/ds/compile/promote_service.ex`

Compile/eval tables:

- `runtime.ds_compile_reports`
- `runtime.ds_eval_reports`

Deterministic identifiers:

- `dataset_hash`: split/seed/example-ids fingerprint
- `job_hash`: compile job spec fingerprint
- `report_id`: deterministic hash over signature + job + selected artifact
- `eval_id`: deterministic hash over report + artifact + split

Replayable compile behavior:

1. Dataset export normalizes receipts/traces and deterministic splits.
2. Candidate evaluation persists per-split scores.
3. Selected artifact is persisted in compile report.
4. Promote/rollback mutates pointer and appends pointer audit.

## 10) Replay compatibility and evolution

Required compatibility rules:

1. Additive schema evolution by default.
2. Versioned reason taxonomy (`runtime-policy-reasons.v1`) is stable.
3. Runtime re-evaluation uses deterministic `evaluation_hash` from policy/budget/context.
4. Breaking payload changes require explicit version bump and migration/upcaster plan.

## 11) Validation artifacts

Primary contract coverage:

- `apps/openagents-runtime/test/openagents_runtime/ds/signatures/catalog_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/ds/predict_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/ds/policy_registry_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/ds/policy_evaluator_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/ds/compile/dataset_exporter_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/ds/compile/compile_service_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/ds/compile/promote_service_test.exs`

Spend policy and settlement coverage:

- `apps/openagents-runtime/test/openagents_runtime/spend/authorizations_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/spend/reservations_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/tools/tool_runner_execution_test.exs`
