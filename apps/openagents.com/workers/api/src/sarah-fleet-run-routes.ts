import { Effect, Schema as S } from 'effect'

import {
  FleetRunAuthorityError,
  makeFleetRunAuthorityRepository,
  publicFleetRunAuthorityRecord,
  type FleetRunAuthorityRepositoryShape,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'
import type {
  KhalaSyncHyperdriveBinding,
  KhalaSyncPushSqlClient,
  MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { defaultMakeKhalaSyncSqlClient } from './khala-sync-push-routes'

type HttpResponse = globalThis.Response

export const SARAH_FLEET_RUNS_PATH = '/api/sarah/fleet-runs'
export const SARAH_FLEET_RUNS_ROUTE_REF =
  'route.sarah.fleet_runs.authority.v1'
export const SARAH_FLEET_RUN_REQUEST_MAX_BYTES = 32 * 1024

export const SarahRelationshipMode = S.Literals([
  'prospect',
  'customer',
  'operator',
  'administrator',
])
export type SarahRelationshipMode = typeof SarahRelationshipMode.Type

export const SarahFleetRunPolicy = S.Struct({
  source: S.Literal('openagents_server_policy'),
  relationshipMode: SarahRelationshipMode,
  codingFleetStartAllowed: S.Boolean,
  fleetObservationAllowed: S.Boolean,
  retrievalScope: S.Literals(['public_only', 'owner_fleet_runs']),
  responsePosture: S.Literals(['guided', 'state_oriented']),
  uiDensity: S.Literals(['standard', 'dense']),
  administratorToolsAllowed: S.Boolean,
})
export type SarahFleetRunPolicy = typeof SarahFleetRunPolicy.Type

export type SarahFleetRunAuthenticatedOwner = Readonly<{
  userId: string
  email: string
  appendRefreshedSessionCookies?:
    | ((response: HttpResponse) => HttpResponse)
    | undefined
}>

export const sarahFleetRunPolicyForMode = (
  relationshipMode: SarahRelationshipMode,
): SarahFleetRunPolicy => {
  const authenticated = relationshipMode !== 'prospect'
  const stateOriented =
    relationshipMode === 'operator' || relationshipMode === 'administrator'
  return S.decodeUnknownSync(SarahFleetRunPolicy)({
    source: 'openagents_server_policy',
    relationshipMode,
    codingFleetStartAllowed: authenticated,
    fleetObservationAllowed: authenticated,
    retrievalScope: authenticated ? 'owner_fleet_runs' : 'public_only',
    responsePosture: stateOriented ? 'state_oriented' : 'guided',
    uiDensity: stateOriented ? 'dense' : 'standard',
    administratorToolsAllowed: relationshipMode === 'administrator',
  })
}

type SarahFleetRunRouteEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
}>

export type SarahFleetRunRouteDependencies<
  Bindings extends SarahFleetRunRouteEnv,
> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<SarahFleetRunAuthenticatedOwner | undefined>
  resolveRelationshipMode: (
    owner: SarahFleetRunAuthenticatedOwner,
    env: Bindings,
  ) => Promise<SarahRelationshipMode>
  bindingForEnv?:
    | ((env: Bindings) => KhalaSyncHyperdriveBinding | undefined)
    | undefined
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  makeRepository?:
    | ((sql: SyncSql) => FleetRunAuthorityRepositoryShape)
    | undefined
}>

class SarahFleetRunRequestBodyError extends Error {}

class SarahFleetRunDependencyError extends S.TaggedErrorClass<SarahFleetRunDependencyError>()(
  'SarahFleetRunDependencyError',
  { cause: S.Defect },
) {}

const collectBoundedRequestChunks = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
  chunks: ReadonlyArray<Uint8Array> = [],
  byteLength = 0,
): Promise<Readonly<{ chunks: ReadonlyArray<Uint8Array>; byteLength: number }>> => {
  const next = await reader.read()
  if (next.done) {
    return { chunks, byteLength }
  }
  const nextByteLength = byteLength + next.value.byteLength
  if (nextByteLength > SARAH_FLEET_RUN_REQUEST_MAX_BYTES) {
    await reader.cancel().catch(() => undefined)
    throw new SarahFleetRunRequestBodyError()
  }
  return collectBoundedRequestChunks(
    reader,
    [...chunks, next.value],
    nextByteLength,
  )
}

