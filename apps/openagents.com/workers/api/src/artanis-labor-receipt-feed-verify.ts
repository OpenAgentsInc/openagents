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
  ArtanisLaborReceiptError,
  serializeArtanisLaborUnattendedRequestReceipt,
  verifyArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborReceiptTerminalState,
  type ArtanisLaborUnattendedRequestReceipt,
} from './artanis-labor-request-receipt'
import type {
  ArtanisLaborReceiptFeedProjection,
  ArtanisLaborReceiptFeedRow,
} from './artanis-labor-receipt-routes'
import { parseJsonUnknown } from './json-boundary'

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

// --- Untrusted-bytes parse boundary --------------------------------------
//
// The verify functions above operate on a typed `ArtanisLaborReceiptFeedRow` /
// `ArtanisLaborReceiptFeedProjection`. But a THIRD PARTY does not start with a
// typed projection - it starts with raw JSON bytes downloaded from `GET
// /api/public/artanis/labor-receipts`. Without a parse boundary it would have to
// cast that untrusted JSON to the typed shape (an unchecked `as`), defeating the
// whole point of independent verification. These functions are that boundary:
// they validate the wire shape from untrusted bytes into typed rows BEFORE the
// content-address re-derivation runs, so a consumer can go from bytes straight to
// a verified result. They mint no authority and read only public fields.

const FEED_SCHEMA_VERSION = 'openagents.artanis_labor_receipt_feed.v1'

const TERMINAL_STATES: ReadonlyArray<ArtanisLaborReceiptTerminalState> = [
  'skipped_config_disabled',
  'refused',
  'requested_pending_delivery',
  'accepted_released',
  'rejected_refunded',
]

const isFeedRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const isFeedTerminalState = (
  value: unknown,
): value is ArtanisLaborReceiptTerminalState =>
  typeof value === 'string' &&
  (TERMINAL_STATES as ReadonlyArray<string>).includes(value)

const requireFeedString = (value: unknown, label: string): string => {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new ArtanisLaborReceiptError(`Feed ${label} must be a non-empty string.`)
  }
  return value
}

const requireNumberOrNull = (value: unknown, label: string): number | null => {
  if (value === null) {
    return null
  }
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value
  }
  throw new ArtanisLaborReceiptError(`Feed ${label} must be a number or null.`)
}

const requireStringOrNull = (value: unknown, label: string): string | null => {
  if (value === null) {
    return null
  }
  if (typeof value === 'string') {
    return value
  }
  throw new ArtanisLaborReceiptError(`Feed ${label} must be a string or null.`)
}

// Validate one untrusted JSON value into a typed feed row. Only structural type
// checks happen here; the placed-vs-pre-request invariant and the ref match are
// enforced by the subsequent `verifyArtanisLaborReceiptFeedRow` call.
const parseFeedRow = (value: unknown): ArtanisLaborReceiptFeedRow => {
  if (!isFeedRecord(value)) {
    throw new ArtanisLaborReceiptError('Feed row must be a JSON object.')
  }
  if (value.schema !== 'artanis.labor.unattended_request_receipt.v1') {
    throw new ArtanisLaborReceiptError('Feed row schema is unrecognized.')
  }
  if (!isFeedTerminalState(value.terminalState)) {
    throw new ArtanisLaborReceiptError('Feed row terminalState is unrecognized.')
  }
  const { lifecycleRefs } = value
  if (
    !Array.isArray(lifecycleRefs) ||
    !lifecycleRefs.every(
      (ref) => typeof ref === 'string' && ref.trim().length > 0,
    )
  ) {
    throw new ArtanisLaborReceiptError(
      'Feed row lifecycleRefs must be an array of non-empty strings.',
    )
  }
  return {
    receiptRef: requireFeedString(value.receiptRef, 'row receiptRef'),
    schema: 'artanis.labor.unattended_request_receipt.v1',
    terminalState: value.terminalState,
    tickRef: requireFeedString(value.tickRef, 'row tickRef'),
    artanisActorRef: requireFeedString(value.artanisActorRef, 'row artanisActorRef'),
    workRequestId: requireStringOrNull(value.workRequestId, 'row workRequestId'),
    budgetMsat: requireNumberOrNull(value.budgetMsat, 'row budgetMsat'),
    issuedAtIso: requireFeedString(value.issuedAtIso, 'row issuedAtIso'),
    lifecycleRefs: [...(lifecycleRefs as ReadonlyArray<string>)],
  }
}

export type ArtanisLaborReceiptFeedParse = Readonly<{
  schemaVersion: typeof FEED_SCHEMA_VERSION
  rows: ReadonlyArray<ArtanisLaborReceiptFeedRow>
}>

// Parse untrusted public-feed JSON bytes into typed, structurally-valid rows.
// Validates the envelope (`schemaVersion`) and every row's field types, but does
// NOT re-derive content-addresses - that is `verifyArtanisLaborReceiptFeed`'s
// job. Use `parseAndVerifyArtanisLaborReceiptFeed` to do both in one call.
export const parseArtanisLaborReceiptFeed = (
  serialized: string,
): ArtanisLaborReceiptFeedParse => {
  let decoded: unknown
  try {
    decoded = parseJsonUnknown(serialized)
  } catch {
    throw new ArtanisLaborReceiptError('Feed wire form is not valid JSON.')
  }
  if (!isFeedRecord(decoded)) {
    throw new ArtanisLaborReceiptError('Feed wire form must be a JSON object.')
  }
  if (decoded.schemaVersion !== FEED_SCHEMA_VERSION) {
    throw new ArtanisLaborReceiptError('Feed schemaVersion is unrecognized.')
  }
  if (!Array.isArray(decoded.rows)) {
    throw new ArtanisLaborReceiptError('Feed rows must be an array.')
  }
  return {
    schemaVersion: FEED_SCHEMA_VERSION,
    rows: decoded.rows.map(parseFeedRow),
  }
}

// One-call third-party entry point: take the raw bytes from `GET
// /api/public/artanis/labor-receipts`, validate the wire shape, then re-derive
// every row's content-address and confirm it matches the served ref. A
// non-throwing return means every served receipt is structurally valid AND
// self-consistent under its own content address - all from untrusted bytes, with
// no `as`-cast and no trust in the server. Mints no authority.
export const parseAndVerifyArtanisLaborReceiptFeed = (
  serialized: string,
): ArtanisLaborReceiptFeedVerification => {
  const parsed = parseArtanisLaborReceiptFeed(serialized)
  const receipts = parsed.rows.map(verifyArtanisLaborReceiptFeedRow)
  return { verifiedRowCount: receipts.length, receipts }
}
