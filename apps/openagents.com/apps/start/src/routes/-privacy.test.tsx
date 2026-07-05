import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { PRIVACY_LAST_UPDATED, PrivacyPage } from './-privacy-page'

describe('Start privacy route', () => {
  test('server-renders the Privacy Policy document', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    expect(html).toContain('data-route="privacy"')
    expect(html).toContain('Privacy Policy')
    expect(html).toContain(PRIVACY_LAST_UPDATED)
    expect(html).toContain(
      'This document is published so the policy is available now.',
    )
    expect(html).toContain('1. Information We Collect')
    expect(html).toContain('10. Contact Us')
    expect(html).toContain('mailto:chris@openagents.com')
    expect(html).toContain('href="/"')
  })

  test('keeps private/unsafe fields out of the rendered shell', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    expect(html).not.toContain('sk-')
    expect(html).not.toContain('mnemonic')
  })
})