const readBoundedJson = async (request: Request): Promise<unknown> => {
  const contentLength = request.headers.get('content-length')
  const declaredLength =
    contentLength === null || contentLength.trim() === ''
      ? undefined
      : Number(contentLength)
  if (
    declaredLength !== undefined &&
    (!Number.isSafeInteger(declaredLength) ||
      declaredLength < 0 ||
      declaredLength > SARAH_FLEET_RUN_REQUEST_MAX_BYTES)
  ) {
    throw new SarahFleetRunRequestBodyError()
  }
  if (request.body === null) {
    return {}
  }
  const collected = await collectBoundedRequestChunks(request.body.getReader())
  const bytes = new Uint8Array(collected.byteLength)
  collected.chunks.reduce((offset, chunk) => {
    bytes.set(chunk, offset)
    return offset + chunk.byteLength
  }, 0)
  const text = new TextDecoder().decode(bytes)
  if (text.trim() === '') {
    return {}
  }
  try {
    return parseJsonUnknown(text)
  } catch {
    throw new SarahFleetRunRequestBodyError()
  }
}

const authorityErrorStatus = (
  error: FleetRunAuthorityError,
): Readonly<{ status: number; retryable: boolean }> => {
  const byKind: Record<
    FleetRunAuthorityError['kind'],
    Readonly<{ status: number; retryable: boolean }>
  > = {
    invalid_request: { status: 400, retryable: false },
    idempotency_conflict: { status: 409, retryable: false },
    run_not_found: { status: 404, retryable: false },
    pylon_not_authorized: { status: 403, retryable: false },
    pylon_unavailable: { status: 409, retryable: true },
    claim_conflict: { status: 409, retryable: false },
    claim_not_found: { status: 404, retryable: false },
    claim_expired: { status: 409, retryable: true },
    storage_unavailable: { status: 503, retryable: true },
  }
  return byKind[error.kind]
}

const authorityErrorResponse = (
  error: FleetRunAuthorityError,
): HttpResponse => {
  const mapped = authorityErrorStatus(error)
  return noStoreJsonResponse(
    {
      ok: false,
      error: {
        code: error.kind,
        retryable: mapped.retryable,
      },
      routeRef: SARAH_FLEET_RUNS_ROUTE_REF,
    },
    { status: mapped.status },
  )
}

const serviceUnavailable = (code: string): HttpResponse =>
  noStoreJsonResponse(
    {
      ok: false,
      error: { code, retryable: true },
      routeRef: SARAH_FLEET_RUNS_ROUTE_REF,
    },
    { status: 503 },
  )

const invalidRequest = (): HttpResponse =>
  noStoreJsonResponse(
    {
      ok: false,
      error: { code: 'invalid_request', retryable: false },
      routeRef: SARAH_FLEET_RUNS_ROUTE_REF,
    },
    { status: 400 },
  )

const authorityOutcome = <A>(
  operation: Effect.Effect<A, FleetRunAuthorityError>,
): Effect.Effect<
  | Readonly<{ kind: 'failure'; error: FleetRunAuthorityError }>
  | Readonly<{ kind: 'success'; value: A }>
> =>
  operation.pipe(
    Effect.match({
      onFailure: error => ({ kind: 'failure' as const, error }),
      onSuccess: value => ({ kind: 'success' as const, value }),
    }),
  )

const promiseOutcome = <A>(
  operation: () => Promise<A>,
): Effect.Effect<
  | Readonly<{ kind: 'failure'; error: SarahFleetRunDependencyError }>
  | Readonly<{ kind: 'success'; value: A }>
> =>
  Effect.tryPromise({
    try: operation,
    catch: cause => new SarahFleetRunDependencyError({ cause }),
  }).pipe(
    Effect.match({
      onFailure: error => ({ kind: 'failure' as const, error }),
      onSuccess: value => ({ kind: 'success' as const, value }),
    }),
  )

const bindingConnectionString = (
  binding: KhalaSyncHyperdriveBinding | undefined,
): string | undefined => {
  const connectionString = binding?.connectionString
  return typeof connectionString === 'string' && connectionString.length > 0
    ? connectionString
    : undefined
}

export const makeSarahFleetRunRoutes = <
  Bindings extends SarahFleetRunRouteEnv,
