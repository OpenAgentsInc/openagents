import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { handleModelsList } from './models-routes'
import { MODEL_PRICING_TABLE } from './pricing'

const run = (request: Request, deps: Parameters<typeof handleModelsList>[1]) =>
  Effect.runPromise(handleModelsList(request, deps))

describe('handleModelsList', () => {
  it('404s when the gateway is disabled (inert posture)', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: false },
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({ error: 'inference_gateway_disabled' })
  })

  it('405s on non-GET methods', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'POST' }),
      { enabled: true },
    )
    expect(response.status).toBe(405)
  })

  it('serves the OpenAI-compatible catalog when enabled', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true, nowEpochSeconds: () => 1_700_000_000 },
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as {
      object: string
      data: ReadonlyArray<{ id: string; created: number; object: string }>
    }
    expect(body.object).toBe('list')
    expect(body.data.length).toBe(MODEL_PRICING_TABLE.length)
    expect(body.data.every(m => m.object === 'model')).toBe(true)
    expect(body.data.every(m => m.created === 1_700_000_000)).toBe(true)
  })
})
