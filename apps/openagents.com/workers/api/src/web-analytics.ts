import {
  ANALYTICS_SCHEMA_VERSION,
  type AnalyticsEvent,
  AnalyticsEventBatch,
} from '@openagentsinc/analytics'
import { Context, Effect, Layer, Schema as S } from 'effect'

import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'

export const WEB_ANALYTICS_INGEST_PATH = '/api/analytics/events'
export const WEB_ANALYTICS_ADMIN_PATH = '/api/admin/web-analytics'
export const WEB_ANALYTICS_MAX_BODY_BYTES = 8 * 1024
export const WEB_ANALYTICS_RETENTION_DAYS = 30

type AdminSession = Readonly<{
  user: Readonly<{ email: string }>
}>

export type WebAnalyticsBindings = Readonly<{
  WEB_ANALYTICS_ENABLED?: string | undefined
}>

export type WebAnalyticsRouteDependencies<
  Session extends AdminSession,
  Bindings extends WebAnalyticsBindings,
> = Readonly<{
  db: (env: Bindings) => D1Database
  isOpenAgentsAdminEmail: (email: string) => boolean
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  appendRefreshedSessionCookies: (
    response: Response,
    session: Session,
  ) => Response
  nowIso?: () => string
}>

export type WebAnalyticsWindow = '24h' | '7d' | '30d'

export type WebAnalyticsSummary = Readonly<{
  ok: true
  schemaVersion: 'openagents.analytics.summary.v1'
  generatedAt: string
  window: WebAnalyticsWindow
  pageViews: number
  totalNamedEvents: number
  namedEvents: ReadonlyArray<{
    name: string
    count: number
  }>
  topPages: ReadonlyArray<{
    routeId: string
    pageViews: number
  }>
  daily: ReadonlyArray<{
    day: string
    pageViews: number
    namedEvents: number
  }>
}>

export class WebAnalyticsPersistenceError extends S.TaggedErrorClass<WebAnalyticsPersistenceError>()(
  'WebAnalyticsPersistenceError',
  {
    operation: S.String,
    cause: S.Defect(),
  },
) {}

export type WebAnalyticsConfigurationShape = Readonly<{
  enabled: boolean
  retentionDays: number
}>

export class WebAnalyticsConfiguration extends Context.Service<
  WebAnalyticsConfiguration,
  WebAnalyticsConfigurationShape
>()('@openagentsinc/web-analytics/Configuration') {}

export type WebAnalyticsPersistenceShape = Readonly<{
  store: (
    events: ReadonlyArray<AnalyticsEvent>,
    receivedAt: string,
  ) => Effect.Effect<void, WebAnalyticsPersistenceError>
  prune: (
    cutoffIso: string,
  ) => Effect.Effect<void, WebAnalyticsPersistenceError>
  readSummary: (
    sinceIso: string,
    window: WebAnalyticsWindow,
    generatedAt: string,
  ) => Effect.Effect<WebAnalyticsSummary, WebAnalyticsPersistenceError>
}>

export class WebAnalyticsPersistence extends Context.Service<
  WebAnalyticsPersistence,
  WebAnalyticsPersistenceShape
>()('@openagentsinc/web-analytics/Persistence') {}

export type WebAnalyticsIngestionShape = Readonly<{
  ingest: (
    payload: unknown,
    receivedAt: string,
  ) => Effect.Effect<void, WebAnalyticsPersistenceError | S.SchemaError>
}>

export class WebAnalyticsIngestion extends Context.Service<
  WebAnalyticsIngestion,
  WebAnalyticsIngestionShape
>()('@openagentsinc/web-analytics/Ingestion') {}

export type WebAnalyticsRollupsShape = Readonly<{
  summary: (
    window: WebAnalyticsWindow,
    generatedAt: string,
  ) => Effect.Effect<WebAnalyticsSummary, WebAnalyticsPersistenceError>
}>