>(
  dependencies: SarahFleetRunRouteDependencies<Bindings>,
) => {
  const handle = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> =>
    Effect.gen(function* () {
      if (request.method !== 'POST' && request.method !== 'GET') {
        return methodNotAllowed(['GET', 'POST'])
      }

      const authentication = yield* promiseOutcome(() =>
        dependencies.authenticateOwner(request, env, ctx),
      )
      if (authentication.kind === 'failure') {
        return serviceUnavailable('authentication_unavailable')
      }
      if (authentication.value === undefined) {
        return noStoreJsonResponse(
          {
            ok: false,
            error: { code: 'unauthenticated', retryable: false },
            routeRef: SARAH_FLEET_RUNS_ROUTE_REF,
          },
          { status: 401 },
        )
      }
      const owner = authentication.value
      const respond = (response: HttpResponse): HttpResponse =>
        owner.appendRefreshedSessionCookies?.(response) ?? response

      const policyResult = yield* promiseOutcome(async () =>
        sarahFleetRunPolicyForMode(
          S.decodeUnknownSync(SarahRelationshipMode)(
            await dependencies.resolveRelationshipMode(owner, env),
          ),
        ),
      )
      if (policyResult.kind === 'failure') {
        return respond(serviceUnavailable('relationship_policy_unavailable'))
      }
      const policy = policyResult.value
      if (
        !policy.codingFleetStartAllowed ||
        !policy.fleetObservationAllowed
      ) {
        return respond(
          noStoreJsonResponse(
            {
              ok: false,
              error: {
                code: 'relationship_not_authorized',
                retryable: false,
              },
              policy,
              routeRef: SARAH_FLEET_RUNS_ROUTE_REF,
            },
            { status: 403 },
          ),
        )
      }

      const url = new URL(request.url)
      const allowedQueryKeys = request.method === 'GET' ? ['runRef'] : []
      if (
        [...url.searchParams.keys()].some(
          key => !allowedQueryKeys.includes(key),
        )
      ) {
        return respond(invalidRequest())
      }

      const connectionString = bindingConnectionString(
        (dependencies.bindingForEnv ?? (value => value.KHALA_SYNC_DB))(env),
      )
      if (connectionString === undefined) {
        return respond(serviceUnavailable('storage_unavailable'))
      }

      const makeSqlClient =
        dependencies.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
      const clientResult = yield* promiseOutcome(() =>
        makeSqlClient(connectionString),
      )
      if (clientResult.kind === 'failure') {
        return respond(serviceUnavailable('storage_unavailable'))
      }
      const client: KhalaSyncPushSqlClient = clientResult.value
      const storageOperation = Effect.gen(function* () {
        const repository =
          dependencies.makeRepository?.(client.sql) ??
          makeFleetRunAuthorityRepository({ sql: client.sql })

        if (request.method === 'POST') {
          const bodyResult = yield* promiseOutcome(() =>
            readBoundedJson(request),
          )
          if (bodyResult.kind === 'failure') {
            return respond(
              bodyResult.error.cause instanceof SarahFleetRunRequestBodyError
                ? invalidRequest()
                : serviceUnavailable('storage_unavailable'),
            )
          }
          const outcome = yield* authorityOutcome(
            repository.start({
              ownerUserId: owner.userId,
              request: bodyResult.value,
            }),
          )
          return respond(
            outcome.kind === 'failure'
              ? authorityErrorResponse(outcome.error)
              : noStoreJsonResponse({
                  ok: true,
                  duplicate: outcome.value.duplicate,
                  policy,
                  routeRef: SARAH_FLEET_RUNS_ROUTE_REF,
                  run: publicFleetRunAuthorityRecord(outcome.value.record),
                }),
          )
        }

        const runRef = url.searchParams.get('runRef')
        if (runRef === null) {
          return respond(invalidRequest())
        }
        const outcome = yield* authorityOutcome(
          repository.observe({ ownerUserId: owner.userId, runRef }),
        )
        return respond(
          outcome.kind === 'failure'
            ? authorityErrorResponse(outcome.error)
            : noStoreJsonResponse({
                ok: true,
                policy,
                routeRef: SARAH_FLEET_RUNS_ROUTE_REF,
                run: publicFleetRunAuthorityRecord(outcome.value.record),
              }),
        )
      }).pipe(
        Effect.catchDefect(() =>
          Effect.succeed(respond(serviceUnavailable('storage_unavailable'))),
        ),
        Effect.ensuring(
          Effect.tryPromise({
            try: () => client.end(),
            catch: cause => new SarahFleetRunDependencyError({ cause }),
          }).pipe(Effect.ignore),
        ),
      )

      return yield* storageOperation
    })

  return { handle }
}
