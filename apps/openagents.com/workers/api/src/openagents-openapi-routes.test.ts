import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { OpenAgentsOpenApiEndpoint } from './openagents-openapi'
import { handleOpenAgentsOpenApi } from './openagents-openapi-routes'

type OpenApiDocument = Readonly<{
  components: Readonly<{ securitySchemes: Record<string, unknown> }>
  info: Readonly<{ title: string; version: string }>
  openapi: string
  paths: Record<string, Record<string, unknown>>
  tags?: ReadonlyArray<{ name: string }>
}>

const runRoute = (method = 'GET'): Promise<Response> =>
  Effect.runPromise(
    handleOpenAgentsOpenApi(
      new Request(`https://openagents.com${OpenAgentsOpenApiEndpoint}`, {
        method,
      }),
    ),
  )

describe('OpenAgents OpenAPI route', () => {
  test('serves active API discovery without the retired money or Sites graphs', async () => {
    const response = await runRoute()
    const body = (await response.json()) as OpenApiDocument
    const pathNames = Object.keys(body.paths)

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('content-type')).toBe('application/json')
    expect(body.openapi).toBe('3.1.0')
    expect(body.info.title).toBe('OpenAgents Autopilot API')
    expect(pathNames.length).toBeGreaterThan(50)
    expect(pathNames).toContain('/api/public/product-promises')
    expect(pathNames).not.toEqual(
      expect.arrayContaining([
        expect.stringMatching(
          /(?:billing|checkout|credits?|payments?|wallet|payout|settlement|treasury|tips?|markets?|sites?)(?:\/|$)/i,
        ),
      ]),
    )
    expect(body.tags?.map(tag => tag.name)).not.toEqual(
      expect.arrayContaining(['Payments', 'Sites']),
    )
    expect(containsProviderSecretMaterial(JSON.stringify(body))).toBe(false)
  })

  test('retains authentication metadata for active contracts', async () => {
    const response = await runRoute()
    const body = (await response.json()) as OpenApiDocument

    expect(body.components.securitySchemes).toEqual(expect.any(Object))
    expect(Object.keys(body.components.securitySchemes).length).toBeGreaterThan(
      0,
    )
  })

  test('rejects non-GET methods', async () => {
    const response = await runRoute('POST')

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
    await expect(response.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
  })
})
