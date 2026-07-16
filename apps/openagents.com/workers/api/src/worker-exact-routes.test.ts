import { Effect } from 'effect'
import { describe, expect, test } from 'vite-plus/test'

import {
  type Env,
  exactRouteHandlerForPath,
  exactRoutePathManifest,
} from './index'
import {
  PublicProductPromisesEndpoint,
  PublicProductPromisesVersion,
} from './product-promises'

const retiredCapabilityPath =
  /(?:^|[\/-])(?:adjutant|balances?|billing|checkout|credits?|markets?|marketplace|payments?|payouts?|settled|settlements?|sites?|tips?|treasury|wallets?|work-requests)(?:[\/-]|$)|paid-privacy|l402/i

describe('Worker exact route manifest', () => {
  test('retains a substantial active route graph without retired money or Sites capability', () => {
    expect(exactRoutePathManifest.length).toBeGreaterThan(50)
    expect(exactRoutePathManifest).toContain('/api/public/home')
    expect(exactRoutePathManifest).toContain(PublicProductPromisesEndpoint)
    expect(exactRoutePathManifest).toContain('/api/openapi.json')
    expect(
      exactRoutePathManifest.filter(path => retiredCapabilityPath.test(path)),
    ).toEqual([])
  })

  test('does not contain duplicate exact paths', () => {
    expect(new Set(exactRoutePathManifest).size).toBe(
      exactRoutePathManifest.length,
    )
  })

  test('serves the documented public product-promise registry through the production route table', async () => {
    const handler = exactRouteHandlerForPath(PublicProductPromisesEndpoint)
    expect(handler).toBeDefined()

    const response = await Effect.runPromise(
      handler!(
        new Request(`https://openagents.com${PublicProductPromisesEndpoint}`),
        {} as Env,
        {} as ExecutionContext,
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toContain('no-store')
    await expect(response.json()).resolves.toMatchObject({
      registryVersion: PublicProductPromisesVersion,
      schemaVersion: 'openagents.product_promises.v1',
    })
  })

  test('rejects unsupported methods on the public product-promise registry', async () => {
    const handler = exactRouteHandlerForPath(PublicProductPromisesEndpoint)
    expect(handler).toBeDefined()

    const response = await Effect.runPromise(
      handler!(
        new Request(`https://openagents.com${PublicProductPromisesEndpoint}`, {
          method: 'POST',
        }),
        {} as Env,
        {} as ExecutionContext,
      ),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })
})
