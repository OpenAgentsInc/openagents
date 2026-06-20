// Sandbox compute service — paid receipt projection.
//
// To address blocker.product_promises.cloud_sandbox_paid_receipt_missing,
// this module defines the PURE projection of a dereferenceable PAID sandbox
// rental receipt. It reconciles the sandbox's SURFACE receipt ref
// (`receipt.cloud.sandbox_compute.rental.<id>`) advertised by the service route
// with the actual LEDGER receipt ref
// (`receipt.cloud.sandbox_compute.rental.charge.<id>`) written by the
// atomic cloud-metering seam (`cloud-metering.ts`).
//
// HONEST SCOPE: this is the receipt SHAPE projection, not a live billed product.
// The promise cloud.sandbox_compute_service.v1 STAYS red. Nothing here flips it
// green. A green flip requires a real customer to rent a metered sandbox, run
// work in it, and produce this dereferenceable receipt with owner sign-off per
// proof.claim_upgrade_receipts.v1.

import {
  SANDBOX_COMPUTE_PRIMITIVE,
  type Sandbox,
  sandboxRentalReceiptRef,
} from './sandbox-compute-service-routes'
import { cloudChargeReceiptRef } from './cloud-metering'

export const SANDBOX_RENTAL_RECEIPT_SCHEMA =
  'openagents.cloud.sandbox_compute.rental_receipt.v1' as const

export type SandboxRentalReceipt = Readonly<{
  schemaVersion: typeof SANDBOX_RENTAL_RECEIPT_SCHEMA
  // The surface receipt ref advertised by the sandbox route
  receiptRef: string
  // The ledger receipt ref written by the cloud-metering seam
  ledgerReceiptRef: string
  sandboxId: string
  accountRef: string
  image: string
  ttlSeconds: number
  // The actual metered usage (e.g., wallSeconds, cpuSeconds)
  usage: Readonly<Record<string, number>>
  // The final receipt-first billed amount in msat
  chargeMsat: number
  billed: boolean
}>

// Projects a dereferenceable SandboxRentalReceipt from the core rental details,
// binding the surface ref to the ledger ref and recording the billed usage.
export const buildSandboxRentalReceipt = (
  sandbox: Sandbox,
  usage: Readonly<Record<string, number>>,
  chargeMsat: number,
  billed: boolean,
): SandboxRentalReceipt => {
  return {
    schemaVersion: SANDBOX_RENTAL_RECEIPT_SCHEMA,
    receiptRef: sandboxRentalReceiptRef(sandbox.sandboxId),
    ledgerReceiptRef: cloudChargeReceiptRef(
      SANDBOX_COMPUTE_PRIMITIVE,
      sandbox.sandboxId,
    ),
    sandboxId: sandbox.sandboxId,
    accountRef: sandbox.accountRef,
    image: sandbox.image,
    ttlSeconds: sandbox.ttlSeconds,
    usage,
    chargeMsat,
    billed,
  }
}
