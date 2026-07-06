import { Effect, Match as M, Schema as S } from 'effect'

import {
  ClientGroupId,
  ClientId,
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationEnvelope,
  MutationId,
  MutatorName,
  PushRequest,
  SyncSchemaVersion,
} from '@openagentsinc/khala-sync'
import {
  executePush as executeKhalaSyncPush,
  readRuntimeTurnById,
  RUNTIME_RECORD_EVENT_MUTATOR_NAME,
  type MutatorRegistry,
} from '@openagentsinc/khala-sync-server'

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
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  TokenUsageLedgerUnsafePayload,
  TokenUsageLedgerValidationError,
} from './token-usage-ledger'
import type { MeteringHook, MeteringOutcome } from './inference/metering-hook'

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
const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/

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
  meteringHook?: ((env: Bindings) => MeteringHook) | undefined
  nowIso?: () => string
  publishDelta?: (
    env: Bindings,
    input: Readonly<{
      eventRef: string
      observedAt: string
      tokensServedDelta: number
    }>,
  ) => Effect.Effect<void, unknown>
  publishInsufficientCreditEvent?: (
    env: Bindings,
    input: KhalaCloudRuntimeInsufficientCreditEventInput,
  ) => Effect.Effect<KhalaCloudRuntimeInsufficientCreditEventPublishOutcome>
}>

export type KhalaCloudRuntimeInsufficientCreditEventInput = Readonly<{
  chargeReceiptRef: string | null
  lane: KhalaCloudRuntimeUsageIngest['lane']
  observedAt: string
  ownerUserId: string
  threadId: string
  tokenUsageEventRef: string
  turnId: string
}>

export type KhalaCloudRuntimeInsufficientCreditEventPublishOutcome = Readonly<{
  eventRef: string | null
  published: boolean
  reason?:
    | 'event_already_recorded'
    | 'khala_sync_storage_unconfigured'
    | 'runtime_turn_not_found'
    | 'runtime_turn_owner_mismatch'
    | 'runtime_event_rejected'
    | 'runtime_event_storage_failed'
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

const ownerCreditAccountRef = (ownerUserId: string): string =>
  `agent:${ownerUserId}`

const safeRefPart = (value: string): string => {
  const sanitized = value.replace(/[^A-Za-z0-9._:-]/g, '_').slice(0, 96)
  return sanitized === '' || !/^[A-Za-z0-9]/.test(sanitized)
    ? 'ref'
    : sanitized
}

const safeCausalityRef = (value: string | null): ReadonlyArray<string> =>
  value !== null && SAFE_REF_PATTERN.test(value) ? [value] : []

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
      ].join(':'),
    ),
  )

const meteringContextFromUsage = (
  input: Readonly<{
    body: KhalaCloudRuntimeUsageIngestBody
    digest: string
  }>,
) => {
  const counts = khalaCloudRuntimeUsageTokenCounts(input.body.usage)
  return {
    accountRef: ownerCreditAccountRef(input.body.ownerUserId),
    adapterId: input.body.backendProfile ?? input.body.provider,
    fundingKind: 'card' as const,
    requestId: `khala-cloud-runtime.${input.digest.slice(0, 32)}`,
    requestedModel: input.body.model,
    servedModel: input.body.model,
    streamed: false,
    usage: {
      ...(counts.cacheReadTokens === 0
        ? {}
        : { cachedPromptTokens: counts.cacheReadTokens }),
      completionTokens: counts.outputTokens,
      promptTokens: counts.inputTokens,
      totalTokens: counts.totalTokens,
    },
  }
}

export type KhalaCloudRuntimeInsufficientCreditPublisherDeps = Readonly<{
  binding: KhalaSyncHyperdriveBinding | undefined
  registry: MutatorRegistry
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
}>

