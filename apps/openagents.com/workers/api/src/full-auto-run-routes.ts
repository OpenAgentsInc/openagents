import {
  FullAutoRunProjectionAuthorityError,
  type FullAutoRunProjectionAuthorityRepositoryShape,
  type SyncSql,
  makeFullAutoRunProjectionRepository,
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

// FA-RUN-05 (#8981): Desktop publishes a public-safe, structured live
// projection of the signed-in user's currently active FullAutoRun; mobile
// (#8982) fetches "my active run" through the SAME authenticated ergonomics
// `/api/fleet-runs` already established (`sarah-fleet-run-routes.ts`).
//
// v1, pending reconciliation with #8972 (FA-RUN-04, the future canonical
// bounded run report/receipt schema) once that lands.
export const FULL_AUTO_RUNS_PATH = '/api/full-auto-runs'
export const FULL_AUTO_RUNS_ROUTE_REF = 'route.full_auto_runs.projection.v1'
export const FULL_AUTO_RUN_REQUEST_MAX_BYTES = 16 * 1024

export type FullAutoRunRoutesAuthenticatedOwner = Readonly<{
  userId: string
  decorateResponseHeaders?: HttpHeadersDecorator | undefined
}>

type FullAutoRunRouteEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
}>

export type FullAutoRunRouteDependencies<Bindings extends FullAutoRunRouteEnv> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<FullAutoRunRoutesAuthenticatedOwner | undefined>
  bindingForEnv?: ((env: Bindings) => KhalaSyncHyperdriveBinding | undefined) | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  makeRepository?: ((sql: SyncSql) => FullAutoRunProjectionAuthorityRepositoryShape) | undefined
}>

class FullAutoRunRequestBodyError extends Error {}

class FullAutoRunDependencyError extends S.TaggedErrorClass<FullAutoRunDependencyError>()(
  'FullAutoRunDependencyError',
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
  if (nextByteLength > FULL_AUTO_RUN_REQUEST_MAX_BYTES) {
    await reader.cancel().catch(() => undefined)
    throw new FullAutoRunRequestBodyError()
  }
  return collectBoundedRequestChunks(reader, [...chunks, next.value], nextByteLength)
}

const readBoundedJson = async (request: Request): Promise<unknown> => {
  const contentLength = request.headers.get('content-length')
  const declaredLength =
    contentLength === null || contentLength.trim() === '' ? undefined : Number(contentLength)
  if (
    declaredLength !== undefined &&
    (!Number.isSafeInteger(declaredLength) || declaredLength < 0 || declaredLength > FULL_AUTO_RUN_REQUEST_MAX_BYTES)
  ) {
    throw new FullAutoRunRequestBodyError()
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
    throw new FullAutoRunRequestBodyError()
  }
}

const authorityErrorStatus = (
  error: FullAutoRunProjectionAuthorityError,
): Readonly<{ status: number; retryable: boolean }> =>
  error.kind === 'invalid_request'
    ? { status: 400, retryable: false }
    : { status: 503, retryable: true }

const authorityErrorResponse = (error: FullAutoRunProjectionAuthorityError): JsonHttpResult => {
  const mapped = authorityErrorStatus(error)
  return noStoreJsonResult(
    { ok: false, error: { code: error.kind, retryable: mapped.retryable }, routeRef: FULL_AUTO_RUNS_ROUTE_REF },
    { status: mapped.status },
  )
}

const serviceUnavailable = (code: string): JsonHttpResult =>
  noStoreJsonResult(
    { ok: false, error: { code, retryable: true }, routeRef: FULL_AUTO_RUNS_ROUTE_REF },
    { status: 503 },
  )

const invalidRequest = (): JsonHttpResult =>
  noStoreJsonResult(
    { ok: false, error: { code: 'invalid_request', retryable: false }, routeRef: FULL_AUTO_RUNS_ROUTE_REF },
    { status: 400 },
  )

const promiseOutcome = <A>(
  operation: () => Promise<A>,
): Effect.Effect<
  | Readonly<{ kind: 'failure'; error: FullAutoRunDependencyError }>
  | Readonly<{ kind: 'success'; value: A }>
> =>
  Effect.tryPromise({
    try: operation,
    catch: cause => new FullAutoRunDependencyError({ cause }),
  }).pipe(
    Effect.match({
      onFailure: error => ({ kind: 'failure' as const, error }),
      onSuccess: value => ({ kind: 'success' as const, value }),
    }),
  )

const authorityOutcome = <A>(
  operation: Effect.Effect<A, FullAutoRunProjectionAuthorityError>,
): Effect.Effect<
  | Readonly<{ kind: 'failure'; error: FullAutoRunProjectionAuthorityError }>
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

export const makeFullAutoRunRoutes = <Bindings extends FullAutoRunRouteEnv>(
  dependencies: FullAutoRunRouteDependencies<Bindings>,
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
          { ok: false, error: { code: 'unauthenticated', retryable: false }, routeRef: FULL_AUTO_RUNS_ROUTE_REF },
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
        const repository =
          dependencies.makeRepository?.(client.sql) ?? makeFullAutoRunProjectionRepository({ sql: client.sql })

        if (request.method === 'POST') {
          const bodyResult = yield* promiseOutcome(() => readBoundedJson(request))
          if (bodyResult.kind === 'failure') {
            return respond(
              bodyResult.error.cause instanceof FullAutoRunRequestBodyError
                ? invalidRequest()
                : serviceUnavailable('storage_unavailable'),
            )
          }
          const body = bodyResult.value
          if (typeof body !== 'object' || body === null || !('run' in body)) {
            return respond(invalidRequest())
          }
          const run = (body as Readonly<{ run: unknown }>).run
          if (run !== null && (typeof run !== 'object' || Array.isArray(run))) {
            return respond(invalidRequest())
          }
          const outcome = yield* authorityOutcome(
            repository.publish({
              ownerUserId: owner.userId,
              // deno-lint-ignore no-explicit-any
              run: run as never,
            }),
          )
          return respond(
            outcome.kind === 'failure'
              ? authorityErrorResponse(outcome.error)
              : noStoreJsonResult({ ok: true, routeRef: FULL_AUTO_RUNS_ROUTE_REF, projection: outcome.value.projection }),
          )
        }

        const outcome = yield* authorityOutcome(repository.observe({ ownerUserId: owner.userId }))
        return respond(
          outcome.kind === 'failure'
            ? authorityErrorResponse(outcome.error)
            : noStoreJsonResult({ ok: true, routeRef: FULL_AUTO_RUNS_ROUTE_REF, projection: outcome.value.projection }),
        )
      }).pipe(
        Effect.catchDefect(() => Effect.succeed(respond(serviceUnavailable('storage_unavailable')))),
        Effect.ensuring(
          Effect.tryPromise({
            try: () => client.end(),
            catch: cause => new FullAutoRunDependencyError({ cause }),
          }).pipe(Effect.ignore),
        ),
      )

      return yield* storageOperation
    })

  return { handle }
}
