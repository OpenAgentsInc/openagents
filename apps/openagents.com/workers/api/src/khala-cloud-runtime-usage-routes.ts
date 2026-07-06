import { Effect, Match as M, Schema as S } from 'effect'

import {
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  sha256Hex,
} from './agent-registration'
import {
  readAgentBearerToken as bearerTokenFromRequest,
} from './auth/bearer-token'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, parseJsonUnknown } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  TokenUsageLedgerUnsafePayload,
  TokenUsageLedgerValidationError,
} from './token-usage-ledger'

type HttpResponse = globalThis.Response

export const KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH =
  '/api/khala/cloud/runtime-turn-usage'

export const KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION =
  'openagents.khala_cloud_runtime_turn_usage.v1' as const

const KHALA_CLOUD_RUNTIME_RESULT_SCHEMA_VERSION =
  'openagents.khala_cloud_runtime_turn_usage_result.v1' as const
const KHALA_CLOUD_RUNTIME_PRODUCER_SYSTEM = 'omega' as const
const KHALA_CLOUD_RUNTIME_SOURCE_ROUTE = 'omega_hosted_gemini' as const
const KHALA_CLOUD_RUNTIME_DEMAND_KIND = 'external' as const
const KHALA_CLOUD_RUNTIME_DEMAND_SOURCE =
  'khala_mobile_org_cloud_runtime' as const
const KHALA_CLOUD_RUNTIME_DEMAND_CLIENT = 'khala-code-mobile' as const
const KHALA_CLOUD_RUNTIME_DEMAND_CHANNEL = 'khala_api' as const
const MAX_BODY_BYTES = 128 * 1024

const NonEmptyString = S.Trim.check(S.isMinLength(1), S.isMaxLength(512))
const BoundedSafeRef = S.Trim.check(S.isMinLength(1), S.isMaxLength(1024))
const NonNegativeInt = S.Int.check(S.isGreaterThanOrEqualTo(0))

class KhalaCloudRuntimeUsage extends S.Class<KhalaCloudRuntimeUsage>(
  'KhalaCloudRuntimeUsage',
)({
  usageRef: BoundedSafeRef,
  inputTokens: NonNegativeInt,
  outputTokens: NonNegativeInt,
  reasoningTokens: S.optionalKey(NonNegativeInt),
  cacheReadInputTokens: S.optionalKey(NonNegativeInt),
  cacheWriteInputTokens: S.optionalKey(NonNegativeInt),
  totalTokens: S.optionalKey(NonNegativeInt),
}) {}

class KhalaCloudRuntimeUsageIngestBody extends S.Class<KhalaCloudRuntimeUsageIngestBody>(
  'KhalaCloudRuntimeUsageIngestBody',
)({
  schemaVersion: S.Literal(KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION),
  ownerUserId: NonEmptyString,
  threadId: NonEmptyString,
  turnId: NonEmptyString,
  lane: S.Literals(['codex_app_server', 'claude_pylon', 'hosted_khala']),
  provider: NonEmptyString,
  model: NonEmptyString,
  backendProfile: S.optionalKey(NonEmptyString),
  observedAt: S.optionalKey(S.String.check(S.isMaxLength(80))),
  pylonRef: S.optionalKey(BoundedSafeRef),
  executorRef: S.optionalKey(BoundedSafeRef),
  runtimeEventId: S.optionalKey(BoundedSafeRef),
  usage: KhalaCloudRuntimeUsage,
}) {}

export type KhalaCloudRuntimeUsageIngest =
  typeof KhalaCloudRuntimeUsageIngestBody.Type

export type KhalaCloudRuntimeTokenCounts = Readonly<{
  cacheReadTokens: number
  cacheWrite1hTokens: number
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  totalTokens: number
}>

export type KhalaCloudRuntimeUsageDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  ledger: (env: Bindings) => TokenUsageLedgerShape
  nowIso?: () => string
  publishDelta?: (
    env: Bindings,
    input: Readonly<{
      eventRef: string
      observedAt: string
      tokensServedDelta: number
    }>,
  ) => Effect.Effect<void, unknown>
}>

class KhalaCloudRuntimeUnauthorized extends S.TaggedErrorClass<KhalaCloudRuntimeUnauthorized>()(
  'KhalaCloudRuntimeUnauthorized',
  {},
) {}

class KhalaCloudRuntimeForbidden extends S.TaggedErrorClass<KhalaCloudRuntimeForbidden>()(
  'KhalaCloudRuntimeForbidden',
  { reason: S.String },
) {}

class KhalaCloudRuntimeValidationError extends S.TaggedErrorClass<KhalaCloudRuntimeValidationError>()(
  'KhalaCloudRuntimeValidationError',
  { reason: S.String },
) {}

class KhalaCloudRuntimeStorageError extends S.TaggedErrorClass<KhalaCloudRuntimeStorageError>()(
  'KhalaCloudRuntimeStorageError',
  { operation: S.String, reason: S.String },
) {}

type KhalaCloudRuntimeRouteError =
  | KhalaCloudRuntimeForbidden
  | KhalaCloudRuntimeStorageError
  | KhalaCloudRuntimeUnauthorized
  | KhalaCloudRuntimeValidationError

