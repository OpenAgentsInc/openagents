import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { routeSiteCrawlSurfaceRequest } from './site-crawl-surfaces-routes'

const run = (path: string, init?: RequestInit): Promise<Response> => {
  const request = new Request(`https://openagents.com${path}`, init)
  const effect = routeSiteCrawlSurfaceRequest(request)
  if (effect === undefined) throw new Error(`route did not match: ${path}`)
  return Effect.runPromise(effect)
}

describe('site crawl-discovery surfaces', () => {
  test('other paths pass through', () => {
    expect(
      routeSiteCrawlSurfaceRequest(new Request('https://openagents.com/')),
    ).toBeUndefined()
  })

  test('GET /robots.txt allows everything and points at the sitemap', async () => {
    const res = await run('/robots.txt')
    expect(res.status).toBe(200)
    expect(res.headers.get('content-type')).toContain('text/plain')
    const body = await res.text()
    expect(body).toContain('User-agent: *')
    expect(body).toContain('Allow: /')
    expect(body).toContain('Sitemap: https://openagents.com/sitemap.xml')
  })

  test('GET /sitemap.xml lists the developer-facing surfaces', async () => {
    const res = await run('/sitemap.xml')
    expect(res.status).toBe(200)
    expect(res.headers.get('content-type')).toContain('application/xml')
    const body = await res.text()
    expect(body).toContain('<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">')
    expect(body).toContain('<loc>https://openagents.com/AGENTS.md</loc>')
    expect(body).toContain('<loc>https://openagents.com/.well-known/mcp.json</loc>')
    expect(body).toContain('<loc>https://openagents.com/.well-known/ai-catalog.json</loc>')
  })

  test('POST is method-not-allowed', async () => {
    const res = await run('/robots.txt', { method: 'POST' })
    expect(res.status).toBe(405)
  })
})
