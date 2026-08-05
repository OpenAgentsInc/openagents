import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { PRIVACY_LAST_UPDATED, PrivacyPage } from './-privacy-page'

describe('Start /privacy route', () => {
  test('server-renders the whole Privacy Policy document, not a mount shim', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    expect(html).toContain('data-route="privacy"')
    expect(html).toContain('aria-label="Privacy Policy"')
    expect(html).toContain('aria-label="Primary navigation"')
    expect(html).toContain('© 2026 OpenAgents, Inc.')
    expect(html).toContain('<h1')
    expect(html).toContain('Privacy Policy')
    expect(html).toContain(PRIVACY_LAST_UPDATED)
  })

  test('renders every numbered section as a heading, in document order', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    const headings = [
      '1. Information We Collect',
      '2. How We Use Information',
      '3. How We Share Information',
      '4. Retention',
      '5. Cookies and Tracking',
      '6. Data Security',
      '7. Your Choices and Rights',
      '8. Links to Other Sites',
      '9. Changes to This Policy',
      '10. Contact Us',
    ]

    let cursor = -1
    for (const heading of headings) {
      const index = html.indexOf(`>${heading}</h2>`)
      expect(index, `${heading} renders as an <h2>`).toBeGreaterThan(cursor)
      cursor = index
    }
  })

  test('keeps the verbatim legal copy the policy is made of', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    expect(html).toContain(
      'This Privacy Policy describes how OpenAgents, Inc. (“we,” “us,” or “our”) handles personal information that we collect through our website',
    )
    expect(html).toContain('Information you provide.')
    expect(html).toContain('Information collected automatically.')
    expect(html).toContain('API and agent data.')
    expect(html).toContain('Claim Your Agent')
    expect(html).toContain('Do Not Track')
    expect(html).toContain(
      'OpenAgents, Inc., 1101 W 34th St. #581, Austin, TX 78705.',
    )
    // Dropped when the interim React page was replaced; it must stay dropped.
    expect(html).not.toContain('wording is being reviewed')
  })

  test('rich-inline runs render inside their sentence, not as sibling blocks', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    expect(html).toMatch(
      /our website <a [^>]*href="https:\/\/openagents\.com">https:\/\/openagents\.com<\/a>, our APIs and inference gateway/,
    )
    expect(html).toMatch(
      /contact us at <a [^>]*href="mailto:chris@openagents\.com">chris@openagents\.com<\/a>, or by mail at/,
    )
    expect(html).toMatch(
      /<span [^>]*>Information you provide\.<\/span> We collect information you give us, such as:/,
    )
  })

  test('bullet lists render as real list items', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    expect(html).toContain(
      '<li>Account and contact information — your name, email address, and the username and password or credentials you use to access the Services, including via third-party login (such as GitHub).</li>',
    )
    expect(html).toContain(
      '<li>Depending on your location, you may have additional rights under applicable privacy laws; contact us to exercise them.</li>',
    )
  })

  test('keeps private/unsafe fields out of the rendered document', () => {
    const html = renderToStaticMarkup(<PrivacyPage />)

    expect(html).not.toContain('sk-')
    expect(html).not.toContain('mnemonic')
  })

  test('the page module carries no Effect Native dependency', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-privacy-page.tsx'),
      'utf8',
    )

    expect(source).not.toContain('@effect-native')
    expect(source).toContain("from './-legal-components'")
    // The copy is still unreviewed legal text; the notice must stay in source.
    expect(source).toContain('PENDING OWNER / LEGAL REVIEW')
  })

  test('the route shell mounts the plain-React privacy page', () => {
    const routeSource = readFileSync(
      join(process.cwd(), 'src/routes/privacy.tsx'),
      'utf8',
    )

    expect(routeSource).toContain("from './-privacy-page'")
    expect(routeSource).toContain('PrivacyPage')
    expect(routeSource).not.toContain('effect-native')
  })
})
