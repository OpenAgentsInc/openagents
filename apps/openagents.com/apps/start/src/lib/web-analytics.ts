import {
  ANALYTICS_SCHEMA_VERSION,
  AnalyticsClient,
  type AnalyticsEventBatch,
  type AnalyticsEventName,
  AnalyticsTransportError,
  makeAnalyticsClientLayer,
} from '@openagentsinc/analytics'
import { Effect, ManagedRuntime } from 'effect'

const INGEST_URL = '/api/analytics/events'
const FLUSH_INTERVAL_MS = 5_000
const analyticsEnabled =
  import.meta.env.DEV || import.meta.env.VITE_WEB_ANALYTICS_ENABLED === 'true'

const webAnalyticsRuntime = ManagedRuntime.make(
  makeAnalyticsClientLayer(
    {
      send: (batch: AnalyticsEventBatch) =>
        Effect.tryPromise({
          try: async () => {
            const body = JSON.stringify(batch)
            if (
              typeof document !== 'undefined' &&
              document.visibilityState === 'hidden' &&
              typeof navigator !== 'undefined' &&
              typeof navigator.sendBeacon === 'function' &&
              navigator.sendBeacon(
                INGEST_URL,
                new Blob([body], { type: 'application/json' }),
              )
            ) {
              return
            }

            const response = await fetch(INGEST_URL, {
              method: 'POST',
              body,
              cache: 'no-store',
              credentials: 'omit',
              headers: {
                'content-type': 'application/json',
              },
              keepalive: true,
            })
            if (!response.ok) {
              throw new Error(`analytics ingest returned ${response.status}`)
            }
          },
          catch: cause => new AnalyticsTransportError({ cause }),
        }),
    },
    {
      nextId: () =>
        Effect.sync(() =>
          typeof crypto !== 'undefined' &&
          typeof crypto.randomUUID === 'function'
            ? crypto.randomUUID()
            : `event.web.${Date.now().toString(36)}.${Math.random().toString(36).slice(2)}`,
        ),
      nowIso: () => Effect.sync(() => new Date().toISOString()),
    },
    {
      client: 'web',
      maxBatchSize: 10,
      maxBufferedEvents: 100,
    },
  ),
)

const runFailSoft = (
  effect: Effect.Effect<unknown, never, AnalyticsClient>,
) => {
  if (!analyticsEnabled) return
  webAnalyticsRuntime.runFork(effect)
}

export const canonicalWebRoute = (pathname: string): string => {
  if (pathname === '/') return '/'
  if (pathname === '/splash') return '/splash'
  if (pathname === '/login') return '/login'
  if (pathname === '/blog' || pathname === '/blog/') return '/blog'
  if (pathname.startsWith('/blog/')) return '/blog/:slug'
  if (pathname === '/docs' || pathname === '/docs/') return '/docs'
  if (pathname.startsWith('/docs/')) return '/docs/:slug'
  if (pathname === '/admin/analytics') return '/admin/analytics'
  if (pathname.startsWith('/forum')) return '/forum'
  if (pathname.startsWith('/trace/')) return '/trace/:id'
  if (pathname.startsWith('/promises')) return '/promises'
  return '/other'
}

const record = (name: AnalyticsEventName, pathname: string): void => {
  const routeId = canonicalWebRoute(pathname)
  runFailSoft(
    Effect.flatMap(AnalyticsClient, client => client.track(name, routeId)),
  )
}

export const trackPageView = (pathname: string): void => {
  record('page_view', pathname)
}

export const trackNamedAnalyticsEvent = (
  name: Exclude<AnalyticsEventName, 'page_view'>,
  pathname: string,
): void => {
  record(name, pathname)
}

const flush = (): void => {
  runFailSoft(Effect.flatMap(AnalyticsClient, client => client.flush()))
}

export const startWebAnalytics = (): (() => void) => {
  if (
    !analyticsEnabled ||
    typeof window === 'undefined' ||
    typeof document === 'undefined'
  ) {
    return () => undefined
  }

  const interval = window.setInterval(flush, FLUSH_INTERVAL_MS)
  const onPageHide = () => flush()
  const onVisibilityChange = () => {
    if (document.visibilityState === 'hidden') flush()
  }
  window.addEventListener('pagehide', onPageHide)
  document.addEventListener('visibilitychange', onVisibilityChange)

  return () => {
    window.clearInterval(interval)
    window.removeEventListener('pagehide', onPageHide)
    document.removeEventListener('visibilitychange', onVisibilityChange)
    flush()
  }
}

export const WEB_ANALYTICS_SCHEMA_VERSION = ANALYTICS_SCHEMA_VERSION
