import { Effect, Match as M, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  sha256Hex,
} from './agent-registration'
import {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifStep,
  AtifTrajectory,
  atifTraceTripwire,
  validateAtifTrajectory,
} from './atif-trace-schema'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, parseJsonUnknown } from './json-boundary'
import {
  type PylonApiAssignmentRecord,
  type PylonApiStore,
  pylonApiStoreErrorFromUnknown,
} from './pylon-api'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  TokenUsageLedgerUnsafePayload,
  TokenUsageLedgerValidationError,
} from './token-usage-ledger'
import {
  type TraceStore,
  traceStoreErrorFromUnknown,
} from './trace-store-d1'
import {
  redactTraceValue,
  type TraceRedactionReport,
} from './inference/trace-redaction'

type HttpResponse = globalThis.Response

export const PYLON_CODEX_TURN_INGEST_PATH = '/api/pylon/codex/turns'

const PYLON_CODEX_SCHEMA_VERSION = 'openagents.pylon.codex_turn.v1' as const
const PYLON_CODEX_MODEL_NAME = 'openagents/pylon-codex' as const
const PYLON_CODEX_PROVIDER = 'pylon-codex-own-capacity' as const
const PYLON_CODEX_PRODUCER_SYSTEM = 'omega' as const
const PYLON_CODEX_SOURCE_ROUTE = 'omega_hosted_gemini' as const
const PYLON_CODEX_DEMAND_KIND = 'own_capacity' as const
const PYLON_CODEX_DEMAND_SOURCE = 'khala_coding_delegation' as const
const MAX_BODY_BYTES = 2 * 1024 * 1024

const NonEmptyString = S.Trim.check(S.isMinLength(1), S.isMaxLength(512))
const BoundedText = S.String.check(S.isMaxLength(64 * 1024))
const NonNegativeInt = S.Int.check(S.isGreaterThanOrEqualTo(0))
const PositiveInt = S.Int.check(S.isGreaterThanOrEqualTo(1))

class PylonCodexUsage extends S.Class<PylonCodexUsage>('PylonCodexUsage')({
  inputTokens: NonNegativeInt,
  cachedInputTokens: S.optionalKey(NonNegativeInt),
  outputTokens: NonNegativeInt,
  reasoningOutputTokens: S.optionalKey(NonNegativeInt),
}) {}

class PylonCodexTurnItem extends S.Class<PylonCodexTurnItem>(
  'PylonCodexTurnItem',
)({
  ordinal: PositiveInt,
  itemType: S.Literals([
    'agent_message',
    'reasoning',
    'command_execution',
    'file_change',
    'mcp_tool_call',
    'web_search',
    'error',
    'unknown',
  ]),
  status: S.optionalKey(S.String.check(S.isMaxLength(80))),
  message: S.optionalKey(BoundedText),
  reasoningSummary: S.optionalKey(BoundedText),
  commandLabel: S.optionalKey(S.String.check(S.isMaxLength(120))),
  exitCode: S.optionalKey(S.Number),
  outputBytes: S.optionalKey(NonNegativeInt),
  changeCount: S.optionalKey(NonNegativeInt),
  toolName: S.optionalKey(S.String.check(S.isMaxLength(120))),
}) {}

class PylonCodexTurnIngestBody extends S.Class<PylonCodexTurnIngestBody>(
  'PylonCodexTurnIngestBody',
)({
  schemaVersion: S.Literal(PYLON_CODEX_SCHEMA_VERSION),
  assignmentRef: NonEmptyString,
  leaseRef: NonEmptyString,
  pylonRef: NonEmptyString,
  runRef: S.optionalKey(NonEmptyString),
  sessionRef: S.optionalKey(NonEmptyString),
  workspaceRef: S.optionalKey(NonEmptyString),
  turnIndex: PositiveInt,
  observedAt: S.optionalKey(S.String.check(S.isMaxLength(80))),
  usage: PylonCodexUsage,
  items: S.Array(PylonCodexTurnItem),
}) {}

export type PylonCodexTurnIngest =
  typeof PylonCodexTurnIngestBody.Type

export type PylonCodexTokenCounts = Readonly<{
  cacheReadTokens: number
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  totalTokens: number
}>

type PylonCodexTurnIngestDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  ledger: (env: Bindings) => TokenUsageLedgerShape
  makeId?: () => string
  nowIso?: () => string
  pylonStore: (env: Bindings) => Pick<PylonApiStore, 'readAssignment'>
  publishDelta?: (
    env: Bindings,
    input: Readonly<{
      eventRef: string
      observedAt: string
      tokensServedDelta: number
    }>,
  ) => Effect.Effect<void, unknown>
  traceStore: (env: Bindings) => TraceStore
}>

class PylonCodexUnauthorized extends S.TaggedErrorClass<PylonCodexUnauthorized>()(
  'PylonCodexUnauthorized',
  {},
) {}

class PylonCodexForbidden extends S.TaggedErrorClass<PylonCodexForbidden>()(
  'PylonCodexForbidden',
  { reason: S.String },
) {}

class PylonCodexNotFound extends S.TaggedErrorClass<PylonCodexNotFound>()(
  'PylonCodexNotFound',
  { reason: S.String },
) {}

class PylonCodexValidationError extends S.TaggedErrorClass<PylonCodexValidationError>()(
  'PylonCodexValidationError',
  { reason: S.String },
) {}

class PylonCodexTraceRejected extends S.TaggedErrorClass<PylonCodexTraceRejected>()(
  'PylonCodexTraceRejected',
  { findings: S.Array(S.String), redactionReport: S.optionalKey(S.Unknown) },
) {}

class PylonCodexStorageError extends S.TaggedErrorClass<PylonCodexStorageError>()(
  'PylonCodexStorageError',
  { operation: S.String, reason: S.String },
) {}

type PylonCodexRouteError =
  | PylonCodexForbidden
  | PylonCodexNotFound
  | PylonCodexStorageError
  | PylonCodexTraceRejected
  | PylonCodexUnauthorized
  | PylonCodexValidationError

const routeErrorResponse = (error: PylonCodexRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      PylonCodexUnauthorized: () => unauthorized(),
      PylonCodexForbidden: error =>
        noStoreJsonResponse(
          { error: 'pylon_codex_forbidden', reason: error.reason },
          { status: 403 },
        ),
      PylonCodexNotFound: error =>
        noStoreJsonResponse(
          { error: 'pylon_codex_not_found', reason: error.reason },
          { status: 404 },
        ),
      PylonCodexValidationError: error =>
        noStoreJsonResponse(
          { error: 'pylon_codex_validation_error', reason: error.reason },
          { status: 400 },
        ),
      PylonCodexTraceRejected: error =>
        noStoreJsonResponse(
          {
            error: 'pylon_codex_trace_rejected',
            findings: error.findings,
            redactionReport: error.redactionReport,
          },
          { status: 422 },
        ),
      PylonCodexStorageError: error =>
        noStoreJsonResponse(
          {
            error: 'pylon_codex_storage_error',
            operation: error.operation,
            reason: error.reason,
          },
          { status: 503 },
        ),
    }),
    M.exhaustive,
  )

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')
  if (authorization === null) {
    return undefined
  }
  const [scheme, token] = authorization.split(' ')
  return scheme?.toLowerCase() === 'bearer' &&
    token !== undefined &&
    token.startsWith(AGENT_TOKEN_PREFIX)
    ? token
    : undefined
}

const requireAgent = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<ProgrammaticAgentSession, PylonCodexUnauthorized> => {
  const token = bearerTokenFromRequest(request)
  if (token === undefined) {
    return Effect.fail(new PylonCodexUnauthorized({}))
  }
  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new PylonCodexUnauthorized({}),
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStore(env),
          token,
          dependencies.nowIso,
        ),
    }),
    session =>
      session === undefined
        ? Effect.fail(new PylonCodexUnauthorized({}))
        : Effect.succeed(session),
  )
}

const routeNowIso = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

const ownerUserIdForAgent = (session: ProgrammaticAgentSession): string => {
  const linked = session.credential.openauthUserId?.trim()
  return linked === undefined || linked === '' ? session.user.id : linked
}

const storageReason = (error: unknown): string =>
  error instanceof TokenUsageLedgerStorageError ||
  error instanceof TokenUsageLedgerUnsafePayload ||
  error instanceof TokenUsageLedgerValidationError
    ? error._tag
    : error instanceof Error
      ? error.message
      : String(error)

