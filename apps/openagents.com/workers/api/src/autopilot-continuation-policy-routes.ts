import { Effect, Match as M, Schema as S } from 'effect'

import type { AgentRegistrationStore } from './agent-registration'
import {
  AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_DAY,
  AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_RUN,
  AUTOPILOT_CONTINUATION_MAX_PER_DAY_LIMIT,
  AUTOPILOT_CONTINUATION_MAX_PER_RUN_LIMIT,
  AutopilotContinuationPolicyError,
  type AutopilotContinuationStore,
  autopilotContinuationPolicyProjection,
} from './autopilot-continuation-policy'
import {
  authenticateCustomerOrderAgentRequest,
  CustomerOrderAgentAuthFailure,
  type CustomerOrderAgentScope,
} from './customer-order-agent-auth'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { readJsonObject } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

const AutopilotContinuationPolicyUpdateRequest = S.Struct({
  enabled: S.Boolean,
  maxContinuationsPerDay: S.optionalKey(S.Number),
  maxContinuationsPerRun: S.optionalKey(S.Number),
})
type AutopilotContinuationPolicyUpdateRequest =
  typeof AutopilotContinuationPolicyUpdateRequest.Type

const boundedCounter = (
  field: string,
  value: number | undefined,
  fallback: number,
  maximum: number,
): Effect.Effect<number, AutopilotContinuationPolicyError> => {
  if (value === undefined) {
    return Effect.succeed(fallback)
  }

  if (!Number.isInteger(value) || value < 1 || value > maximum) {
    return Effect.fail(
      new AutopilotContinuationPolicyError({
        kind: 'validation_error',
        reason: `${field} must be an integer between 1 and ${maximum}.`,
      }),
    )
  }

  return Effect.succeed(value)
}

type AutopilotContinuationRouteEnv = Readonly<Record<string, unknown>>

type AutopilotContinuationPolicyRoutesDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  makeStore: (env: Bindings) => AutopilotContinuationStore
  nowIso?: () => string
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: Readonly<{ userId: string }> }> | undefined>
}>

const hasBearerAuthorization = (request: Request): boolean =>
  request.headers.get('authorization')?.trim().toLowerCase().startsWith(
    'bearer ',
  ) === true

const authenticateContinuationPolicyRequest = <
  Bindings extends AutopilotContinuationRouteEnv,
>(
  dependencies: AutopilotContinuationPolicyRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  input: Readonly<{
    ctx: ExecutionContext
    nowIso: () => string
    requiredScope: CustomerOrderAgentScope
  }>,
): Effect.Effect<
  Readonly<{ ownerUserId: string }>,
  AutopilotContinuationPolicyError | CustomerOrderAgentAuthFailure
> =>
  hasBearerAuthorization(request) ||
  dependencies.requireBrowserSession === undefined
    ? authenticateCustomerOrderAgentRequest(
        request,
        dependencies.agentStore(env),
        {
          nowIso: input.nowIso,
          requiredScope: input.requiredScope,
        },
      ).pipe(Effect.map(auth => ({ ownerUserId: auth.ownerUserId })))
    : Effect.gen(function* () {
        const session = yield* Effect.tryPromise({
          catch: error =>
            new AutopilotContinuationPolicyError({
              kind: 'storage_error',
              reason: error instanceof Error ? error.message : String(error),
            }),
          try: () =>
            dependencies.requireBrowserSession?.(request, env, input.ctx) ??
            Promise.resolve(undefined),
        })

        if (session === undefined) {
          return yield* new CustomerOrderAgentAuthFailure({
            failureKind: 'missing_credentials',
            reason:
              'Autopilot continuation policy requires a browser session or agent token.',
          })
        }

        return { ownerUserId: session.user.userId }
      })

const policyErrorResponse = (
  error: AutopilotContinuationPolicyError,
): HttpResponse =>
  noStoreJsonResponse(
    {
      error: `autopilot_continuation_policy_${error.kind}`,
      reason: error.reason,
    },
    { status: error.kind === 'storage_error' ? 500 : 400 },
  )

const readContinuationPolicy = <
  Bindings extends AutopilotContinuationRouteEnv,
>(
  dependencies: AutopilotContinuationPolicyRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
    const auth = yield* authenticateContinuationPolicyRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.read',
      },
    )
    const record = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotContinuationPolicyError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).readPolicy(auth.ownerUserId),
    })

    return noStoreJsonResponse({
      generatedAt: nowIso,
      policy: autopilotContinuationPolicyProjection(record, nowIso),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(policyErrorResponse(error))),
  )

const updateContinuationPolicy = <
  Bindings extends AutopilotContinuationRouteEnv,
>(
  dependencies: AutopilotContinuationPolicyRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
    const auth = yield* authenticateContinuationPolicyRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.write',
      },
    )
    const body = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotContinuationPolicyError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: async () =>
        S.decodeUnknownSync(AutopilotContinuationPolicyUpdateRequest)(
          await readJsonObject(request),
        ),
    })
    const maxContinuationsPerRun = yield* boundedCounter(
      'maxContinuationsPerRun',
      body.maxContinuationsPerRun,
      AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_RUN,
      AUTOPILOT_CONTINUATION_MAX_PER_RUN_LIMIT,
    )
    const maxContinuationsPerDay = yield* boundedCounter(
      'maxContinuationsPerDay',
      body.maxContinuationsPerDay,
      AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_DAY,
      AUTOPILOT_CONTINUATION_MAX_PER_DAY_LIMIT,
    )
    const store = dependencies.makeStore(env)
    const existing = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotContinuationPolicyError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => store.readPolicy(auth.ownerUserId),
    })
    const record = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotContinuationPolicyError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        store.upsertPolicy({
          createdAt: existing?.createdAt ?? nowIso,
          enabled: body.enabled,
          maxContinuationsPerDay,
          maxContinuationsPerRun,
          updatedAt: nowIso,
          userId: auth.ownerUserId,
        }),
    })

    return noStoreJsonResponse({
      generatedAt: nowIso,
      policy: autopilotContinuationPolicyProjection(record, nowIso),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(policyErrorResponse(error))),
  )

export const makeAutopilotContinuationPolicyRoutes = <
  Bindings extends AutopilotContinuationRouteEnv,
>(
  dependencies: AutopilotContinuationPolicyRoutesDependencies<Bindings>,
) => ({
  routeAutopilotContinuationPolicyRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> =>
    M.value(request.method).pipe(
      M.when('GET', () =>
        readContinuationPolicy(dependencies, request, env, ctx)
      ),
      M.when('PUT', () =>
        updateContinuationPolicy(dependencies, request, env, ctx)
      ),
      M.orElse(() => Effect.succeed(methodNotAllowed(['GET', 'PUT']))),
    ),
})
