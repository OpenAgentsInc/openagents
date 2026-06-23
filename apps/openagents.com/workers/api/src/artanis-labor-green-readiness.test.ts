import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_LABOR_LIVE_ENABLEMENT_BLOCKER,
  ARTANIS_LABOR_UNATTENDED_RECEIPTS_BLOCKER,
  ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET,
  artanisLaborGreenGateMet,
  projectArtanisLaborGreenReadinessProjection,
} from './artanis-labor-green-readiness'
import {
  buildArtanisLaborGreenReadinessProjection,
  buildArtanisLaborReceiptFeedProjection,
  handlePublicArtanisLaborGreenReadinessApi,
} from './artanis-labor-receipt-routes'
import {
  makeInMemoryArtanisLaborUnattendedReceiptStore,
  sealArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborSealedReceipt,
} from './artanis-labor-receipt-store'
import type {
  ArtanisLaborAcceptanceOutcome,
  ArtanisLaborRequesterOutcome,
} from './artanis-labor-requester'
import type { ArtanisLaborGreenReadinessProjection } from './artanis-labor-green-readiness'

const requestedOutcome = (n: number): ArtanisLaborRequesterOutcome => ({
  budgetMsat: 2_000_000,
  kind: 'requested',
  receipt: {
    jobEventId: String(n).padStart(64, '0'),
    topicId: `topic_${n}`,
    workRequestId: `work_request_${n}`,
  },
  reserveReceiptRef: `receipt.labor_escrow.reserve.artanis_${n}`,
})

const acceptedOutcome = (n: number): ArtanisLaborAcceptanceOutcome => ({
  kind: 'accepted',
  releaseReceiptRef: `receipt.labor_escrow.release.artanis_${n}`,
})

const skippedOutcome: ArtanisLaborRequesterOutcome = {
  kind: 'skipped',
  reason: 'config_disabled',
}

const base = {
  artanisActorRef: 'agent:artanis',
  nowIso: '2026-06-20T12:00:00.000Z',
}

// A placed receipt: an enabled tick reserved escrow on a real work request.
const sealPlaced = (n: number): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef: `tick.placed.${n}`,
    requestOutcome: requestedOutcome(n),
  })

const sealReleased = (n: number): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef: `tick.released.${n}`,
    acceptanceOutcome: acceptedOutcome(n),
    requestOutcome: requestedOutcome(n),
  })

// A skipped (config-disabled) receipt: gates ran but nothing was placed.
const sealSkipped = (n: number): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef: `tick.skipped.${n}`,
    requestOutcome: skippedOutcome,
  })

const FIXED_NOW = '2026-06-20T13:00:00.000Z'

const readinessOf = (
  sealed: ReadonlyArray<ArtanisLaborSealedReceipt>,
): ArtanisLaborGreenReadinessProjection =>
  projectArtanisLaborGreenReadinessProjection(
    buildArtanisLaborReceiptFeedProjection({ sealed, generatedAt: FIXED_NOW }),
    FIXED_NOW,
  )