const requireOwnedAssignment = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  session: ProgrammaticAgentSession,
  body: PylonCodexTurnIngestBody,
): Effect.Effect<
  PylonApiAssignmentRecord,
  PylonCodexForbidden | PylonCodexNotFound | PylonCodexStorageError
> =>
  Effect.gen(function* () {
    const assignment = yield* Effect.tryPromise({
      catch: error =>
        new PylonCodexStorageError({
          operation: 'pylon_assignment_read',
          reason: pylonApiStoreErrorFromUnknown(error).reason,
        }),
      try: () => dependencies.pylonStore(env).readAssignment(body.assignmentRef),
    })
    if (assignment === undefined) {
      return yield* new PylonCodexNotFound({
        reason: 'Pylon assignment was not found.',
      })
    }
    if (assignment.ownerAgentUserId !== session.user.id) {
      return yield* new PylonCodexForbidden({
        reason: 'Pylon assignment belongs to another agent.',
      })
    }
    if (assignment.pylonRef !== body.pylonRef) {
      return yield* new PylonCodexForbidden({
        reason: 'Pylon assignment is not assigned to this pylon.',
      })
    }
    return assignment
  })

const boundedText = (value: string | undefined, fallback: string): string => {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed === '') {
    return fallback
  }
  return trimmed.length > 16_000
    ? `${trimmed.slice(0, 16_000)}\n[TRUNCATED]`
    : trimmed
}

export const codexTurnUsageTokenCounts = (
  usage: PylonCodexUsage,
): PylonCodexTokenCounts => {
  const inputTokens = Math.max(0, Math.trunc(usage.inputTokens))
  const reasoningTokens = Math.max(
    0,
    Math.trunc(usage.reasoningOutputTokens ?? 0),
  )
  const outputTokens =
    Math.max(0, Math.trunc(usage.outputTokens)) + reasoningTokens
  return {
    cacheReadTokens: Math.max(0, Math.trunc(usage.cachedInputTokens ?? 0)),
    inputTokens,
    outputTokens,
    reasoningTokens,
    totalTokens: inputTokens + outputTokens,
  }
}

const itemToAtifStep = (
  item: PylonCodexTurnItem,
  stepId: number,
): AtifStep => {
  if (item.itemType === 'agent_message') {
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: boundedText(item.message, 'Codex produced an agent message.'),
      model_name: PYLON_CODEX_MODEL_NAME,
    })
  }

  if (item.itemType === 'reasoning') {
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: 'Codex produced a reasoning summary.',
      reasoning_content: boundedText(
        item.reasoningSummary ?? item.message,
        'Codex reasoning summary was present.',
      ),
      model_name: PYLON_CODEX_MODEL_NAME,
    })
  }

  if (item.itemType === 'command_execution') {
    const callId = `codex-command-${item.ordinal}`
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: `Codex completed a command execution${typeof item.exitCode === 'number' ? ` with exit code ${item.exitCode}` : ''}.`,
      model_name: PYLON_CODEX_MODEL_NAME,
      tool_calls: [
        {
          tool_call_id: callId,
          function_name: 'command_execution',
          arguments: {
            commandLabel: item.commandLabel ?? 'shell_command',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: callId,
            content: `exitCode=${typeof item.exitCode === 'number' ? item.exitCode : 'unknown'} outputBytes=${item.outputBytes ?? 0}`,
          },
        ],
      },
    })
  }

  if (item.itemType === 'file_change') {
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message: `Codex reported ${item.changeCount ?? 0} file change(s).`,
      model_name: PYLON_CODEX_MODEL_NAME,
    })
  }

  if (item.itemType === 'mcp_tool_call' || item.itemType === 'web_search') {
    const callId = `codex-tool-${item.ordinal}`
    return new AtifStep({
      step_id: stepId,
      source: 'agent',
      message:
        item.itemType === 'web_search'
          ? 'Codex performed a web search.'
          : 'Codex performed an MCP tool call.',
      model_name: PYLON_CODEX_MODEL_NAME,
      tool_calls: [
        {
          tool_call_id: callId,
          function_name:
            item.itemType === 'web_search'
              ? 'web_search'
              : 'mcp_tool_call',
          arguments: {
            toolName: item.toolName ?? item.itemType,
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: callId,
            content: item.status ?? 'completed',
          },
        ],
      },
    })
  }

  return new AtifStep({
    step_id: stepId,
    source: 'agent',
    message:
      item.itemType === 'error'
        ? 'Codex reported an execution error.'
        : 'Codex emitted a structured event.',
    model_name: PYLON_CODEX_MODEL_NAME,
  })
}

