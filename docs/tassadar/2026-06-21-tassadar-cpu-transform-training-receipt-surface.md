# Tassadar CPU-Transform Training Receipt Status Surface

Date: 2026-06-21

Promise: `models.tassadar_percepta_executor.v1`

Public route:
`/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts`

## What This Clears

This clears no product blocker by itself.

The remaining blocker is still active:

- `blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing`

The route is a public status surface for the missing receipt gate. It makes the
required evidence shape inspectable without claiming that the training happened.

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

## Missing Receipt Gates

The route reports all real CPU-transform training gates as false:

- Pylon assignment receipt available: false
- accepted-work receipt available: false
- verifier verdict receipt available: false
- real settlement receipt available: false
- trained artifact digest available: false
- green gate satisfied: false

The expected future receipt pattern is:

`receipt.models.tassadar_percepta_executor.cpu_transform_training.{assignmentRef}`

## Boundaries

This is read-only. It does not dispatch work, spend, settle, accept a closeout,
write a training artifact, promote a model, serve inference, or transition the
promise to green.

Green still requires an actual Pylon CPU-transform training assignment with
accepted work, verifier verdicts, real settlement refs where money moved, a
public-safe trained artifact digest, and the receipt-first upgrade gate under
`proof.claim_upgrade_receipts.v1`.
