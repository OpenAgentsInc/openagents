import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { ArtanisAccountsPage } from './-artanis-accounts-page'

describe('Start Artanis operator account observability route', () => {
  test('server-renders the owner-only account dashboard shell', () => {
    const html = renderToStaticMarkup(<ArtanisAccountsPage />)

    expect(html).toContain('data-route="artanis-accounts"')
    expect(html).toContain('Operator account observability')
    expect(html).toContain(
      'Owner-only status for Codex and Claude coding accounts: live cooldowns, usage windows, and manual reset controls.',
    )
    expect(html).toContain('Unauthorized')
    expect(html).toContain('/api/operator/accounts/status')
    expect(html).toContain('/api/operator/accounts/reset')
    expect(html).toContain(
      'This surface is operator evidence and control only. It does not grant dispatch, spend, settlement, provider-account ownership transfer, or cross-owner routing authority.',
    )
  })

  test('shows an honest empty state instead of fabricated account rows', () => {
    const html = renderToStaticMarkup(<ArtanisAccountsPage />)

    expect(html).toContain('No operator account rows are available.')
    expect(html).not.toMatch(/rate-limited|accountRefHash|codex-acc-|claude-acc-/i)
  })
})
