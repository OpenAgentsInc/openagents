import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  Nip90MarketReceiptStore,
  Nip90MarketSettlementReceiptRecord,
} from './nip90-market-receipts'
import { makePublicNip90MarketReceiptRoutes } from './public-nip90-market-receipt-routes'

const marketReceipt = (
  input: Partial<Nip90MarketSettlementReceiptRecord> &
    Pick<Nip90MarketSettlementReceiptRecord, 'receiptRef'>,
): Nip90MarketSettlementReceiptRecord => {
  const { receiptRef, ...overrides } = input

  return {
    amountMsats: 2_000,
    createdAt: '2026-06-10T07:00:00.000Z',
    jobRef: `buy_mode_job_${receiptRef}`,
    receiptRef,
    requestEventRef: `event.request.${receiptRef}`,
    resultEventRef: `event.result.${receiptRef}`,
    settledAt: '2026-06-10T07:10:00.000Z',
    state: 'settled',
    streamKind: 'compute',
    ...overrides,
  }
}

const storeFor = (
  records: ReadonlyArray<Nip90MarketSettlementReceiptRecord>,
): Nip90MarketReceiptStore => ({
  listSettledMarketReceipts: () => Promise.resolve(records),
  readSettledMarketReceiptByRef: receiptRef =>
    Promise.resolve(
      records.find(record => record.receiptRef === receiptRef) ?? null,
    ),
})

const routesFor = (store: Nip90MarketReceiptStore) =>
  makePublicNip90MarketReceiptRoutes<{ store: Nip90MarketReceiptStore }>({
    makeStore: env => env.store,
  })

const route = async (
  store: Nip90MarketReceiptStore,
  receiptRef: string,
  init?: RequestInit,
) => {
  const response = routesFor(store).routePublicNip90MarketReceiptRequest(
    new Request(
      `https://openagents.com/api/public/nip90-market/receipts/${encodeURIComponent(
        receiptRef,
      )}`,
      init,
    ),
    { store },
  )

  if (response === undefined) {
    throw new Error('receipt route did not match')
  }

  return Effect.runPromise(response)
}

describe('public NIP-90 market receipt routes', () => {
  test('serves settled buy-mode receipts without private payment material', async () => {
    const response = await route(
      storeFor([
        marketReceipt({
          receiptRef: 'receipt.nip90_market.compute.settlement.one',
        }),
      ]),
      'receipt.nip90_market.compute.settlement.one',
    )
    const body = await response.json() as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.receipt).toMatchObject({
      amountSats: 2,
      receiptRef: 'receipt.nip90_market.compute.settlement.one',
      schemaVersion: 'openagents.nip90_market.receipt.v1',
      state: 'settled',
      streamKind: 'compute',
    })
    expect(JSON.stringify(body)).not.toMatch(
      /lnbc|bolt11|invoice|preimage|payment_hash|wallet|mnemonic|private_key|counterpartyWallet/,
    )
  })

  test('does not expose pending or unsafe receipt projections', async () => {
    const pending = await route(
      storeFor([
        marketReceipt({
          receiptRef: 'receipt.nip90_market.compute.pending',
          state: 'settlement_blocked',
        }),
      ]),
      'receipt.nip90_market.compute.pending',
    )
    const unsafe = await route(
      storeFor([
        marketReceipt({
          receiptRef: 'receipt.nip90_market.compute.unsafe',
          resultEventRef: 'lnbc10n1rawinvoice',
        }),
      ]),
      'receipt.nip90_market.compute.unsafe',
    )

    expect(pending.status).toBe(404)
    expect(unsafe.status).toBe(404)
  })

  test('rejects mutations', async () => {
    const response = await route(
      storeFor([]),
      'receipt.nip90_market.compute.settlement.one',
      { method: 'POST' },
    )

    expect(response.status).toBe(405)
  })
})
