import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  DISCOVERY_SURFACE_PATHS,
  renderDiscoverySurface,
} from './discovery-surfaces'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const get = (path: string): Request =>
  new Request(`https://openagents.com${path}`, { method: 'GET' })

describe('discovery surfaces (EPIC #6049 Phase 1 — agent discovery)', () => {
  test('serves all four surfaces with the right content type', async () => {
    for (const path of DISCOVERY_SURFACE_PATHS) {
      const response = await run(renderDiscoverySurface(get(path), path))
      expect(response.status).toBe(200)
      const contentType = response.headers.get('content-type') ?? ''
      if (path === '/llms.txt') {
        expect(contentType).toContain('text/plain')
      } else {
        expect(contentType).toContain('text/markdown')
      }
      const body = await response.text()
      expect(body.length).toBeGreaterThan(0)
    }
  })

  test('llms.txt describes OpenAgents inference honestly: OpenAI-compatible, pay-per-call, the models', async () => {
    const response = await run(renderDiscoverySurface(get('/llms.txt'), '/llms.txt'))
    const body = await response.text()
    expect(body).toContain('OpenAI-compatible')
    expect(body).toContain('pay-per-call')
    expect(body).toContain('openai/gpt-oss-20b')
    expect(body).toContain('openagents/khala-mini')
    expect(body).toContain('openagents/khala-code')
    expect(body).toContain('/v1/chat/completions')
    expect(body).toContain('/mpp/v1/chat/completions')
    expect(body).toContain('Lightning')
    expect(body.toLowerCase()).toContain('contributor payout')
  })

  test('is crawlable: public + cacheable, no auth, no robots block', async () => {
    const response = await run(renderDiscoverySurface(get('/agents.md'), '/agents.md'))
    expect(response.headers.get('cache-control')).toContain('public')
    // No www-authenticate / auth gate.
    expect(response.headers.get('www-authenticate')).toBeNull()
  })

  test('agents.md documents the 402 machine-payment flow + the rails', async () => {
    const response = await run(renderDiscoverySurface(get('/agents.md'), '/agents.md'))
    const body = await response.text()
    expect(body).toContain('402 Payment Required')
    expect(body).toContain('WWW-Authenticate')
    expect(body).toContain('USDC')
    expect(body).toContain('Bitcoin/Spark')
  })

  test('rejects non-GET/HEAD with 405', async () => {
    const response = await run(
      renderDiscoverySurface(
        new Request('https://openagents.com/llms.txt', { method: 'POST' }),
        '/llms.txt',
      ),
    )
    expect(response.status).toBe(405)
  })
})