export class WebAnalyticsRollups extends Context.Service<
  WebAnalyticsRollups,
  WebAnalyticsRollupsShape
>()('@openagentsinc/web-analytics/Rollups') {}

const isEnabled = (value: string | undefined): boolean =>
  value === '1' || value === 'true' || value === 'on'

export const webAnalyticsConfigurationLayer = (
  enabledValue: string | undefined,
) =>
  Layer.succeed(WebAnalyticsConfiguration, {
    enabled: isEnabled(enabledValue),
    retentionDays: WEB_ANALYTICS_RETENTION_DAYS,
  })

type CountRow = Readonly<{ count: number | string }>
type NamedEventRow = Readonly<{ event_name: string; count: number | string }>
type TopPageRow = Readonly<{ route_id: string; page_views: number | string }>
type DailyRow = Readonly<{
  day: string
  page_views: number | string
  named_events: number | string
}>

const asNumber = (value: number | string | undefined): number => {
  const parsed = Number(value ?? 0)
  return Number.isFinite(parsed) ? parsed : 0
}

export const webAnalyticsPersistenceLayer = (db: D1Database) =>
  Layer.succeed(WebAnalyticsPersistence, {
    store: (events, receivedAt) =>
      Effect.tryPromise({
        try: async () => {
          if (events.length === 0) return
          await db.batch(
            events.map(event =>
              db
                .prepare(
                  `INSERT INTO web_analytics_events (
                     event_id, event_name, client_kind, route_id,
                     occurred_at, received_at, schema_version
                   ) VALUES (?, ?, ?, ?, ?, ?, ?)
                   ON CONFLICT (event_id) DO NOTHING`,
                )
                .bind(
                  event.eventId,
                  event.name,
                  event.client,
                  event.routeId,
                  event.occurredAt,
                  receivedAt,
                  ANALYTICS_SCHEMA_VERSION,
                ),
            ),
          )
        },
        catch: cause =>
          new WebAnalyticsPersistenceError({
            operation: 'store',
            cause,
          }),
      }),
    prune: cutoffIso =>
      Effect.tryPromise({
        try: async () => {
          await db
            .prepare('DELETE FROM web_analytics_events WHERE received_at < ?')
            .bind(cutoffIso)
            .run()
        },
        catch: cause =>
          new WebAnalyticsPersistenceError({
            operation: 'prune',
            cause,
          }),
      }),
    readSummary: (sinceIso, window, generatedAt) =>
      Effect.tryPromise({
        try: async () => {
          const [pageViews, totalNamedEvents, namedEvents, topPages, daily] =
            await Promise.all([
              db
                .prepare(
                  `SELECT COUNT(*) AS count
                     FROM web_analytics_events
                    WHERE event_name = 'page_view' AND received_at >= ?`,
                )
                .bind(sinceIso)
                .first<CountRow>(),
              db
                .prepare(
                  `SELECT COUNT(*) AS count
                     FROM web_analytics_events
                    WHERE event_name <> 'page_view' AND received_at >= ?`,
                )
                .bind(sinceIso)
                .first<CountRow>(),
              db
                .prepare(
                  `SELECT event_name, COUNT(*) AS count
                     FROM web_analytics_events
                    WHERE event_name <> 'page_view' AND received_at >= ?
                    GROUP BY event_name
                    ORDER BY count DESC, event_name ASC`,
                )
                .bind(sinceIso)
                .all<NamedEventRow>(),
              db
                .prepare(
                  `SELECT route_id, COUNT(*) AS page_views
                     FROM web_analytics_events
                    WHERE event_name = 'page_view' AND received_at >= ?
                    GROUP BY route_id
                    ORDER BY page_views DESC, route_id ASC
                    LIMIT 20`,
                )
                .bind(sinceIso)
                .all<TopPageRow>(),
              db
                .prepare(
                  `SELECT SUBSTR(received_at, 1, 10) AS day,
                          SUM(CASE WHEN event_name = 'page_view' THEN 1 ELSE 0 END) AS page_views,
                          SUM(CASE WHEN event_name <> 'page_view' THEN 1 ELSE 0 END) AS named_events
                     FROM web_analytics_events
                    WHERE received_at >= ?
                    GROUP BY SUBSTR(received_at, 1, 10)
                    ORDER BY day ASC`,
                )
                .bind(sinceIso)
                .all<DailyRow>(),
            ])

          return {
            ok: true,
            schemaVersion: 'openagents.analytics.summary.v1',
            generatedAt,
            window,
            pageViews: asNumber(pageViews?.count),
            totalNamedEvents: asNumber(totalNamedEvents?.count),
            namedEvents: (namedEvents.results ?? []).map(row => ({
              name: row.event_name,
              count: asNumber(row.count),
            })),
            topPages: (topPages.results ?? []).map(row => ({
              routeId: row.route_id,
              pageViews: asNumber(row.page_views),
            })),
            daily: (daily.results ?? []).map(row => ({
              day: row.day,
              pageViews: asNumber(row.page_views),
              namedEvents: asNumber(row.named_events),
            })),
          } satisfies WebAnalyticsSummary
        },
        catch: cause =>
          new WebAnalyticsPersistenceError({
            operation: 'readSummary',
            cause,
          }),
      }),
  })

