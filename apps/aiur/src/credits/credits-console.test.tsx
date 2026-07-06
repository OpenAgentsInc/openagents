import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { CreditsConsole } from './credits-console'

describe('CreditsConsole (initial render smoke)', () => {
  test('renders the search box and the recent-grants ledger without a selected user', () => {
    const html = renderToStaticMarkup(<CreditsConsole />)

    expect(html).toContain('Find a user')
    expect(html).toContain('data-testid="credits-search-input"')
    expect(html).toContain('Recent grants')
    // No target selected yet — the grant/clawback forms must not render.
    expect(html).not.toContain('data-testid="grant-amount-input"')
    expect(html).not.toContain('data-testid="clawback-amount-input"')
  })
})
