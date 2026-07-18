import {
  FullAutoRunControlAuthorityError,
  type FullAutoRunControlAuthorityRepositoryShape,
  type SyncSql,
  makeFullAutoRunControlAuthority,
} from '@openagentsinc/khala-sync-server'
import { Effect, Schema as S } from 'effect'

import {
  type HttpHeadersDecorator,
  type JsonHttpResult,
  decorateJsonHttpResultHeaders,
  methodNotAllowedResult,
  noStoreJsonResult,
} from './http/responses'
import { parseJsonUnknown } from './json-boundary'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { defaultMakeKhalaSyncSqlClient } from './khala-sync-push-routes'

// MOB-FA-02 (#8994): the sibling mutation route to `/api/full-auto-runs`
// (`full-auto-run-routes.ts`). Desktop is not always reachable when the
// phone wants to Pause/Resume/Stop a run (asleep, offline, owner away from
// the Mac) -- so this route is deliberately server-mediated and
// eventually-consistent, matching how the existing projection route's
// Desktop->server publish already rides a periodic heartbeat instead of a
// live socket:
//
//  - mobile POSTs `{ intent: {...} }` to durably record a Pause/Resume/Stop
//    request; the server never applies it, only records it `pending` and
//    returns immediately;
//  - Desktop POSTs `{ outcome: {...} }` on its next heartbeat tick, after
//    actually applying the intent through `full-auto-run-actions.ts`;
//  - both sides GET the same list to observe pending vs. applied/rejected --
//    mobile polls for its own intent's outcome, Desktop pulls every still-
//    pending intent to apply.
export const FULL_AUTO_RUN_CONTROL_INTENTS_PATH = '/api/full-auto-runs/control-intents'
export const FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF = 'route.full_auto_runs.control_intents.v1'
export const FULL_AUTO_RUN_CONTROL_INTENT_REQUEST_MAX_BYTES = 8 * 1024

export type FullAutoRunControlRoutesAuthenticatedOwner = Readonly<{
  userId: string
  decorateResponseHeaders?: HttpHeadersDecorator | undefined
}>

type FullAutoRunControlRouteEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
}>

export type FullAutoRunControlRouteDependencies<Bindings extends FullAutoRunControlRouteEnv> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<FullAutoRunControlRoutesAuthenticatedOwner | undefined>
  bindingForEnv?: ((env: Bindings) => KhalaSyncHyperdriveBinding | undefined) | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  makeAuthority?: ((sql: SyncSql) => FullAutoRunControlAuthorityRepositoryShape) | undefined
}>

class FullAutoRunControlRequestBodyError extends Error {}

class FullAutoRunControlDependencyError extends S.TaggedErrorClass<FullAutoRunControlDependencyError>()(
  'FullAutoRunControlDependencyError',
  { cause: S.Defect() },
) {}

const collectBoundedRequestChunks = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
  chunks: ReadonlyArray<Uint8Array> = [],
  byteLength = 0,
): Promise<Readonly<{ chunks: ReadonlyArray<Uint8Array>; byteLength: number }>> => {
  const next = await reader.read()
  if (next.done) return { chunks, byteLength }
  const nextByteLength = byteLength + next.value.byteLength
  if (nextByteLength > FULL_AUTO_RUN_CONTROL_INTENT_REQUEST_MAX_BYTES) {
    await reader.cancel().catch(() => undefined)
    throw new FullAutoRunControlRequestBodyError()
  }
  return collectBoundedRequestChunks(reader, [...chunks, next.value], nextByteLength)
}

const readBoundedJson = async (request: Request): Promise<unknown> => {
  const contentLength = request.headers.get('content-length')
  const declaredLength =
    contentLength === null || contentLength.trim() === '' ? undefined : Number(contentLength)
  if (
    declaredLength !== undefined &&
    (!Number.isSafeInteger(declaredLength) || declaredLength < 0 || declaredLength > FULL_AUTO_RUN_CONTROL_INTENT_REQUEST_MAX_BYTES)
  ) {
    throw new FullAutoRunControlRequestBodyError()
  }
  if (request.body === null) return {}
  const collected = await collectBoundedRequestChunks(request.body.getReader())
  const bytes = new Uint8Array(collected.byteLength)
  collected.chunks.reduce((offset, chunk) => {
    bytes.set(chunk, offset)
    return offset + chunk.byteLength
  }, 0)
  const text = new TextDecoder().decode(bytes)
  if (text.trim() === '') return {}
  try {
    return parseJsonUnknown(text)
  } catch {
    throw new FullAutoRunControlRequestBodyError()
  }
}

const authorityErrorStatus = (
  error: FullAutoRunControlAuthorityError,
): Readonly<{ status: number; retryable: boolean }> => {
  if (error.kind === 'invalid_request') return { status: 400, retryable: false }
  if (error.kind === 'intent_not_found') return { status: 404, retryable: false }
  if (error.kind === 'intent_exists') return { status: 409, retryable: false }
  return { status: 503, retryable: true }
}