export const WebAnalyticsIngestionLive = Layer.effect(
  WebAnalyticsIngestion,
  Effect.gen(function* () {
    const persistence = yield* WebAnalyticsPersistence
    const config = yield* WebAnalyticsConfiguration

    return WebAnalyticsIngestion.of({
      ingest: Effect.fn('WebAnalyticsIngestion.ingest')(function* (
        payload: unknown,
        receivedAt: string,
      ) {
        const batch = yield* S.decodeUnknownEffect(AnalyticsEventBatch)(payload)
        yield* persistence.store(batch.events, receivedAt)

        const receivedMs = Date.parse(receivedAt)
        const cutoffMs =
          (Number.isFinite(receivedMs) ? receivedMs : Date.now()) -
          config.retentionDays * 24 * 60 * 60 * 1_000
        yield* persistence
          .prune(new Date(cutoffMs).toISOString())
          .pipe(Effect.catch(() => Effect.void))
      }),
    })
  }),
)

const windowMilliseconds: Record<WebAnalyticsWindow, number> = {
  '24h': 24 * 60 * 60 * 1_000,
  '7d': 7 * 24 * 60 * 60 * 1_000,
  '30d': 30 * 24 * 60 * 60 * 1_000,
}

export const WebAnalyticsRollupsLive = Layer.effect(
  WebAnalyticsRollups,
  Effect.gen(function* () {
    const persistence = yield* WebAnalyticsPersistence
    return WebAnalyticsRollups.of({
      summary: Effect.fn('WebAnalyticsRollups.summary')(function* (
        window: WebAnalyticsWindow,
        generatedAt: string,
      ) {
        const generatedMs = Date.parse(generatedAt)
        const since = new Date(
          (Number.isFinite(generatedMs) ? generatedMs : Date.now()) -
            windowMilliseconds[window],
        ).toISOString()
        return yield* persistence.readSummary(since, window, generatedAt)
      }),
    })
  }),
)

const readBoundedJson = async (request: Request): Promise<unknown> => {
  const contentLength = Number(request.headers.get('content-length') ?? 0)
  if (
    Number.isFinite(contentLength) &&
    contentLength > WEB_ANALYTICS_MAX_BODY_BYTES
  ) {
    throw new Error('payload_too_large')
  }
  const body = await request.text()
  if (
    new TextEncoder().encode(body).byteLength > WEB_ANALYTICS_MAX_BODY_BYTES
  ) {
    throw new Error('payload_too_large')
  }
  return JSON.parse(body) as unknown
}

