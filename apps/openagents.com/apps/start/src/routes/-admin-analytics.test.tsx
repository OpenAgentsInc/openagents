import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  showsProductLogout,
  usesPersistentProductHeader,
} from '../lib/persistent-product-header'
import { isKnownStartDocumentPath } from '../route-table'
import { fetchAnalyticsSummary } from './-admin-analytics-fetch'
import {
  ADMIN_ANALYTICS_LOGIN_HREF,
  AnalyticsDashboard,
  analyticsAuthRedirect,
} from './-admin-analytics-page'

const summary = {
  ok: true as const,
  generatedAt: '2026-07-25T18:00:00.000Z',
  window: '7d' as const,
  pageViews: 42,
  totalNamedEvents: 5,
  namedEvents: [{ name: 'github_view', count: 5 }],
  topPages: [{ routeId: '/', pageViews: 32 }],
  daily: [{ day: '2026-07-25', pageViews: 42, namedEvents: 5 }],
}

describe('admin analytics', () => {
  test('renders aggregate page and conversion counts', () => {
    const html = renderToStaticMarkup(<AnalyticsDashboard summary={summary} />)
    expect(html).toContain('Website analytics')
    expect(html).toContain('Page views')
    expect(html).toContain('github_view')
    expect(html).toContain('42 views · 5 events')
  })

  test('preserves admin authorization responses', async () => {
    const unauthorized = await fetchAnalyticsSummary(
      '7d',
      async () => new Response(null, { status: 401 }),
    )
    const forbidden = await fetchAnalyticsSummary(
      '7d',
      async () => new Response(null, { status: 403 }),
    )
    expect(unauthorized.tag).toBe('unauthorized')
    expect(forbidden.tag).toBe('forbidden')
    expect(analyticsAuthRedirect(unauthorized)).toBe(ADMIN_ANALYTICS_LOGIN_HREF)
    expect(analyticsAuthRedirect(forbidden)).toBeUndefined()
  })

  test('serves the protected dashboard through the Start document route', () => {
    expect(isKnownStartDocumentPath('/admin/analytics')).toBe(true)
    expect(usesPersistentProductHeader('/')).toBe(true)
    expect(usesPersistentProductHeader('/splash')).toBe(true)
    expect(usesPersistentProductHeader('/admin/analytics')).toBe(true)
    expect(usesPersistentProductHeader('/login')).toBe(false)
    expect(showsProductLogout('/admin/analytics')).toBe(true)
    expect(showsProductLogout('/')).toBe(false)
    expect(showsProductLogout('/splash')).toBe(false)
  })
})
