// Public, read-only feed projection for consolidated Artanis unattended labor
// request receipts (#4731, blocker
// artanis_labor_unattended_request_receipts_missing). The receipt module builds,
// serializes, content-addresses, parses, and verifies one receipt; the store
// module persists and serves them tamper-evidently. What was still missing is
// the READ surface a route can hand to a third party: a pure projection that
// folds a set of sealed receipts into one public-safe feed (summary + rows), and
// a GET handler that dereferences either the whole feed or a single receipt by
// its content-addressed ref.
//
// Safe-by-construction: no mutation, no secrets, no spend, no authority. It only
// re-projects already public-safe, content-addressed receipts, re-verifying each
// against the ref it is keyed under so the feed can never serve a receipt
// addressed by the wrong name.

import { Effect } from 'effect'

import {
  projectArtanisLaborGreenReadinessProjection,
  type ArtanisLaborGreenReadinessProjection,
} from './artanis-labor-green-readiness'
import {
  verifyArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborReceiptTerminalState,
} from './artanis-labor-request-receipt'
import type {
  ArtanisLaborSealedReceipt,
  ArtanisLaborUnattendedReceiptStore,
} from './artanis-labor-receipt-store'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  liveAtReadStaleness,
  type PublicProjectionStalenessContract,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

// The terminal states a feed can summarize, in lifecycle order so the summary
// reads as the request lifecycle from least to most advanced.
const TERMINAL_STATE_ORDER: ReadonlyArray<ArtanisLaborReceiptTerminalState> = [
  'skipped_config_disabled',
  'refused',
  'requested_pending_delivery',
  'accepted_released',
  'rejected_refunded',
]

export type ArtanisLaborReceiptFeedRow = Readonly<{
  receiptRef: string
  schema: 'artanis.labor.unattended_request_receipt.v1'
  terminalState: ArtanisLaborReceiptTerminalState
  tickRef: string
  artanisActorRef: string
  workRequestId: string | null
  budgetMsat: number | null
  issuedAtIso: string
  lifecycleRefs: ReadonlyArray<string>
}>

export type ArtanisLaborReceiptFeedSummary = Readonly<{
  receiptCount: number
  // Count of receipts per terminal state, keyed in lifecycle order. States with
  // zero receipts are still present (value 0) so the shape is stable for clients.
  byTerminalState: Readonly<Record<ArtanisLaborReceiptTerminalState, number>>
  // Receipts that reserved escrow (a placed work request) vs. pre-request ones.
  placedRequestCount: number
}>

export type ArtanisLaborReceiptFeedFilter = Readonly<{
  receiptRef?: string
  terminalState?: ArtanisLaborReceiptTerminalState
}>

export type ArtanisLaborReceiptFeedProjection = Readonly<{
  kind: 'artanis_labor_unattended_request_receipt_feed'
  schemaVersion: 'openagents.artanis_labor_receipt_feed.v1'
  publicSafe: true
  generatedAt: string
  staleness: PublicProjectionStalenessContract
  filter: ArtanisLaborReceiptFeedFilter
  summary: ArtanisLaborReceiptFeedSummary
  rows: ReadonlyArray<ArtanisLaborReceiptFeedRow>
  authorityBoundary: string
}>

const FEED_AUTHORITY_BOUNDARY =
  'This feed exposes no private data, grants no production authority, moves no money, and changes no registry state. It only re-projects already public-safe, content-addressed Artanis labor receipts and re-verifies each against the ref it is keyed under.'

const isPlaced = (state: ArtanisLaborReceiptTerminalState): boolean =>
  state === 'requested_pending_delivery' ||
  state === 'accepted_released' ||
  state === 'rejected_refunded'

// Re-verify the sealed receipt against its own ref before projecting it. The
// store already verifies on read; doing it again here means the feed can never
// serve a receipt whose bytes no longer address its name, even if the store is
// later swapped for a backing that trusts itself.
const sealedToRow = (
  sealed: ArtanisLaborSealedReceipt,
): ArtanisLaborReceiptFeedRow => {
  const receipt = verifyArtanisLaborUnattendedRequestReceipt(
    sealed.serialized,
    sealed.receiptRef,
  )
  return {
    receiptRef: sealed.receiptRef,
    schema: receipt.schema,
    terminalState: receipt.terminalState,
    tickRef: receipt.tickRef,
    artanisActorRef: receipt.artanisActorRef,
    workRequestId: receipt.workRequestId,
    budgetMsat: receipt.budgetMsat,
    issuedAtIso: receipt.issuedAtIso,
    lifecycleRefs: receipt.lifecycleRefs,
  }
}

const emptyByTerminalState = (): Record<
  ArtanisLaborReceiptTerminalState,
  number
> => ({
  skipped_config_disabled: 0,
  refused: 0,
  requested_pending_delivery: 0,
  accepted_released: 0,
  rejected_refunded: 0,
})

