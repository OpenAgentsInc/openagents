import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleLander2Page, renderLander2Html } from './lander2-routes'
import { makeD1TokenUsageLedger } from './token-usage-ledger'

// Same fake-D1 pattern as public-khala-tokens-served-routes.test.ts: answers
// only the tokens-served SUM query, proving the route reads the canonical
// ledger path.
const fakeTokensServedDb = (tokensServed: number): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    first: <T>(): Promise<T> =>
      Promise.resolve({ tokens_served: tokensServed } as T),
  })
  return { prepare } as unknown as D1Database
}

describe('lander2 server-rendered landing page', () => {
  test('renders the token total inline with existing landing copy', () => {
    const html = renderLander2Html(7_714_151_995)
    expect(html).toContain('7,714,151,995')
    expect(html).toContain('Tokens Served:')
    expect(html).toContain('OpenAgents')
    expect(html).toContain('WHAT IS KHALA?')
    expect(html).toContain('JOIN THE TASSADAR TRAINING RUN')
    expect(html).toContain('href="/stats"')
    expect(html).toContain('href="/khala"')
    expect(html).toContain('href="/tassadar"')
    // Experiment surface stays unlisted.
    expect(html).toContain('name="robots" content="noindex"')
    // No bundle: the only script is the inline counter refresher.
    expect(html).not.toContain('/assets/index-')
  })

  test('GET returns text/html with the ledger total server-rendered', async () => {
    const response = await Effect.runPromise(
      handleLander2Page(new Request('https://openagents.com/lander2'), {
        ledger: makeD1TokenUsageLedger(fakeTokensServedDb(1_234_567)),
      }),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/html')
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('server-timing')).toContain('d1;dur=')
    const body = await response.text()
    expect(body).toContain('1,234,567')
  })

  test('non-GET methods are rejected', async () => {
    const response = await Effect.runPromise(
      handleLander2Page(
        new Request('https://openagents.com/lander2', { method: 'POST' }),
        { ledger: makeD1TokenUsageLedger(fakeTokensServedDb(1)) },
      ),
    )
    expect(response.status).toBe(405)
  })
})