const routeErrorResponse = (
  error: KhalaCloudRuntimeRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      KhalaCloudRuntimeUnauthorized: () => unauthorized(),
      KhalaCloudRuntimeForbidden: error =>
        noStoreJsonResponse(
          { error: 'khala_cloud_runtime_forbidden', reason: error.reason },
          { status: 403 },
        ),
      KhalaCloudRuntimeValidationError: error =>
        noStoreJsonResponse(
          { error: 'khala_cloud_runtime_validation_error', reason: error.reason },
          { status: 400 },
        ),
      KhalaCloudRuntimeStorageError: error =>
        noStoreJsonResponse(
          {
            error: 'khala_cloud_runtime_storage_error',
            operation: error.operation,
            reason: error.reason,
          },
          { status: 503 },
        ),
    }),
    M.exhaustive,
  )

const requireAgent = <Bindings>(
  dependencies: KhalaCloudRuntimeUsageDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<ProgrammaticAgentSession, KhalaCloudRuntimeUnauthorized> => {
  const token = bearerTokenFromRequest(request)
  if (token === undefined) {
    return Effect.fail(new KhalaCloudRuntimeUnauthorized({}))
  }
  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new KhalaCloudRuntimeUnauthorized({}),
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStore(env),
          token,
          dependencies.nowIso,
        ),
    }),
    session =>
      session === undefined
        ? Effect.fail(new KhalaCloudRuntimeUnauthorized({}))
        : Effect.succeed(session),
  )
}