export const pylonCodexTurnToAtifTrajectory = (
  body: PylonCodexTurnIngestBody,
): AtifTrajectory => {
  const counts = codexTurnUsageTokenCounts(body.usage)
  const items =
    body.items.length === 0
      ? [
          new AtifStep({
            step_id: 1,
            source: 'agent',
            message: 'Codex completed a turn.',
            model_name: PYLON_CODEX_MODEL_NAME,
          }),
        ]
      : body.items.map((item, index) => itemToAtifStep(item, index + 1))

  const finalIndex = items.length - 1
  const final = items[finalIndex]
  const steps =
    final === undefined
      ? items
      : items.map((step, index) =>
          index === finalIndex
            ? new AtifStep({
                ...step,
                metrics: {
                  prompt_tokens: counts.inputTokens,
                  completion_tokens: counts.outputTokens,
                },
              })
            : step,
        )

  return new AtifTrajectory({
    schema_version: ATIF_PINNED_SCHEMA_VERSION,
    trajectory_id: `pylon_codex:${body.assignmentRef}:turn:${body.turnIndex}`,
    session_id: body.sessionRef ?? body.assignmentRef,
    visibility: 'owner_only',
    agent: {
      name: 'Pylon Codex',
      version: 'pylon-codex-v1',
      model_name: PYLON_CODEX_MODEL_NAME,
    },
    steps,
    final_metrics: {
      total_prompt_tokens: counts.inputTokens,
      total_completion_tokens: counts.outputTokens,
      total_steps: steps.length,
    },
  })
}

const stableTurnDigest = (
  body: PylonCodexTurnIngestBody,
): Effect.Effect<string> =>
  Effect.promise(() =>
    sha256Hex(
      [
        body.assignmentRef,
        body.leaseRef,
        body.pylonRef,
        body.sessionRef ?? 'session.pending',
        String(body.turnIndex),
      ].join(':'),
    ),
  )

const tokenUsageEventBody = (
  input: Readonly<{
    body: PylonCodexTurnIngestBody
    digest: string
    observedAt: string
    ownerUserId: string
    session: ProgrammaticAgentSession
  }>,
) => {
  const counts = codexTurnUsageTokenCounts(input.body.usage)
  return {
    schemaVersion: 'openagents.token_usage_event.v1' as const,
    actor: {
      accountRef: `agent:${input.session.user.id}`,
      userId: input.ownerUserId,
    },
    backendProfile: PYLON_CODEX_PROVIDER,
    demand: {
      demandKind: PYLON_CODEX_DEMAND_KIND,
      demandSource: PYLON_CODEX_DEMAND_SOURCE,
    },
    eventId: `event.inference.served-tokens.pylon-codex.${input.digest.slice(0, 32)}`,
    idempotencyKey: `khala:pylon-codex:turn:${input.digest}`,
    model: PYLON_CODEX_MODEL_NAME,
    observedAt: input.observedAt,
    privacy: { leaderboardEligible: false, privacyOptOut: false },
    producerSystem: PYLON_CODEX_PRODUCER_SYSTEM,
    provider: PYLON_CODEX_PROVIDER,
    safeMetadata: {
      assignmentRef: input.body.assignmentRef,
      leaseRef: input.body.leaseRef,
      pylonRef: input.body.pylonRef,
      codexUsageSplit: {
        cachedInputTokens: input.body.usage.cachedInputTokens ?? 0,
        inputTokens: input.body.usage.inputTokens,
        outputTokens: input.body.usage.outputTokens,
        reasoningOutputTokens: input.body.usage.reasoningOutputTokens ?? 0,
      },
      costCaveat: 'owner_capacity_provider_cost_unknown',
      usageBasis: 'codex_sdk_turn_completed',
    },
    sourceRefs: {
      ...(input.body.runRef === undefined ? {} : { runRef: input.body.runRef }),
      ...(input.body.sessionRef === undefined
        ? {}
        : { sessionRef: input.body.sessionRef }),
      taskRef: input.body.assignmentRef,
      ...(input.body.workspaceRef === undefined
        ? {}
        : { repositoryRef: input.body.workspaceRef }),
    },
    sourceRoute: PYLON_CODEX_SOURCE_ROUTE,
    tokenCounts: {
      cacheReadTokens: counts.cacheReadTokens,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: counts.inputTokens,
      outputTokens: counts.outputTokens,
      reasoningTokens: counts.reasoningTokens,
      totalTokens: counts.totalTokens,
    },
    usageTruth: 'exact' as const,
  }
}

