// Public green-readiness projection for artanis.labor_requester.v1 (#4731,
// EPIC #5531 DE-8). The receipt machinery is complete: the requester surface
// runs a gated tick (artanis-labor-requester.ts), the tick driver seals its
// outcome into a content-addressed receipt (artanis-labor-tick-driver.ts), the
// store persists it tamper-evidently (artanis-labor-receipt-store.ts), and the
// public feed serves + re-verifies it (artanis-labor-receipt-routes.ts). What
// was still missing is the surface that maps that feed onto the TWO named
// green-flip blockers so a reviewer (or the operator recording the transition)
// can read a single dereferenceable JSON to see whether the gate is met:
//
//   - blocker.product_promises.artanis_labor_live_enablement_missing
//       cleared once at least one PLACED unattended request receipt exists (a
//       receipt that reserved escrow: requested_pending_delivery,
//       accepted_released, or rejected_refunded). A placed receipt can only be
//       minted by an ENABLED tick (the requester refuses to reserve escrow when
//       config-disabled), so its existence is the live-enablement proof.
//
//   - blocker.product_promises.artanis_labor_unattended_request_receipts_missing
//       cleared once the count of placed unattended request receipts reaches the
//       code-anchored target.
//
// This module is projection-only. It re-derives nothing the feed has not already
// content-address-verified, mints no payment/identity/settlement authority, and
// reads only public-safe receipt fields. It cannot create a receipt or flip a
// blocker; an absent or refused receipt can only ever leave the gate unmet.

import type {
  ArtanisLaborReceiptFeedProjection,
  ArtanisLaborReceiptFeedRow,
} from './artanis-labor-receipt-routes'
import type { ArtanisLaborReceiptTerminalState } from './artanis-labor-request-receipt'
import {
  liveAtReadStaleness,
  type PublicProjectionStalenessContract,
} from './public-projection-staleness'

// The number of placed unattended request receipts the gate calls for before the
// unattended-request-receipts blocker can clear. Code-anchored so the projection
// and any future verifier read the same threshold. Mirrors the responder /
// evolution-loop ten-tick convention.
export const ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET = 10

// The exact blocker refs this readiness surface maps to, so the projection names
// the same tokens the registry carries on the promise.
export const ARTANIS_LABOR_LIVE_ENABLEMENT_BLOCKER =
  'blocker.product_promises.artanis_labor_live_enablement_missing'
export const ARTANIS_LABOR_UNATTENDED_RECEIPTS_BLOCKER =
  'blocker.product_promises.artanis_labor_unattended_request_receipts_missing'

// A "placed" receipt reserved escrow on a real work request. Only an enabled
// tick can reach these states; a skipped (config_disabled) or refused tick never
// does. This is the same predicate the feed summary uses for placedRequestCount.
const PLACED_STATES: ReadonlySet<ArtanisLaborReceiptTerminalState> = new Set([
  'requested_pending_delivery',
  'accepted_released',
  'rejected_refunded',
])

// Only refs that themselves look like a public-safe ref token are projected, so
// nothing unexpected smuggled into a feed row can leak through this surface.
const safeRefPattern = /^[A-Za-z0-9._:/-]{1,200}$/

const safeRef = (value: string | null): string | null =>
  value !== null && safeRefPattern.test(value) ? value : null

export type ArtanisLaborPlacedRequest = Readonly<{
  // The content-addressed receipt ref a reader can dereference at
  // /api/public/artanis/labor-receipts?receiptRef=<ref>.
  receiptRef: string
  terminalState: ArtanisLaborReceiptTerminalState
  tickRef: string
  workRequestId: string | null
  budgetMsat: number | null
  issuedAtIso: string
}>

export type ArtanisLaborGreenReadinessProjection = Readonly<{
  kind: 'artanis_labor_requester_green_readiness'
  publicSafe: true
  authorityBoundary: string
  staleness: PublicProjectionStalenessContract
  // The blocker tokens this surface tracks, named so the operator recording the
  // transition can cite the exact refs being cleared.
  blockerRefs: ReadonlyArray<string>
  // The number of placed receipts required before the receipts blocker clears.
  unattendedRequestTarget: number
  // The placed-request count over the projected feed window.
  placedRequestCount: number
  // True once at least one placed receipt exists. A placed receipt can only be
  // minted by an enabled tick, so this is the live-enablement proof.
  liveEnablementProven: boolean
  // True once placedRequestCount reaches the target on dereferenceable receipts.
  unattendedRequestReceiptsProven: boolean
  // True iff BOTH dimensions are proven. This is the green gate predicate; it
  // never includes the separate owner sign-off, which stays out of band.
  greenGateMet: boolean
  // Receipt counts per terminal state, carried through from the feed summary so
  // a reviewer sees how many ticks ran the gates without placing (skipped /
  // refused) alongside the placed ones.
  byTerminalState: Readonly<Record<ArtanisLaborReceiptTerminalState, number>>
  // The placed unattended request receipts, each with its dereferenceable ref.
  placedRequests: ReadonlyArray<ArtanisLaborPlacedRequest>
  generatedAt: string
  notes: ReadonlyArray<string>
}>

