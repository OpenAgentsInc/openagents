import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildArtanisLaborReceiptFeedProjection,
  handlePublicArtanisLaborReceiptsApi,
  type ArtanisLaborReceiptFeedProjection,
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

const requestedOutcome: ArtanisLaborRequesterOutcome = {
  budgetMsat: 2_000_000,
  kind: 'requested',
  receipt: {
    jobEventId: 'a'.repeat(64),
    topicId: 'topic_1',
    workRequestId: 'work_request_1',
  },
  reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1',
}

const acceptedOutcome: ArtanisLaborAcceptanceOutcome = {
  kind: 'accepted',
  releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
}

const skippedOutcome: ArtanisLaborRequesterOutcome = {
  kind: 'skipped',
  reason: 'config_disabled',
}

const base = {
  artanisActorRef: 'agent:artanis',
  nowIso: '2026-06-20T12:00:00.000Z',
}

const sealPending = (tickRef: string): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef,
    requestOutcome: requestedOutcome,
  })

const sealReleased = (tickRef: string): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef,
    acceptanceOutcome: acceptedOutcome,
    requestOutcome: requestedOutcome,
  })

const sealSkipped = (tickRef: string): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef,
    requestOutcome: skippedOutcome,
  })

const FIXED_NOW = '2026-06-20T13:00:00.000Z'

const runHandler = async (
  url: string,
  sealed: ReadonlyArray<ArtanisLaborSealedReceipt>,
  method = 'GET',
): Promise<{ status: number; body: ArtanisLaborReceiptFeedProjection }> => {
  const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
  for (const s of sealed) {
    await store.put(s)
  }
  const response = await Effect.runPromise(
    handlePublicArtanisLaborReceiptsApi(new Request(url, { method }), {
      store,
      nowIso: () => FIXED_NOW,
    }),
  )
  return {
    status: response.status,
    body: (await response.json()) as ArtanisLaborReceiptFeedProjection,
  }
}

describe('artanis labor receipt feed projection', () => {
  test('empty set yields a stable zeroed summary', () => {
    const projection = buildArtanisLaborReceiptFeedProjection({
      sealed: [],
      generatedAt: FIXED_NOW,
    })
    expect(projection.publicSafe).toBe(true)
    expect(projection.summary.receiptCount).toBe(0)
    expect(projection.summary.placedRequestCount).toBe(0)
    expect(projection.summary.byTerminalState).toEqual({
      skipped_config_disabled: 0,
      refused: 0,
      requested_pending_delivery: 0,
      accepted_released: 0,
      rejected_refunded: 0,
    })
    expect(projection.rows).toEqual([])
  })

  test('folds a mixed set into per-terminal-state counts', () => {
    const projection = buildArtanisLaborReceiptFeedProjection({
      sealed: [sealPending('tick.a'), sealReleased('tick.b'), sealSkipped('tick.c')],
      generatedAt: FIXED_NOW,
    })
    expect(projection.summary.receiptCount).toBe(3)
    expect(projection.summary.placedRequestCount).toBe(2)
    expect(projection.summary.byTerminalState.requested_pending_delivery).toBe(1)
    expect(projection.summary.byTerminalState.accepted_released).toBe(1)
    expect(projection.summary.byTerminalState.skipped_config_disabled).toBe(1)
    expect(projection.rows).toHaveLength(3)
  })

  test('row carries the receipt projection but no private material', () => {
    const sealed = sealPending('tick.a')
    const projection = buildArtanisLaborReceiptFeedProjection({
      sealed: [sealed],
      generatedAt: FIXED_NOW,
    })
    const [row] = projection.rows
    expect(row?.receiptRef).toBe(sealed.receiptRef)
    expect(row?.terminalState).toBe('requested_pending_delivery')
    expect(row?.workRequestId).toBe('work_request_1')
    expect(row?.budgetMsat).toBe(2_000_000)
  })

  test('terminalState filter narrows rows but not the summary', () => {
    const projection = buildArtanisLaborReceiptFeedProjection({
      sealed: [sealPending('tick.a'), sealReleased('tick.b'), sealSkipped('tick.c')],
      filter: { terminalState: 'accepted_released' },
      generatedAt: FIXED_NOW,
    })
    expect(projection.summary.receiptCount).toBe(3)
    expect(projection.rows).toHaveLength(1)
    expect(projection.rows[0]?.terminalState).toBe('accepted_released')
  })

  test('a tampered sealed receipt is refused by the projection', () => {
    const sealed = sealPending('tick.a')
    expect(() =>
      buildArtanisLaborReceiptFeedProjection({
        sealed: [{ ...sealed, receiptRef: `${sealed.receiptRef}-evil` }],
        generatedAt: FIXED_NOW,
      }),
    ).toThrow()
  })
})

describe('artanis labor receipt feed handler', () => {
  test('non-GET is rejected', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const response = await Effect.runPromise(
      handlePublicArtanisLaborReceiptsApi(
        new Request('https://x/api/public/artanis/labor-receipts', {
          method: 'POST',
        }),
        { store },
      ),
    )
    expect(response.status).toBe(405)
  })

  test('GET lists all stored receipts deterministically', async () => {
    const { status, body } = await runHandler(
      'https://x/api/public/artanis/labor-receipts',
      [sealPending('tick.a'), sealReleased('tick.b')],
    )
    expect(status).toBe(200)
    expect(body.summary.receiptCount).toBe(2)
    expect(body.generatedAt).toBe(FIXED_NOW)
    expect(body.rows.map(r => r.tickRef)).toEqual(['tick.a', 'tick.b'])
  })

  test('GET ?receiptRef= dereferences a single receipt', async () => {
    const pending = sealPending('tick.a')
    const released = sealReleased('tick.b')
    const { body } = await runHandler(
      `https://x/api/public/artanis/labor-receipts?receiptRef=${pending.receiptRef}`,
      [pending, released],
    )
    expect(body.rows).toHaveLength(1)
    expect(body.rows[0]?.receiptRef).toBe(pending.receiptRef)
    expect(body.summary.receiptCount).toBe(1)
  })

  test('GET ?receiptRef= for an unknown ref returns an empty feed, not an error', async () => {
    const { status, body } = await runHandler(
      'https://x/api/public/artanis/labor-receipts?receiptRef=receipt.artanis_labor.unattended_request.0000000000000000',
      [sealPending('tick.a')],
    )
    expect(status).toBe(200)
    expect(body.rows).toEqual([])
    expect(body.summary.receiptCount).toBe(0)
  })
})