const storeTrace = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    body: PylonCodexTurnIngestBody
    digest: string
    nowIso: string
    ownerUserId: string
    session: ProgrammaticAgentSession
  }>,
): Effect.Effect<
  Readonly<{
    created: boolean
    redactionReport: TraceRedactionReport
    uuid: string
  }>,
  PylonCodexStorageError | PylonCodexTraceRejected | PylonCodexValidationError
> =>
  Effect.gen(function* () {
    const mapped = pylonCodexTurnToAtifTrajectory(input.body)
    const mappedIssues = validateAtifTrajectory(mapped)
    if (mappedIssues.length > 0) {
      return yield* new PylonCodexValidationError({
        reason: mappedIssues.map(issue => issue.message).join(' '),
      })
    }

    const { value: redacted, report: redactionReport } =
      redactTraceValue(mapped)
    const trajectory = redacted as AtifTrajectory
    const redactedIssues = validateAtifTrajectory(trajectory)
    if (redactedIssues.length > 0) {
      return yield* new PylonCodexValidationError({
        reason: redactedIssues.map(issue => issue.message).join(' '),
      })
    }

    const tripwireFindings = atifTraceTripwire(trajectory)
    if (tripwireFindings.length > 0) {
      return yield* new PylonCodexTraceRejected({
        findings: tripwireFindings.map(finding => finding.code),
        redactionReport,
      })
    }

    const stored = yield* Effect.tryPromise({
      catch: error =>
        new PylonCodexStorageError({
          operation: 'trace_store_create',
          reason: traceStoreErrorFromUnknown(error).reason,
        }),
      try: () =>
        dependencies.traceStore(env).createTrace({
          traceUuid: routeMakeId(dependencies),
          ownerUserId: input.ownerUserId,
          agentRef: `agent:${input.session.user.id}`,
          schemaVersion: trajectory.schema_version,
          trajectoryId: trajectory.trajectory_id,
          sessionId: trajectory.session_id ?? null,
          visibility: 'owner_only',
          stepCount: trajectory.steps.length,
          trajectory,
          trajectoryR2Key: null,
          blobRefs: [],
          idempotencyKey: `pylon-codex:${input.digest}`,
          trainingConsent: false,
          license: null,
          contentDigest: null,
          rewardEligible: false,
          rewardAmountSats: null,
          uploadSource: 'agent',
          demandKind: PYLON_CODEX_DEMAND_KIND,
          demandSource: PYLON_CODEX_DEMAND_SOURCE,
          nowIso: input.nowIso,
        }),
    })

    return {
      created: stored.created,
      redactionReport,
      uuid: stored.record.traceUuid,
    }
  })

type PylonCodexTraceDropDiagnostic = Readonly<{
  findings?: ReadonlyArray<string>
  operation?: string
  reason: string
  redactionReport?: TraceRedactionReport
}>

type PylonCodexTraceOutcome =
  | Readonly<{
      kind: 'stored'
      trace: Readonly<{
        created: boolean
        redactionReport: TraceRedactionReport
        uuid: string
      }>
    }>
  | Readonly<{
      kind: 'dropped'
      diagnostic: PylonCodexTraceDropDiagnostic
    }>

const traceDropDiagnostic = (
  error:
    | PylonCodexStorageError
    | PylonCodexTraceRejected
    | PylonCodexValidationError,
): PylonCodexTraceDropDiagnostic =>
  M.value(error).pipe(
    M.tags({
      PylonCodexStorageError: error => ({
        operation: error.operation,
        reason: 'trace_store_unavailable',
      }),
      PylonCodexTraceRejected: error => ({
        findings: error.findings,
        reason: 'trace_rejected_after_redaction',
      }),
      PylonCodexValidationError: () => ({
        reason: 'trace_projection_invalid',
      }),
    }),
    M.exhaustive,
  )