const AUTHORITY_BOUNDARY =
  'Read-only green-readiness projection over the public Artanis labor receipt feed. Grants no dispatch, spend, escrow, settlement, or registry authority and cannot create a receipt, enable a tick, or flip a blocker. It only maps already-content-address-verified, public-safe receipts onto the two named green-flip blockers; a missing or refused receipt can only ever leave the gate unmet. greenGateMet reflects the mechanical receipt evidence only and never includes the separate owner sign-off required to record the yellow->green transition.'

const isPlaced = (state: ArtanisLaborReceiptTerminalState): boolean =>
  PLACED_STATES.has(state)

const toPlacedRequest = (
  row: ArtanisLaborReceiptFeedRow,
): ArtanisLaborPlacedRequest => ({
  budgetMsat: row.budgetMsat,
  issuedAtIso: row.issuedAtIso,
  receiptRef: row.receiptRef,
  terminalState: row.terminalState,
  tickRef: row.tickRef,
  workRequestId: safeRef(row.workRequestId),
})

// Fold a labor receipt feed projection into the green-readiness view. The feed
// has already re-verified every row against its own content address, so this
// only classifies and counts; it adds no trust. The summary counts come from the
// feed's own (pre-filter) summary so a filtered feed can never understate the
// gate, and the placed-request rows are recomputed from the feed rows so the
// dereferenceable refs are exactly those the feed serves.
export const projectArtanisLaborGreenReadinessProjection = (
  feed: ArtanisLaborReceiptFeedProjection,
  nowIso: string,
): ArtanisLaborGreenReadinessProjection => {
  const placedRequests = feed.rows
    .filter(row => isPlaced(row.terminalState))
    .map(toPlacedRequest)

  // The headline placed count comes from the feed's own summary (computed over
  // the full set before any filter), so it is the honest total even if the feed
  // was fetched with a terminalState filter.
  const placedRequestCount = feed.summary.placedRequestCount

  const liveEnablementProven = placedRequestCount > 0
  const unattendedRequestReceiptsProven =
    placedRequestCount >= ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET
  const greenGateMet =
    liveEnablementProven && unattendedRequestReceiptsProven

  return {
    authorityBoundary: AUTHORITY_BOUNDARY,
    blockerRefs: [
      ARTANIS_LABOR_LIVE_ENABLEMENT_BLOCKER,
      ARTANIS_LABOR_UNATTENDED_RECEIPTS_BLOCKER,
    ],
    byTerminalState: feed.summary.byTerminalState,
    generatedAt: nowIso,
    greenGateMet,
    kind: 'artanis_labor_requester_green_readiness',
    liveEnablementProven,
    notes: [
      `A placed unattended request receipt is one whose terminalState reserved escrow (requested_pending_delivery, accepted_released, or rejected_refunded). Only an operator-ENABLED tick can reach a placed state; a config-disabled tick is sealed as skipped_config_disabled and never places. The gate requires ${ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET} placed receipts.`,
      `liveEnablementProven clears ${ARTANIS_LABOR_LIVE_ENABLEMENT_BLOCKER}: the first placed receipt is proof a real enabled tick reserved escrow.`,
      `unattendedRequestReceiptsProven clears ${ARTANIS_LABOR_UNATTENDED_RECEIPTS_BLOCKER}: ${ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET} placed receipts accrued from unattended ticks.`,
      'Each placedRequests entry is dereferenceable at /api/public/artanis/labor-receipts?receiptRef=<receiptRef>, where a reader can re-derive the content address over the row\'s own public fields and confirm it matches the served ref.',
      'greenGateMet is the mechanical receipt-evidence predicate only. The yellow->green flip additionally requires the owner-signed promise_transition recorded via POST /api/operator/product-promises/transitions; a passing readiness surface is not the flip itself.',
    ],
    placedRequestCount,
    placedRequests,
    publicSafe: true,
    staleness: liveAtReadStaleness([
      'artanis_labor_unattended_request_receipt_stored',
    ]),
    unattendedRequestReceiptsProven,
    unattendedRequestTarget: ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET,
  }
}

// Kept exported for any server-side verifier that wants to assert the gate
// predicate without re-parsing the projection JSON.
export const artanisLaborGreenGateMet = (
  readiness: ArtanisLaborGreenReadinessProjection,
): boolean => readiness.greenGateMet
