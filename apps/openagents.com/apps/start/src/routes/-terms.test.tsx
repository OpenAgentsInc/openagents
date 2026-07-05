import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { TERMS_LAST_UPDATED, TermsPage } from './-terms-page'

describe('Start terms route', () => {
  test('server-renders the Terms of Service document', () => {
    const html = renderToStaticMarkup(<TermsPage />)

    expect(html).toContain('data-route="terms"')
    expect(html).toContain('Terms of Service')
    expect(html).toContain(TERMS_LAST_UPDATED)
    expect(html).toContain(
      'This document is published so the terms are available now.',
    )
    expect(html).toContain('1. The OpenAgents Platform')
    expect(html).toContain('12. Contact')
    expect(html).toContain('mailto:chris@openagents.com')
    expect(html).toContain('href="/"')
  })

  test('keeps private/unsafe fields out of the rendered shell', () => {
    const html = renderToStaticMarkup(<TermsPage />)

    expect(html).not.toContain('sk-')
    expect(html).not.toContain('mnemonic')
  })
})
