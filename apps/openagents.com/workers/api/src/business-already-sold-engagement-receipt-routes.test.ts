import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  firstAlreadySoldBusinessQuickWinReceipt,
  makeInMemoryBusinessAlreadySoldEngagementReceiptStore,
} from './business-already-sold-engagement-receipt'
import {
  BusinessAlreadySoldEngagementReceiptsEndpoint,
  makeBusinessAlreadySoldEngagementReceiptRoutes,
} from './business-already-sold-engagement-receipt-routes'

type ReceiptListJson = Readonly<{
  receipts: ReadonlyArray<Record<string, unknown>>
}>

type ReceiptReadJson = Readonly<{
  receipt: Record<string, unknown>
}>

describe('business already-sold engagement receipt routes', () => {
  it('lists paid business receipts with public-safe opaque rows', async () => {
    const routes = makeBusinessAlreadySoldEngagementReceiptRoutes({
      makeReceiptStore: () =>
        makeInMemoryBusinessAlreadySoldEngagementReceiptStore([
          firstAlreadySoldBusinessQuickWinReceipt,
        ]),
    })

    const responseEffect =
      routes.routeBusinessAlreadySoldEngagementReceiptRequest(
        new Request(
          `https://openagents.com${BusinessAlreadySoldEngagementReceiptsEndpoint}?view=paid-business-receipts`,
        ),
        {},
      )
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(200)
    const json = (await response.json()) as ReceiptListJson

    expect(json).toMatchObject({
      schema: 'openagents.business.already_sold_engagement.payment_receipts.v1',
      paidBusinessReceiptRecorded: true,
      totals: {
        receiptCount: 1,
        paidBusinessReceiptCount: 1,
      },
    })
    expect(json.receipts[0]).not.toHaveProperty('buyerPaidRef')
  })

  it('dereferences a public-safe receipt read', async () => {
    const routes = makeBusinessAlreadySoldEngagementReceiptRoutes({
      makeReceiptStore: () =>
        makeInMemoryBusinessAlreadySoldEngagementReceiptStore([
          firstAlreadySoldBusinessQuickWinReceipt,
        ]),
    })

    const responseEffect =
      routes.routeBusinessAlreadySoldEngagementReceiptRequest(
        new Request(
          `https://openagents.com${BusinessAlreadySoldEngagementReceiptsEndpoint}/${encodeURIComponent(firstAlreadySoldBusinessQuickWinReceipt.receiptRef)}`,
        ),
        {},
      )
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(200)
    const json = (await response.json()) as ReceiptReadJson

    expect(json.receipt).toMatchObject({
      receiptRef: firstAlreadySoldBusinessQuickWinReceipt.receiptRef,
      buyerRef: 'buyer.business.opaque.legal.001',
      verticalDescriptor: 'legal',
    })
    expect(json.receipt).not.toHaveProperty('buyerPaidRef')
  })

  it('returns 404 for an unknown receipt ref', async () => {
    const routes = makeBusinessAlreadySoldEngagementReceiptRoutes({
      makeReceiptStore: () =>
        makeInMemoryBusinessAlreadySoldEngagementReceiptStore([]),
    })

    const responseEffect =
      routes.routeBusinessAlreadySoldEngagementReceiptRequest(
        new Request(
          `https://openagents.com${BusinessAlreadySoldEngagementReceiptsEndpoint}/missing`,
        ),
        {},
      )
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(404)
  })
})
