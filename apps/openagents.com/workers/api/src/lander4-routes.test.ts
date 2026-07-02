import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleLander4Page, renderLander4Html } from './lander4-routes'
import { makeD1TokenUsageLedger } from './token-usage-ledger'

const fakeTokensServedDb = (tokensServed: number): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    first: <T>(): Promise<T> =>
      Promise.resolve({ tokens_served: tokensServed } as T),
  })
  return { prepare } as unknown as D1Database
}

describe('lander4 business landing page', () => {
  test('renders the business fold with live /business copy and the sales CTA', () => {
    const html = renderLander4Html(7_714_151_995)
    expect(html).toContain('Agents that work.')
    expect(html).toContain(
      'Hire agents from the OpenAgents network to get real work done',
    )
    expect(html).toContain('delivered with verifiable receipts')
    expect(html).toContain('a human-review gate sits before anything ships, sends, or spends')
    expect(html).toContain('>Talk to Sales</a>')
    expect(html).toContain('href="/business-new"')
    // v2 design system: house mono headline with the blue terminal period,
    // shared nav, promise-registry-style register instead of a card grid.
    expect(html).toContain('Agents that work<span class="mark">.</span>')
    expect(html).toContain('BerkeleyMono-Bold.woff2')
    expect(html).toContain('aria-label="Primary"')
    expect(html).toContain('business.coding_quick_win.v1')
    expect(html).toContain('class="chip live"')
    expect(html).toContain('href="/docs/product-promises"')
    expect(html).toContain('7,714,151,995')
    expect(html).toContain('name="robots" content="noindex"')
    // Speed architecture preserved: no app bundle, no lazy scene.
    expect(html).not.toContain('/assets/index-')
    expect(html).not.toContain('lander3-scene')
  })

  test('GET renders the ledger total; non-GET rejected', async () => {
    const response = await Effect.runPromise(
      handleLander4Page(new Request('https://openagents.com/lander4'), {
        ledger: makeD1TokenUsageLedger(fakeTokensServedDb(9_000_001)),
      }),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/html')
    expect(await response.text()).toContain('9,000,001')

    const rejected = await Effect.runPromise(
      handleLander4Page(
        new Request('https://openagents.com/lander4', { method: 'POST' }),
        { ledger: makeD1TokenUsageLedger(fakeTokensServedDb(1)) },
      ),
    )
    expect(rejected.status).toBe(405)
  })
})
