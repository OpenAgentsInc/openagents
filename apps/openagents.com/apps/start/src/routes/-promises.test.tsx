import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { PromisesPage } from './-promises-page'

describe('Start /promises route', () => {
  test('server-renders the route contract and hero copy', () => {
    const html = renderToStaticMarkup(<PromisesPage />)

    expect(html).toContain('data-route="promises"')
    expect(html).toContain('Product promises')
    expect(html).toContain(
      'A visual map of what OpenAgents says it does, what is live, what is gated, and what should be reported when reality does not match the claim.',
    )
  })

  test('preserves the nav and JSON/docs/forum links', () => {
    const html = renderToStaticMarkup(<PromisesPage />)

    expect(html).toContain('href="/docs/product-promises"')
    expect(html).toContain('href="/api/public/product-promises"')
    expect(html).toContain('href="/forum/f/product-promises"')
  })

  test('renders the honest idle state instead of fabricated registry rows', () => {
    const html = renderToStaticMarkup(<PromisesPage />)

    expect(html).toContain('Waiting for live registry.')
    expect(html).toContain('Waiting for /api/public/product-promises.')
    expect(html).toContain('None listed.')
  })

  test('preserves the claim-upgrade audit panel heading and rule', () => {
    const html = renderToStaticMarkup(<PromisesPage />)

    expect(html).toContain('proof.claim_upgrade_receipts.v1')
    expect(html).toContain('Claim-upgrade audit panel')
    expect(html).toContain(
      'A passing receipt is evidence for a flip, never the flip itself.',
    )
    expect(html).toContain('href="/api/public/product-promises/transitions"')
    expect(html).toContain('href="/api/public/product-promises/audit"')
  })
})
