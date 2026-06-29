import { describe, expect, test } from 'vitest'

import {
  makeD1ArtanisLaborUnattendedReceiptStore,
  sealArtanisLaborUnattendedRequestReceipt,
} from './artanis-labor-receipt-store'
import { ArtanisLaborReceiptError } from './artanis-labor-request-receipt'
import type {
  ArtanisLaborAcceptanceOutcome,
  ArtanisLaborRequesterOutcome,
} from './artanis-labor-requester'

// Minimal in-memory fake of the subset of D1 the store uses (prepare/bind/run/
// first/all). It models a single keyed table with INSERT OR IGNORE semantics so
// the store's idempotency and tamper-evident reads can be exercised without a
// real D1 binding.

type Row = { receipt_ref: string; serialized_json: string; created_at: string }

const makeFakeD1 = (): D1Database & { rows: Row[] } => {
  const rows: Row[] = []

  const statement = (query: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt: D1PreparedStatement = {
      bind: (...values: ReadonlyArray<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T,>() => {
        // SELECT ... WHERE receipt_ref = ?
        const ref = String(bound[0])
        const found = rows.find(r => r.receipt_ref === ref)
        return (found === undefined ? null : (found as unknown)) as T | null
      },
      all: async <T,>() => ({
        meta: {} as D1Meta & Record<string, unknown>,
        results: [...rows].sort((a, b) =>
          a.created_at < b.created_at ? -1 : a.created_at > b.created_at ? 1 : 0,
        ) as unknown as T[],
        success: true as const,
      }),
      run: async <T,>() => {
        // INSERT OR IGNORE INTO ... (receipt_ref, serialized_json, terminal_state, created_at)
        const [receipt_ref, serialized_json, , created_at] = bound as [
          string,
          string,
          string,
          string,
        ]
        const existed = rows.some(r => r.receipt_ref === receipt_ref)
        if (!existed) {
          rows.push({ created_at, receipt_ref, serialized_json })
        }
        return {
          meta: { changes: existed ? 0 : 1 } as D1Meta &
            Record<string, unknown>,
          results: [] as unknown as T[],
          success: true as const,
        }
      },
      raw: async () => {
        throw new Error(`Unexpected D1 raw: ${query}`)
      },
    }
    return stmt
  }

  return {
    batch: () => Promise.reject(new Error('batch unused')),
    dump: () => Promise.reject(new Error('dump unused')),
    exec: () => Promise.reject(new Error('exec unused')),
    prepare: (query: string) => statement(query),
    rows,
    withSession: () => {
      throw new Error('session unused')
    },
  } as unknown as D1Database & { rows: Row[] }
}

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

let clock = 0
const nowIso = () =>
  `2026-06-20T13:00:0${clock++}.000Z`

describe('D1 artanis unattended labor receipt store', () => {
  test('put stores then get returns the same sealed receipt by ref', async () => {
    const store = makeD1ArtanisLaborUnattendedReceiptStore(makeFakeD1(), nowIso)
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    expect((await store.put(sealed)).kind).toBe('stored')
    const fetched = await store.get(sealed.receiptRef)
    expect(fetched?.receiptRef).toBe(sealed.receiptRef)
    expect(fetched?.serialized).toBe(sealed.serialized)
    expect(fetched?.receipt).toEqual(sealed.receipt)
  })

  test('put is idempotent on the content-addressed ref', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisLaborUnattendedReceiptStore(db, nowIso)
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    expect((await store.put(sealed)).kind).toBe('stored')
    expect((await store.put(sealed)).kind).toBe('already_stored')
    expect(db.rows.length).toBe(1)
  })

  test('list returns distinct lifecycles in created-at order', async () => {
    const store = makeD1ArtanisLaborUnattendedReceiptStore(makeFakeD1(), nowIso)
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
    const listed = await store.list()
    expect(listed.map(s => s.receiptRef)).toEqual([
      pending.receiptRef,
      released.receiptRef,
    ])
  })

  test('get returns undefined for an unknown ref', async () => {
    const store = makeD1ArtanisLaborUnattendedReceiptStore(makeFakeD1(), nowIso)
    expect(
      await store.get(
        'receipt.artanis_labor.unattended_request.deadbeefdeadbeef',
      ),
    ).toBeUndefined()
  })

  test('a tampered persisted row fails the tamper-evident read', async () => {
    const db = makeFakeD1()
    const store = makeD1ArtanisLaborUnattendedReceiptStore(db, nowIso)
    const sealed = sealArtanisLaborUnattendedRequestReceipt({
      ...base,
      requestOutcome: requestedOutcome,
    })
    await store.put(sealed)
    // Corrupt the stored bytes so they no longer address the keyed ref.
    const row = db.rows[0]
    if (row !== undefined) {
      row.serialized_json = row.serialized_json.replace(
        'tick.public.artanis.2026-06-20T12:00',
        'tick.public.artanis.evil',
      )
    }
    await expect(store.get(sealed.receiptRef)).rejects.toBeInstanceOf(
      ArtanisLaborReceiptError,
    )
  })

  test('put refuses an internally inconsistent sealed receipt', async () => {
    const store = makeD1ArtanisLaborUnattendedReceiptStore(makeFakeD1(), nowIso)
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
})
