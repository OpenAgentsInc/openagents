import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleBusinessAgentGuide,
  handleBusinessNewPage,
  renderBusinessNewHtml,
} from './business-new-routes'
import { makeD1TokenUsageLedger } from './token-usage-ledger'

const fakeTokensServedDb = (tokensServed: number): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    first: <T>(): Promise<T> =>
      Promise.resolve({ tokens_served: tokensServed } as T),
  })
  return { prepare } as unknown as D1Database
}

describe('business-new restructured page', () => {
  test('renders the business register and a real intake form', () => {
    const html = renderBusinessNewHtml(7_714_151_995)
    expect(html).toContain('Agents that work<span class="mark">.</span>')
    expect(html).toContain('Coding &amp; agent work')
    expect(html).toContain('Inference / AI on tap')
    expect(html).toContain('business.coding_quick_win.v1')
    expect(html).toContain('action="/api/public/business-signup"')
    expect(html).toContain('name="businessName"')
    expect(html).toContain('name="contactEmail"')
    expect(html).toContain('name="phone"')
    expect(html).toContain('name="helpWith"')
    expect(html).toContain('name="requestSlackChannel"')
    // Shared nav marks Business as the current page; Khala conversation stays a path.
    expect(html).toContain('href="/business" aria-current="page"')
    expect(html).toContain('href="/khala"')
    expect(html).toContain('rel="canonical" href="https://openagents.com/business"')
    expect(html).toContain('href="/business/agents.md"')
    expect(html).toContain('application/ld+json')
    expect(html).toContain('Questions buyers and agents ask')
    expect(html).toContain('sourceRef=ai_search')
    expect(html).toContain('name="sourceRef"')
    expect(html).not.toContain('name="robots" content="noindex"')
    expect(html).not.toContain('/assets/index-')
  })

  test('GET renders the ledger total and preserves AI-search source attribution; non-GET rejected', async () => {
    const response = await Effect.runPromise(
      handleBusinessNewPage(
        new Request('https://openagents.com/business?source=ai-search'),
        {
          ledger: makeD1TokenUsageLedger(fakeTokensServedDb(5_500_000)),
        },
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('x-lander-edge-cache')).toBe('bypass')
    const body = await response.text()
    expect(body).toContain('5,500,000')
    expect(body).toContain(
      'id="business-source-ref" name="sourceRef" value="ai_search"',
    )

    const rejected = await Effect.runPromise(
      handleBusinessNewPage(
        new Request('https://openagents.com/business-new', { method: 'POST' }),
        { ledger: makeD1TokenUsageLedger(fakeTokensServedDb(1)) },
      ),
    )
    expect(rejected.status).toBe(405)
  })

  test('serves a crawlable agent-readable business guide', async () => {
    const response = await Effect.runPromise(
      handleBusinessAgentGuide(
        new Request('https://openagents.com/business/agents.md'),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/markdown')
    expect(response.headers.get('cache-control')).toContain('public')

    const body = await response.text()
    expect(body).toContain('# OpenAgents Business')
    expect(body).toContain('https://openagents.com/business?sourceRef=ai_search')
    expect(body).toContain('operator-assisted')
    expect(body).toContain('sourceRef=ai_search')
    expect(body).toContain('/api/public/business/funnel-dashboard')

    const rejected = await Effect.runPromise(
      handleBusinessAgentGuide(
        new Request('https://openagents.com/business/agents.md', {
          method: 'POST',
        }),
      ),
    )
    expect(rejected.status).toBe(405)
  })
})