// Fold a set of sealed receipts into one public-safe feed. The summary is always
// computed over the FULL set (before the filter) so a filter can never hide the
// headline counts; only `rows` is filtered.
export const buildArtanisLaborReceiptFeedProjection = (
  input: Readonly<{
    sealed: ReadonlyArray<ArtanisLaborSealedReceipt>
    filter?: ArtanisLaborReceiptFeedFilter
    generatedAt: string
  }>,
): ArtanisLaborReceiptFeedProjection => {
  const filter = input.filter ?? {}
  const allRows = input.sealed.map(sealedToRow)

  const byTerminalState = emptyByTerminalState()
  let placedRequestCount = 0
  for (const row of allRows) {
    byTerminalState[row.terminalState] += 1
    if (isPlaced(row.terminalState)) {
      placedRequestCount += 1
    }
  }

  const rows = allRows.filter(row => {
    if (filter.receiptRef !== undefined && row.receiptRef !== filter.receiptRef) {
      return false
    }
    if (
      filter.terminalState !== undefined &&
      row.terminalState !== filter.terminalState
    ) {
      return false
    }
    return true
  })

  return {
    kind: 'artanis_labor_unattended_request_receipt_feed',
    schemaVersion: 'openagents.artanis_labor_receipt_feed.v1',
    publicSafe: true,
    generatedAt: input.generatedAt,
    staleness: liveAtReadStaleness([
      'artanis_labor_unattended_request_receipt_stored',
    ]),
    filter,
    summary: {
      receiptCount: allRows.length,
      byTerminalState,
      placedRequestCount,
    },
    rows,
    authorityBoundary: FEED_AUTHORITY_BOUNDARY,
  }
}

const isTerminalState = (
  value: string | null,
): value is ArtanisLaborReceiptTerminalState =>
  value !== null &&
  (TERMINAL_STATE_ORDER as ReadonlyArray<string>).includes(value)

const parseFeedFilter = (url: URL): ArtanisLaborReceiptFeedFilter => {
  const receiptRef = url.searchParams.get('receiptRef')
  const terminalState = url.searchParams.get('terminalState')
  return {
    ...(receiptRef === null || receiptRef === '' ? {} : { receiptRef }),
    ...(isTerminalState(terminalState) ? { terminalState } : {}),
  }
}

// GET handler. With `?receiptRef=` it dereferences one receipt directly via the
// store (a point read, not a list scan); otherwise it lists the feed. Optional
// `?terminalState=` narrows the listed rows. Read-only, no-store.
export const handlePublicArtanisLaborReceiptsApi = (
  request: Request,
  input: Readonly<{
    store: ArtanisLaborUnattendedReceiptStore
    nowIso?: () => string
  }>,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.promise(async () => {
    const filter = parseFeedFilter(new URL(request.url))
    const generatedAt = input.nowIso?.() ?? currentIsoTimestamp()

    // A ref-targeted request is a point read so the feed never scans the whole
    // store just to serve one receipt. An unknown ref yields an empty feed
    // (rows = [], summary all zero) rather than a 404, so a client can poll a
    // ref it expects to appear without branching on status codes.
    if (filter.receiptRef !== undefined) {
      const found = await input.store.get(filter.receiptRef).catch(() => undefined)
      return noStoreJsonResponse(
        buildArtanisLaborReceiptFeedProjection({
          sealed: found === undefined ? [] : [found],
          filter,
          generatedAt,
        }),
      )
    }

    const sealed = await input.store.list().catch(() => [])
    return noStoreJsonResponse(
      buildArtanisLaborReceiptFeedProjection({ sealed, filter, generatedAt }),
    )
  })
}

// Build the green-readiness projection for artanis.labor_requester.v1 from the
// store. It reuses the same feed projection the public receipt feed serves (so
// the readiness counts and dereferenceable refs are exactly those the feed
// exposes), then folds it onto the two named green-flip blockers. Pure read,
// no-store, mints no authority.
export const buildArtanisLaborGreenReadinessProjection = async (
  input: Readonly<{
    store: ArtanisLaborUnattendedReceiptStore
    generatedAt: string
  }>,
): Promise<ArtanisLaborGreenReadinessProjection> => {
  const sealed = await input.store.list().catch(() => [])
  const feed = buildArtanisLaborReceiptFeedProjection({
    sealed,
    generatedAt: input.generatedAt,
  })
  return projectArtanisLaborGreenReadinessProjection(feed, input.generatedAt)
}

// GET handler for the green-readiness surface. Read-only, no-store.
export const handlePublicArtanisLaborGreenReadinessApi = (
  request: Request,
  input: Readonly<{
    store: ArtanisLaborUnattendedReceiptStore
    nowIso?: () => string
  }>,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.promise(async () => {
    const generatedAt = input.nowIso?.() ?? currentIsoTimestamp()
    return noStoreJsonResponse(
      await buildArtanisLaborGreenReadinessProjection({ store: input.store, generatedAt }),
    )
  })
}
