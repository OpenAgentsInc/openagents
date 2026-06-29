# Tassadar Percepta Architecture Receipt

Date: 2026-06-20

Promise: `models.tassadar_percepta_executor.v1`

Public route:
`/api/public/models/tassadar-percepta-executor/architecture-receipts`

Related CPU-transform receipt status route:
`/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts`

Receipt ref:
`receipt.models.tassadar_percepta_executor.architecture.bundle.v1`

## What This Clears

This clears only:

- `blocker.product_promises.percepta_executor_architecture_receipts_missing`

The current registry keeps the promise planned because the remaining blocker is
still real:

- `blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing`

## Receipt Contents

The public projection binds the Tassadar Percepta Executor direction to four
public-safe component sets:

1. Psionic compiled-executor bundles:
   `fixtures/tassadar/runs/compiled_kernel_suite_v0/deployments/`
2. W3 baseline-D frozen-executor learned-interface artifacts:
   `fixtures/tassadar/w3_student_sweep_20260612/`
3. The exact-trace replay verifier boundary:
   `promise:compute.tassadar_executor_poc.v1`
4. Artifact-lineage refs from the model/spec and W3 report.

The route exposes refs and digests only: model profile refs, compiled/frozen
executor refs, learned-interface refs, checkpoint/interface/eval hashes, and
verifier refs.

## Boundaries

This is not a trained-model receipt. It is not a Pylon CPU-transform training
receipt. It does not create an inference endpoint, model promotion, settlement,
paid contributor claim, or green product-promise transition.

The CPU-transform status route is also non-claiming: it cites this architecture
receipt and the Artanis distillation dataset receipt as inputs, but reports no
Pylon assignment, accepted work, verifier verdict, real settlement, trained
artifact digest, or green gate.

Green still requires Pylon CPU-transform training receipts with assignment
refs, accepted work refs, verifier verdict refs, and settlement refs where real
money moved, followed by owner-signed receipt-first upgrade under
`proof.claim_upgrade_receipts.v1`.
