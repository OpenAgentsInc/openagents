import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleLander3Page, renderLander3Html } from './lander3-routes'
import { makeD1TokenUsageLedger } from './token-usage-ledger'

const fakeTokensServedDb = (tokensServed: number): D1Database => {
  const prepare = () => ({
    bind: () => prepare(),
    first: <T>(): Promise<T> =>
      Promise.resolve({ tokens_served: tokensServed } as T),
  })
  return { prepare } as unknown as D1Database
}

describe('lander3 server-rendered landing page with lazy hero scene', () => {
  test('keeps the lander2 speed architecture', () => {
    const html = renderLander3Html(7_714_151_995)
    expect(html).toContain('7,714,151,995')
    expect(html).toContain('Tokens Served:')
    expect(html).toContain('OpenAgents')
    expect(html).toContain('WHAT IS KHALA?')
    expect(html).toContain('JOIN THE TASSADAR TRAINING RUN')
    expect(html).toContain('class="backdrop"')
    expect(html).toContain('name="robots" content="noindex"')
    // Still no main-app bundle.
    expect(html).not.toContain('/assets/index-')
  })

  test('loads the hero scene lazily and fades it in', () => {
    const html = renderLander3Html(1)
    // The scene layer starts invisible and fades in on ready.
    expect(html).toContain('#scene{position:fixed;inset:0;opacity:0;transition:opacity 900ms ease}')
    expect(html).toContain('#scene.ready{opacity:1}')
    // The bundle is a dynamic import after load+idle — never a blocking tag.
    expect(html).toContain('import("/assets/lander3-scene.js")')
    expect(html).not.toContain('<script src="/assets/lander3-scene.js"')
    expect(html).not.toContain('rel="modulepreload"')
    expect(html).toContain('addEventListener("load",boot)')
    // Honest fallbacks: reduced motion and Save-Data keep the CSS grid.
    expect(html).toContain('prefers-reduced-motion: reduce')
    expect(html).toContain('saveData')
    // Fade only after the scene reported a rendered frame.
    expect(html).toContain('oa:hero:first-frame')
  })

  test('GET renders the ledger total; non-GET rejected', async () => {
    const response = await Effect.runPromise(
      handleLander3Page(new Request('https://openagents.com/lander3'), {
        ledger: makeD1TokenUsageLedger(fakeTokensServedDb(42_000_000)),
      }),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/html')
    expect(await response.text()).toContain('42,000,000')

    const rejected = await Effect.runPromise(
      handleLander3Page(
        new Request('https://openagents.com/lander3', { method: 'POST' }),
        { ledger: makeD1TokenUsageLedger(fakeTokensServedDb(1)) },
      ),
    )
    expect(rejected.status).toBe(405)
  })
})