const authorityErrorResponse = (error: FullAutoRunControlAuthorityError): JsonHttpResult => {
  const mapped = authorityErrorStatus(error)
  return noStoreJsonResult(
    { ok: false, error: { code: error.kind, retryable: mapped.retryable }, routeRef: FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF },
    { status: mapped.status },
  )
}

const serviceUnavailable = (code: string): JsonHttpResult =>
  noStoreJsonResult(
    { ok: false, error: { code, retryable: true }, routeRef: FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF },
    { status: 503 },
  )

const invalidRequest = (): JsonHttpResult =>
  noStoreJsonResult(
    { ok: false, error: { code: 'invalid_request', retryable: false }, routeRef: FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF },
    { status: 400 },
  )

const promiseOutcome = <A>(
  operation: () => Promise<A>,
): Effect.Effect<
  | Readonly<{ kind: 'failure'; error: FullAutoRunControlDependencyError }>
  | Readonly<{ kind: 'success'; value: A }>
> =>
  Effect.tryPromise({
    try: operation,
    catch: cause => new FullAutoRunControlDependencyError({ cause }),
  }).pipe(
    Effect.match({
      onFailure: error => ({ kind: 'failure' as const, error }),
      onSuccess: value => ({ kind: 'success' as const, value }),
    }),
  )

const authorityOutcome = <A>(
  operation: Effect.Effect<A, FullAutoRunControlAuthorityError>,
): Effect.Effect<
  | Readonly<{ kind: 'failure'; error: FullAutoRunControlAuthorityError }>
  | Readonly<{ kind: 'success'; value: A }>
> =>
  operation.pipe(
    Effect.match({
      onFailure: error => ({ kind: 'failure' as const, error }),
      onSuccess: value => ({ kind: 'success' as const, value }),
    }),
  )

const bindingConnectionString = (binding: KhalaSyncHyperdriveBinding | undefined): string | undefined => {
  const connectionString = binding?.connectionString
  return typeof connectionString === 'string' && connectionString.length > 0 ? connectionString : undefined
}

const FULL_AUTO_RUN_CONTROL_ACTIONS = ['pause', 'resume', 'stop'] as const
const FULL_AUTO_RUN_CONTROL_OUTCOME_STATUSES = ['applied', 'rejected'] as const
const FULL_AUTO_RUN_CONTROL_REJECTION_REASONS = [
  'run_not_found', 'illegal_transition', 'workspace_mismatch',
  'lane_not_eligible', 'desktop_unreachable', 'storage_unavailable',
] as const
type FullAutoRunControlRejectionReasonValue = typeof FULL_AUTO_RUN_CONTROL_REJECTION_REASONS[number]
const FULL_AUTO_RUN_CONTROL_LIFECYCLE_STATES = [
  'draft', 'running', 'pausing', 'paused', 'retrying',
  'stalled', 'completed', 'failed', 'stopped', 'cap_reached',
] as const
type FullAutoRunControlLifecycleStateValue = typeof FULL_AUTO_RUN_CONTROL_LIFECYCLE_STATES[number]

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const isNonEmptyString = (value: unknown): value is string =>
  typeof value === 'string' && value.trim().length > 0

const parseDispatchRequest = (
  body: unknown,
): Readonly<{ intentId: string; idempotencyKey: string; runRef: string; action: 'pause' | 'resume' | 'stop' }> | null => {
  if (!isRecord(body)) return null
  const intent = body.intent
  if (!isRecord(intent)) return null
  const { intentId, idempotencyKey, runRef, action } = intent
  if (!isNonEmptyString(intentId) || !isNonEmptyString(idempotencyKey) || !isNonEmptyString(runRef)) return null
  if (typeof action !== 'string' || !(FULL_AUTO_RUN_CONTROL_ACTIONS as ReadonlyArray<string>).includes(action)) return null
  return { intentId, idempotencyKey, runRef, action: action as 'pause' | 'resume' | 'stop' }
}

const parseOutcomeReport = (
  body: unknown,
): Readonly<{
  intentId: string
  status: 'applied' | 'rejected'
  rejectionReason?: FullAutoRunControlRejectionReasonValue
  resultLifecycleState?: FullAutoRunControlLifecycleStateValue
}> | null => {
  if (!isRecord(body)) return null
  const outcome = body.outcome
  if (!isRecord(outcome)) return null
  const { intentId, status, rejectionReason, resultLifecycleState } = outcome
  if (!isNonEmptyString(intentId)) return null
  if (typeof status !== 'string' || !(FULL_AUTO_RUN_CONTROL_OUTCOME_STATUSES as ReadonlyArray<string>).includes(status)) return null
  if (
    rejectionReason !== undefined
    && (typeof rejectionReason !== 'string' || !(FULL_AUTO_RUN_CONTROL_REJECTION_REASONS as ReadonlyArray<string>).includes(rejectionReason))
  ) return null
  if (
    resultLifecycleState !== undefined
    && (typeof resultLifecycleState !== 'string' || !(FULL_AUTO_RUN_CONTROL_LIFECYCLE_STATES as ReadonlyArray<string>).includes(resultLifecycleState))
  ) return null
  return {
    intentId,
    status: status as 'applied' | 'rejected',
    ...(rejectionReason === undefined ? {} : { rejectionReason: rejectionReason as FullAutoRunControlRejectionReasonValue }),
    ...(resultLifecycleState === undefined ? {} : { resultLifecycleState: resultLifecycleState as FullAutoRunControlLifecycleStateValue }),
  }
}

