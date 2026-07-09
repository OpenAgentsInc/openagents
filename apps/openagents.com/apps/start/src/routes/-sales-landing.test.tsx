import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { SalesLandingPage } from './-sales-landing-page'
import {
  formatCount,
  fetchKhalaTokensServed,
  fetchKhalaCodePlans,
  LIVE_VALUE_PENDING,
} from './-sales-landing-data'

describe('WEB-1 sales-landing preview route', () => {
  test('server-renders the review-only sales landing scaffold', () => {
    const html = renderToStaticMarkup(<SalesLandingPage />)

    // Clearly-marked, review-only preview — never the live homepage.
    expect(html).toContain('data-route="sales-landing-preview"')
    expect(html).toContain('data-sales-landing-preview-banner=""')
    expect(html).toContain('proposed sales landing, not the live homepage')

    // Reused-approved hero copy (verbatim, not newly authored).
    expect(html).toContain('Software, built by agents.')

    // The two named CTAs are wired.
    expect(html).toContain('https://openagents.com/sarah')
    expect(html).toContain('/business#business-intake')
    expect(html).toContain('Talk to Sarah')

    // Live sections render their honest pending state under SSR (no fabricated
    // counters or prices before the client fetch resolves).
    expect(html).toContain('data-sales-landing-live-stats=""')
    expect(html).toContain('data-sales-landing-live-pricing=""')
    expect(html).toContain(LIVE_VALUE_PENDING)

    // Copy that is not yet approved is explicitly labeled, never faked.
    expect(html).toContain('TODO(owner-copy)')
  })

  test('formatCount shows the pending placeholder for missing values', () => {
    expect(formatCount(null)).toBe(LIVE_VALUE_PENDING)
    expect(formatCount(undefined)).toBe(LIVE_VALUE_PENDING)
    expect(formatCount(1234567)).toBe('1,234,567')
  })

  test('live fetchers fail soft to null on a non-ok response', async () => {
    const notOk = (async () =>
      new Response('nope', { status: 503 })) as unknown as typeof fetch
    expect(await fetchKhalaTokensServed(notOk)).toBeNull()
    expect(await fetchKhalaCodePlans(notOk)).toBeNull()
  })

  test('fetchKhalaTokensServed parses the tokensServed scalar', async () => {
    const ok = (async () =>
      new Response(
        JSON.stringify({
          schemaVersion: 'openagents.public_khala_tokens_served.v1',
          tokensServed: 42,
          generatedAt: '2026-07-08T00:00:00.000Z',
        }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      )) as unknown as typeof fetch
    const snapshot = await fetchKhalaTokensServed(ok)
    expect(snapshot?.tokensServed).toBe(42)
  })
})