const routeIngest = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, PylonCodexRouteError> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = yield* requireAgent(dependencies, request, env)

    const rawBody = yield* Effect.tryPromise({
      catch: () =>
        new PylonCodexValidationError({
          reason: 'Request body could not be read.',
        }),
      try: () => request.text(),
    })
    if (new TextEncoder().encode(rawBody).length > MAX_BODY_BYTES) {
      return yield* new PylonCodexValidationError({
        reason: `Pylon Codex turn payload exceeds the ${MAX_BODY_BYTES}-byte limit.`,
      })
    }

    const body = yield* Effect.try({
      catch: error =>
        new PylonCodexValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'Request body does not match the Pylon Codex turn schema.',
        }),
      try: () =>
        decodeUnknownWithSchema(
          PylonCodexTurnIngestBody,
          rawBody.trim() === '' ? {} : parseJsonUnknown(rawBody),
        ),
    })

    yield* requireOwnedAssignment(dependencies, env, session, body)

    const ownerUserId = ownerUserIdForAgent(session)
    const observedAt = body.observedAt ?? routeNowIso(dependencies)
    const digest = yield* stableTurnDigest(body)
    const tokenBody = tokenUsageEventBody({
      body,
      digest,
      observedAt,
      ownerUserId,
      session,
    })
    const counts = codexTurnUsageTokenCounts(body.usage)

    const tokenResult = yield* dependencies
      .ledger(env)
      .ingestEvent(tokenBody)
      .pipe(
        Effect.mapError(
          error =>
            new PylonCodexStorageError({
              operation: 'token_usage_ingest',
              reason: storageReason(error),
            }),
        ),
      )

    if (
      tokenResult.inserted &&
      dependencies.publishDelta !== undefined &&
      counts.inputTokens + counts.outputTokens > 0
    ) {
      yield* dependencies
        .publishDelta(env, {
          eventRef: tokenBody.eventId,
          observedAt,
          tokensServedDelta: counts.inputTokens + counts.outputTokens,
        })
        .pipe(Effect.catch(() => Effect.void))
    }

    const traceOutcome: PylonCodexTraceOutcome = yield* storeTrace(
      dependencies,
      env,
      {
        body,
        digest,
        nowIso: observedAt,
        ownerUserId,
        session,
      },
    ).pipe(
      Effect.match({
        onFailure: error => ({
          diagnostic: traceDropDiagnostic(error),
          kind: 'dropped' as const,
        }),
        onSuccess: trace => ({
          kind: 'stored' as const,
          trace,
        }),
      }),
    )

    const trace =
      traceOutcome.kind === 'stored'
        ? {
            created: traceOutcome.trace.created,
            uuid: traceOutcome.trace.uuid,
            visibility: 'owner_only' as const,
          }
        : {
            diagnostic: traceOutcome.diagnostic,
            dropped: true,
            visibility: 'owner_only' as const,
          }

    const redactionReport =
      traceOutcome.kind === 'stored'
        ? traceOutcome.trace.redactionReport
        : traceOutcome.diagnostic.redactionReport

    return noStoreJsonResponse({
      schemaVersion: 'openagents.pylon.codex_turn_ingest_result.v1',
      assignmentRef: body.assignmentRef,
      insertedTokenUsage: tokenResult.inserted,
      tokensServedDelta: tokenResult.inserted
        ? counts.inputTokens + counts.outputTokens
        : 0,
      tokenUsageEventRef: tokenBody.eventId,
      trace,
      ...(redactionReport === undefined ? {} : { redactionReport }),
    })
  })

export const makePylonCodexTurnIngestRoutes = <Bindings>(
  dependencies: PylonCodexTurnIngestDependencies<Bindings>,
) => ({
  handlePylonCodexTurnIngestApi: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> => {
    if (new URL(request.url).pathname !== PYLON_CODEX_TURN_INGEST_PATH) {
      return Effect.succeed(noStoreJsonResponse({ error: 'not_found' }, { status: 404 }))
    }
    return routeIngest(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
})