export const makeFullAutoRunControlRoutes = <Bindings extends FullAutoRunControlRouteEnv>(
  dependencies: FullAutoRunControlRouteDependencies<Bindings>,
) => {
  const handle = (request: Request, env: Bindings, ctx: ExecutionContext): Effect.Effect<JsonHttpResult> =>
    Effect.gen(function* () {
      if (request.method !== 'POST' && request.method !== 'GET') {
        return methodNotAllowedResult(['GET', 'POST'])
      }

      const authentication = yield* promiseOutcome(() => dependencies.authenticateOwner(request, env, ctx))
      if (authentication.kind === 'failure') {
        return serviceUnavailable('authentication_unavailable')
      }
      if (authentication.value === undefined) {
        return noStoreJsonResult(
          { ok: false, error: { code: 'unauthenticated', retryable: false }, routeRef: FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF },
          { status: 401 },
        )
      }
      const owner = authentication.value
      const respond = <Body>(result: JsonHttpResult<Body>): JsonHttpResult<Body> =>
        owner.decorateResponseHeaders === undefined
          ? result
          : decorateJsonHttpResultHeaders(result, owner.decorateResponseHeaders)

      const url = new URL(request.url)
      if ([...url.searchParams.keys()].length > 0) {
        return respond(invalidRequest())
      }

      const connectionString = bindingConnectionString(
        (dependencies.bindingForEnv ?? (value => value.KHALA_SYNC_DB))(env),
      )
      if (connectionString === undefined) {
        return respond(serviceUnavailable('storage_unavailable'))
      }

      const makeSqlClient = dependencies.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
      const clientResult = yield* promiseOutcome(() => makeSqlClient(connectionString))
      if (clientResult.kind === 'failure') {
        return respond(serviceUnavailable('storage_unavailable'))
      }
      const client: KhalaSyncPushSqlClient = clientResult.value
      const storageOperation = Effect.gen(function* () {
        const authority =
          dependencies.makeAuthority?.(client.sql) ?? makeFullAutoRunControlAuthority({ sql: client.sql })

        if (request.method === 'POST') {
          const bodyResult = yield* promiseOutcome(() => readBoundedJson(request))
          if (bodyResult.kind === 'failure') {
            return respond(
              bodyResult.error.cause instanceof FullAutoRunControlRequestBodyError
                ? invalidRequest()
                : serviceUnavailable('storage_unavailable'),
            )
          }
          const body = bodyResult.value

          const dispatchRequest = parseDispatchRequest(body)
          if (dispatchRequest !== null) {
            const outcome = yield* authorityOutcome(authority.dispatch({ ownerUserId: owner.userId, request: dispatchRequest }))
            return respond(
              outcome.kind === 'failure'
                ? authorityErrorResponse(outcome.error)
                : noStoreJsonResult({ ok: true, routeRef: FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF, intent: outcome.value }),
            )
          }

          const outcomeReport = parseOutcomeReport(body)
          if (outcomeReport !== null) {
            const outcome = yield* authorityOutcome(authority.reportOutcome({ ownerUserId: owner.userId, outcome: outcomeReport }))
            return respond(
              outcome.kind === 'failure'
                ? authorityErrorResponse(outcome.error)
                : noStoreJsonResult({ ok: true, routeRef: FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF, intent: outcome.value }),
            )
          }

          return respond(invalidRequest())
        }

        const outcome = yield* authorityOutcome(authority.list({ ownerUserId: owner.userId }))
        return respond(
          outcome.kind === 'failure'
            ? authorityErrorResponse(outcome.error)
            : noStoreJsonResult({ ok: true, routeRef: FULL_AUTO_RUN_CONTROL_INTENTS_ROUTE_REF, intents: outcome.value }),
        )
      }).pipe(
        Effect.catchDefect(() => Effect.succeed(respond(serviceUnavailable('storage_unavailable')))),
        Effect.ensuring(
          Effect.tryPromise({
            try: () => client.end(),
            catch: cause => new FullAutoRunControlDependencyError({ cause }),
          }).pipe(Effect.ignore),
        ),
      )

      return yield* storageOperation
    })

  return { handle }
}
