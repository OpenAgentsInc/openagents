import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ContainerPathFetch } from './http/container-fetch'
import type {
  TreasuryTransactionRecord,
  TreasuryTransactionStore,
} from './treasury-page-routes'
import {
  executeTreasuryPayout,
  handleOperatorSparkTreasuryFundingDestinationApi,
  handleOperatorSparkTreasuryFundingInvoiceApi,
  handleOperatorTreasuryFundingDestinationApi,
  handleOperatorTreasuryPayoutApi,
  handleOperatorTreasuryRecipientConfirmationApi,
  handleOperatorTreasuryRecipientReportApi,
  handleOperatorTreasuryStatusApi,
  handleOperatorTreasuryTransactionReconcileApi,
  handlePublicTreasuryLaunchStatusApi,
  reconcilePendingTreasuryTransactions,
  treasuryPayoutPlan,
} from './treasury-routes'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

const jsonResponse = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    status,
  })

const healthzPayload = (configured: boolean) => ({
  accessTokenConfigured: configured,
  mnemonicConfigured: configured,
  ok: true,
  serviceTokenConfigured: configured,
  service: 'openagents-mdk-treasury',
})

const makeMemoryTransactionStore = (
  seed: ReadonlyArray<TreasuryTransactionRecord> = [],
): TreasuryTransactionStore & {
  rows: Map<string, TreasuryTransactionRecord>
} => {
  const rows = new Map(seed.map(record => [record.id, record]))

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
        rows.set(input.id, { ...row, settledAt: null, state: 'failed' })
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

const pendingOutTransaction = (
  id: string,
  paymentRef: string | null = 'payment_secret_1',
): TreasuryTransactionRecord => ({
  amountSat: 50005,
  bolt11: null,
  createdAt: '2026-06-17T18:00:00.000Z',
  direction: 'out',
  expiresAt: null,
  failureReasonRef: null,
  id,
  owedRef: null,
  owedSat: null,
  paymentRef,
  recipientConfirmationRef: null,
  recipientConfirmationState: 'unconfirmed',
  recipientConfirmedAt: null,
  recipientRef: null,
  redactedDestinationRef: null,
  settledAt: null,
  state: 'pending',
})

const settledRecipientOutTransaction = (
  input: Readonly<{
    amountSat: number
    confirmed?: boolean
    id: string
    owedRef?: string | null
    owedSat?: number | null
    recipientRef: string
  }>,
): TreasuryTransactionRecord => ({
  ...pendingOutTransaction(input.id, `payment_secret_${input.id}`),
  amountSat: input.amountSat,
  owedRef: input.owedRef ?? null,
  owedSat: input.owedSat ?? null,
  recipientConfirmationRef:
    input.confirmed === true
      ? `recipient_confirmation.public.${input.id}`
      : null,
  recipientConfirmationState:
    input.confirmed === true ? 'confirmed_received' : 'unconfirmed',
  recipientConfirmedAt:
    input.confirmed === true ? '2026-06-17T18:10:00.000Z' : null,
  recipientRef: input.recipientRef,
  redactedDestinationRef: `destination.redacted.${input.id}`,
  settledAt: '2026-06-17T18:05:00.000Z',
  state: 'settled',
})

describe('public treasury launch status', () => {
  test('reports unprovisioned when no container binding exists', async () => {
    const response = await run(
      handlePublicTreasuryLaunchStatusApi(
        new Request('https://openagents.com/api/public/treasury/launch-status'),
        { requireAdminApiToken: () => Promise.resolve(false) },
      ),
    )
    const body = (await response.json()) as { state: string }

    expect(response.status).toBe(200)
    expect(body.state).toBe('unprovisioned')
  })

  test('projects honest unconfigured state from container healthz', async () => {
    const response = await run(
      handlePublicTreasuryLaunchStatusApi(
        new Request('https://openagents.com/api/public/treasury/launch-status'),
        {
          fetchTreasury: () =>
            Promise.resolve(jsonResponse(200, healthzPayload(false))),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )
    const body = (await response.json()) as {
      configured: { mnemonic: boolean }
      state: string
    }

    expect(body.state).toBe('unconfigured')
    expect(body.configured.mnemonic).toBe(false)
  })

  test('reports unavailable when the container cannot be reached', async () => {
    const response = await run(
      handlePublicTreasuryLaunchStatusApi(
        new Request('https://openagents.com/api/public/treasury/launch-status'),
        {
          fetchTreasury: () => Promise.reject(new Error('container down')),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )
    const body = (await response.json()) as { state: string }

    expect(body.state).toBe('unavailable')
  })

  test('never leaks wallet material in the public projection', async () => {
    const response = await run(
      handlePublicTreasuryLaunchStatusApi(
        new Request('https://openagents.com/api/public/treasury/launch-status'),
        {
          fetchTreasury: () =>
            Promise.resolve(
              jsonResponse(200, {
                ...healthzPayload(true),
                bolt12Offer: 'lno1shouldnotleak',
                mnemonic: 'should not leak',
              }),
            ),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )
    const raw = JSON.stringify(await response.json())

    expect(raw).not.toContain('lno1shouldnotleak')
    expect(raw).not.toContain('should not leak')
  })

  test('rejects non-GET methods', async () => {
    const response = await run(
      handlePublicTreasuryLaunchStatusApi(
        new Request(
          'https://openagents.com/api/public/treasury/launch-status',
          { method: 'POST' },
        ),
        { requireAdminApiToken: () => Promise.resolve(false) },
      ),
    )

    expect(response.status).toBe(405)
  })
})

describe('operator treasury status', () => {
  test('requires the admin api token', async () => {
    const response = await run(
      handleOperatorTreasuryStatusApi(
        new Request('https://openagents.com/api/operator/treasury/status'),
        {
          fetchTreasury: () =>
            Promise.resolve(jsonResponse(200, healthzPayload(true))),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('serves health and balance to an authorized operator', async () => {
    const response = await run(
      handleOperatorTreasuryStatusApi(
        new Request('https://openagents.com/api/operator/treasury/status'),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/balance'
                ? jsonResponse(200, {
                    balanceSat: 21000,
                    feeBudgetMsat: 231000,
                    maxSendableSat: 20700,
                  })
                : jsonResponse(200, healthzPayload(true)),
            ),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      balance: { maxSendableSat: number }
      state: string
    }

    expect(response.status).toBe(200)
    expect(body.state).toBe('configured')
    expect(body.balance.maxSendableSat).toBe(20700)
  })

  test('reports unprovisioned with 503 when no binding exists', async () => {
    const response = await run(
      handleOperatorTreasuryStatusApi(
        new Request('https://openagents.com/api/operator/treasury/status'),
        { requireAdminApiToken: () => Promise.resolve(true) },
      ),
    )

    expect(response.status).toBe(503)
  })

  test('surfaces a ready x-claim smoke preflight from dispatch stats', async () => {
    const response = await run(
      handleOperatorTreasuryStatusApi(
        new Request('https://openagents.com/api/operator/treasury/status'),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/balance'
                ? jsonResponse(200, { maxSendableSat: 20700 })
                : jsonResponse(200, healthzPayload(true)),
            ),
          readRewardDispatchStats: () =>
            Promise.resolve({
              dailySatsCap: 5000,
              enabled: true,
              liquidityBufferSats: 500,
              pendingPaymentCount: 0,
              perRunRewardCap: 1,
              requestedDispatchCount: 1,
              todayReservedSats: 0,
            }),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      rewardDispatchSmokePreflight: {
        blockingReasonRefs: ReadonlyArray<string>
        checks: ReadonlyArray<{ name: string; ok: boolean }>
        ready: boolean
      }
    }

    expect(response.status).toBe(200)
    expect(body.rewardDispatchSmokePreflight.ready).toBe(true)
    expect(body.rewardDispatchSmokePreflight.blockingReasonRefs).toHaveLength(0)
    expect(
      body.rewardDispatchSmokePreflight.checks.every(check => check.ok),
    ).toBe(true)
    // Public-safe: the preflight report leaks no balance figure or amount.
    expect(JSON.stringify(body.rewardDispatchSmokePreflight)).not.toContain(
      '20700',
    )
  })

  test('surfaces a blocking x-claim smoke preflight when caps fail', async () => {
    const response = await run(
      handleOperatorTreasuryStatusApi(
        new Request('https://openagents.com/api/operator/treasury/status'),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/balance'
                ? jsonResponse(200, { maxSendableSat: 100 })
                : jsonResponse(200, healthzPayload(true)),
            ),
          readRewardDispatchStats: () =>
            Promise.resolve({
              dailySatsCap: 5000,
              enabled: false,
              liquidityBufferSats: 500,
              pendingPaymentCount: 1,
              perRunRewardCap: 1,
              requestedDispatchCount: 0,
              todayReservedSats: 0,
            }),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      rewardDispatchSmokePreflight: {
        blockingReasonRefs: ReadonlyArray<string>
        ready: boolean
      }
    }

    expect(response.status).toBe(200)
    expect(body.rewardDispatchSmokePreflight.ready).toBe(false)
    expect(
      body.rewardDispatchSmokePreflight.blockingReasonRefs.length,
    ).toBeGreaterThan(0)
  })

  test('omits the smoke preflight when no dispatch stats reader is wired', async () => {
    const response = await run(
      handleOperatorTreasuryStatusApi(
        new Request('https://openagents.com/api/operator/treasury/status'),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/balance'
                ? jsonResponse(200, { maxSendableSat: 20700 })
                : jsonResponse(200, healthzPayload(true)),
            ),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect('rewardDispatchSmokePreflight' in body).toBe(false)
  })
})

describe('operator treasury funding destination', () => {
  test('requires the admin api token', async () => {
    const response = await run(
      handleOperatorTreasuryFundingDestinationApi(
        new Request(
          'https://openagents.com/api/operator/treasury/funding-destination',
        ),
        {
          fetchTreasury: () =>
            Promise.resolve(jsonResponse(200, { bolt12Offer: 'lno1x' })),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('serves both funding rails to an authorized operator', async () => {
    const response = await run(
      handleOperatorTreasuryFundingDestinationApi(
        new Request(
          'https://openagents.com/api/operator/treasury/funding-destination',
        ),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/offer'
                ? jsonResponse(200, {
                    bolt11Invoice: 'lnbc1example',
                    bolt12Offer: 'lno1example',
                    nodeId: 'ff'.repeat(33),
                  })
                : jsonResponse(404, { error: 'not_found' }),
            ),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      funding: { bolt11Invoice: string; bolt12Offer: string }
    }

    expect(response.status).toBe(200)
    expect(body.funding.bolt12Offer).toBe('lno1example')
    expect(body.funding.bolt11Invoice).toBe('lnbc1example')
  })

  test('returns 503 when the container has no funding destination', async () => {
    const response = await run(
      handleOperatorTreasuryFundingDestinationApi(
        new Request(
          'https://openagents.com/api/operator/treasury/funding-destination',
        ),
        {
          fetchTreasury: () => Promise.reject(new Error('container down')),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(503)
  })
})

describe('operator spark treasury funding destination', () => {
  test('requires the admin api token', async () => {
    const response = await run(
      handleOperatorSparkTreasuryFundingDestinationApi(
        new Request(
          'https://openagents.com/api/operator/treasury/spark-funding-destination',
        ),
        {
          fetchSparkTreasury: () =>
            Promise.resolve(
              jsonResponse(200, { rail: 'spark', sparkAddress: 'sp1x' }),
            ),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('serves spark funding rails to an authorized operator', async () => {
    const response = await run(
      handleOperatorSparkTreasuryFundingDestinationApi(
        new Request(
          'https://openagents.com/api/operator/treasury/spark-funding-destination',
        ),
        {
          fetchSparkTreasury: path =>
            Promise.resolve(
              path === '/spark/funding-destination'
                ? jsonResponse(200, {
                    lightningAddress: 'oaabc123@spark.money',
                    rail: 'spark',
                    sparkAddress: 'sp1example',
                  })
                : jsonResponse(404, { error: 'not_found' }),
            ),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      funding: {
        lightningAddress: string
        rail: string
        sparkAddress: string
      }
      service: string
    }

    expect(response.status).toBe(200)
    expect(body.service).toBe('spark_treasury')
    expect(body.funding.rail).toBe('spark')
    expect(body.funding.sparkAddress).toBe('sp1example')
    expect(body.funding.lightningAddress).toBe('oaabc123@spark.money')
  })

  test('returns 503 when the spark container funding destination is unavailable', async () => {
    const response = await run(
      handleOperatorSparkTreasuryFundingDestinationApi(
        new Request(
          'https://openagents.com/api/operator/treasury/spark-funding-destination',
        ),
        {
          fetchSparkTreasury: () =>
            Promise.resolve(jsonResponse(503, { error: 'spark_down' })),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(503)
    await expect(response.json()).resolves.toEqual({
      error: 'spark_treasury_funding_destination_unavailable',
    })
  })
})

describe('operator spark treasury funding invoice', () => {
  const invoiceRequest = (body: unknown) =>
    new Request(
      'https://openagents.com/api/operator/treasury/spark-funding-invoice',
      {
        body: JSON.stringify(body),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

  test('requires the admin api token', async () => {
    const response = await run(
      handleOperatorSparkTreasuryFundingInvoiceApi(
        invoiceRequest({ amountSat: 52000 }),
        {
          fetchSparkTreasury: () =>
            Promise.resolve(
              jsonResponse(200, {
                amountSat: 52000,
                bolt11Invoice: 'lnbc520u1test',
                rail: 'spark',
              }),
            ),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('requires a positive integer amount', async () => {
    const response = await run(
      handleOperatorSparkTreasuryFundingInvoiceApi(
        invoiceRequest({ amountSat: 0 }),
        {
          fetchSparkTreasury: () =>
            Promise.resolve(jsonResponse(500, { error: 'should_not_call' })),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'amount_sat_must_be_positive_integer',
    })
  })

  test('mints a Spark treasury BOLT11 invoice for an authorized operator', async () => {
    const response = await run(
      handleOperatorSparkTreasuryFundingInvoiceApi(
        invoiceRequest({ amountSat: 52000 }),
        {
          fetchSparkTreasury: (path, init) => {
            expect(path).toBe('/spark/funding-invoice')
            expect(init?.method).toBe('POST')
            expect(JSON.parse(String(init?.body))).toEqual({
              amountSat: 52000,
            })

            return Promise.resolve(
              jsonResponse(200, {
                amountSat: 52000,
                bolt11Invoice: 'lnbc520u1test',
                expiresInSeconds: 3600,
                rail: 'spark',
              }),
            )
          },
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      invoice: {
        amountSat: number
        bolt11Invoice: string
        expiresInSeconds: number
        rail: string
      }
      service: string
    }

    expect(response.status).toBe(200)
    expect(body.service).toBe('spark_treasury')
    expect(body.invoice).toEqual({
      amountSat: 52000,
      bolt11Invoice: 'lnbc520u1test',
      expiresInSeconds: 3600,
      rail: 'spark',
    })
  })

  test('returns 503 when the Spark treasury invoice is unavailable', async () => {
    const response = await run(
      handleOperatorSparkTreasuryFundingInvoiceApi(
        invoiceRequest({ amountSat: 52000 }),
        {
          fetchSparkTreasury: () =>
            Promise.resolve(jsonResponse(503, { error: 'spark_down' })),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(503)
    await expect(response.json()).resolves.toEqual({
      error: 'spark_down',
    })
  })
})

describe('operator treasury transaction reconciliation', () => {
  const reconcileRequest = (body: unknown) =>
    new Request(
      'https://openagents.com/api/operator/treasury/transactions/reconcile',
      {
        body: JSON.stringify(body),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

  test('requires the admin api token', async () => {
    const response = await run(
      handleOperatorTreasuryTransactionReconcileApi(
        reconcileRequest({ transactionId: 'treasury_payout_1' }),
        {
          fetchTreasury: () =>
            Promise.resolve(jsonResponse(200, { status: 'succeeded' })),
          requireAdminApiToken: () => Promise.resolve(false),
          transactionStore: makeMemoryTransactionStore(),
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('settles a pending treasury row when MDK reports succeeded', async () => {
    const store = makeMemoryTransactionStore([
      pendingOutTransaction('treasury_payout_1', 'payment_secret_succeeded'),
    ])
    const response = await run(
      handleOperatorTreasuryTransactionReconcileApi(
        reconcileRequest({ transactionId: 'treasury_payout_1' }),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/payments/payment_secret_succeeded'
                ? jsonResponse(200, {
                    paymentId: 'payment_secret_succeeded',
                    status: 'succeeded',
                  })
                : jsonResponse(404, { error: 'not_found' }),
            ),
          requireAdminApiToken: () => Promise.resolve(true),
          transactionStore: store,
        },
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      paymentStatus: 'succeeded',
      previousState: 'pending',
      reconciledState: 'settled',
      transactionId: 'treasury_payout_1',
      updated: true,
      wallet: 'treasury',
    })
    expect(JSON.stringify(body)).not.toContain('payment_secret_succeeded')
    expect(store.rows.get('treasury_payout_1')).toMatchObject({
      state: 'settled',
    })
  })

  test('marks a pending treasury row failed when MDK reports failed', async () => {
    const store = makeMemoryTransactionStore([
      pendingOutTransaction('treasury_payout_1', 'payment_secret_failed'),
    ])
    const response = await run(
      handleOperatorTreasuryTransactionReconcileApi(
        reconcileRequest({ transactionId: 'treasury_payout_1' }),
        {
          fetchTreasury: () =>
            Promise.resolve(jsonResponse(200, { status: 'failed' })),
          requireAdminApiToken: () => Promise.resolve(true),
          transactionStore: store,
        },
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      paymentStatus: 'failed',
      reconciledState: 'failed',
      updated: true,
    })
    expect(JSON.stringify(body)).not.toContain('payment_secret_failed')
    expect(store.rows.get('treasury_payout_1')?.state).toBe('failed')
  })

  test('leaves a row pending when the container still has no terminal outcome', async () => {
    const store = makeMemoryTransactionStore([
      pendingOutTransaction('treasury_payout_1', 'payment_secret_pending'),
    ])
    const response = await run(
      handleOperatorTreasuryTransactionReconcileApi(
        reconcileRequest({ transactionId: 'treasury_payout_1' }),
        {
          fetchTreasury: () =>
            Promise.resolve(jsonResponse(200, { status: 'pending' })),
          requireAdminApiToken: () => Promise.resolve(true),
          transactionStore: store,
        },
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      paymentStatus: 'pending',
      reconciledState: 'pending',
      updated: false,
    })
    expect(store.rows.get('treasury_payout_1')?.state).toBe('pending')
  })

  test('uses the tips-buffer container for tips-buffer payout rows', async () => {
    const store = makeMemoryTransactionStore([
      pendingOutTransaction('tips_buffer_payout_1', 'payment_secret_buffer'),
    ])
    const called: Array<string> = []
    const response = await run(
      handleOperatorTreasuryTransactionReconcileApi(
        reconcileRequest({ transactionId: 'tips_buffer_payout_1' }),
        {
          fetchTipsBuffer: path => {
            called.push(path)

            return Promise.resolve(jsonResponse(200, { status: 'succeeded' }))
          },
          fetchTreasury: path => {
            called.push(`treasury:${path}`)

            return Promise.resolve(jsonResponse(404, { error: 'wrong' }))
          },
          requireAdminApiToken: () => Promise.resolve(true),
          transactionStore: store,
        },
      ),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body.wallet).toBe('tips_buffer')
    expect(called).toEqual(['/payments/payment_secret_buffer'])
    expect(store.rows.get('tips_buffer_payout_1')?.state).toBe('settled')
  })

  test('refuses non-reconcilable redacted payment refs', async () => {
    const store = makeMemoryTransactionStore([
      pendingOutTransaction('treasury_payout_1', 'payment.treasury.abcdef123'),
    ])
    const response = await run(
      handleOperatorTreasuryTransactionReconcileApi(
        reconcileRequest({ transactionId: 'treasury_payout_1' }),
        {
          fetchTreasury: () =>
            Promise.resolve(jsonResponse(200, { status: 'succeeded' })),
          requireAdminApiToken: () => Promise.resolve(true),
          transactionStore: store,
        },
      ),
    )
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(409)
    expect(body.error).toBe('treasury_payment_ref_not_reconcilable')
    expect(store.rows.get('treasury_payout_1')?.state).toBe('pending')
  })

  test('scheduled reconciliation persists terminal outcomes for pending outbound rows', async () => {
    const store = makeMemoryTransactionStore([
      pendingOutTransaction('treasury_payout_succeeded', 'payment_secret_done'),
      pendingOutTransaction(
        'treasury_payout_pending',
        'payment_secret_pending',
      ),
      pendingOutTransaction(
        'tips_buffer_payout_failed',
        'payment_secret_failed',
      ),
      pendingOutTransaction(
        'treasury_payout_redacted',
        'payment.treasury.abcdef123',
      ),
    ])
    const fetchTreasury: ContainerPathFetch = path =>
      Promise.resolve(
        path === '/payments/payment_secret_done'
          ? jsonResponse(200, { status: 'succeeded' })
          : path === '/payments/payment_secret_pending'
            ? jsonResponse(200, { status: 'pending' })
            : jsonResponse(404, { error: 'not_found' }),
      )
    const fetchTipsBuffer: ContainerPathFetch = path =>
      Promise.resolve(
        path === '/payments/payment_secret_failed'
          ? jsonResponse(200, { status: 'failed' })
          : jsonResponse(404, { error: 'not_found' }),
      )

    const summary = await reconcilePendingTreasuryTransactions({
      fetchTipsBuffer,
      fetchTreasury,
      transactionStore: store,
    })

    expect(summary).toEqual({
      blocked: 1,
      checked: 4,
      failed: 1,
      pending: 1,
      settled: 1,
      updated: 2,
    })
    expect(store.rows.get('treasury_payout_succeeded')?.state).toBe('settled')
    expect(store.rows.get('tips_buffer_payout_failed')?.state).toBe('failed')
    expect(store.rows.get('treasury_payout_pending')?.state).toBe('pending')
    expect(store.rows.get('treasury_payout_redacted')?.state).toBe('pending')
  })
})

describe('treasury payout plan policy', () => {
  test('pays the full amount when spendable covers it', () => {
    expect(
      treasuryPayoutPlan({ intendedAmountSat: 1000, maxSendableSat: 1500 }),
    ).toEqual({ kind: 'full', paidAmountSat: 1000 })
  })

  test('falls back to 10% of spendable when it cannot cover the payout', () => {
    expect(
      treasuryPayoutPlan({ intendedAmountSat: 1000, maxSendableSat: 990 }),
    ).toEqual({ kind: 'fractional_fallback_10pct', paidAmountSat: 99 })
  })

  test('applies 10% of the then-current spendable on successive payouts', () => {
    const first = treasuryPayoutPlan({
      intendedAmountSat: 1000,
      maxSendableSat: 480,
    })
    expect(first).toEqual({
      kind: 'fractional_fallback_10pct',
      paidAmountSat: 48,
    })
    const second = treasuryPayoutPlan({
      intendedAmountSat: 1000,
      maxSendableSat: 432,
    })
    expect(second).toEqual({
      kind: 'fractional_fallback_10pct',
      paidAmountSat: 43,
    })
  })

  test('reports depleted when 10% rounds below one sat', () => {
    expect(
      treasuryPayoutPlan({ intendedAmountSat: 1000, maxSendableSat: 9 }),
    ).toEqual({ kind: 'depleted' })
    expect(
      treasuryPayoutPlan({ intendedAmountSat: 1000, maxSendableSat: null }),
    ).toEqual({ kind: 'depleted' })
  })
})

describe('operator treasury payout', () => {
  const payoutRequest = (body: unknown) =>
    new Request('https://openagents.com/api/operator/treasury/payout', {
      body: JSON.stringify(body),
      headers: { 'content-type': 'application/json' },
      method: 'POST',
    })

  test('requires the admin api token', async () => {
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({ amountSat: 1000, destination: 'lno1x' }),
        {
          fetchTreasury: () => Promise.resolve(jsonResponse(200, {})),
          requireAdminApiToken: () => Promise.resolve(false),
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('applies the fractional fallback against live spendable balance', async () => {
    const paid: Array<{ amountSat: number; destination: string }> = []
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({ amountSat: 1000, destination: 'lno1recipient' }),
        {
          fetchTreasury: (path, init) => {
            if (path === '/balance') {
              return Promise.resolve(
                jsonResponse(200, { balanceSat: 990, maxSendableSat: 990 }),
              )
            }

            if (path === '/pay' && init?.method === 'POST') {
              paid.push(JSON.parse(init.body ?? '{}'))

              return Promise.resolve(
                jsonResponse(200, {
                  paymentId: 'pay_1',
                  status: 'succeeded',
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      intendedAmountSat: number
      paidAmountSat: number
      policyApplied: string
      status: string
    }

    expect(response.status).toBe(200)
    expect(body.policyApplied).toBe('fractional_fallback_10pct')
    expect(body.intendedAmountSat).toBe(1000)
    expect(body.paidAmountSat).toBe(99)
    expect(body.status).toBe('succeeded')
    expect(paid).toEqual([{ amountSat: 99, destination: 'lno1recipient' }])
  })

  test('refuses with 409 when the treasury is depleted', async () => {
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({ amountSat: 1000, destination: 'lno1recipient' }),
        {
          fetchTreasury: () =>
            Promise.resolve(
              jsonResponse(200, { balanceSat: 9, maxSendableSat: 9 }),
            ),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(409)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('treasury_depleted')
  })

  // #5078: pay an offline recipient via a static Lightning Address fallback.
  const payDestinationsFetch =
    (
      paid: Array<string>,
      succeedFor: (destination: string) => boolean,
    ): ContainerPathFetch =>
    (path, init) => {
      if (path === '/balance') {
        return Promise.resolve(
          jsonResponse(200, { balanceSat: 100000, maxSendableSat: 100000 }),
        )
      }
      if (path === '/pay' && init?.method === 'POST') {
        const destination = String(
          JSON.parse(init.body ?? '{}').destination ?? '',
        )
        paid.push(destination)
        return succeedFor(destination)
          ? Promise.resolve(
              jsonResponse(200, { paymentId: 'pay_x', status: 'succeeded' }),
            )
          : Promise.resolve(jsonResponse(502, { error: 'pay_failed' }))
      }
      return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
    }

  test('retries transient null sendability before calling the treasury depleted', async () => {
    const paid: Array<string> = []
    let balanceReads = 0
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({ amountSat: 5000, destination: 'lno1recipient' }),
        {
          fetchTreasury: (path, init) => {
            if (path === '/balance') {
              balanceReads += 1
              return Promise.resolve(
                balanceReads === 1
                  ? jsonResponse(200, {
                      balanceSat: 40312,
                      maxSendableSat: null,
                    })
                  : jsonResponse(200, {
                      balanceSat: 40312,
                      maxSendableSat: 39909,
                    }),
              )
            }

            if (path === '/pay' && init?.method === 'POST') {
              paid.push(String(JSON.parse(init.body ?? '{}').destination ?? ''))

              return Promise.resolve(
                jsonResponse(200, {
                  paymentId: 'pay_retry_sendability',
                  status: 'succeeded',
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      paidAmountSat: number
      status: string
    }

    expect(response.status).toBe(200)
    expect(balanceReads).toBe(2)
    expect(body.paidAmountSat).toBe(5000)
    expect(body.status).toBe('succeeded')
    expect(paid).toEqual(['lno1recipient'])
  })

  test('primary success does not use the fallback destination (#5078)', async () => {
    const paid: Array<string> = []
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 1000,
          destination: 'lno1recipient',
          fallbackDestination: 'oab38ad12345abcd9@spark.money',
        }),
        {
          fetchTreasury: payDestinationsFetch(paid, d => d === 'lno1recipient'),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as { paidVia: string }
    expect(body.paidVia).toBe('primary')
    expect(paid).toEqual(['lno1recipient'])
  })

  test('returns safe payout diagnostics for successful operator sends', async () => {
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({ amountSat: 5000, destination: 'lno1recipient' }),
        {
          fetchTreasury: (path, init) => {
            if (path === '/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 60000,
                  maxSendableSat: 55000,
                }),
              )
            }

            if (path === '/pay' && init?.method === 'POST') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceChanged: true,
                  balanceDeltaSat: -5001,
                  balanceSatAfter: 54999,
                  balanceSatBefore: 60000,
                  destinationKind: 'bolt11',
                  eventOutcomeStatus: 'succeeded',
                  feeBudgetMsatAfter: 900,
                  feeBudgetMsatBefore: 1000,
                  paymentHash: 'raw_hash_not_returned_in_diagnostics',
                  paymentHashPresent: true,
                  paymentId: 'pay_success_diag',
                  paymentIdPresent: true,
                  preflightBalanceMaxSendableSat: 55000,
                  preflightCoverageSat: 48000,
                  preflightMaxSendableSat: 53000,
                  preflightRouteAvailable: true,
                  preimage: 'raw_preimage_not_returned_in_diagnostics',
                  preimagePresent: true,
                  resultReturned: true,
                  status: 'succeeded',
                  timeoutSecs: 50,
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(200)
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as {
      diagnostics: {
        balanceDeltaSat: number | null
        eventOutcomeStatus: string | null
        paymentHashPresent: boolean | null
        paymentIdPresent: boolean | null
        preflightCoverageSat: number | null
        preflightRouteAvailable: boolean | null
        preimagePresent: boolean | null
      }
    }

    expect(body.diagnostics).toMatchObject({
      balanceDeltaSat: -5001,
      eventOutcomeStatus: 'succeeded',
      paymentHashPresent: true,
      paymentIdPresent: true,
      preflightCoverageSat: 48000,
      preflightRouteAvailable: true,
      preimagePresent: true,
    })
    expect(bodyText).not.toContain('raw_hash_not_returned_in_diagnostics')
    expect(bodyText).not.toContain('raw_preimage_not_returned_in_diagnostics')
  })

  test('records public-safe recipient attribution for operator payouts', async () => {
    const store = makeMemoryTransactionStore()
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 50000,
          destination: 'recipient@spark.money',
          owedRef: 'owed.public.recognition.20260617',
          owedSat: 50000,
          recipientRef: 'agent:recipient_user',
        }),
        {
          fetchTreasury: (path, init) => {
            if (path === '/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 100000,
                  maxSendableSat: 100000,
                }),
              )
            }

            if (path === '/pay' && init?.method === 'POST') {
              return Promise.resolve(
                jsonResponse(200, {
                  paymentId: 'pay_recipient_attr',
                  status: 'succeeded',
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:03.000Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_recipient_attr',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: input.settled ? '2026-06-17T18:00:04.000Z' : null,
              state: input.settled ? 'settled' : 'pending',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: () =>
            Promise.resolve({ ok: true, bolt11: 'lnbc-resolved' }),
        },
      ),
    )
    const bodyText = await response.text()
    const row = store.rows.get('treasury_payout_recipient_attr')

    expect(response.status).toBe(200)
    expect(row).toMatchObject({
      amountSat: 50000,
      owedRef: 'owed.public.recognition.20260617',
      owedSat: 50000,
      recipientConfirmationState: 'unconfirmed',
      recipientRef: 'agent:recipient_user',
      state: 'settled',
    })
    expect(row?.redactedDestinationRef).toMatch(
      /^destination\.redacted\.treasury_payout\.[a-f0-9]{32}$/,
    )
    expect(bodyText).not.toContain('recipient@spark.money')
  })

  test('uses the funded Spark treasury rail for Lightning Address payouts (#5183)', async () => {
    const store = makeMemoryTransactionStore()
    const sparkPayCalls: Array<Record<string, unknown>> = []
    const mdkPayCalls: Array<Record<string, unknown>> = []
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 50000,
          destination: 'recipient@spark.money',
          owedRef: 'owed.public.recognition.20260617',
          owedSat: 50000,
          recipientRef: 'agent:recipient_user',
        }),
        {
          fetchSparkTreasury: (path, init) => {
            if (path === '/spark/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 75000,
                  maxSendableSat: 75000,
                  rail: 'spark',
                }),
              )
            }

            if (path === '/spark/pay' && init?.method === 'POST') {
              sparkPayCalls.push(JSON.parse(init.body ?? '{}'))

              return Promise.resolve(
                jsonResponse(200, {
                  method: 'lnurl_pay',
                  paymentHash: 'raw_hash_not_returned',
                  paymentRef: 'payment.redacted.spark_treasury.abc123',
                  preimage: 'raw_preimage_not_returned',
                  rail: 'spark',
                  status: 'succeeded',
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          fetchTreasury: (path, init) => {
            if (path === '/pay' && init?.method === 'POST') {
              mdkPayCalls.push(JSON.parse(init.body ?? '{}'))
            }

            return Promise.resolve(
              path === '/balance'
                ? jsonResponse(200, {
                    balanceSat: 100000,
                    maxSendableSat: 100000,
                  })
                : jsonResponse(404, { error: 'not_found' }),
            )
          },
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:03.000Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_spark_success',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: input.settled ? '2026-06-17T18:00:04.000Z' : null,
              state: input.settled ? 'settled' : 'pending',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: (address, amountSat) =>
            Promise.resolve(
              address === 'recipient@spark.money'
                ? { ok: true, bolt11: `lnbc-resolved-spark-${amountSat}` }
                : { ok: false, reason: 'unexpected_address' },
            ),
        },
      ),
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as {
      attempts: ReadonlyArray<{ outcome: string; rail: string }>
      paidAmountSat: number
      paidVia: string
      paymentRef: string
      policyApplied: string
      status: string
    }
    const row = store.rows.get('treasury_payout_spark_success')

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      paidAmountSat: 50000,
      paidVia: 'spark_treasury',
      paymentRef: 'payment.redacted.spark_treasury.abc123',
      policyApplied: 'full',
      status: 'succeeded',
    })
    expect(body.attempts).toEqual([
      expect.objectContaining({
        outcome: 'accepted',
        rail: 'spark_treasury',
      }),
    ])
    expect(sparkPayCalls).toEqual([
      expect.objectContaining({
        amountSat: 50000,
        destination: 'lnbc-resolved-spark-50000',
      }),
    ])
    expect(mdkPayCalls).toEqual([])
    expect(row).toMatchObject({
      amountSat: 50000,
      owedRef: 'owed.public.recognition.20260617',
      owedSat: 50000,
      paymentRef: 'payment.redacted.spark_treasury.abc123',
      recipientRef: 'agent:recipient_user',
      state: 'settled',
    })
    expect(bodyText).not.toContain('recipient@spark.money')
    expect(bodyText).not.toContain('raw_hash_not_returned')
    expect(bodyText).not.toContain('raw_preimage_not_returned')
  })

  test('falls back to MDK when Spark treasury is not sufficiently funded (#5183)', async () => {
    const mdkPayCalls: Array<Record<string, unknown>> = []
    const sparkPayCalls: Array<Record<string, unknown>> = []
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 50000,
          destination: 'recipient@spark.money',
        }),
        {
          fetchSparkTreasury: (path, init) => {
            if (path === '/spark/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 250,
                  maxSendableSat: 250,
                }),
              )
            }

            if (path === '/spark/pay' && init?.method === 'POST') {
              sparkPayCalls.push(JSON.parse(init.body ?? '{}'))
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          fetchTreasury: (path, init) => {
            if (path === '/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 100000,
                  maxSendableSat: 100000,
                }),
              )
            }

            if (path === '/pay' && init?.method === 'POST') {
              mdkPayCalls.push(JSON.parse(init.body ?? '{}'))

              return Promise.resolve(
                jsonResponse(200, {
                  paymentId: 'pay_mdk_after_spark_insufficient',
                  status: 'succeeded',
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: () =>
            Promise.resolve({ ok: true, bolt11: 'lnbc-resolved-50000' }),
        },
      ),
    )
    const body = (await response.json()) as {
      attempts: ReadonlyArray<{
        diagnostics: { failureStage: string | null }
        outcome: string
        rail: string
        reasonRef: string | null
      }>
      paidAmountSat: number
      paidVia: string
    }

    expect(response.status).toBe(200)
    expect(body.paidAmountSat).toBe(50000)
    expect(body.paidVia).toBe('primary')
    expect(body.attempts).toMatchObject([
      {
        diagnostics: { failureStage: 'spark_treasury_insufficient' },
        outcome: 'failed',
        rail: 'spark_treasury',
        reasonRef:
          'reason.public.treasury_payout.insufficient_spendable_balance',
      },
      {
        outcome: 'accepted',
        rail: 'mdk_treasury',
      },
    ])
    expect(sparkPayCalls).toEqual([])
    expect(mdkPayCalls).toEqual([
      {
        amountSat: 50000,
        destination: 'lnbc-resolved-50000',
      },
    ])
  })

  test('does not fall back to MDK after a Spark treasury dispatch failure (#5183)', async () => {
    const store = makeMemoryTransactionStore()
    const mdkPayCalls: Array<Record<string, unknown>> = []
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 50000,
          destination: 'recipient@spark.money',
          recipientRef: 'agent:orrery',
        }),
        {
          fetchSparkTreasury: (path, init) => {
            if (path === '/spark/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 75000,
                  maxSendableSat: 75000,
                }),
              )
            }

            if (path === '/spark/pay' && init?.method === 'POST') {
              return Promise.resolve(
                jsonResponse(502, {
                  error: 'spark_treasury_pay_failed',
                  failureStage: 'spark_pay',
                  reasonRef: 'reason.public.treasury_payout.no_route',
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          fetchTreasury: (path, init) => {
            if (path === '/pay' && init?.method === 'POST') {
              mdkPayCalls.push(JSON.parse(init.body ?? '{}'))
            }

            return Promise.resolve(
              path === '/balance'
                ? jsonResponse(200, {
                    balanceSat: 100000,
                    maxSendableSat: 100000,
                  })
                : jsonResponse(404, { error: 'not_found' }),
            )
          },
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:05.000Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_spark_failed',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: null,
              state: 'failed',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: () =>
            Promise.resolve({ ok: true, bolt11: 'lnbc-resolved-spark-50000' }),
        },
      ),
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as {
      paidVia: string
      reasonRef: string
    }

    expect(response.status).toBe(502)
    expect(body.paidVia).toBe('spark_treasury')
    expect(body.reasonRef).toBe('reason.public.treasury_payout.no_route')
    expect(mdkPayCalls).toEqual([])
    expect(store.rows.get('treasury_payout_spark_failed')).toMatchObject({
      amountSat: 50000,
      failureReasonRef: 'reason.public.treasury_payout.no_route',
      paymentRef: null,
      recipientRef: 'agent:orrery',
      state: 'failed',
    })
    expect(bodyText).not.toContain('recipient@spark.money')
  })

  test('primary fail retries the Lightning Address fallback (#5078)', async () => {
    const paid: Array<string> = []
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 1000,
          destination: 'lno1recipient',
          fallbackDestination: 'oab38ad12345abcd9@spark.money',
        }),
        {
          // The Lightning Address fallback is resolved to a BOLT11 via LNURL
          // before the MDK send (#5078); the resolver is stubbed here.
          fetchTreasury: payDestinationsFetch(
            paid,
            d => d === 'lnbc-resolved-fallback',
          ),
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: () =>
            Promise.resolve({ ok: true, bolt11: 'lnbc-resolved-fallback' }),
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      attempts: ReadonlyArray<{
        attempt: string
        diagnostics: {
          containerStatus: string | null
          payResponseStatus: number | null
          resolvedDestinationKind: string | null
          sourceDestinationKind: string | null
        }
        outcome: string
      }>
      paidVia: string
    }
    expect(body.paidVia).toBe('fallback')
    expect(body.attempts).toMatchObject([
      {
        attempt: 'primary',
        diagnostics: { payResponseStatus: 502 },
        outcome: 'failed',
      },
      {
        attempt: 'fallback',
        diagnostics: {
          containerStatus: 'succeeded',
          payResponseStatus: 200,
          resolvedDestinationKind: 'bolt11',
          sourceDestinationKind: 'lightning_address',
        },
        outcome: 'accepted',
      },
    ])
    // The MDK send receives the resolved BOLT11, not the raw address.
    expect(paid).toEqual(['lno1recipient', 'lnbc-resolved-fallback'])
  })

  test('primary fail with no fallback still fails cleanly (#5078)', async () => {
    const paid: Array<string> = []
    const store = makeMemoryTransactionStore()
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({ amountSat: 1000, destination: 'lno1recipient' }),
        {
          fetchTreasury: payDestinationsFetch(paid, () => false),
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:01.000Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_failed_1',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: null,
              state:
                input.failureReasonRef !== undefined &&
                input.failureReasonRef !== null
                  ? 'failed'
                  : input.settled
                    ? 'settled'
                    : 'pending',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )

    expect(response.status).toBe(502)
    const body = (await response.json()) as {
      error: string
      reason: string
      reasonRef: string
    }
    expect(body.error).toBe('treasury_pay_failed')
    expect(body.reason).toBe('reason.public.treasury_payout.failed')
    expect(body.reasonRef).toBe('reason.public.treasury_payout.failed')
    expect(paid).toEqual(['lno1recipient'])
    expect(store.rows.get('treasury_payout_failed_1')).toMatchObject({
      amountSat: 1000,
      failureReasonRef: 'reason.public.treasury_payout.failed',
      paymentRef: null,
      state: 'failed',
    })
  })

  test('times out stuck container pay calls with safe diagnostics', async () => {
    const store = makeMemoryTransactionStore()
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({ amountSat: 1000, destination: 'lno1recipient' }),
        {
          fetchTreasury: path => {
            if (path === '/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 100000,
                  maxSendableSat: 100000,
                }),
              )
            }

            if (path === '/pay') {
              return new Promise<Response>(() => {})
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          payRequestTimeoutMs: 1,
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:01.250Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_pay_timeout',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: null,
              state: 'failed',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as {
      attempts: ReadonlyArray<{
        diagnostics: {
          failureStage: string | null
          resultReturned: boolean | null
          timeoutSecs: number | null
        }
        reasonRef: string | null
      }>
      diagnostics: {
        failureStage: string | null
        resultReturned: boolean | null
        timeoutSecs: number | null
      }
      reasonRef: string
    }

    expect(response.status).toBe(502)
    expect(body.reasonRef).toBe('reason.public.treasury_payout.timeout')
    expect(body.diagnostics).toMatchObject({
      failureStage: 'pay_request_timeout',
      resultReturned: false,
      timeoutSecs: 1,
    })
    expect(body.attempts).toMatchObject([
      {
        diagnostics: {
          failureStage: 'pay_request_timeout',
          resultReturned: false,
          timeoutSecs: 1,
        },
        reasonRef: 'reason.public.treasury_payout.timeout',
      },
    ])
    expect(bodyText).not.toContain('lno1recipient')
    expect(store.rows.get('treasury_payout_pay_timeout')).toMatchObject({
      amountSat: 1000,
      failureReasonRef: 'reason.public.treasury_payout.timeout',
      paymentRef: null,
      state: 'failed',
    })
  })

  test('classifies the daemon reason when the container error is generic', async () => {
    const store = makeMemoryTransactionStore()
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 5000,
          destination: 'recipient@spark.money',
        }),
        {
          fetchTreasury: (path, init) => {
            if (path === '/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 100000,
                  maxSendableSat: 100000,
                }),
              )
            }

            if (path === '/pay' && init?.method === 'POST') {
              return Promise.resolve(
                jsonResponse(502, {
                  error: 'tips_buffer_pay_failed',
                  reason: 'No route found for payment.',
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:01.500Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_failed_reason',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: null,
              state: 'failed',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: () =>
            Promise.resolve({ ok: true, bolt11: 'lnbc-resolved' }),
        },
      ),
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as { reasonRef: string }

    expect(response.status).toBe(502)
    expect(body.reasonRef).toBe('reason.public.treasury_payout.no_route')
    expect(bodyText).not.toContain('No route found')
    expect(bodyText).not.toContain('recipient@spark.money')
    expect(store.rows.get('treasury_payout_failed_reason')).toMatchObject({
      amountSat: 5000,
      failureReasonRef: 'reason.public.treasury_payout.no_route',
      paymentRef: null,
      state: 'failed',
    })
  })

  test('prefers container-classified safe payout diagnostics', async () => {
    const store = makeMemoryTransactionStore()
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 40000,
          destination: 'recipient@spark.money',
        }),
        {
          fetchTreasury: (path, init) => {
            if (path === '/balance') {
              return Promise.resolve(
                jsonResponse(200, {
                  balanceSat: 100000,
                  maxSendableSat: 100000,
                }),
              )
            }

            if (path === '/pay' && init?.method === 'POST') {
              return Promise.resolve(
                jsonResponse(502, {
                  balanceChanged: false,
                  balanceSatAfter: 100000,
                  balanceSatBefore: 100000,
                  destinationKind: 'bolt11',
                  errorCauseMessageSummary: 'route_failed',
                  errorCode: 'err_private_route',
                  errorKeySummary: 'name:code:cause',
                  errorMessageSummary: 'no_route_found_for_payment',
                  errorName: 'mdk_error',
                  error: 'treasury_pay_failed',
                  failureStage: 'pay_throws',
                  feeBudgetMsatAfter: 42,
                  feeBudgetMsatBefore: 42,
                  messageFingerprint:
                    '9aedda5a994a799337d6c5398271f2468702ee95b305d0d08f3a7c8f14eabf19',
                  paymentIdPresent: false,
                  payResponseStatus: 502,
                  preflightBalanceMaxSendableSat: 99000,
                  preflightMaxSendableSat: 94000,
                  preparedPaymentMethodKind: 'bolt11Invoice',
                  preferSparkForBolt11: true,
                  reason: 'raw private daemon route failure',
                  reasonClass: 'no_route',
                  reasonRef: 'reason.public.treasury_payout.no_route',
                  resultReturned: false,
                  timeoutSecs: 50,
                }),
              )
            }

            return Promise.resolve(jsonResponse(404, { error: 'not_found' }))
          },
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:01.750Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_failed_container_classified',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: null,
              state: 'failed',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: () =>
            Promise.resolve({ ok: true, bolt11: 'lnbc-resolved' }),
        },
      ),
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as {
      diagnostics: {
        balanceChanged: boolean | null
        balanceDeltaSat: number | null
        balanceSatAfter: number | null
        balanceSatBefore: number | null
        containerStatus: string | null
        destinationKind: string | null
        errorCauseMessageSummary: string | null
        errorCode: string | null
        errorKeySummary: string | null
        errorMessageSummary: string | null
        errorName: string | null
        eventOutcomeStatus: string | null
        failureStage: string | null
        feeBudgetMsatAfter: number | null
        feeBudgetMsatBefore: number | null
        messageFingerprint: string | null
        paymentHashPresent: boolean | null
        paymentIdPresent: boolean | null
        payResponseStatus: number | null
        preflightBalanceMaxSendableSat: number | null
        preflightCoverageSat: number | null
        preflightMaxSendableSat: number | null
        preflightRouteAvailable: boolean | null
        preparedAmountSat: number | null
        preparedFeeSats: number | null
        preparedLightningFeeSats: number | null
        preparedPaymentMethodKind: string | null
        preparedSparkTransferFeeSats: number | null
        preferSparkForBolt11: boolean | null
        preimagePresent: boolean | null
        reasonClass: string | null
        resolvedDestinationKind: string | null
        resultReturned: boolean | null
        sourceDestinationKind: string | null
        timeoutSecs: number | null
      }
      reasonRef: string
    }

    expect(response.status).toBe(502)
    expect(body.reasonRef).toBe('reason.public.treasury_payout.no_route')
    expect(body.diagnostics).toEqual({
      balanceChanged: false,
      balanceDeltaSat: null,
      balanceSatAfter: 100000,
      balanceSatBefore: 100000,
      containerStatus: null,
      destinationKind: 'bolt11',
      errorCauseMessageSummary: 'route_failed',
      errorCode: 'err_private_route',
      errorKeySummary: 'name:code:cause',
      errorMessageSummary: 'no_route_found_for_payment',
      errorName: 'mdk_error',
      eventOutcomeStatus: null,
      failureStage: 'pay_throws',
      feeBudgetMsatAfter: 42,
      feeBudgetMsatBefore: 42,
      messageFingerprint:
        '9aedda5a994a799337d6c5398271f2468702ee95b305d0d08f3a7c8f14eabf19',
      paymentHashPresent: null,
      paymentIdPresent: false,
      payResponseStatus: 502,
      preflightBalanceMaxSendableSat: 99000,
      preflightCoverageSat: null,
      preflightMaxSendableSat: 94000,
      preflightRouteAvailable: null,
      preparedAmountSat: null,
      preparedFeeSats: null,
      preparedLightningFeeSats: null,
      preparedPaymentMethodKind: 'bolt11invoice',
      preparedSparkTransferFeeSats: null,
      preferSparkForBolt11: true,
      preimagePresent: null,
      reasonClass: 'no_route',
      resolvedDestinationKind: 'bolt11',
      resultReturned: false,
      sourceDestinationKind: 'lightning_address',
      timeoutSecs: 50,
    })
    expect(bodyText).not.toContain('raw private daemon route failure')
    expect(bodyText).not.toContain('recipient@spark.money')
    expect(
      store.rows.get('treasury_payout_failed_container_classified'),
    ).toMatchObject({
      amountSat: 40000,
      failureReasonRef: 'reason.public.treasury_payout.no_route',
      paymentRef: null,
      state: 'failed',
    })
  })

  test('records Lightning Address resolution failures with safe reason refs', async () => {
    const store = makeMemoryTransactionStore()
    const response = await run(
      handleOperatorTreasuryPayoutApi(
        payoutRequest({
          amountSat: 50000,
          destination: 'recipient@spark.money',
        }),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/balance'
                ? jsonResponse(200, {
                    balanceSat: 51690,
                    maxSendableSat: 51173,
                  })
                : jsonResponse(404, { error: 'not_found' }),
            ),
          recordPayoutTransaction: input =>
            store.insert({
              amountSat: input.amountSat,
              bolt11: null,
              createdAt: '2026-06-17T18:00:02.000Z',
              direction: 'out',
              expiresAt: null,
              failureReasonRef: input.failureReasonRef ?? null,
              id: 'treasury_payout_failed_resolution',
              owedRef: input.owedRef ?? null,
              owedSat: input.owedSat ?? null,
              paymentRef: input.paymentRef,
              recipientConfirmationRef: null,
              recipientConfirmationState: 'unconfirmed',
              recipientConfirmedAt: null,
              recipientRef: input.recipientRef ?? null,
              redactedDestinationRef: input.redactedDestinationRef ?? null,
              settledAt: null,
              state: 'failed',
            }),
          requireAdminApiToken: () => Promise.resolve(true),
          resolveLightningAddress: () =>
            Promise.resolve({
              ok: false,
              reason: 'amount_out_of_range_1000_10000000_msat',
            }),
        },
      ),
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as { reasonRef: string }

    expect(response.status).toBe(502)
    expect(body.reasonRef).toBe(
      'reason.public.treasury_payout.lightning_address_resolution_failed.amount_out_of_range_1000_10000000_msat',
    )
    expect(bodyText).not.toContain('recipient@spark.money')
    expect(store.rows.get('treasury_payout_failed_resolution')).toMatchObject({
      amountSat: 50000,
      failureReasonRef:
        'reason.public.treasury_payout.lightning_address_resolution_failed.amount_out_of_range_1000_10000000_msat',
      paymentRef: null,
      state: 'failed',
    })
  })

  test('executeTreasuryPayout core falls back then reports paidVia (#5078)', async () => {
    const paid: Array<string> = []
    const fallback = await executeTreasuryPayout(
      {
        fetchTreasury: payDestinationsFetch(
          paid,
          d => d === 'lnbc-resolved-fallback',
        ),
        requireAdminApiToken: () => Promise.resolve(true),
        resolveLightningAddress: () =>
          Promise.resolve({ ok: true, bolt11: 'lnbc-resolved-fallback' }),
      },
      {
        amountSat: 1000,
        destination: 'lno1recipient',
        fallbackDestination: 'oab38ad12345abcd9@spark.money',
      },
    )
    expect(fallback.kind).toBe('paid')
    if (fallback.kind === 'paid') {
      expect(fallback.paidVia).toBe('fallback')
    }

    const noFallback = await executeTreasuryPayout(
      {
        fetchTreasury: payDestinationsFetch([], () => false),
        requireAdminApiToken: () => Promise.resolve(true),
      },
      { amountSat: 1000, destination: 'lno1recipient' },
    )
    expect(noFallback.kind).toBe('refused')
    if (noFallback.kind === 'refused') {
      expect(noFallback.reason).toBe('treasury_pay_failed')
    }
  })
})

describe('operator treasury recipient report', () => {
  test('summarizes owed, settled-sent, confirmed-received, and over-send state', async () => {
    const store = makeMemoryTransactionStore([
      settledRecipientOutTransaction({
        amountSat: 25000,
        confirmed: true,
        id: 'send_1',
        owedRef: 'owed.public.recognition.20260617',
        owedSat: 50000,
        recipientRef: 'agent:recipient_user',
      }),
      settledRecipientOutTransaction({
        amountSat: 30000,
        id: 'send_2',
        owedRef: 'owed.public.recognition.20260617',
        owedSat: 50000,
        recipientRef: 'agent:recipient_user',
      }),
    ])
    const response = await run(
      handleOperatorTreasuryRecipientReportApi(
        new Request(
          'https://openagents.com/api/operator/treasury/recipient-report?recipientRef=agent:recipient_user',
        ),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          transactionStore: store,
        },
      ),
    )
    const bodyText = await response.text()
    const body = JSON.parse(bodyText) as {
      confirmedReceivedSat: number
      owedSat: number
      overSent: boolean
      settledSentSat: number
      transactions: ReadonlyArray<Record<string, unknown>>
    }

    expect(response.status).toBe(200)
    expect(body.owedSat).toBe(50000)
    expect(body.settledSentSat).toBe(55000)
    expect(body.confirmedReceivedSat).toBe(25000)
    expect(body.overSent).toBe(true)
    expect(body.transactions).toHaveLength(2)
    expect(bodyText).not.toContain('payment_secret')
  })

  test('marks a settled payout as recipient-confirmed', async () => {
    const store = makeMemoryTransactionStore([
      settledRecipientOutTransaction({
        amountSat: 50000,
        id: 'send_confirm',
        owedRef: 'owed.public.recognition.20260617',
        owedSat: 50000,
        recipientRef: 'agent:recipient_user',
      }),
    ])
    const response = await run(
      handleOperatorTreasuryRecipientConfirmationApi(
        new Request(
          'https://openagents.com/api/operator/treasury/recipient-confirmations',
          {
            body: JSON.stringify({
              confirmationRef: 'receipt.public.spark_backup.balance_detected',
              transactionId: 'send_confirm',
            }),
            method: 'POST',
          },
        ),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          transactionStore: store,
        },
      ),
    )
    const body = (await response.json()) as {
      recipientConfirmationState: string
    }
    const row = store.rows.get('send_confirm')

    expect(response.status).toBe(200)
    expect(body.recipientConfirmationState).toBe('confirmed_received')
    expect(row).toMatchObject({
      recipientConfirmationRef: 'receipt.public.spark_backup.balance_detected',
      recipientConfirmationState: 'confirmed_received',
    })
  })
})
