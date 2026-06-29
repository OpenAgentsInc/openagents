import { describe, expect, test } from 'vitest'

import { ArtanisLaborReceiptError } from './artanis-labor-request-receipt'
import {
  makeInMemoryArtanisLaborUnattendedReceiptStore,
  sealArtanisLaborUnattendedRequestReceipt,
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

const base = {
  artanisActorRef: 'agent:artanis',
  nowIso: '2026-06-20T12:00:00.000Z',
  tickRef: 'tick.public.artanis.2026-06-20T12:00',
}

describe('artanis unattended labor receipt seal + store', () => {
  test('seal folds build/serialize/deriveRef into a consistent artifact', () => {
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    expect(sealed.receipt.terminalState).toBe('requested_pending_delivery')
    expect(sealed.receiptRef).toMatch(
      /^receipt\.artanis_labor\.unattended_request\.[0-9a-f]{16}$/,
    )
    expect(JSON.parse(sealed.serialized).schema).toBe(
      'artanis.labor.unattended_request_receipt.v1',
    )
  })

  test('seal is deterministic: same lifecycle -> identical ref and bytes', () => {
    const a = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      acceptanceOutcome: acceptedOutcome,
      requestOutcome: requestedOutcome,
    })
    const b = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      acceptanceOutcome: acceptedOutcome,
      requestOutcome: requestedOutcome,
    })
    expect(a.receiptRef).toBe(b.receiptRef)
    expect(a.serialized).toBe(b.serialized)
  })

  test('put stores then get returns the same sealed receipt by ref', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    const put = await store.put(sealed)
    expect(put.kind).toBe('stored')

    const fetched = await store.get(sealed.receiptRef)
    expect(fetched?.receiptRef).toBe(sealed.receiptRef)
    expect(fetched?.serialized).toBe(sealed.serialized)
    expect(fetched?.receipt).toEqual(sealed.receipt)
  })

  test('put is idempotent on the content-addressed ref', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    expect((await store.put(sealed)).kind).toBe('stored')
    expect((await store.put(sealed)).kind).toBe('already_stored')
    expect(store.rows.size).toBe(1)
  })

  test('distinct lifecycles store under distinct refs', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const pending = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    const released = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      acceptanceOutcome: acceptedOutcome,
      requestOutcome: requestedOutcome,
    })
    await store.put(pending)
    await store.put(released)
    expect(pending.receiptRef).not.toBe(released.receiptRef)
    expect((await store.list()).map(s => s.receiptRef)).toEqual([
      pending.receiptRef,
      released.receiptRef,
    ])
  })

  test('get returns undefined for an unknown ref', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    expect(await store.get('receipt.artanis_labor.unattended_request.deadbeefdeadbeef')).toBeUndefined()
  })

  test('put refuses a sealed receipt whose ref does not address its bytes', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    await expect(
      store.put({
        ...sealed,
        receiptRef: 'receipt.artanis_labor.unattended_request.0000000000000000',
      }),
    ).rejects.toBeInstanceOf(ArtanisLaborReceiptError)
  })

  test('put refuses a sealed receipt whose object disagrees with its bytes', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    const tampered = {
      ...sealed,
      receipt: { ...sealed.receipt, tickRef: 'tick.public.artanis.evil' },
    }
    await expect(store.put(tampered)).rejects.toBeInstanceOf(
      ArtanisLaborReceiptError,
    )
  })
})
