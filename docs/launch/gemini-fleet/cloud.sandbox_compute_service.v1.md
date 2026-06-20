# cloud.sandbox_compute_service.v1 — paid sandbox receipt shape

Promise: `cloud.sandbox_compute_service.v1` (state: **red** — unchanged).

## Blocker advanced (NOT cleared)

`blocker.product_promises.cloud_sandbox_paid_receipt_missing`

The promise needs a dereferenceable receipt that proves a metered sandbox rental. The previous scaffold was missing a single unified receipt shape that could bind the disparate ref formats:
- The surface ref (`receipt.cloud.sandbox_compute.rental.<id>`)
- The ledger ref written by `cloud-metering.ts` (`receipt.cloud.sandbox_compute.rental.charge.<id>`)

This change adds `apps/openagents.com/workers/api/src/cloud/sandbox-compute-receipt.ts` which supplies the `SandboxRentalReceipt` schema and projection logic, reconciling these two refs into one pure, typed receipt artifact shape.

HONESTY: This is a pure projection shape only; the promise stays red. Nothing is live and no real credits are billed.
