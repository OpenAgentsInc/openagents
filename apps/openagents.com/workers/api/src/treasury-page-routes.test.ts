import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type TreasuryTransactionRecord,
  type TreasuryTransactionStore,
  makeTreasuryPageRoutes,
} from './treasury-page-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

const jsonResponse = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    status,
  })

const makeMemoryStore = (): TreasuryTransactionStore & {
  rows: Map<string, TreasuryTransactionRecord>
} => {
  const rows = new Map<string, TreasuryTransactionRecord>()

  return {
    confirmReceived: input => {
      const row = rows.get(input.id)

      if (
        row !== undefined &&
        row.direction === 'out' &&
        row.state === 'settled'
      ) {
        rows.set(input.id, {
          ...row,
          recipientConfirmationRef: input.confirmationRef,
          recipientConfirmationState: 'confirmed_received',
          recipientConfirmedAt: input.recipientConfirmedAt,
        })
      }

      return Promise.resolve()
    },
    expire: input => {
      const row = rows.get(input.id)

      if (row !== undefined && row.state === 'pending') {
        rows.set(input.id, { ...row, state: 'expired' })
      }

      return Promise.resolve()
    },
    fail: input => {
      const row = rows.get(input.id)

      if (row !== undefined && row.state === 'pending') {
        rows.set(input.id, { ...row, state: 'failed', settledAt: null })
      }

      return Promise.resolve()
    },
    insert: record => {
      rows.set(record.id, record)

      return Promise.resolve()
    },
    listRecent: limit =>
      Promise.resolve(
        [...rows.values()]
          .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
          .slice(0, limit),
      ),
    listPendingOutbound: limit =>
      Promise.resolve(
        [...rows.values()]
          .filter(
            row =>
              row.direction === 'out' &&
              row.state === 'pending' &&
              row.paymentRef !== null,
          )
          .sort((a, b) => a.createdAt.localeCompare(b.createdAt))
          .slice(0, limit),
      ),
    listByRecipient: input =>
      Promise.resolve(
        [...rows.values()]
          .filter(row => row.recipientRef === input.recipientRef)
          .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
          .slice(0, input.limit),
      ),
    read: id => Promise.resolve(rows.get(id)),
    rows,
    settle: input => {
      const row = rows.get(input.id)

      if (row !== undefined && row.state === 'pending') {
        rows.set(input.id, {
          ...row,
          amountSat: input.amountSat,
          settledAt: input.settledAt,
          state: 'settled',
        })
      }

      return Promise.resolve()
    },
  }
}

const unattributed = () => ({
  owedRef: null,
  owedSat: null,
  recipientConfirmationRef: null,
  recipientConfirmationState: 'unconfirmed' as const,
  recipientConfirmedAt: null,
  recipientRef: null,
  redactedDestinationRef: null,
})

const settledOut = (
  id: string,
  amountSat: number,
): TreasuryTransactionRecord => ({
  amountSat,
  bolt11: null,
  createdAt: '2026-06-10T18:00:00.000Z',
  direction: 'out',
  expiresAt: null,
  failureReasonRef: null,
  id,
  paymentRef: 'internal_payment_ref_should_not_leak',
  ...unattributed(),
  settledAt: '2026-06-10T18:00:05.000Z',
  state: 'settled',
})

const makeRoutes = (input: {
  containerResponses?: Record<string, Response>
  store?: TreasuryTransactionStore
}) =>
  makeTreasuryPageRoutes({
    fetchTreasury: path =>
      Promise.resolve(
        input.containerResponses?.[path] ??
          jsonResponse(404, { error: 'not_found' }),
      ),
    makeUuid: () => 'uuid-1',
    nowIso: () => '2026-06-10T18:30:00.000Z',
    store: input.store,
  })

