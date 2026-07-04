// Khala Sync fleet-intent consumption route (KS-3.2, #8292).
//
// `GET /api/internal/khala-sync/fleet-intents?scope=&after=&limit=` — the
// admin-bearer-gated internal seam through which an ENFORCEMENT loop (the
// Pylon supervisor, polling with its `OPENAGENTS_ADMIN_API_TOKEN`) observes
// the durable operator intents that the fleet mutators record in
// `khala_sync_fleet_intents` (docs/khala-sync/MUTATORS.md; migrations
// 0004/0005 in `@openagentsinc/khala-sync-server`).
//
// HONEST V1 CONTRACT: an applied fleet mutation is a durably recorded
// operator request plus a projected post-image — it does not change
// dispatch behavior by itself. This route closes the OBSERVATION half of
// that gap: consumers poll `?after=<last seen intent id>` (oldest-first,
// bounded pages, `nextAfter` watermark) and apply the requested behavior on
// their side. Wiring the Pylon supervisor loop to poll + enforce is the
// follow-up lane tracked on epic #8282.
//
// AUTH: same admin bearer mechanism as the other internal khala-sync routes
// (`requireAdminApiToken`, injected as `requireOperator` — mirrors
// `handleKhalaSyncDbSmoke`). Not part of the public OpenAPI surface. The
// response carries only what the intents table holds: bounded public-safe
// refs, the intent token, the requesting user id, and mutation refs.
//
// STORAGE: authoritative read through the `KHALA_SYNC_DB` Hyperdrive
// binding — one bounded single-statement SELECT via
// `readPendingFleetIntents` (transaction-mode safe, SPEC §4). The real
// postgres.js client is dynamically imported ONLY when no `makeSqlClient`
// is injected; tests inject fakes so CI never needs a database.

import { Effect } from 'effect'

import {
  DEFAULT_FLEET_INTENTS_LIMIT,
  encodeFleetIntentRow,
  MAX_FLEET_INTENTS_LIMIT,
  readPendingFleetIntents as readPendingFleetIntentsFromPostgres,
  type FleetIntentRow,
  type ReadPendingFleetIntentsInput,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { defaultMakeKhalaSyncSqlClient } from './khala-sync-push-routes'

type HttpResponse = globalThis.Response

export const KHALA_SYNC_FLEET_INTENTS_PATH =
  '/api/internal/khala-sync/fleet-intents'
export const KHALA_SYNC_FLEET_INTENTS_ROUTE_REF =
  'route.internal.khala_sync.fleet_intents.v0_1'

const FLEET_RUN_SCOPE_PATTERN = /^scope\.fleet_run\.[A-Za-z0-9][A-Za-z0-9._:-]*$/

/** Injectable read seam so route tests never need a database. */
export type ReadPendingFleetIntentsFn = (
  sql: SyncSql,
  input: ReadPendingFleetIntentsInput,
) => Promise<ReadonlyArray<FleetIntentRow>>

export type KhalaSyncFleetIntentsDependencies = Readonly<{
  /** Same admin bearer predicate the other internal khala-sync routes use. */
  requireOperator: () => Promise<boolean>
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /** Injectable client factory (default: postgres.js, Worker-runtime only). */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable read seam for route tests. Default: the real reader. */
  readPendingFleetIntents?: ReadPendingFleetIntentsFn | undefined
}>

const invalidRequest = (reason: string): HttpResponse =>
  noStoreJsonResponse(
    { error: 'invalid_request', ok: false, reason },
    { status: 400 },
  )

const parseNonNegativeInt = (raw: string): number | undefined => {
  if (!/^\d+$/.test(raw)) return undefined
  const value = Number(raw)
  return Number.isSafeInteger(value) ? value : undefined
}

/**
 * `GET /api/internal/khala-sync/fleet-intents?scope=&after=&limit=` —
 * admin bearer only.
 *
 * Success: `{ ok: true, intents, nextAfter, upToDate, routeRef }` where
 * `intents` are oldest-first `FleetIntentRow`s with `id > after`,
 * `nextAfter` is the new poll watermark (the last returned id, or the
 * requested `after` when the page is empty), and `upToDate` is true when
 * the page was not truncated by `limit`. Binding absent: honest
 * `{ ok: false, reason }` (HTTP 200, mirrors the db-smoke convention).
 * Storage failures: `{ ok: false, error }` with HTTP 503 and no detail
 * echo (connection errors can embed DSNs).
 */
export const handleKhalaSyncFleetIntents = (
  request: Request,
  deps: KhalaSyncFleetIntentsDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    if (!(await deps.requireOperator())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const url = new URL(request.url)
    const scopeRaw = url.searchParams.get('scope')
    if (scopeRaw !== null && !FLEET_RUN_SCOPE_PATTERN.test(scopeRaw)) {
      return invalidRequest(
        'scope must be a fleet run scope (scope.fleet_run.<runId>).',
      )
    }
    const afterRaw = url.searchParams.get('after')
    const after = afterRaw === null ? 0 : parseNonNegativeInt(afterRaw)
    if (after === undefined) {
      return invalidRequest('after must be a non-negative integer intent id.')
    }
    const limitRaw = url.searchParams.get('limit')
    const parsedLimit =
      limitRaw === null
        ? DEFAULT_FLEET_INTENTS_LIMIT
        : parseNonNegativeInt(limitRaw)
    if (parsedLimit === undefined || parsedLimit < 1) {
      return invalidRequest('limit must be a positive integer.')
    }
    const limit = Math.min(parsedLimit, MAX_FLEET_INTENTS_LIMIT)

    if (
      deps.binding === undefined ||
      typeof deps.binding.connectionString !== 'string' ||
      deps.binding.connectionString.length === 0
    ) {
      return noStoreJsonResponse({
        ok: false,
        reason:
          'Khala Sync storage is not configured on this deployment ' +
          '(env.KHALA_SYNC_DB Hyperdrive binding is absent).',
        routeRef: KHALA_SYNC_FLEET_INTENTS_ROUTE_REF,
      })
    }

    const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
    const readIntents =
      deps.readPendingFleetIntents ?? readPendingFleetIntentsFromPostgres

    let client: KhalaSyncPushSqlClient | undefined
    try {
      client = await makeSqlClient(deps.binding.connectionString)
      const intents = await readIntents(client.sql, {
        afterId: after,
        limit,
        ...(scopeRaw === null ? {} : { scope: scopeRaw }),
      })
      const lastId = intents[intents.length - 1]?.id
      return noStoreJsonResponse({
        intents: intents.map(row => encodeFleetIntentRow(row)),
        nextAfter: lastId ?? after,
        ok: true,
        routeRef: KHALA_SYNC_FLEET_INTENTS_ROUTE_REF,
        upToDate: intents.length < limit,
      })
    } catch {
      // Driver failures can embed connection strings — never echo them.
      return noStoreJsonResponse(
        {
          error: 'khala_sync_fleet_intents_read_failed',
          ok: false,
          routeRef: KHALA_SYNC_FLEET_INTENTS_ROUTE_REF,
        },
        { status: 503 },
      )
    } finally {
      if (client !== undefined) {
        try {
          await client.end()
        } catch {
          // best-effort teardown: never mask the real result with a close
          // error; the `max: 1` client is dropped with the isolate anyway.
        }
      }
    }
  })
