import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { OpsConsole } from './ops-console'

describe('OpsConsole (initial render smoke)', () => {
  test('renders the health strip, users panel, runs panel, and daily sales ledger panel shells without throwing', () => {
    const html = renderToStaticMarkup(<OpsConsole />)

    expect(html).toContain('Health strip')
    expect(html).toContain('Recent signups')
    expect(html).toContain('Recent org-cloud runs')
    expect(html).toContain('Daily sales ledger')
    expect(html).toContain('data-testid="recent-users-list"')
    expect(html).toContain('data-testid="ops-runs-list"')
  })
})
