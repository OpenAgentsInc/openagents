import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { LandingPreviewPage } from './-landing-preview-page'

describe('Start landing preview route', () => {
  test('server-renders the review-only two-door landing candidate', () => {
    const html = renderToStaticMarkup(<LandingPreviewPage />)

    expect(html).toContain('data-route="landing-preview"')
    expect(html).toContain('data-landing-preview=""')
    expect(html).toContain('preview - proposed landing page, not the live homepage')
    expect(html).toContain('Software, built by agents.')
    expect(html).toContain('Build it myself')
    expect(html).toContain('Build it for me')
    expect(html).toContain('100% open source')
    expect(html).toContain('OpenAI-compatible free API')
    expect(html).toContain('Human-review gate before publish/send/spend')
    expect(html).toContain('every claim: /docs/product-promises')
    expect(html).toContain('href="/khala"')
    expect(html).toContain('href="/business"')
  })
})
