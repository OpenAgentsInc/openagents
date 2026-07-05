// Khala Sync runtime-intent dispatch-consumer routes (#8388).
//
// `GET /api/internal/khala-sync/runtime-intents?ownerUserId=&after=&limit=`
// — the admin-bearer-gated internal seam through which the Pylon-side
// runtime dispatch consumer (`apps/pylon/src/orchestration/
// runtime-intent-enforcement.ts`) observes the durable control intents the
// runtime.* mutators record in `khala_sync_runtime_control_intents`
// (`packages/khala-sync-server/src/runtime-mutators.ts`, migration 0029;
// the resumable `seq` watermark column is migration 0032). Mirrors
// `khala-sync-fleet-intents-routes.ts` precisely: consumers poll
// `?after=<last seen seq>` (oldest-first, bounded pages, `nextAfter`
// watermark) and dispatch the requested behavior locally.
//
// `GET /api/internal/khala-sync/chat-message?threadId=&messageId=` — the
// companion seam the same consumer uses to resolve a `turn.start` intent's
// `bodyRef` (the `chat_message.<messageId>` convention, see
// docs/khala-code/2026-07-04-mobile-tailnet-handshake.md) into the real
// prompt text recorded by `chat.appendMessage`
// (`packages/khala-sync-server/src/chat-mutators.ts`). Same admin-bearer
// trust boundary as every other `/api/internal/khala-sync/*` route — this
// is an internal system-operator seam, not a public/user-scoped read.
//
// STORAGE: authoritative reads through the `KHALA_SYNC_DB` Hyperdrive
// binding, one bounded single-statement query each (transaction-mode safe,
// SPEC §4). The real postgres.js client is dynamically imported ONLY when
// no `makeSqlClient` is injected; tests inject fakes so CI never needs a
// database.

import { Effect } from 'effect'

