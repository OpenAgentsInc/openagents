import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { handleObserverPage, renderObserverHtml } from './observer-routes'

const req = (init: RequestInit = {}) =>
  new Request('https://openagents.com/observer', init)

describe('/observer landing page', () => {
  test('renders the Observer product document', () => {
    const html = renderObserverHtml()
    expect(html).toContain('<title>Observer — OpenAgents</title>')
    expect(html).toContain('Proof-design for software built by agents.')
    expect(html).toContain('AssuranceSpec')
    expect(html).toContain('A link is not a verdict.')
    // The real dogfood subject binding quoted from docs/assurance/README.md —
    // exact ProductSpec digest, not a mock.
    expect(html).toContain(
      'fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1',
    )
    expect(html).toContain('CW-AC-01')
    // The admitted dogfood result is current while broader adapters stay planned.
    expect(html).toContain('revision 2 · reviewed and admitted')
    expect(html).toContain('18 CONFIRMED · 18 falsifiers REFUTED')
    expect(html).not.toContain('all still marked needs_design')
    expect(html).toContain('PLANNED')
    for (const identifier of [
      'false_green_fixture_assert',
      'false_green_api_mirror',
      'false_green_mocked_seam',
      'false_green_coverage_theater',
      'false_green_round_up',
    ]) {
      expect(html).toContain(identifier)
    }
    // Receipt verdict vocabulary from ASSURANCE_SPEC.md.
    for (const verdict of ['CONFIRMED', 'REFUTED', 'INCONCLUSIVE']) {
      expect(html).toContain(verdict)
    }
    // No public-claim overreach: the page routes claims to the registry.
    expect(html).toContain('product-promise registry')
  })

  test('GET serves HTML with no-store', async () => {
    const response = await Effect.runPromise(handleObserverPage(req()))
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/html')
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(await response.text()).toContain('Observer')
  })

  test('HEAD answers 200 with an empty body', async () => {
    const response = await Effect.runPromise(
      handleObserverPage(req({ method: 'HEAD' })),
    )
    expect(response.status).toBe(200)
    expect(await response.text()).toBe('')
  })

  test('non-GET/HEAD methods are rejected with 405', async () => {
    const response = await Effect.runPromise(
      handleObserverPage(req({ method: 'POST' })),
    )
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET, HEAD')
  })
})
