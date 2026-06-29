# Tassadar CPU-Transform Training Receipt Status Surface

Date: 2026-06-21

Promise: `models.tassadar_percepta_executor.v1`

Public route:
`/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts`

## What This Clears

This clears the old missing CPU-transform receipt blocker for the bounded
fixture scope only:

- `blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing`

The remaining blockers are still active:

- `blocker.product_promises.tassadar_cpu_transform_real_settlement_missing`
- `blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing`

The route is a public receipt surface for one bounded Pylon v1.0 CPU
computation-transform fixture. It makes the evidence inspectable without
claiming a trained model, paid earning path, model promotion, or green promise.

## Inputs Already Visible

The status surface cites the two prerequisite public-safe inputs that already
exist:

1. Architecture receipt:
   `/api/public/models/tassadar-percepta-executor/architecture-receipts`
   with `receipt.models.tassadar_percepta_executor.architecture.bundle.v1`.
2. Artanis distillation dataset receipt:
   `/api/public/artanis/tassadar-distillation-dataset`
   with
   `receipt.training.tassadar_distillation_dataset.artanis_admin_verified_trace_refs.v1`.

Those inputs are necessary context, not training proof. They do not create a
trained model, accepted Pylon work, verifier verdict, settlement, inference
endpoint, or green transition.

## Bounded Receipt

The route projects one public-safe fixture receipt:

- assignment:
  `assignment.models.tassadar_percepta_executor.cpu_transform_fixture.v1`
- receipt:
  `receipt.models.tassadar_percepta_executor.cpu_transform_training.cpu_transform_fixture_v1`
- verifier verdict:
  `verdict.tassadar_cpu_transform.exact_replay.cpu_transform_fixture_v1`
- checkpoint digest:
  `artifact.tassadar_percepta_executor.cpu_transform_checkpoint.sha256.8feaf5488599a4b618b8d2188ed8ea0b68ec9fb5f58a55db3064e52ae9ff73d9`

The fixture runs one CPU-only optimization step over a public linear transform
fixture. It records loss improvement from `1666666` to `546296` micros and marks
the replay verdict accepted.

## Remaining Gates

The route reports the bounded receipt gates as true, while keeping settlement
and green authority false:

- Pylon assignment receipt available: true
- accepted-work receipt available: true
- verifier verdict receipt available: true
- real settlement receipt available: false
- trained artifact digest available: true for the fixture checkpoint only
- green gate satisfied: false

The expected future receipt pattern is:

`receipt.models.tassadar_percepta_executor.cpu_transform_training.{assignmentRef}`

## Boundaries

This is read-only. It does not dispatch work, spend, settle, promote a model,
serve inference, or transition the promise to green.

Green still requires real settlement refs where money moved where applicable,
owner sign-off, and the receipt-first upgrade gate under
`proof.claim_upgrade_receipts.v1`. The current fixture receipt is not a broad
CPU-transform training completion claim.