import {
  DEFAULT_RUNTIME_INTENTS_LIMIT,
  encodeRuntimeControlIntentRow,
  MAX_RUNTIME_INTENTS_LIMIT,
  readChatMessageById as readChatMessageByIdFromPostgres,
  readPendingRuntimeControlIntents as readPendingRuntimeControlIntentsFromPostgres,
  type ReadPendingRuntimeControlIntentsInput,
  type RuntimeChatMessageRow,
  type RuntimeControlIntentRow,
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

export const KHALA_SYNC_RUNTIME_INTENTS_PATH =
  '/api/internal/khala-sync/runtime-intents'
export const KHALA_SYNC_RUNTIME_INTENTS_ROUTE_REF =
  'route.internal.khala_sync.runtime_intents.v0_1'

export const KHALA_SYNC_CHAT_MESSAGE_READ_PATH =
  '/api/internal/khala-sync/chat-message'
export const KHALA_SYNC_CHAT_MESSAGE_READ_ROUTE_REF =
  'route.internal.khala_sync.chat_message_read.v0_1'

const OWNER_USER_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/
const REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/

/** Injectable read seam so route tests never need a database. */
export type ReadPendingRuntimeControlIntentsFn = (
  sql: SyncSql,
  input: ReadPendingRuntimeControlIntentsInput,
) => Promise<ReadonlyArray<RuntimeControlIntentRow>>

export type ReadChatMessageByIdFn = (
  sql: SyncSql,
  input: { readonly messageId: string; readonly threadId?: string },
) => Promise<RuntimeChatMessageRow | null>

export type KhalaSyncRuntimeIntentsDependencies = Readonly<{
  /** Same admin bearer predicate the other internal khala-sync routes use. */
  requireOperator: () => Promise<boolean>
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /** Injectable client factory (default: postgres.js, Worker-runtime only). */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Injectable read seam for route tests. Default: the real reader. */
  readPendingRuntimeControlIntents?: ReadPendingRuntimeControlIntentsFn | undefined
}>

export type KhalaSyncChatMessageReadDependencies = Readonly<{
  requireOperator: () => Promise<boolean>
  binding: KhalaSyncHyperdriveBinding | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  readChatMessageById?: ReadChatMessageByIdFn | undefined
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

const bindingConnectionString = (
  binding: KhalaSyncHyperdriveBinding | undefined,
): string | undefined =>
  binding !== undefined &&
  typeof binding.connectionString === 'string' &&
  binding.connectionString.length > 0
    ? binding.connectionString
    : undefined

const storageNotConfigured = (routeRef: string): HttpResponse =>
  noStoreJsonResponse({
    ok: false,
    reason:
      'Khala Sync storage is not configured on this deployment ' +
      '(env.KHALA_SYNC_DB Hyperdrive binding is absent).',
    routeRef,
  })

/**
 * `GET /api/internal/khala-sync/runtime-intents?ownerUserId=&after=&limit=`
 * — admin bearer only.
 *
 * Success: `{ ok: true, intents, nextAfter, upToDate, routeRef }` where
 * `intents` are oldest-first `RuntimeControlIntentRow`s with
 * `seq > after`, `nextAfter` is the new poll watermark (the last returned
 * seq, or the requested `after` when the page is empty), and `upToDate` is
 * true when the page was not truncated by `limit`. Binding absent: honest
 * `{ ok: false, reason }` (HTTP 200). Storage failures: `{ ok: false,
 * error }` with HTTP 503 and no detail echo (connection errors can embed
 * DSNs).
 */
export const handleKhalaSyncRuntimeIntents = (
  request: Request,
  deps: KhalaSyncRuntimeIntentsDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    if (!(await deps.requireOperator())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const url = new URL(request.url)
    const ownerUserIdRaw = url.searchParams.get('ownerUserId')
    if (ownerUserIdRaw !== null && !OWNER_USER_ID_PATTERN.test(ownerUserIdRaw)) {
      return invalidRequest('ownerUserId must be a bounded safe-ref string.')
    }
    const afterRaw = url.searchParams.get('after')
    const after = afterRaw === null ? 0 : parseNonNegativeInt(afterRaw)
    if (after === undefined) {
      return invalidRequest('after must be a non-negative integer seq.')
    }
    const limitRaw = url.searchParams.get('limit')
    const parsedLimit =
      limitRaw === null
        ? DEFAULT_RUNTIME_INTENTS_LIMIT
        : parseNonNegativeInt(limitRaw)
    if (parsedLimit === undefined || parsedLimit < 1) {
      return invalidRequest('limit must be a positive integer.')
    }
    const limit = Math.min(parsedLimit, MAX_RUNTIME_INTENTS_LIMIT)

    const connectionString = bindingConnectionString(deps.binding)
    if (connectionString === undefined) {
      return storageNotConfigured(KHALA_SYNC_RUNTIME_INTENTS_ROUTE_REF)
    }

    const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
    const readIntents =
      deps.readPendingRuntimeControlIntents ??
      readPendingRuntimeControlIntentsFromPostgres

    let client: KhalaSyncPushSqlClient | undefined
    try {
      client = await makeSqlClient(connectionString)
      const intents = await readIntents(client.sql, {
        afterSeq: after,
        limit,
        ...(ownerUserIdRaw === null ? {} : { ownerUserId: ownerUserIdRaw }),
      })
      const lastSeq = intents[intents.length - 1]?.seq
      return noStoreJsonResponse({
        intents: intents.map(row => encodeRuntimeControlIntentRow(row)),
        nextAfter: lastSeq ?? after,
        ok: true,
        routeRef: KHALA_SYNC_RUNTIME_INTENTS_ROUTE_REF,
        upToDate: intents.length < limit,
      })
    } catch {
      // Driver failures can embed connection strings — never echo them.
      return noStoreJsonResponse(
        {
          error: 'khala_sync_runtime_intents_read_failed',
          ok: false,
          routeRef: KHALA_SYNC_RUNTIME_INTENTS_ROUTE_REF,
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

/**
 * `GET /api/internal/khala-sync/chat-message?threadId=&messageId=` —
 * admin bearer only. Resolves one `chat_message` row for the runtime
 * dispatch consumer's `bodyRef` convention (`chat_message.<messageId>`).
 *
 * Success: `{ ok: true, message }` when found, `{ ok: true, message: null
 * }` when the message id (or thread/message pairing) does not exist — the
 * caller treats a null message as a real error condition, never a silent
 * skip. Binding absent / storage failure follow the same honest
 * conventions as every other internal khala-sync route.
 */
export const handleKhalaSyncChatMessageRead = (
  request: Request,
  deps: KhalaSyncChatMessageReadDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    if (!(await deps.requireOperator())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const url = new URL(request.url)
    const messageId = url.searchParams.get('messageId')
    if (messageId === null || !REF_PATTERN.test(messageId)) {
      return invalidRequest('messageId is required and must be a bounded safe-ref string.')
    }
    const threadIdRaw = url.searchParams.get('threadId')
    if (threadIdRaw !== null && !REF_PATTERN.test(threadIdRaw)) {
      return invalidRequest('threadId must be a bounded safe-ref string.')
    }

    const connectionString = bindingConnectionString(deps.binding)
    if (connectionString === undefined) {
      return storageNotConfigured(KHALA_SYNC_CHAT_MESSAGE_READ_ROUTE_REF)
    }

    const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
    const readMessage = deps.readChatMessageById ?? readChatMessageByIdFromPostgres

    let client: KhalaSyncPushSqlClient | undefined
    try {
      client = await makeSqlClient(connectionString)
      const message = await readMessage(client.sql, {
        messageId,
        ...(threadIdRaw === null ? {} : { threadId: threadIdRaw }),
      })
      return noStoreJsonResponse({
        message,
        ok: true,
        routeRef: KHALA_SYNC_CHAT_MESSAGE_READ_ROUTE_REF,
      })
    } catch {
      return noStoreJsonResponse(
        {
          error: 'khala_sync_chat_message_read_failed',
          ok: false,
          routeRef: KHALA_SYNC_CHAT_MESSAGE_READ_ROUTE_REF,
        },
        { status: 503 },
      )
    } finally {
      if (client !== undefined) {
        try {
          await client.end()
        } catch {
          // best-effort teardown, mirrors the sibling routes above.
        }
      }
    }
  })
