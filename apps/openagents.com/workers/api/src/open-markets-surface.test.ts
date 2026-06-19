import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { openAgentsOpenApiDocument } from './openagents-openapi'
import {
  handleLiquidityMarketSkeletonApi,
  handleOpenMarketsSurfaceApi,
  handleRiskMarketSkeletonApi,
} from './open-markets-routes'
import {
  LiquidityMarketSkeletonEndpoint,
  LiquidityMarketSkeletonProjection,
  RiskMarketSkeletonEndpoint,
  RiskMarketSkeletonProjection,
  projectLiquidityMarketSkeleton,
  projectRiskMarketSkeleton,
} from './open-markets-skeletons'
import {
  OpenMarketsSurfaceEndpoint,
  OpenMarketsSurfaceProjection,
  projectOpenMarketsSurface,
} from './open-markets-surface'

describe('Open-markets unified surface', () => {
  const projection = projectOpenMarketsSurface({
    generatedAt: '2026-06-19T22:15:00.000Z',
  })

  test('decodes against its schema and stays a labeled scaffold', () => {
    expect(
      S.decodeUnknownSync(OpenMarketsSurfaceProjection)(projection),
    ).toEqual(projection)
    expect(projection.status).toBe('unified_surface_scaffold')
    expect(projection.promiseRef).toBe(
      'promise:markets.open_protocol_markets.v1',
    )
    expect(projection.generatedAt).toBe('2026-06-19T22:15:00.000Z')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
  })

  test('enumerates all six Episode 213 markets with honest state', () => {
    const ids = projection.markets.map(market => market.marketId).sort()
    expect(ids).toEqual([
      'compute',
      'data',
      'labor',
      'liquidity',
      'risk',
      'verification',
    ])

    const byId = new Map(
      projection.markets.map(market => [market.marketId, market]),
    )
    expect(byId.get('labor')?.state).toBe('live_scoped')
    expect(byId.get('labor')?.hasSettledReceipt).toBe(true)
    expect(byId.get('verification')?.state).toBe('live_scoped')
    expect(byId.get('verification')?.hasSettledReceipt).toBe(true)
    expect(byId.get('compute')?.state).toBe('shipped_not_broadly_live')
    expect(byId.get('data')?.state).toBe('shipped_not_broadly_live')
    expect(byId.get('liquidity')?.state).toBe('skeleton')
    expect(byId.get('risk')?.state).toBe('skeleton')
  })

  test('skeleton + shipped markets have no settled receipt', () => {
    for (const market of projection.markets) {
      if (
        market.state === 'skeleton' ||
        market.state === 'shipped_not_broadly_live'
      ) {
        expect(market.hasSettledReceipt).toBe(false)
      }
    }
  })

  test('market counts are honest and self-consistent', () => {
    expect(projection.marketCounts.total).toBe(6)
    expect(projection.marketCounts.liveScoped).toBe(2)
    expect(projection.marketCounts.shippedNotBroadlyLive).toBe(2)
    expect(projection.marketCounts.skeleton).toBe(2)
    expect(projection.marketCounts.unbuilt).toBe(0)
    expect(projection.marketCounts.withSettledReceipt).toBe(2)
    expect(projection.skeletonMarketIds).toEqual(['liquidity', 'risk'])
  })

  test('carries unsafe-copy guards against overclaiming', () => {
    expect(projection.unsafeCopy).toContain('promise is green')
    const liquidity = projection.markets.find(
      market => market.marketId === 'liquidity',
    )
    expect(liquidity?.unsafeCopy).toContain('inert scaffolding')
  })

  test('GET returns the projection no-store; non-GET is 405', async () => {
    const ok = await Effect.runPromise(
      handleOpenMarketsSurfaceApi(
        new Request('https://openagents.com/api/public/markets/open-markets'),
      ),
    )
    expect(ok.status).toBe(200)
    expect(ok.headers.get('cache-control')).toBe('no-store')
    const body = (await ok.json()) as { surfaceId: string; markets: unknown[] }
    expect(body.surfaceId).toBe('markets.open_protocol_markets.v1')
    expect(body.markets).toHaveLength(6)

    const denied = await Effect.runPromise(
      handleOpenMarketsSurfaceApi(
        new Request(
          'https://openagents.com/api/public/markets/open-markets',
          { method: 'POST' },
        ),
      ),
    )
    expect(denied.status).toBe(405)
  })
})

