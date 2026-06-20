// Independent, consumer-side tamper verification for the public Artanis labor
// receipt feed (#4731, blocker artanis_labor_unattended_request_receipts_missing).
//
// The serving side already re-verifies every sealed receipt against the ref it is
// keyed under before projecting it (artanis-labor-receipt-routes.ts), but a THIRD
// PARTY reading the public JSON feed had no library function to confirm the same
// thing for itself. A feed row carries exactly the eight canonical receipt fields
// plus the server's claimed `receiptRef`, so a consumer can reconstruct the
// canonical receipt from the row's own public fields, re-derive its
// content-address, and confirm it matches the ref the server served it under -
// WITHOUT trusting the server. This module is that consumer-side verifier.
//
// It mints no payment, identity, or settlement authority and reads no private
// material: it only re-derives a content-address over already public fields and
// compares it to the served name.

import {
  serializeArtanisLaborUnattendedRequestReceipt,
  verifyArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborUnattendedRequestReceipt,
} from './artanis-labor-request-receipt'
import type {
  ArtanisLaborReceiptFeedProjection,
  ArtanisLaborReceiptFeedRow,
} from './artanis-labor-receipt-routes'

// Reconstruct the canonical receipt object from a feed row's public fields. The
// row deliberately carries exactly the canonical field set, so this is a lossless
// projection back to the receipt the row was minted from. The reconstructed
// object is NOT yet trusted - it must go through serialize + verify, which re-runs
// the public-safety guard, the placed-vs-pre-request invariant, and the canonical
// form check before any ref comparison.
const receiptFromFeedRow = (
  row: ArtanisLaborReceiptFeedRow,
): ArtanisLaborUnattendedRequestReceipt => ({
  artanisActorRef: row.artanisActorRef,
  budgetMsat: row.budgetMsat,
  issuedAtIso: row.issuedAtIso,
  lifecycleRefs: [...row.lifecycleRefs],
  schema: row.schema,
  terminalState: row.terminalState,
  tickRef: row.tickRef,
  workRequestId: row.workRequestId,
})

// Confirm one public feed row is self-consistent: its claimed `receiptRef` must
// content-address the row's own public fields. Returns the validated receipt on
// success; throws (ArtanisLaborReceiptError) if the row is internally
// inconsistent (e.g. a "refused" row carrying a budget) or if the served
// `receiptRef` does not address the row's fields. A consumer who only has the
// public feed JSON can therefore detect tampering on its own.
export const verifyArtanisLaborReceiptFeedRow = (
  row: ArtanisLaborReceiptFeedRow,
): ArtanisLaborUnattendedRequestReceipt => {
  const serialized = serializeArtanisLaborUnattendedRequestReceipt(
    receiptFromFeedRow(row),
  )
  return verifyArtanisLaborUnattendedRequestReceipt(serialized, row.receiptRef)
}

export type ArtanisLaborReceiptFeedVerification = Readonly<{
  // Number of rows that were independently re-derived and matched their served
  // ref. Equals the feed's row count when the feed verifies clean.
  verifiedRowCount: number
  receipts: ReadonlyArray<ArtanisLaborUnattendedRequestReceipt>
}>

// Verify every row in a public feed projection. Throws on the FIRST row whose
// served ref does not content-address its own fields, so a consumer can treat a
// non-throwing return as "every served receipt is self-consistent". This is the
// entry point an external auditor calls against `GET
// /api/public/artanis/labor-receipts`. It asserts no authority - it only
// re-derives content-addresses over public fields.
export const verifyArtanisLaborReceiptFeed = (
  feed: ArtanisLaborReceiptFeedProjection,
): ArtanisLaborReceiptFeedVerification => {
  const receipts = feed.rows.map(verifyArtanisLaborReceiptFeedRow)
  return { verifiedRowCount: receipts.length, receipts }
}