const routeNowIso = <Bindings>(
  dependencies: KhalaCloudRuntimeUsageDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const storageReason = (error: unknown): string =>
  error instanceof TokenUsageLedgerStorageError ||
  error instanceof TokenUsageLedgerUnsafePayload ||
  error instanceof TokenUsageLedgerValidationError
    ? error._tag
    : error instanceof Error
      ? error.message
      : String(error)

export const khalaCloudRuntimeUsageTokenCounts = (
  usage: KhalaCloudRuntimeUsage,
): KhalaCloudRuntimeTokenCounts => {
  const inputTokens = Math.max(0, Math.trunc(usage.inputTokens))
  const reasoningTokens = Math.max(0, Math.trunc(usage.reasoningTokens ?? 0))
  const outputTokens =
    Math.max(0, Math.trunc(usage.outputTokens)) + reasoningTokens
  const minimumTotal = inputTokens + outputTokens
  return {
    cacheReadTokens: Math.max(0, Math.trunc(usage.cacheReadInputTokens ?? 0)),
    cacheWrite1hTokens: Math.max(
      0,
      Math.trunc(usage.cacheWriteInputTokens ?? 0),
    ),
    inputTokens,
    outputTokens,
    reasoningTokens,
    totalTokens: Math.max(minimumTotal, Math.trunc(usage.totalTokens ?? 0)),
  }
}

const validateExactUsage = (
  body: KhalaCloudRuntimeUsageIngestBody,
): Effect.Effect<void, KhalaCloudRuntimeValidationError> => {
  const counts = khalaCloudRuntimeUsageTokenCounts(body.usage)
  if (counts.inputTokens + counts.outputTokens <= 0) {
    return Effect.fail(
      new KhalaCloudRuntimeValidationError({
        reason:
          'Khala cloud runtime usage must include at least one exact input or output token.',
      }),
    )
  }
  if (
    body.usage.totalTokens !== undefined &&
    body.usage.totalTokens < counts.inputTokens + counts.outputTokens
  ) {
    return Effect.fail(
      new KhalaCloudRuntimeValidationError({
        reason:
          'Khala cloud runtime usage totalTokens cannot be lower than the exact input/output total.',
      }),
    )
  }
  return Effect.void
}

const requireOwnerPostingAuthority = (
  session: ProgrammaticAgentSession,
  body: KhalaCloudRuntimeUsageIngestBody,
): Effect.Effect<void, KhalaCloudRuntimeForbidden> => {
  const linkedOwner = session.credential.openauthUserId?.trim()
  if (linkedOwner !== undefined && linkedOwner !== '' && linkedOwner !== body.ownerUserId) {
    return Effect.fail(
      new KhalaCloudRuntimeForbidden({
        reason:
          'linked user-pylon agents may only post runtime usage for their own owner user id',
      }),
    )
  }
  return Effect.void
}

const stableUsageDigest = (
  body: KhalaCloudRuntimeUsageIngestBody,
): Effect.Effect<string> =>
  Effect.promise(() =>
    sha256Hex(
      [
        'khala-cloud-runtime',
        body.ownerUserId,
        body.threadId,
        body.turnId,
        body.lane,
        body.provider,
        body.model,
        body.usage.usageRef,
      ].join(':'),
    ),
  )

const tokenUsageEventBody = (
  input: Readonly<{
    body: KhalaCloudRuntimeUsageIngestBody
    digest: string
    observedAt: string
    session: ProgrammaticAgentSession
  }>,
) => {
  const counts = khalaCloudRuntimeUsageTokenCounts(input.body.usage)
  return {
    schemaVersion: 'openagents.token_usage_event.v1' as const,
    actor: {
      accountRef: `agent:${input.session.user.id}`,
      userId: input.body.ownerUserId,
    },
    backendProfile: input.body.backendProfile ?? input.body.provider,
    demand: {
      demandChannel: KHALA_CLOUD_RUNTIME_DEMAND_CHANNEL,
      demandClient: KHALA_CLOUD_RUNTIME_DEMAND_CLIENT,
      demandKind: KHALA_CLOUD_RUNTIME_DEMAND_KIND,
      demandSource: KHALA_CLOUD_RUNTIME_DEMAND_SOURCE,
    },
    eventId: `event.inference.served-tokens.khala-cloud-runtime.${input.digest.slice(0, 32)}`,
    idempotencyKey: `khala:cloud-runtime-turn:${input.digest}`,
    model: input.body.model,
    observedAt: input.observedAt,
    privacy: { leaderboardEligible: false, privacyOptOut: false },
    producerSystem: KHALA_CLOUD_RUNTIME_PRODUCER_SYSTEM,
    provider: input.body.provider,
    roleRef: 'coder',
    safeMetadata: {
      executorMode: 'org_cloud',
      lane: input.body.lane,
      provider: input.body.provider,
      model: input.body.model,
      threadId: input.body.threadId,
      turnId: input.body.turnId,
      usageBasis: 'khala_runtime_usage_recorded',
      usageRef: input.body.usage.usageRef,
      ...(input.body.runtimeEventId === undefined
        ? {}
        : { runtimeEventId: input.body.runtimeEventId }),
      ...(input.body.pylonRef === undefined
        ? {}
        : { pylonRef: input.body.pylonRef }),
      ...(input.body.executorRef === undefined
        ? {}
        : { executorRef: input.body.executorRef }),
    },
    sourceRefs: {
      runRef: input.body.threadId,
      sessionRef: input.body.threadId,
      taskRef: input.body.turnId,
    },
    sourceRoute: KHALA_CLOUD_RUNTIME_SOURCE_ROUTE,
    tokenCounts: {
      cacheReadTokens: counts.cacheReadTokens,
      cacheWrite1hTokens: counts.cacheWrite1hTokens,
      cacheWrite5mTokens: 0,
      inputTokens: counts.inputTokens,
      outputTokens: counts.outputTokens,
      reasoningTokens: counts.reasoningTokens,
      totalTokens: counts.totalTokens,
    },
    usageTruth: 'exact' as const,
  }
}

const routeUsageIngest = <Bindings>(
  dependencies: KhalaCloudRuntimeUsageDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, KhalaCloudRuntimeRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireAgent(dependencies, request, env)

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new KhalaCloudRuntimeValidationError({
          reason: 'Request body could not be read.',
        }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_BODY_BYTES) {
      return yield* new KhalaCloudRuntimeValidationError({
        reason: `Khala cloud runtime usage payload exceeds the ${MAX_BODY_BYTES}-byte limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new KhalaCloudRuntimeValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the Khala cloud runtime usage schema.',
        }),
      try: () =>
        decodeUnknownWithSchema(
          KhalaCloudRuntimeUsageIngestBody,
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody),
        ),
    })

    yield* requireOwnerPostingAuthority(session, body)
    yield* validateExactUsage(body)

    const observedAt = body.observedAt ?? routeNowIso(dependencies)
    const digest = yield* stableUsageDigest(body)
    const tokenBody = tokenUsageEventBody({
      body,
      digest,
      observedAt,
      session,
    })
    const counts = khalaCloudRuntimeUsageTokenCounts(body.usage)
    const tokensServed = counts.inputTokens + counts.outputTokens

    const tokenResult = yield* dependencies
      .ledger(env)
      .ingestEvent(tokenBody)
      .pipe(
        Effect.mapError(
          error =>
            new KhalaCloudRuntimeStorageError({
              operation: 'token_usage_ingest',
              reason: storageReason(error),
            }),
        ),
      )

    if (
      tokenResult.inserted &&
      dependencies.publishDelta !== undefined &&
      tokensServed > 0
    ) {
      yield* dependencies
        .publishDelta(env, {
          eventRef: tokenBody.eventId,
          observedAt,
          tokensServedDelta: tokensServed,
        })
        .pipe(Effect.catch(() => Effect.void))
    }

    return noStoreJsonResponse({
      schemaVersion: KHALA_CLOUD_RUNTIME_RESULT_SCHEMA_VERSION,
      insertedTokenUsage: tokenResult.inserted,
      lane: body.lane,
      ownerUserId: body.ownerUserId,
      threadId: body.threadId,
      tokenUsageEventRef: tokenBody.eventId,
      tokensServedDelta: tokenResult.inserted ? tokensServed : 0,
      turnId: body.turnId,
      usageTruth: 'exact' as const,
    })
  })

export const makeKhalaCloudRuntimeUsageRoutes = <Bindings>(
  dependencies: KhalaCloudRuntimeUsageDependencies<Bindings>,
) => ({
  handleKhalaCloudRuntimeUsageIngestApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (new URL(request.url).pathname !== KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeUsageIngest(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
})
