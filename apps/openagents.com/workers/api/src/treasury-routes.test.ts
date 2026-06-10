import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleOperatorTreasuryFundingDestinationApi,
  handleOperatorTreasuryStatusApi,
  handlePublicTreasuryLaunchStatusApi,
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
