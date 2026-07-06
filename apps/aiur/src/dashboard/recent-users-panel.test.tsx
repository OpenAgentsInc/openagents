import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { RecentUsersPanel } from './recent-users-panel'

describe('RecentUsersPanel (initial render smoke)', () => {
  test('renders the panel shell without throwing', () => {
    const html = renderToStaticMarkup(<RecentUsersPanel />)

    expect(html).toContain('Recent signups')
    expect(html).toContain('data-testid="recent-users-panel"')
    expect(html).toContain('data-testid="recent-users-list"')
    // Initial render (before the fetch resolves) is the loading state.
    expect(html).toContain('Loading...')
  })
})