describe('public treasury api', () => {
  test('serves balance and public-safe transactions', async () => {
    const store = makeMemoryStore()
    await store.insert(settledOut('treasury_payout_1', 48))
    const routes = makeRoutes({
      containerResponses: {
        '/balance': jsonResponse(200, { balanceSat: 442, maxSendableSat: 432 }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request('https://openagents.com/api/public/treasury'),
      )!,
    )
    const body = (await response.json()) as {
      balance: { balanceSat: number }
      transactions: Array<Record<string, unknown>>
    }

    expect(response.status).toBe(200)
    expect(body.balance.balanceSat).toBe(442)
    expect(body.transactions).toHaveLength(1)
    expect(body.transactions[0]).toMatchObject({
      amountSat: 48,
      direction: 'out',
      state: 'settled',
    })
    expect(JSON.stringify(body)).not.toContain('should_not_leak')
    expect(JSON.stringify(body)).not.toContain('paymentRef')
    expect(JSON.stringify(body)).not.toContain('failureReasonRef')
    expect(JSON.stringify(body)).not.toContain('bolt11')
  })

  test('aggregates MDK and Spark rails into one public balance', async () => {
    const containerResponses = {
      '/balance': jsonResponse(200, {
        balanceSat: 442,
        maxSendableSat: 432,
      }),
      '/spark/balance': jsonResponse(200, {
        balanceSat: 50000,
        maxSendableSat: 50000,
        rail: 'spark',
      }),
    }
    const response = await run(
      makeRoutes({ containerResponses }).routeTreasuryPageRequest(
        new Request('https://openagents.com/api/public/treasury'),
      )!,
    )
    const body = (await response.json()) as {
      balance: {
        balanceSat: number
        maxSendableSat: number
        rails: ReadonlyArray<{
          balanceSat: number | null
          rail: string
          state: string
        }>
      }
    }

    expect(response.status).toBe(200)
    expect(body.balance.balanceSat).toBe(50442)
    expect(body.balance.maxSendableSat).toBe(50432)
    expect(body.balance.rails).toEqual([
      expect.objectContaining({
        balanceSat: 442,
        rail: 'mdk',
        state: 'ok',
      }),
      expect.objectContaining({
        balanceSat: 50000,
        rail: 'spark',
        state: 'ok',
      }),
    ])

    const page = await run(
      makeRoutes({
        containerResponses: {
          '/balance': jsonResponse(200, {
            balanceSat: 442,
            maxSendableSat: 432,
          }),
          '/spark/balance': jsonResponse(200, {
            balanceSat: 50000,
            maxSendableSat: 50000,
          }),
        },
      }).routeTreasuryPageRequest(
        new Request('https://openagents.com/treasury'),
      )!,
    )
    const html = await page.text()

    expect(html).toContain('50442 sats')
    expect(html).toContain('MDK: 442 sats')
    expect(html).toContain('Spark: 50000 sats')
  })

  test('keeps failed payout reason refs out of the public projection', async () => {
    const store = makeMemoryStore()
    await store.insert({
      amountSat: 50000,
      bolt11: null,
      createdAt: '2026-06-10T18:05:00.000Z',
      direction: 'out',
      expiresAt: null,
      failureReasonRef:
        'reason.public.treasury_payout.lightning_address_resolution_failed.amount_out_of_range_1000_10000000_msat',
      id: 'treasury_payout_failed',
      paymentRef: null,
      ...unattributed(),
      settledAt: null,
      state: 'failed',
    })
    const routes = makeRoutes({
      containerResponses: {
        '/balance': jsonResponse(200, { balanceSat: 442, maxSendableSat: 432 }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request('https://openagents.com/api/public/treasury'),
      )!,
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as {
      transactions: Array<Record<string, unknown>>
    }

    expect(response.status).toBe(200)
    expect(body.transactions).toEqual([
      expect.objectContaining({
        amountSat: 50000,
        direction: 'out',
        state: 'failed',
      }),
    ])
    expect(bodyText).not.toContain('failureReasonRef')
    expect(bodyText).not.toContain('lightning_address_resolution_failed')
  })

  test('hides unpaid pending donation invoices from the public list', async () => {
    const store = makeMemoryStore()
    await store.insert({
      amountSat: 0,
      bolt11: 'lnbc1unpaid',
      createdAt: '2026-06-10T18:10:00.000Z',
      direction: 'in',
      expiresAt: '2026-06-10T19:10:00.000Z',
      failureReasonRef: null,
      id: 'treasury_donation_unpaid',
      paymentRef: 'cd'.repeat(32),
      ...unattributed(),
      settledAt: null,
      state: 'pending',
    })
    await store.insert(settledOut('treasury_payout_1', 48))
    const routes = makeRoutes({
      containerResponses: {
        '/balance': jsonResponse(200, { balanceSat: 442, maxSendableSat: 432 }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request('https://openagents.com/api/public/treasury'),
      )!,
    )
    const body = (await response.json()) as {
      transactions: Array<{ direction: string }>
    }

    expect(body.transactions).toHaveLength(1)
    expect(body.transactions[0]?.direction).toBe('out')

    const page = await run(
      routes.routeTreasuryPageRequest(
        new Request('https://openagents.com/treasury'),
      )!,
    )
    const html = await page.text()
    expect(html).not.toContain('pending')
  })
})

describe('treasury page', () => {
  test('renders balance, transactions, and the donate button', async () => {
    const store = makeMemoryStore()
    await store.insert(settledOut('treasury_payout_1', 48))
    const routes = makeRoutes({
      containerResponses: {
        '/balance': jsonResponse(200, { balanceSat: 442, maxSendableSat: 432 }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request('https://openagents.com/treasury'),
      )!,
    )
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(html).toContain('442 sats')
    expect(html).toContain('/treasury/donate')
    expect(html).toContain('48 sats')
    expect(html).not.toContain('should_not_leak')
  })

  test('does not match unrelated paths', () => {
    const routes = makeRoutes({})

    expect(
      routes.routeTreasuryPageRequest(
        new Request('https://openagents.com/treasuryx'),
      ),
    ).toBeUndefined()
  })
})

describe('donation flow', () => {
  test('creates a pending donation and redirects to its page', async () => {
    const store = makeMemoryStore()
    const routes = makeRoutes({
      containerResponses: {
        '/donation-invoice': jsonResponse(200, {
          bolt11: 'lnbc1donationexample',
          expiresAt: 1781200000,
          paymentHash: 'ab'.repeat(32),
        }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request('https://openagents.com/treasury/donate'),
      )!,
    )

    expect(response.status).toBe(303)
    expect(response.headers.get('location')).toBe(
      '/treasury/donations/treasury_donation_uuid-1',
    )
    const row = store.rows.get('treasury_donation_uuid-1')
    expect(row).toMatchObject({
      direction: 'in',
      state: 'pending',
    })
  })

  test('renders the pending donation page with QR and invoice', async () => {
    const store = makeMemoryStore()
    await store.insert({
      amountSat: 0,
      bolt11: 'lnbc1donationexample',
      createdAt: '2026-06-10T18:00:00.000Z',
      direction: 'in',
      expiresAt: '2026-06-10T19:30:00.000Z',
      failureReasonRef: null,
      id: 'treasury_donation_uuid-1',
      paymentRef: 'ab'.repeat(32),
      ...unattributed(),
      settledAt: null,
      state: 'pending',
    })
    const routes = makeRoutes({
      containerResponses: {
        [`/received/${'ab'.repeat(32)}`]: jsonResponse(200, {
          amountSat: null,
          received: false,
        }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request(
          'https://openagents.com/treasury/donations/treasury_donation_uuid-1',
        ),
      )!,
    )
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(html).toContain('lnbc1donationexample')
    expect(html).toContain('class="qr"')
    expect(html).toContain('waiting for payment')
  })

  test('settles the donation when the container reports receipt', async () => {
    const store = makeMemoryStore()
    await store.insert({
      amountSat: 0,
      bolt11: 'lnbc1donationexample',
      createdAt: '2026-06-10T18:00:00.000Z',
      direction: 'in',
      expiresAt: '2026-06-10T19:30:00.000Z',
      failureReasonRef: null,
      id: 'treasury_donation_uuid-1',
      paymentRef: 'ab'.repeat(32),
      ...unattributed(),
      settledAt: null,
      state: 'pending',
    })
    const routes = makeRoutes({
      containerResponses: {
        [`/received/${'ab'.repeat(32)}`]: jsonResponse(200, {
          amountSat: 210,
          received: true,
        }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request(
          'https://openagents.com/treasury/donations/treasury_donation_uuid-1',
        ),
      )!,
    )
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(html).toContain('Donation received')
    expect(html).toContain('210 sats')
    expect(store.rows.get('treasury_donation_uuid-1')).toMatchObject({
      amountSat: 210,
      state: 'settled',
    })
  })

  test('expires a stale pending donation', async () => {
    const store = makeMemoryStore()
    await store.insert({
      amountSat: 0,
      bolt11: 'lnbc1donationexample',
      createdAt: '2026-06-10T17:00:00.000Z',
      direction: 'in',
      expiresAt: '2026-06-10T18:00:00.000Z',
      failureReasonRef: null,
      id: 'treasury_donation_uuid-1',
      paymentRef: 'ab'.repeat(32),
      ...unattributed(),
      settledAt: null,
      state: 'pending',
    })
    const routes = makeRoutes({
      containerResponses: {
        [`/received/${'ab'.repeat(32)}`]: jsonResponse(200, {
          amountSat: null,
          received: false,
        }),
      },
      store,
    })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request(
          'https://openagents.com/treasury/donations/treasury_donation_uuid-1',
        ),
      )!,
    )

    expect(response.status).toBe(410)
    expect(store.rows.get('treasury_donation_uuid-1')?.state).toBe('expired')
  })

  test('404s for unknown donations', async () => {
    const routes = makeRoutes({ store: makeMemoryStore() })

    const response = await run(
      routes.routeTreasuryPageRequest(
        new Request(
          'https://openagents.com/treasury/donations/treasury_donation_nope',
        ),
      )!,
    )

    expect(response.status).toBe(404)
  })
})
