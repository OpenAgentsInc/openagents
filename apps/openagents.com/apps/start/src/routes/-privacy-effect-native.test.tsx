import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  PRIVACY_LAST_UPDATED,
  PrivacyEffectNativePage,
  initialPrivacyLandingState,
  privacyLandingView,
} from './-privacy-effect-native-page'

describe('EN-4 /privacy Effect Native route', () => {
  test('server render is only a thin mount shim, not privacy-content React', () => {
    const html = renderToStaticMarkup(<PrivacyEffectNativePage />)

    expect(html).toContain('data-route="privacy"')
    expect(html).toContain('data-privacy-effect-native-root=""')
    // Legal body is client-mounted via Effect Native, not SSR'd as React content.
    // (aria-label may mention the page title; assert against document body copy only.)
    expect(html).not.toContain(PRIVACY_LAST_UPDATED)
    expect(html).not.toContain('1. Information We Collect')
    expect(html).not.toContain('10. Contact Us')
    expect(html).not.toContain('chris@openagents.com')
    expect(html).not.toContain('mailto:chris@openagents.com')
  })

  test('authored content is a typed Effect Native tree with key legal copy', () => {
    const tree = privacyLandingView(initialPrivacyLandingState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({
      tag: 'Stack',
      key: 'privacy-root',
    })
    expect(serialized).toContain('"catalogVersion":"effect-native/v39"')
    expect(serialized).toContain('Privacy Policy')
    expect(serialized).toContain(PRIVACY_LAST_UPDATED)
    expect(serialized).toContain(
      'This document is published so the policy is available now. The wording is being reviewed and may be updated.',
    )
    expect(serialized).toContain('1. Information We Collect')
    expect(serialized).toContain('2. How We Use Information')
    expect(serialized).toContain('3. How We Share Information')
    expect(serialized).toContain('4. Retention')
    expect(serialized).toContain('5. Cookies and Tracking')
    expect(serialized).toContain('6. Data Security')
    expect(serialized).toContain('7. Your Choices and Rights')
    expect(serialized).toContain('8. Links to Other Sites')
    expect(serialized).toContain('9. Changes to This Policy')
    expect(serialized).toContain('10. Contact Us')
    expect(serialized).toContain('https://openagents.com')
    expect(serialized).toContain('chris@openagents.com')
    expect(serialized).toContain('mailto:chris@openagents.com')
    expect(serialized).toContain('OpenAgents, Inc., 1101 W 34th St. #581, Austin, TX 78705')
    expect(serialized).toContain('Information you provide.')
    expect(serialized).toContain('API and agent data.')
    expect(serialized).toContain('Claim Your Agent')
    expect(serialized).toContain('Do Not Track')
    expect(serialized).not.toContain('className')
  })

  test('source boundary uses Effect Native packages instead of launch-ui / React content', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-privacy-effect-native-page.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).not.toContain('lucide-react')
    expect(source).not.toContain('@/components/ui/')
    expect(source).not.toContain('launch-ui')
    expect(source).not.toContain('-legal-components')
  })

  test('route shell mounts the Effect Native page only', () => {
    const routeSource = readFileSync(
      join(process.cwd(), 'src/routes/privacy.tsx'),
      'utf8',
    )

    expect(routeSource).toContain("from './-privacy-effect-native-page'")
    expect(routeSource).toContain('PrivacyEffectNativePage')
    expect(routeSource).not.toContain('-privacy-page')
  })
})