describe('Liquidity market skeleton', () => {
  const projection = projectLiquidityMarketSkeleton({
    generatedAt: '2026-06-19T22:15:00.000Z',
  })

  test('decodes and is inert by construction', () => {
    expect(
      S.decodeUnknownSync(LiquidityMarketSkeletonProjection)(projection),
    ).toEqual(projection)
    expect(projection.state).toBe('skeleton')
    expect(projection.inert).toBe(true)
    expect(projection.moneyMovement).toBe('none')
    expect(projection.settledTransactionCount).toBe(0)
    expect(projection.promiseGreen).toBe(false)
    expect(projection.blockerRefs).toContain(
      'blocker.product_promises.liquidity_market_unbuilt',
    )
  })

  test('documents protocol message shapes that are all inert', () => {
    expect(projection.protocolMessages.length).toBeGreaterThan(0)
    for (const message of projection.protocolMessages) {
      expect(message.inert).toBe(true)
      expect(message.fields.length).toBeGreaterThan(0)
    }
    const kinds = projection.protocolMessages.map(message => message.kind)
    expect(kinds).toContain('liquidity.request')
    expect(kinds).toContain('liquidity.settlement_receipt')
  })

  test('GET returns inert projection; non-GET is 405', async () => {
    const ok = await Effect.runPromise(
      handleLiquidityMarketSkeletonApi(
        new Request(
          'https://openagents.com/api/public/markets/liquidity/skeleton',
        ),
      ),
    )
    expect(ok.status).toBe(200)
    const body = (await ok.json()) as { inert: boolean; moneyMovement: string }
    expect(body.inert).toBe(true)
    expect(body.moneyMovement).toBe('none')

    const denied = await Effect.runPromise(
      handleLiquidityMarketSkeletonApi(
        new Request(
          'https://openagents.com/api/public/markets/liquidity/skeleton',
          { method: 'PUT' },
        ),
      ),
    )
    expect(denied.status).toBe(405)
  })
})

describe('Risk market skeleton', () => {
  const projection = projectRiskMarketSkeleton({
    generatedAt: '2026-06-19T22:15:00.000Z',
  })

  test('decodes and is inert, with the agentic-insurance-policy primitive', () => {
    expect(
      S.decodeUnknownSync(RiskMarketSkeletonProjection)(projection),
    ).toEqual(projection)
    expect(projection.state).toBe('skeleton')
    expect(projection.inert).toBe(true)
    expect(projection.moneyMovement).toBe('none')
    expect(projection.settledTransactionCount).toBe(0)
    expect(projection.promiseGreen).toBe(false)
    expect(projection.agenticInsurancePolicyPrimitive.name).toBe(
      'agentic_insurance_policy',
    )
    expect(projection.agenticInsurancePolicyPrimitive.inert).toBe(true)
    expect(
      projection.agenticInsurancePolicyPrimitive.policyFields,
    ).toContain('premiumSats')
    expect(projection.blockerRefs).toContain(
      'blocker.product_promises.risk_market_unbuilt',
    )
  })

  test('GET returns inert projection; non-GET is 405', async () => {
    const ok = await Effect.runPromise(
      handleRiskMarketSkeletonApi(
        new Request('https://openagents.com/api/public/markets/risk/skeleton'),
      ),
    )
    expect(ok.status).toBe(200)
    const body = (await ok.json()) as { inert: boolean; marketId: string }
    expect(body.inert).toBe(true)
    expect(body.marketId).toBe('risk')

    const denied = await Effect.runPromise(
      handleRiskMarketSkeletonApi(
        new Request(
          'https://openagents.com/api/public/markets/risk/skeleton',
          { method: 'DELETE' },
        ),
      ),
    )
    expect(denied.status).toBe(405)
  })
})

describe('Open-markets OpenAPI registration', () => {
  test('registers all three endpoints and response schemas', async () => {
    const document = await Effect.runPromise(openAgentsOpenApiDocument())

    for (const endpoint of [
      OpenMarketsSurfaceEndpoint,
      LiquidityMarketSkeletonEndpoint,
      RiskMarketSkeletonEndpoint,
    ]) {
      const path = (
        document.paths as Record<string, { get?: unknown } | undefined>
      )[endpoint]
      expect(path?.get).toBeDefined()
    }

    const schemas = (
      document.components as { schemas: Record<string, unknown> }
    ).schemas
    expect(schemas).toHaveProperty('OpenMarketsSurfaceProjection')
    expect(schemas).toHaveProperty('LiquidityMarketSkeletonProjection')
    expect(schemas).toHaveProperty('RiskMarketSkeletonProjection')
  })
})
