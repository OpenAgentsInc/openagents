export const ADMIN_ANALYTICS_URL = '/api/admin/web-analytics'

export type AnalyticsWindow = '24h' | '7d' | '30d'

export type AnalyticsSummary = Readonly<{
  ok: true
  generatedAt: string
  window: AnalyticsWindow
  pageViews: number
  totalNamedEvents: number
  namedEvents: ReadonlyArray<{ name: string; count: number }>
  topPages: ReadonlyArray<{ routeId: string; pageViews: number }>
  daily: ReadonlyArray<{
    day: string
    pageViews: number
    namedEvents: number
  }>
}>

export type AnalyticsResult =
  | Readonly<{ tag: 'loaded'; summary: AnalyticsSummary }>
  | Readonly<{ tag: 'forbidden' }>
  | Readonly<{ tag: 'unauthorized' }>
  | Readonly<{ tag: 'failed'; status: number }>

export const fetchAnalyticsSummary = async (
  window: AnalyticsWindow,
  fetchFn: typeof fetch = fetch,
): Promise<AnalyticsResult> => {
  try {
    const response = await fetchFn(`${ADMIN_ANALYTICS_URL}?window=${window}`, {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (response.status === 401) return { tag: 'unauthorized' }
    if (response.status === 403) return { tag: 'forbidden' }
    if (!response.ok) return { tag: 'failed', status: response.status }
    return {
      tag: 'loaded',
      summary: (await response.json()) as AnalyticsSummary,
    }
  } catch {
    return { tag: 'failed', status: 0 }
  }
}
