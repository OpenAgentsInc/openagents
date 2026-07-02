import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleLander5Page, renderLander5Html } from './lander5-routes'
import { makeD1TokenUsageLedger } from './token-usage-ledger'

const fakeTokensServedDb = (tokensServed: number): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    first: <T>(): Promise<T> =>
      Promise.resolve({ tokens_served: tokensServed } as T),
  })
  return { prepare } as unknown as D1Database
}

describe('lander5 dimmed lazy-hero business page', () => {
  test('combines the business fold with the scrimmed lazy scene', () => {
    const html = renderLander5Html(7_714_151_995)
    expect(html).toContain('Agents that work<span class="mark">.</span>')
    expect(html).toContain('>Talk to Sales</a>')
    // The scene loads lazily and fades in behind a ~90% scrim.
    expect(html).toContain('import("/assets/lander3-scene.js")')
    expect(html).toContain('rgba(7,10,15,0.9)')
    expect(html).toContain('#scene.ready{opacity:1}')
    expect(html).toContain('addEventListener("load",boot)')
    expect(html).toContain('prefers-reduced-motion: reduce')
    expect(html).not.toContain('/assets/index-')
  })

  test('GET renders the ledger total; non-GET rejected', async () => {
    const response = await Effect.runPromise(
      handleLander5Page(new Request('https://openagents.com/lander5'), {
        ledger: makeD1TokenUsageLedger(fakeTokensServedDb(123_456_789)),
      }),
    )
    expect(response.status).toBe(200)
    expect(await response.text()).toContain('123,456,789')

    const rejected = await Effect.runPromise(
      handleLander5Page(
        new Request('https://openagents.com/lander5', { method: 'POST' }),
        { ledger: makeD1TokenUsageLedger(fakeTokensServedDb(1)) },
      ),
    )
    expect(rejected.status).toBe(405)
  })
})