export const publishKhalaCloudRuntimeInsufficientCreditEvent = (
  deps: KhalaCloudRuntimeInsufficientCreditPublisherDeps,
  input: KhalaCloudRuntimeInsufficientCreditEventInput,
): Effect.Effect<KhalaCloudRuntimeInsufficientCreditEventPublishOutcome> =>
  Effect.tryPromise({
    catch: () =>
      ({
        eventRef: null,
        published: false,
        reason: 'runtime_event_storage_failed',
      }) satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome,
    try: async () => {
      if (
        deps.binding === undefined ||
        typeof deps.binding.connectionString !== 'string' ||
        deps.binding.connectionString.length === 0
      ) {
        return {
          eventRef: null,
          published: false,
          reason: 'khala_sync_storage_unconfigured',
        } satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome
      }

      const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
      const client = await makeSqlClient(deps.binding.connectionString)
      try {
        const turn = await readRuntimeTurnById(client.sql, {
          turnId: input.turnId,
        })
        if (turn === null) {
          return {
            eventRef: null,
            published: false,
            reason: 'runtime_turn_not_found',
          } satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome
        }
        if (
          turn.ownerUserId !== input.ownerUserId ||
          turn.threadId !== input.threadId
        ) {
          return {
            eventRef: null,
            published: false,
            reason: 'runtime_turn_owner_mismatch',
          } satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome
        }

        const eventRef = `event.khala_cloud_billing.insufficient_credit.${safeRefPart(input.turnId)}`
        const rawEventRef = `billing.insufficient_credit.${safeRefPart(input.turnId)}`
        const event = {
          causalityRefs: [
            ...safeCausalityRef(input.tokenUsageEventRef),
            ...safeCausalityRef(input.chargeReceiptRef),
          ],
          eventId: eventRef,
          kind: 'raw.sidecar_ref',
          observedAt: input.observedAt,
          rawEventKind: 'other',
          rawEventRef,
          redactionClass: 'private_ref',
          schema: 'openagents.khala_runtime_event.v1',
          sequence: Number(turn.eventCount) + 1,
          source: {
            adapterKind: input.lane === 'claude_pylon' ? 'claude_code' : 'codex',
            lane: input.lane,
            surface: 'server',
          },
          threadId: input.threadId,
          turnId: input.turnId,
          visibility: 'private',
        }
        const request = new PushRequest({
          clientGroupId: ClientGroupId.make(
            `server.khala_cloud_billing.${safeRefPart(eventRef)}`,
          ),
          clientId: ClientId.make('openagents.worker.khala_cloud_runtime_usage'),
          mutations: [
            new MutationEnvelope({
              argsJson: JSON.stringify(event),
              mutationId: MutationId.make(1),
              name: MutatorName.make(RUNTIME_RECORD_EVENT_MUTATOR_NAME),
            }),
          ],
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          schemaVersion: SyncSchemaVersion.make(1),
        })
        const response = await executeKhalaSyncPush({
          registry: deps.registry,
          request,
          sql: client.sql,
          userId: input.ownerUserId,
        })
        const result = response.results[0]
        if (result?.status === 'applied') {
          return {
            eventRef,
            published: true,
          } satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome
        }
        if (
          result?.status === 'rejected' &&
          result.errorCode === 'runtime_event_exists'
        ) {
          return {
            eventRef,
            published: true,
            reason: 'event_already_recorded',
          } satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome
        }
        return {
          eventRef,
          published: false,
          reason: 'runtime_event_rejected',
        } satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome
      } finally {
        await client.end()
      }
    },
  }).pipe(Effect.catch(error => Effect.succeed(error)))

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
      accountRef: ownerCreditAccountRef(input.body.ownerUserId),
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

    const meteringHook = dependencies.meteringHook?.(env)
    const tokenCharge: MeteringOutcome | undefined =
      meteringHook === undefined
        ? undefined
        : yield* meteringHook(
            meteringContextFromUsage({
              body,
              digest,
            }),
          )

    const insufficientCreditEvent =
      tokenCharge?.metered === false &&
      tokenCharge.failureReason === 'insufficient_credit' &&
      dependencies.publishInsufficientCreditEvent !== undefined
        ? yield* dependencies
            .publishInsufficientCreditEvent(env, {
              chargeReceiptRef: tokenCharge.receiptRef,
              lane: body.lane,
              observedAt,
              ownerUserId: body.ownerUserId,
              threadId: body.threadId,
              tokenUsageEventRef: tokenBody.eventId,
              turnId: body.turnId,
            })
        : ({
            eventRef: null,
            published: false,
          } satisfies KhalaCloudRuntimeInsufficientCreditEventPublishOutcome)

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
      insufficientCreditEventPublished: insufficientCreditEvent.published,
      insufficientCreditEventRef: insufficientCreditEvent.eventRef,
      insufficientCreditEventReason: insufficientCreditEvent.reason ?? null,
      lane: body.lane,
      ownerUserId: body.ownerUserId,
      threadId: body.threadId,
      tokenChargeFailureReason: tokenCharge?.failureReason ?? null,
      tokenChargeMetered: tokenCharge?.metered ?? false,
      tokenChargeReceiptRef: tokenCharge?.receiptRef ?? null,
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