const hasSameOrigin = (request: Request): boolean => {
  const origin = request.headers.get('origin')
  if (origin === null) return false
  try {
    return new URL(origin).origin === new URL(request.url).origin
  } catch {
    return false
  }
}

const parseWindow = (request: Request): WebAnalyticsWindow => {
  const value = new URL(request.url).searchParams.get('window')
  return value === '24h' || value === '30d' ? value : '7d'
}

export const makeWebAnalyticsRoutes = <
  Session extends AdminSession,
  Bindings extends WebAnalyticsBindings,
>(
  dependencies: WebAnalyticsRouteDependencies<Session, Bindings>,
) => {
  const runWithLayers = <A, E>(
    effect: Effect.Effect<
      A,
      E,
      WebAnalyticsConfiguration | WebAnalyticsPersistence
    >,
    env: Bindings,
  ) =>
    Effect.runPromise(
      effect.pipe(
        Effect.provide(
          Layer.merge(
            webAnalyticsConfigurationLayer(env.WEB_ANALYTICS_ENABLED),
            webAnalyticsPersistenceLayer(dependencies.db(env)),
          ),
        ),
      ),
    )

  return {
    handleIngest: async (
      request: Request,
      env: Bindings,
    ): Promise<Response> => {
      if (request.method !== 'POST') return methodNotAllowed(['POST'])
      if (!isEnabled(env.WEB_ANALYTICS_ENABLED)) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }
      if (!hasSameOrigin(request)) {
        return noStoreJsonResponse(
          { error: 'forbidden_origin' },
          { status: 403 },
        )
      }
      if (
        !request.headers.get('content-type')?.startsWith('application/json')
      ) {
        return noStoreJsonResponse(
          { error: 'unsupported_media_type' },
          { status: 415 },
        )
      }

      let payload: unknown
      try {
        payload = await readBoundedJson(request)
      } catch (cause) {
        return noStoreJsonResponse(
          {
            error:
              cause instanceof Error && cause.message === 'payload_too_large'
                ? 'payload_too_large'
                : 'invalid_json',
          },
          {
            status:
              cause instanceof Error && cause.message === 'payload_too_large'
                ? 413
                : 400,
          },
        )
      }

      const nowIso = (dependencies.nowIso ?? (() => new Date().toISOString()))()
      const result = await runWithLayers(
        Effect.gen(function* () {
          const ingestion = yield* WebAnalyticsIngestion
          yield* ingestion.ingest(payload, nowIso)
        }).pipe(
          Effect.provide(WebAnalyticsIngestionLive),
          Effect.match({
            onFailure: error => ({
              ok: false as const,
              error:
                error._tag === 'WebAnalyticsPersistenceError'
                  ? 'persistence_error'
                  : 'invalid_event_batch',
            }),
            onSuccess: () => ({ ok: true as const }),
          }),
        ),
        env,
      )

      if (result.ok) {
        return new Response(null, {
          status: 204,
          headers: { 'cache-control': 'no-store' },
        })
      }
      return noStoreJsonResponse(
        { error: result.error },
        { status: result.error === 'persistence_error' ? 503 : 400 },
      )
    },

    handleAdminSummary: async (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Promise<Response> => {
      if (request.method !== 'GET') return methodNotAllowed(['GET'])
      if (!isEnabled(env.WEB_ANALYTICS_ENABLED)) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }
      const session = await dependencies.requireBrowserSession(
        request,
        env,
        ctx,
      )
      if (session === undefined) return unauthorized()
      if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
        return forbidden()
      }

      const generatedAt = (
        dependencies.nowIso ?? (() => new Date().toISOString())
      )()
      const summary = await runWithLayers(
        Effect.gen(function* () {
          const rollups = yield* WebAnalyticsRollups
          return yield* rollups.summary(parseWindow(request), generatedAt)
        }).pipe(Effect.provide(WebAnalyticsRollupsLive)),
        env,
      )

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(summary),
        session,
      )
    },
  }
}