describe('artanis labor requester green-readiness projection', () => {
  test('empty feed: both blockers open, gate unmet', () => {
    const readiness = readinessOf([])
    expect(readiness.kind).toBe('artanis_labor_requester_green_readiness')
    expect(readiness.publicSafe).toBe(true)
    expect(readiness.placedRequestCount).toBe(0)
    expect(readiness.liveEnablementProven).toBe(false)
    expect(readiness.unattendedRequestReceiptsProven).toBe(false)
    expect(readiness.greenGateMet).toBe(false)
    expect(readiness.placedRequests).toEqual([])
    expect(readiness.blockerRefs).toEqual([
      ARTANIS_LABOR_LIVE_ENABLEMENT_BLOCKER,
      ARTANIS_LABOR_UNATTENDED_RECEIPTS_BLOCKER,
    ])
    expect(readiness.unattendedRequestTarget).toBe(
      ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET,
    )
  })

  test('skipped-only feed never proves live enablement', () => {
    const readiness = readinessOf([sealSkipped(1), sealSkipped(2), sealSkipped(3)])
    // skipped ticks ran the gates but placed nothing -> still no enablement
    expect(readiness.byTerminalState.skipped_config_disabled).toBe(3)
    expect(readiness.placedRequestCount).toBe(0)
    expect(readiness.liveEnablementProven).toBe(false)
    expect(readiness.greenGateMet).toBe(false)
  })

  test('first placed receipt proves live enablement but not the streak', () => {
    const readiness = readinessOf([sealSkipped(1), sealPlaced(2)])
    expect(readiness.placedRequestCount).toBe(1)
    expect(readiness.liveEnablementProven).toBe(true)
    expect(readiness.unattendedRequestReceiptsProven).toBe(false)
    expect(readiness.greenGateMet).toBe(false)
    expect(readiness.placedRequests).toHaveLength(1)
    expect(readiness.placedRequests[0]?.terminalState).toBe(
      'requested_pending_delivery',
    )
    expect(readiness.placedRequests[0]?.workRequestId).toBe('work_request_2')
  })

  test('reaching the target on placed receipts meets the gate', () => {
    const sealed: ArtanisLaborSealedReceipt[] = []
    // Mix placed-pending and accepted-released; both count as placed.
    for (let i = 0; i < ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET; i += 1) {
      sealed.push(i % 2 === 0 ? sealPlaced(i + 100) : sealReleased(i + 100))
    }
    // A couple of skipped ticks must not change the placed count.
    sealed.push(sealSkipped(900), sealSkipped(901))

    const readiness = readinessOf(sealed)
    expect(readiness.placedRequestCount).toBe(
      ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET,
    )
    expect(readiness.liveEnablementProven).toBe(true)
    expect(readiness.unattendedRequestReceiptsProven).toBe(true)
    expect(readiness.greenGateMet).toBe(true)
    expect(artanisLaborGreenGateMet(readiness)).toBe(true)
    expect(readiness.placedRequests).toHaveLength(
      ARTANIS_LABOR_UNATTENDED_REQUEST_TARGET,
    )
  })

  test('every placed-request ref is a public-safe, dereferenceable token', () => {
    const readiness = readinessOf([sealPlaced(1), sealReleased(2)])
    const safeRefPattern = /^[A-Za-z0-9._:/-]{1,200}$/
    for (const placed of readiness.placedRequests) {
      expect(safeRefPattern.test(placed.receiptRef)).toBe(true)
    }
  })

  test('notes cite the owner sign-off as the separate, un-waived final step', () => {
    const readiness = readinessOf([sealPlaced(1)])
    const joined = readiness.notes.join(' ')
    expect(joined).toContain('owner-signed')
    expect(joined).toContain('/api/operator/product-promises/transitions')
    expect(readiness.authorityBoundary).toContain('owner sign-off')
  })
})

describe('artanis labor green-readiness handler', () => {
  const runHandler = async (
    sealed: ReadonlyArray<ArtanisLaborSealedReceipt>,
    method = 'GET',
  ): Promise<{ status: number; body: ArtanisLaborGreenReadinessProjection }> => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    for (const s of sealed) await store.put(s)
    const response = await Effect.runPromise(
      handlePublicArtanisLaborGreenReadinessApi(
        new Request('https://x/api/public/artanis/labor-green-readiness', {
          method,
        }),
        { store, nowIso: () => FIXED_NOW },
      ),
    )
    return {
      status: response.status,
      body: (await response.json()) as ArtanisLaborGreenReadinessProjection,
    }
  }

  test('non-GET is rejected', async () => {
    const { status } = await runHandler([], 'POST')
    expect(status).toBe(405)
  })

  test('GET projects readiness from the store and matches the pure projection', async () => {
    const sealed = [sealPlaced(1), sealReleased(2), sealSkipped(3)]
    const { status, body } = await runHandler(sealed)
    expect(status).toBe(200)
    expect(body.generatedAt).toBe(FIXED_NOW)
    expect(body.placedRequestCount).toBe(2)
    expect(body.liveEnablementProven).toBe(true)
    expect(body.greenGateMet).toBe(false)

    // The handler path and the direct builder must agree.
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    for (const s of sealed) await store.put(s)
    const built = await buildArtanisLaborGreenReadinessProjection({
      store,
      generatedAt: FIXED_NOW,
    })
    expect(body.placedRequestCount).toBe(built.placedRequestCount)
    expect(body.greenGateMet).toBe(built.greenGateMet)
  })
})
