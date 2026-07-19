import type {
  ManagedSandboxCommandReservation,
  ManagedSandboxEventPage,
  ManagedSandboxProjectionState,
  ManagedSandboxRuntimeEventPage,
  ManagedSandboxTurnOrder,
  RecordManagedSandboxRuntimeEventsResult,
} from '@openagentsinc/khala-sync-server'
import {
  BOX_V1_TRANSLATOR_REF,
  type BoxProjectionCursor,
  BoxV1ActionResponseSchema,
  BoxV1BoxInfoResponseSchema,
  BoxV1BoxListResponseSchema,
  BoxV1CommandResponseSchema,
  BoxV1CreateResponseSchema,
  BoxV1DeleteResponseSchema,
  BoxV1ErrorEnvelopeSchema,
  BoxV1FileReadResponseSchema,
  BoxV1FileWriteResponseSchema,
  BoxV1LimitsResponseSchema,
  BoxV1MeResponseSchema,
  BoxV1ProjectedEventPageSchema,
  BoxV1PromptResponseSchema,
  BoxV1PromptRunResponseSchema,
  type ManagedSandboxCommand,
  ManagedSandboxCommandSchema,
  type ManagedSandboxEvent,
  ManagedSandboxEventSchema,
  type ManagedSandboxReceipt,
  type ManagedSandboxResource,
  ManagedSandboxResourceSchema,
  type ManagedSandboxRuntimeEventInput,
  type ManagedSandboxTurn,
  type ManagedSandboxTurnReceipt,
  SandboxRef,
  capabilityNotImplemented,
  projectManagedSandboxToBoxV1,
} from '@openagentsinc/managed-sandbox-contract'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { parseJsonUnknown } from './json-boundary'
import {
  currentDate,
  isoTimestampAfter,
  randomUuid,
} from './runtime-primitives'

const BASE_PATH = '/v1'
const DEFAULT_LIMIT = 50
const MAX_LIMIT = 100

export type BoxV1Principal = Readonly<{
  actorRef: string
  ownerRef: string
  tenantRef: string
  login: string
  email: string | null
}>

export type BoxV1Policy = Readonly<{
  target: ManagedSandboxResource['target']
  imageDigest: ManagedSandboxResource['imageDigest']
  profileRef: ManagedSandboxResource['profileRef']
  defaultTtlSeconds: number
  maxTtlSeconds: number
  maxActiveBoxes: number
  maxCostMicros: number
  maxCpuMillis: number
  maxNetworkBytes: number
  maxArtifactBytes: number
}>

export class BoxV1FacadeError extends S.TaggedErrorClass<BoxV1FacadeError>()(
  'BoxV1FacadeError',
  {
    code: S.String,
    status: S.Number,
    message: S.String,
    retryable: S.Boolean,
    details: S.optionalKey(S.Unknown),
  },
) {}

export type BoxV1NativeStore = Readonly<{
  reservation: (input: {
    ownerRef: string
    tenantRef: string
    commandRef: string
  }) => Effect.Effect<
    ManagedSandboxCommandReservation | undefined,
    BoxV1FacadeError
  >
  reserve: (
    input: Readonly<{
      command: ManagedSandboxCommand
      initialResource?: ManagedSandboxResource
    }>,
  ) => Effect.Effect<ManagedSandboxCommandReservation, BoxV1FacadeError>
  settle: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    commandRef: string
    expectedResourceGeneration: number
    events: ReadonlyArray<ManagedSandboxEvent>
    outcome: 'succeeded' | 'failed' | 'refused'
    artifactRefs?: ReadonlyArray<string>
    errorCode?: string
    observedAt: string
  }) => Effect.Effect<ManagedSandboxReceipt, BoxV1FacadeError>
  inspect: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
  }) => Effect.Effect<ManagedSandboxResource, BoxV1FacadeError>
  list: (input: {
    ownerRef: string
    tenantRef: string
    limit?: number
  }) => Effect.Effect<ReadonlyArray<ManagedSandboxResource>, BoxV1FacadeError>
  readEvents: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    afterSequence: number
    limit: number
  }) => Effect.Effect<ManagedSandboxEventPage, BoxV1FacadeError>
  turns: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
  }) => Effect.Effect<ReadonlyArray<ManagedSandboxTurnOrder>, BoxV1FacadeError>
  inspectTurn: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    turnRef: string
  }) => Effect.Effect<
    { turn: ManagedSandboxTurn; receipt?: ManagedSandboxTurnReceipt },
    BoxV1FacadeError
  >
  readTurnEvents: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    turnRef: string
    afterTurnSequence: number
    limit: number
  }) => Effect.Effect<ManagedSandboxRuntimeEventPage, BoxV1FacadeError>
  recordRuntimeEvents: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    turnRef: string
    expectedResourceGeneration: number
    events: ReadonlyArray<ManagedSandboxRuntimeEventInput>
    evidenceRefs?: ReadonlyArray<string>
  }) => Effect.Effect<RecordManagedSandboxRuntimeEventsResult, BoxV1FacadeError>
  readProjection: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    translatorRef: string
  }) => Effect.Effect<
    ManagedSandboxProjectionState | undefined,
    BoxV1FacadeError
  >
  advanceProjection: (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    expectedProjectionVersion: number
    cursor: BoxProjectionCursor
    observedAt: string
  }) => Effect.Effect<ManagedSandboxProjectionState, BoxV1FacadeError>
}>

export type BoxV1Runtime = Readonly<{
  dispatch: (input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    turn: ManagedSandboxTurn
    prompt: string
  }) => Effect.Effect<
    ReadonlyArray<ManagedSandboxRuntimeEventInput>,
    BoxV1FacadeError
  >
  sync: (input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    turn: ManagedSandboxTurn
    afterTurnSequence: number
  }) => Effect.Effect<
    ReadonlyArray<ManagedSandboxRuntimeEventInput>,
    BoxV1FacadeError
  >
  interrupt: (input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    turn: ManagedSandboxTurn
    reasonRef: string
    idempotencyRef: string
  }) => Effect.Effect<
    ReadonlyArray<ManagedSandboxRuntimeEventInput>,
    BoxV1FacadeError
  >
  readFile: (input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    path: string
    encoding: 'utf8' | 'base64'
  }) => Effect.Effect<{ content: string; size: number }, BoxV1FacadeError>
  writeFile: (input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    path: string
    encoding: 'utf8' | 'base64'
    content: string
  }) => Effect.Effect<{ size: number }, BoxV1FacadeError>
  command: (input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    command: string
    cwd: string
    timeoutSeconds: number
  }) => Effect.Effect<
    {
      success: boolean
      exitCode: number | null
      signal: string | null
      stdout: string
      stderr: string
      stdoutTruncated: boolean
      stderrTruncated: boolean
      timedOut: boolean
      startedAt: string
      finishedAt: string
    },
    BoxV1FacadeError
  >
  artifact: (input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    path: string
  }) => Effect.Effect<
    { bytes: Uint8Array; contentType: string },
    BoxV1FacadeError
  >
}>

export type BoxV1RuntimeFactory<Bindings> = (
  bindings: Bindings,
  principal: BoxV1Principal,
) => Effect.Effect<BoxV1Runtime, BoxV1FacadeError>

export type BoxV1StoreFactory<Bindings> = (
  bindings: Bindings,
  principal: BoxV1Principal,
) => Effect.Effect<BoxV1NativeStore, BoxV1FacadeError, never>

export class BoxCompatibilityService extends Context.Service<
  BoxCompatibilityService,
  ReturnType<typeof makeBoxCompatibilityService>
>()('@openagentsinc/BoxCompatibilityService') {}

const validationError = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: 'validation_failed',
    status: 400,
    message,
    retryable: false,
    ...(details === undefined ? {} : { details }),
  })

const conflictError = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: 'conflict',
    status: 409,
    message,
    retryable: false,
    ...(details === undefined ? {} : { details }),
  })

export const upstreamUnavailable = (capability: string) =>
  new BoxV1FacadeError({
    code: 'upstream_unavailable',
    status: 503,
    message: `${capability} is not wired to the admitted managed-sandbox runtime`,
    retryable: true,
    details: { capability, translatorRef: BOX_V1_TRANSLATOR_REF },
  })

export const unavailableBoxV1Runtime: BoxV1Runtime = {
  dispatch: () => Effect.fail(upstreamUnavailable('agent_turn')),
  sync: () => Effect.fail(upstreamUnavailable('agent_turn')),
  interrupt: () => Effect.fail(upstreamUnavailable('interrupt')),
  readFile: () => Effect.fail(upstreamUnavailable('file_read')),
  writeFile: () => Effect.fail(upstreamUnavailable('file_write')),
  command: () => Effect.fail(upstreamUnavailable('command')),
  artifact: () => Effect.fail(upstreamUnavailable('artifact_read')),
}

const decode = <A>(
  schema: S.Decoder<A>,
  value: unknown,
): Effect.Effect<A, BoxV1FacadeError> =>
  Effect.try({
    try: () => S.decodeUnknownSync(schema)(value),
    catch: error =>
      validationError(
        'request body or parameter failed schema validation',
        error,
      ),
  })

const jsonBody = (request: Request): Effect.Effect<unknown, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: () => request.json(),
    catch: error => validationError('request body must be valid JSON', error),
  })

const optionalJsonBody = (
  request: Request,
): Effect.Effect<unknown, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const text = await request.text()
      return text.trim().length === 0 ? {} : parseJsonUnknown(text)
    },
    catch: error => validationError('request body must be valid JSON', error),
  })

const digest = (value: string): Effect.Effect<string, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const bytes = await crypto.subtle.digest(
        'SHA-256',
        new TextEncoder().encode(value),
      )
      return [...new Uint8Array(bytes)]
        .map(byte => byte.toString(16).padStart(2, '0'))
        .join('')
    },
    catch: error =>
      new BoxV1FacadeError({
        code: 'upstream_unavailable',
        status: 503,
        message: 'request identity digest is unavailable',
        retryable: true,
        details: error,
      }),
  })

const nowIso = (now: () => Date): string => now().toISOString()

const boundedLimit = (
  value: string | null,
): Effect.Effect<number, BoxV1FacadeError> => {
  if (value === null) return Effect.succeed(DEFAULT_LIMIT)
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed >= 1 && parsed <= MAX_LIMIT
    ? Effect.succeed(parsed)
    : Effect.fail(
        validationError(`limit must be an integer from 1 through ${MAX_LIMIT}`),
      )
}

const ttlSeconds = (
  requested: number | null | undefined,
  policy: BoxV1Policy,
): Effect.Effect<number, BoxV1FacadeError> => {
  if (requested === null) {
    return Effect.fail(
      validationError(
        'infinite Box TTL is outside the bounded OpenAgents Phase-1 profile',
      ),
    )
  }
  const value = requested ?? policy.defaultTtlSeconds
  return Number.isSafeInteger(value) &&
    value >= 1 &&
    value <= policy.maxTtlSeconds
    ? Effect.succeed(value)
    : Effect.fail(
        validationError(
          `ttlSeconds must be an integer from 1 through ${policy.maxTtlSeconds}`,
        ),
      )
}

const idempotencyKey = (request: Request): string =>
  request.headers.get('idempotency-key')?.trim() || randomUuid()

const makeCommandIdentity = (
  principal: BoxV1Principal,
  operation: string,
  key: string,
): Effect.Effect<
  { commandRef: string; idempotencyRef: string; suffix: string },
  BoxV1FacadeError
> =>
  digest(
    `${principal.ownerRef}\n${principal.tenantRef}\n${operation}\n${key}`,
  ).pipe(
    Effect.map(value => ({
      suffix: value.slice(0, 32),
      commandRef: `command.box.${operation}.${value.slice(0, 32)}`,
      idempotencyRef: `idempotency.box.${value.slice(0, 32)}`,
    })),
  )

const activeStates = new Set([
  'provisioning',
  'ready',
  'idle',
  'running',
  'stopping',
  'resuming',
  'deleting',
])

const promptStatus = (status: ManagedSandboxTurnOrder['status']) => {
  switch (status) {
    case 'pending':
      return { status: 'queued' as const, done: false }
    case 'running':
    case 'interrupting':
      return { status: 'running' as const, done: false }
    case 'settled':
      return { status: 'finished' as const, done: true }
    case 'failed':
    case 'interrupted':
      return { status: 'failed' as const, done: true }
  }
}

const eventType = (tag: string): string =>
  ({
    ProvisionRequested: 'box.provisioning',
    GuestReady: 'box.ready',
    RuntimeStarted: 'prompt.started',
    RuntimeTextDelta: 'prompt.response',
    RuntimeToolStarted: 'prompt.tool.started',
    RuntimeToolCompleted: 'prompt.tool.completed',
    RuntimeUsageRecorded: 'prompt.usage',
    RuntimeInterruptRequested: 'prompt.interrupting',
    RuntimeSettled: 'prompt.finished',
    RuntimeFailed: 'prompt.failed',
    RuntimeInterrupted: 'prompt.interrupted',
    StopRequested: 'box.stopping',
    FilesystemCheckpointed: 'filesystem.checkpointed',
    FilesystemCheckpointFailed: 'filesystem.checkpoint_failed',
    GuestStopped: 'box.archived',
    ResumeRequested: 'box.resuming',
    DeleteRequested: 'box.deleting',
    CleanupObserved: 'box.deleted',
    OperationFailed: 'box.operation_failed',
    RecoveryMarked: 'box.recovery_required',
  })[tag] ?? 'box.unknown'

const runtimeEventProjectionData = (
  event: ManagedSandboxEvent,
): Readonly<Record<string, unknown>> => {
  switch (event._tag) {
    case 'RuntimeTextDelta':
      return {
        content: event.content,
        turnEventSequence: event.turnEventSequence,
      }
    case 'RuntimeToolStarted':
      return {
        toolCallRef: event.toolCallRef,
        toolName: event.toolName,
        turnEventSequence: event.turnEventSequence,
      }
    case 'RuntimeToolCompleted':
      return {
        toolCallRef: event.toolCallRef,
        toolName: event.toolName,
        outcome: event.outcome,
        turnEventSequence: event.turnEventSequence,
      }
    case 'RuntimeUsageRecorded':
      return { usage: event.usage, turnEventSequence: event.turnEventSequence }
    case 'RuntimeStarted':
    case 'RuntimeInterruptRequested':
    case 'RuntimeSettled':
    case 'RuntimeFailed':
    case 'RuntimeInterrupted':
      return { turnEventSequence: event.turnEventSequence }
    default:
      return {}
  }
}

const cursorFor = (resourceGeneration: number, sequence: number): string =>
  `boxc.${resourceGeneration}.${sequence}`

const parseCursor = (
  value: string | null,
  resource: ManagedSandboxResource,
): Effect.Effect<number, BoxV1FacadeError> => {
  if (value === null || value.length === 0) return Effect.succeed(0)
  const match = /^boxc\.(\d+)\.(\d+)$/.exec(value)
  if (match === null)
    return Effect.fail(validationError('event cursor is malformed'))
  const generation = Number(match[1])
  const sequence = Number(match[2])
  if (generation !== resource.resourceGeneration) {
    return Effect.fail(
      new BoxV1FacadeError({
        code: 'conflict',
        status: 409,
        message: 'event cursor belongs to a stale sandbox generation',
        retryable: false,
        details: {
          expectedGeneration: resource.resourceGeneration,
          receivedGeneration: generation,
        },
      }),
    )
  }
  return Number.isSafeInteger(sequence) && sequence >= 0
    ? Effect.succeed(sequence)
    : Effect.fail(validationError('event cursor sequence is invalid'))
}

const createInputSchema = S.Struct({
  ttlSeconds: S.optionalKey(S.NullOr(S.Number)),
  env: S.optionalKey(S.Record(S.String, S.String)),
  noEnv: S.optionalKey(S.Boolean),
})

const updateInputSchema = S.Struct({
  name: S.optionalKey(S.String),
  ttlSeconds: S.optionalKey(S.NullOr(S.Number)),
  subdomain: S.optionalKey(S.String),
})

const promptInputSchema = S.Struct({
  provider: S.Literals(['codex', 'claude-code', 'claude']),
  model: S.optionalKey(S.NullOr(S.String)),
  reasoningEffort: S.optionalKey(S.NullOr(S.String)),
  prompt: S.String,
})

const fileWriteInputSchema = S.Struct({
  path: S.String,
  content: S.String,
  encoding: S.optionalKey(S.Literals(['utf8', 'base64'])),
})

const commandInputSchema = S.Struct({
  command: S.String,
  cwd: S.optionalKey(S.String),
  timeoutSeconds: S.optionalKey(S.Number),
})

const assertPath = (value: string): Effect.Effect<string, BoxV1FacadeError> => {
  if (
    value.length < 1 ||
    value.length > 1_024 ||
    value.startsWith('/') ||
    value.split('/').some(segment => segment === '..')
  ) {
    return Effect.fail(
      validationError('path must be a bounded root-relative path'),
    )
  }
  return Effect.succeed(value)
}

const makeBoxCompatibilityService = (input: {
  principal: BoxV1Principal
  policy: BoxV1Policy
  store: BoxV1NativeStore
  runtime: BoxV1Runtime
  now: () => Date
}) => {
  const scope = {
    ownerRef: input.principal.ownerRef,
    tenantRef: input.principal.tenantRef,
  }

  const inspect = (sandboxRef: string) =>
    input.store.inspect({ ...scope, sandboxRef })

  const materializeRuntimeEvents = (
    resource: ManagedSandboxResource,
    events: ReadonlyArray<ManagedSandboxRuntimeEventInput>,
  ): Effect.Effect<ReadonlyArray<ManagedSandboxEvent>, BoxV1FacadeError> =>
    Effect.gen(function* () {
      const materialized: Array<ManagedSandboxEvent> = []
      for (const [offset, event] of events.entries()) {
        const eventDigest = yield* digest(
          `${event.turnRef}\n${event.turnEventSequence}`,
        )
        materialized.push(
          yield* decode(ManagedSandboxEventSchema, {
            ...event,
            schema: 'openagents.managed_sandbox_event.v1',
            eventRef: `event.box.runtime.${eventDigest.slice(0, 32)}`,
            sandboxRef: resource.sandboxRef,
            sequence: resource.lastEventSequence + offset + 1,
          }),
        )
      }
      return materialized
    })

  const syncTurn = (
    resource: ManagedSandboxResource,
    turn: ManagedSandboxTurn,
  ): Effect.Effect<ManagedSandboxTurn, BoxV1FacadeError> =>
    Effect.gen(function* () {
      if (['settled', 'failed', 'interrupted'].includes(turn.status))
        return turn
      const events = yield* input.runtime.sync({
        principal: input.principal,
        resource,
        turn,
        afterTurnSequence: turn.lastEventSequence,
      })
      if (events.length === 0) return turn
      return (yield* input.store.recordRuntimeEvents({
        ...scope,
        sandboxRef: resource.sandboxRef,
        turnRef: turn.turnRef,
        expectedResourceGeneration: resource.resourceGeneration,
        events,
      })).turn
    })

  const admitDispatch = (
    reservation: ManagedSandboxCommandReservation,
    prompt: string,
  ): Effect.Effect<ManagedSandboxTurn, BoxV1FacadeError> =>
    Effect.gen(function* () {
      if (reservation.command._tag !== 'Dispatch') {
        return yield* conflictError(
          'runtime admission requires a dispatch command',
        )
      }
      const inspected = yield* input.store.inspectTurn({
        ...scope,
        sandboxRef: reservation.resource.sandboxRef,
        turnRef: reservation.command.turnRef,
      })
      if (reservation.status !== 'pending') {
        return yield* syncTurn(reservation.resource, inspected.turn)
      }
      const providerEvents = yield* input.runtime.dispatch({
        principal: input.principal,
        resource: reservation.resource,
        turn: inspected.turn,
        prompt,
      })
      if (providerEvents[0]?._tag !== 'RuntimeStarted') {
        return yield* upstreamUnavailable('agent_turn_start')
      }
      const events = yield* materializeRuntimeEvents(
        reservation.resource,
        providerEvents,
      )
      const observedAt = events.at(-1)?.observedAt
      if (observedAt === undefined) {
        return yield* upstreamUnavailable('agent_turn_start')
      }
      yield* input.store.settle({
        ...scope,
        sandboxRef: reservation.resource.sandboxRef,
        commandRef: reservation.command.commandRef,
        expectedResourceGeneration: reservation.resource.resourceGeneration,
        events,
        outcome: 'succeeded',
        observedAt,
      })
      return (yield* input.store.inspectTurn({
        ...scope,
        sandboxRef: reservation.resource.sandboxRef,
        turnRef: reservation.command.turnRef,
      })).turn
    })

  const admitInterrupt = (
    reservation: ManagedSandboxCommandReservation,
  ): Effect.Effect<ManagedSandboxTurn, BoxV1FacadeError> =>
    Effect.gen(function* () {
      if (reservation.command._tag !== 'Interrupt') {
        return yield* conflictError(
          'runtime interrupt requires an interrupt command',
        )
      }
      const inspected = yield* input.store.inspectTurn({
        ...scope,
        sandboxRef: reservation.resource.sandboxRef,
        turnRef: reservation.command.turnRef,
      })
      if (reservation.status !== 'pending') {
        return yield* syncTurn(reservation.resource, inspected.turn)
      }
      const providerEvents = yield* input.runtime.interrupt({
        principal: input.principal,
        resource: reservation.resource,
        turn: inspected.turn,
        reasonRef: reservation.command.reasonRef,
        idempotencyRef: reservation.command.idempotencyRef,
      })
      if (providerEvents[0]?._tag !== 'RuntimeInterruptRequested') {
        return yield* upstreamUnavailable('interrupt')
      }
      const events = yield* materializeRuntimeEvents(
        reservation.resource,
        providerEvents,
      )
      const observedAt = events.at(-1)?.observedAt
      if (observedAt === undefined)
        return yield* upstreamUnavailable('interrupt')
      yield* input.store.settle({
        ...scope,
        sandboxRef: reservation.resource.sandboxRef,
        commandRef: reservation.command.commandRef,
        expectedResourceGeneration: reservation.resource.resourceGeneration,
        events,
        outcome: 'succeeded',
        observedAt,
      })
      return (yield* input.store.inspectTurn({
        ...scope,
        sandboxRef: reservation.resource.sandboxRef,
        turnRef: reservation.command.turnRef,
      })).turn
    })

  const action = (
    tag: 'Stop' | 'Resume' | 'Delete',
    sandboxRef: string,
    key: string,
  ) =>
    Effect.gen(function* () {
      const identity = yield* makeCommandIdentity(
        input.principal,
        tag.toLowerCase(),
        key,
      )
      const existing = yield* input.store.reservation({
        ...scope,
        commandRef: identity.commandRef,
      })
      if (existing !== undefined) {
        if (
          existing.command._tag !== tag ||
          existing.resource.sandboxRef !== sandboxRef
        ) {
          return yield* conflictError(
            'idempotency key is bound to another action',
          )
        }
        return existing
      }
      const resource = yield* inspect(sandboxRef)
      const requestedAt = nowIso(input.now)
      const command = yield* decode(ManagedSandboxCommandSchema, {
        _tag: tag,
        schema: 'openagents.managed_sandbox_command.v1',
        commandRef: identity.commandRef,
        requestedByRef: input.principal.actorRef,
        ...scope,
        idempotencyRef: identity.idempotencyRef,
        requestedAt,
        sandboxRef,
        expectedVersion: resource.version,
        ...(tag === 'Stop' || tag === 'Delete'
          ? { reasonRef: `reason.box_v1.${tag.toLowerCase()}` }
          : {}),
      })
      return yield* input.store.reserve({ command })
    })

  return {
    me: () =>
      decode(BoxV1MeResponseSchema, {
        ok: true,
        type: 'user.info',
        user: { login: input.principal.login, email: input.principal.email },
      }),
    limits: () =>
      Effect.gen(function* () {
        const resources = yield* input.store.list({ ...scope, limit: 200 })
        const active = resources.filter(resource =>
          activeStates.has(resource.facts.lifecycle),
        )
        return yield* decode(BoxV1LimitsResponseSchema, {
          ok: true,
          type: 'limits.info',
          accessTier: 'openagents_managed',
          canStart: active.length < input.policy.maxActiveBoxes,
          activeBoxes: active.length,
          activeStates: [
            ...new Set(active.map(resource => resource.facts.lifecycle)),
          ].sort(),
          maxActiveBoxes: input.policy.maxActiveBoxes,
          billingStatus: 'openagents_receipt_first',
        })
      }),
    list: (options: {
      limit: number
      cursor: string | null
      state: string | null
    }) =>
      Effect.gen(function* () {
        const resources = yield* input.store.list({ ...scope, limit: 200 })
        const projected = resources
          .map(projectManagedSandboxToBoxV1)
          .filter(box => options.state === null || box.state === options.state)
        const cursorIndex =
          options.cursor === null
            ? -1
            : projected.findIndex(box => box.id === options.cursor)
        if (options.cursor !== null && cursorIndex < 0) {
          return yield* validationError('box list cursor is stale or unknown')
        }
        const page = projected.slice(
          cursorIndex + 1,
          cursorIndex + 1 + options.limit,
        )
        const hasMore = cursorIndex + 1 + page.length < projected.length
        return yield* decode(BoxV1BoxListResponseSchema, {
          ok: true,
          type: 'boxes.list',
          boxes: page,
          pageInfo: {
            nextCursor: hasMore ? (page.at(-1)?.id ?? null) : null,
            hasMore,
            limit: options.limit,
          },
        })
      }),
    create: (raw: unknown, key: string) =>
      Effect.gen(function* () {
        const body = yield* decode(createInputSchema, raw)
        if (
          (body.env !== undefined && Object.keys(body.env).length > 0) ||
          body.noEnv === false
        ) {
          return yield* new BoxV1FacadeError({
            code: 'capability_not_implemented',
            status: 501,
            message:
              'Box environment and account-secret attachment are outside Phase 1',
            retryable: false,
            details: {
              sdkMethod: 'create',
              capability: 'account_secret_parity',
            },
          })
        }
        const ttl = yield* ttlSeconds(body.ttlSeconds, input.policy)
        const identity = yield* makeCommandIdentity(
          input.principal,
          'create',
          key,
        )
        const sandboxRef = `sandbox.box.${identity.suffix}`
        const existing = yield* input.store.reservation({
          ...scope,
          commandRef: identity.commandRef,
        })
        if (existing !== undefined) {
          if (
            existing.command._tag !== 'Create' ||
            existing.command.lease.ttlSeconds !== ttl ||
            existing.resource.sandboxRef !== sandboxRef
          ) {
            return yield* conflictError(
              'create idempotency key is bound to different request bytes',
            )
          }
          return yield* decode(BoxV1CreateResponseSchema, {
            ok: true,
            type: 'box.created',
            status: 'provisioning',
            ttlSeconds: existing.resource.lease.ttlSeconds,
            box: projectManagedSandboxToBoxV1(existing.resource),
          })
        }
        const issuedAt = input.now()
        const issuedAtIso = issuedAt.toISOString()
        const expiresAt = isoTimestampAfter(issuedAt, ttl * 1_000)
        const capabilities: ManagedSandboxResource['capabilities'] = [
          ['turn', 'agent_turn'],
          ['command', 'command'],
          ['file_read', 'file_read'],
          ['file_write', 'file_write'],
          ['artifact_read', 'artifact_read'],
        ].map(([suffix, kind]) => ({
          capabilityRef: `capability.box.${identity.suffix}.${suffix}`,
          kind: kind as ManagedSandboxResource['capabilities'][number]['kind'],
          state: 'active' as const,
          expiresAt,
        }))
        const lease = {
          leaseRef: `lease.box.${identity.suffix}`,
          state: 'active' as const,
          issuedAt: issuedAtIso,
          expiresAt,
          ttlSeconds: ttl,
          renewable: true,
        }
        const budget = {
          currency: 'USD' as const,
          maxCostMicros: input.policy.maxCostMicros,
          maxCpuMillis: input.policy.maxCpuMillis,
          maxNetworkBytes: input.policy.maxNetworkBytes,
          maxArtifactBytes: input.policy.maxArtifactBytes,
          maxLifetimeSeconds: ttl,
        }
        const command = yield* decode(ManagedSandboxCommandSchema, {
          _tag: 'Create',
          schema: 'openagents.managed_sandbox_command.v1',
          commandRef: identity.commandRef,
          requestedByRef: input.principal.actorRef,
          ...scope,
          idempotencyRef: identity.idempotencyRef,
          requestedAt: issuedAtIso,
          workUnitRef: `work.box.${identity.suffix}`,
          attachmentRef: `attachment.box.${identity.suffix}`,
          target: input.policy.target,
          imageDigest: input.policy.imageDigest,
          profileRef: input.policy.profileRef,
          lease,
          budget,
          requestedCapabilities: capabilities,
        })
        const initialResource = yield* decode(ManagedSandboxResourceSchema, {
          schema: 'openagents.managed_sandbox.v1',
          sandboxRef,
          ...scope,
          programRef: 'program.managed_agent_sandboxes',
          workUnitRef:
            command._tag === 'Create' ? command.workUnitRef : 'work.invalid',
          attachmentRef:
            command._tag === 'Create'
              ? command.attachmentRef
              : 'attachment.invalid',
          attachmentGeneration: 1,
          resourceGeneration: 1,
          version: 0,
          lastEventSequence: 0,
          target: input.policy.target,
          imageDigest: input.policy.imageDigest,
          profileRef: input.policy.profileRef,
          lease,
          budget,
          capabilities,
          facts: {
            lifecycle: 'provisioning',
            leaseState: 'active',
            guestState: 'starting',
            filesystemState: 'unallocated',
            ingressState: 'closed',
            runtimeState: 'none',
            acceptingWork: false,
            cleanupComplete: false,
          },
          createdAt: issuedAtIso,
          updatedAt: issuedAtIso,
        })
        const reserved = yield* input.store.reserve({
          command,
          initialResource,
        })
        return yield* decode(BoxV1CreateResponseSchema, {
          ok: true,
          type: 'box.created',
          status: 'provisioning',
          ttlSeconds: reserved.resource.lease.ttlSeconds,
          box: projectManagedSandboxToBoxV1(reserved.resource),
        })
      }),
    get: (sandboxRef: string) =>
      inspect(sandboxRef).pipe(
        Effect.flatMap(resource =>
          decode(BoxV1BoxInfoResponseSchema, {
            ok: true,
            type: 'box.info',
            box: projectManagedSandboxToBoxV1(resource),
          }),
        ),
      ),
    update: (sandboxRef: string, raw: unknown, key: string) =>
      Effect.gen(function* () {
        const body = yield* decode(updateInputSchema, raw)
        if (body.name !== undefined || body.subdomain !== undefined) {
          return yield* new BoxV1FacadeError({
            code: 'capability_not_implemented',
            status: 501,
            message: 'Box names and public subdomains are outside Phase 1',
            retryable: false,
            details: { sdkMethod: 'update', capability: 'public_naming' },
          })
        }
        if (body.ttlSeconds === undefined) {
          return yield* validationError('update requires ttlSeconds')
        }
        const ttl = yield* ttlSeconds(body.ttlSeconds, input.policy)
        const identity = yield* makeCommandIdentity(
          input.principal,
          'update',
          key,
        )
        const existing = yield* input.store.reservation({
          ...scope,
          commandRef: identity.commandRef,
        })
        if (existing !== undefined) {
          if (
            existing.command._tag !== 'Update' ||
            existing.command.sandboxRef !== sandboxRef ||
            existing.command.lease?.ttlSeconds !== ttl
          ) {
            return yield* conflictError(
              'update idempotency key is bound to different request bytes',
            )
          }
          return yield* decode(BoxV1BoxInfoResponseSchema, {
            ok: true,
            type: 'box.info',
            box: projectManagedSandboxToBoxV1(existing.resource),
          })
        }
        const resource = yield* inspect(sandboxRef)
        const issuedAt = input.now()
        const lease = {
          leaseRef: `lease.box.${identity.suffix}`,
          state: 'active' as const,
          issuedAt: issuedAt.toISOString(),
          expiresAt: isoTimestampAfter(issuedAt, ttl * 1_000),
          ttlSeconds: ttl,
          renewable: true,
        }
        const command = yield* decode(ManagedSandboxCommandSchema, {
          _tag: 'Update',
          schema: 'openagents.managed_sandbox_command.v1',
          commandRef: identity.commandRef,
          requestedByRef: input.principal.actorRef,
          ...scope,
          idempotencyRef: identity.idempotencyRef,
          requestedAt: issuedAt.toISOString(),
          sandboxRef,
          expectedVersion: resource.version,
          lease,
        })
        const reserved = yield* input.store.reserve({ command })
        return yield* decode(BoxV1BoxInfoResponseSchema, {
          ok: true,
          type: 'box.info',
          box: projectManagedSandboxToBoxV1(reserved.resource),
        })
      }),
    stop: (sandboxRef: string, key: string) =>
      action('Stop', sandboxRef, key).pipe(
        Effect.flatMap(reserved =>
          decode(BoxV1ActionResponseSchema, {
            ok: true,
            type: 'box.stopping',
            id: sandboxRef,
            status: reserved.resource.facts.lifecycle,
            box: projectManagedSandboxToBoxV1(reserved.resource),
          }),
        ),
      ),
    resume: (sandboxRef: string, key: string) =>
      action('Resume', sandboxRef, key).pipe(
        Effect.flatMap(reserved =>
          decode(BoxV1ActionResponseSchema, {
            ok: true,
            type: 'box.resuming',
            id: sandboxRef,
            status: reserved.resource.facts.lifecycle,
            box: projectManagedSandboxToBoxV1(reserved.resource),
          }),
        ),
      ),
    remove: (sandboxRef: string, key: string) =>
      action('Delete', sandboxRef, key).pipe(
        Effect.flatMap(reserved =>
          decode(BoxV1DeleteResponseSchema, {
            ok: true,
            type: 'box.deleted',
            id: sandboxRef,
            status:
              reserved.resource.facts.lifecycle === 'deleted'
                ? 'deleted'
                : 'deleting',
          }),
        ),
      ),
    prompt: (sandboxRef: string, raw: unknown, key: string) =>
      Effect.gen(function* () {
        const body = yield* decode(promptInputSchema, raw)
        if (body.prompt.trim().length === 0 || body.prompt.length > 100_000) {
          return yield* validationError('prompt must be non-empty and bounded')
        }
        const identity = yield* makeCommandIdentity(
          input.principal,
          'prompt',
          key,
        )
        const promptDigest = yield* digest(body.prompt)
        const runtime = {
          provider:
            body.provider === 'codex'
              ? ('codex' as const)
              : ('claude' as const),
          modelRef:
            body.model ??
            (body.provider === 'codex'
              ? 'model.codex.default'
              : 'model.claude.default'),
          harnessRef:
            body.provider === 'codex'
              ? 'harness.openai.codex-sdk.v1'
              : 'harness.anthropic.claude-agent-sdk.v1',
          ...(body.reasoningEffort === undefined ||
          body.reasoningEffort === null
            ? {}
            : { reasoningEffort: body.reasoningEffort }),
        }
        const existing = yield* input.store.reservation({
          ...scope,
          commandRef: identity.commandRef,
        })
        if (existing !== undefined) {
          if (
            existing.command._tag !== 'Dispatch' ||
            existing.command.sandboxRef !== sandboxRef ||
            existing.command.promptDigest !== `sha256:${promptDigest}` ||
            existing.command.runtime.provider !== runtime.provider ||
            existing.command.runtime.modelRef !== runtime.modelRef ||
            existing.command.runtime.harnessRef !== runtime.harnessRef ||
            existing.command.runtime.reasoningEffort !== runtime.reasoningEffort
          ) {
            return yield* conflictError(
              'prompt idempotency key is bound to different request bytes',
            )
          }
          const replayTurnRef = existing.command.turnRef
          const replayTurn = yield* admitDispatch(existing, body.prompt)
          const replayStatus = promptStatus(replayTurn.status)
          return yield* decode(BoxV1PromptResponseSchema, {
            ok: true,
            type: 'prompt.queued',
            id: sandboxRef,
            promptId: replayTurnRef,
            promptRun: {
              id: replayTurnRef,
              promptId: replayTurnRef,
              boxId: sandboxRef,
              ...replayStatus,
              createdAt: existing.command.requestedAt,
              model: body.model ?? null,
              reasoningEffort: body.reasoningEffort ?? null,
            },
            status: 'queued',
            provider: body.provider,
            model: body.model ?? null,
            reasoningEffort: body.reasoningEffort ?? null,
          })
        }
        const resource = yield* inspect(sandboxRef)
        const capability = resource.capabilities.find(
          candidate =>
            candidate.kind === 'agent_turn' && candidate.state === 'active',
        )
        if (capability === undefined) {
          return yield* new BoxV1FacadeError({
            code: 'permission_denied',
            status: 403,
            message: 'sandbox generation has no active agent-turn capability',
            retryable: false,
          })
        }
        const turnRef = `turn.box.${identity.suffix}`
        const requestedAt = nowIso(input.now)
        const command = yield* decode(ManagedSandboxCommandSchema, {
          _tag: 'Dispatch',
          schema: 'openagents.managed_sandbox_command.v1',
          commandRef: identity.commandRef,
          requestedByRef: input.principal.actorRef,
          ...scope,
          idempotencyRef: identity.idempotencyRef,
          requestedAt,
          sandboxRef,
          expectedVersion: resource.version,
          turnRef,
          capabilityRef: capability.capabilityRef,
          promptDigest: `sha256:${promptDigest}`,
          runtime,
        })
        const reserved = yield* input.store.reserve({ command })
        const admittedTurn = yield* admitDispatch(reserved, body.prompt)
        const admittedStatus = promptStatus(admittedTurn.status)
        const promptRun = {
          id: turnRef,
          promptId: turnRef,
          boxId: sandboxRef,
          ...admittedStatus,
          createdAt: requestedAt,
          model: body.model ?? null,
          reasoningEffort: body.reasoningEffort ?? null,
        }
        return yield* decode(BoxV1PromptResponseSchema, {
          ok: true,
          type: 'prompt.queued',
          id: sandboxRef,
          promptId: turnRef,
          promptRun,
          status: 'queued',
          provider: body.provider,
          model: body.model ?? null,
          reasoningEffort: body.reasoningEffort ?? null,
        })
      }),
    promptStatus: (sandboxRef: string, promptId: string) =>
      Effect.gen(function* () {
        const resource = yield* inspect(sandboxRef)
        const inspected = yield* input.store.inspectTurn({
          ...scope,
          sandboxRef,
          turnRef: promptId,
        })
        const turn = yield* syncTurn(resource, inspected.turn)
        const projected = promptStatus(turn.status)
        return yield* decode(BoxV1PromptRunResponseSchema, {
          ok: true,
          type: 'prompt.run',
          id: sandboxRef,
          promptRun: {
            id: turn.turnRef,
            promptId: turn.turnRef,
            boxId: sandboxRef,
            ...projected,
            createdAt: turn.createdAt,
            model: turn.runtime.modelRef,
            reasoningEffort: turn.runtime.reasoningEffort ?? null,
          },
        })
      }),
    interrupt: (sandboxRef: string, key: string) =>
      Effect.gen(function* () {
        const identity = yield* makeCommandIdentity(
          input.principal,
          'interrupt',
          key,
        )
        const existing = yield* input.store.reservation({
          ...scope,
          commandRef: identity.commandRef,
        })
        if (existing !== undefined) {
          if (
            existing.command._tag !== 'Interrupt' ||
            existing.command.sandboxRef !== sandboxRef
          ) {
            return yield* conflictError(
              'interrupt idempotency key is bound to another action',
            )
          }
          const turn = yield* admitInterrupt(existing)
          const resource = yield* inspect(sandboxRef)
          return yield* decode(BoxV1ActionResponseSchema, {
            ok: true,
            type: 'prompt.interrupting',
            id: sandboxRef,
            status:
              turn.status === 'interrupted'
                ? 'settled'
                : resource.facts.runtimeState,
            box: projectManagedSandboxToBoxV1(resource),
          })
        }
        const resource = yield* inspect(sandboxRef)
        const turns = yield* input.store.turns({ ...scope, sandboxRef })
        const turn = [...turns]
          .reverse()
          .find(candidate => candidate.status === 'running')
        if (turn === undefined) {
          return yield* new BoxV1FacadeError({
            code: 'conflict',
            status: 409,
            message: 'sandbox has no interruptible prompt run',
            retryable: false,
          })
        }
        const command = yield* decode(ManagedSandboxCommandSchema, {
          _tag: 'Interrupt',
          schema: 'openagents.managed_sandbox_command.v1',
          commandRef: identity.commandRef,
          requestedByRef: input.principal.actorRef,
          ...scope,
          idempotencyRef: identity.idempotencyRef,
          requestedAt: nowIso(input.now),
          sandboxRef,
          expectedVersion: resource.version,
          turnRef: turn.turnRef,
          reasonRef: 'reason.box_v1.interrupt',
        })
        const reserved = yield* input.store.reserve({ command })
        const interruptedTurn = yield* admitInterrupt(reserved)
        const interruptedResource = yield* inspect(sandboxRef)
        return yield* decode(BoxV1ActionResponseSchema, {
          ok: true,
          type: 'prompt.interrupting',
          id: sandboxRef,
          status:
            interruptedTurn.status === 'interrupted'
              ? 'settled'
              : interruptedResource.facts.runtimeState,
          box: projectManagedSandboxToBoxV1(interruptedResource),
        })
      }),
    events: (
      sandboxRef: string,
      options: { limit: number; cursor: string | null; sort: string },
    ) =>
      Effect.gen(function* () {
        if (options.sort !== 'asc') {
          return yield* validationError(
            'OpenAgents Box-v1 events require sort=asc for stable replay',
          )
        }
        let resource = yield* inspect(sandboxRef)
        const turns = yield* input.store.turns({ ...scope, sandboxRef })
        for (const candidate of turns.filter(turn =>
          ['pending', 'running', 'interrupting'].includes(turn.status),
        )) {
          const inspected = yield* input.store.inspectTurn({
            ...scope,
            sandboxRef,
            turnRef: candidate.turnRef,
          })
          yield* syncTurn(resource, inspected.turn)
          resource = yield* inspect(sandboxRef)
        }
        const afterSequence = yield* parseCursor(options.cursor, resource)
        const page = yield* input.store.readEvents({
          ...scope,
          sandboxRef,
          afterSequence,
          limit: options.limit,
        })
        const projectedEvents = page.events.map(event => ({
          id: event.eventRef,
          type: eventType(event._tag),
          timestamp: Date.parse(event.observedAt),
          ...('turnRef' in event
            ? { taskId: event.turnRef }
            : { taskId: null }),
          data: {
            nativeKind: event._tag,
            nativeEventSequence: event.sequence,
            resourceGeneration: event.resourceGeneration,
            translatorRef: BOX_V1_TRANSLATOR_REF,
            ...runtimeEventProjectionData(event),
          },
        }))
        const nextCursor = cursorFor(
          resource.resourceGeneration,
          page.nextSequence,
        )
        const currentProjection = yield* input.store.readProjection({
          ...scope,
          sandboxRef,
          translatorRef: BOX_V1_TRANSLATOR_REF,
        })
        const projectionCursor: BoxProjectionCursor = {
          translatorRef: BOX_V1_TRANSLATOR_REF,
          nativeEventSequence: page.nextSequence,
          boxCursor: nextCursor,
          omittedNativeKinds: [],
        }
        if (
          (currentProjection?.cursor.nativeEventSequence ?? -1) <
          page.nextSequence
        ) {
          yield* input.store.advanceProjection({
            ...scope,
            sandboxRef,
            expectedProjectionVersion:
              currentProjection?.projectionVersion ?? 0,
            cursor: projectionCursor,
            observedAt: nowIso(input.now),
          })
        }
        const hasMore = page.nextSequence < page.terminalSequence
        return yield* decode(BoxV1ProjectedEventPageSchema, {
          ok: true,
          type: 'events.list',
          id: sandboxRef,
          events: projectedEvents,
          pageInfo: {
            nextCursor: hasMore ? nextCursor : null,
            hasMore,
            limit: options.limit,
          },
          projection: projectionCursor,
        })
      }),
    readFile: (sandboxRef: string, path: string, encoding: 'utf8' | 'base64') =>
      Effect.gen(function* () {
        const safePath = yield* assertPath(path)
        const resource = yield* inspect(sandboxRef)
        const result = yield* input.runtime.readFile({
          principal: input.principal,
          resource,
          path: safePath,
          encoding,
        })
        return yield* decode(BoxV1FileReadResponseSchema, {
          ok: true,
          type: 'file.read',
          success: true,
          path: safePath,
          encoding,
          size: result.size,
          content: result.content,
        })
      }),
    writeFile: (sandboxRef: string, raw: unknown) =>
      Effect.gen(function* () {
        const body = yield* decode(fileWriteInputSchema, raw)
        const path = yield* assertPath(body.path)
        const encoding = body.encoding ?? 'utf8'
        const resource = yield* inspect(sandboxRef)
        const result = yield* input.runtime.writeFile({
          principal: input.principal,
          resource,
          path,
          encoding,
          content: body.content,
        })
        return yield* decode(BoxV1FileWriteResponseSchema, {
          ok: true,
          type: 'file.written',
          success: true,
          path,
          encoding,
          size: result.size,
        })
      }),
    command: (sandboxRef: string, raw: unknown) =>
      Effect.gen(function* () {
        const body = yield* decode(commandInputSchema, raw)
        if (body.command.trim().length === 0 || body.command.length > 16_384) {
          return yield* validationError('command must be non-empty and bounded')
        }
        const cwd = yield* assertPath(body.cwd ?? 'workspace')
        const timeoutSeconds = body.timeoutSeconds ?? 60
        if (
          !Number.isSafeInteger(timeoutSeconds) ||
          timeoutSeconds < 1 ||
          timeoutSeconds > 3_600
        ) {
          return yield* validationError(
            'timeoutSeconds must be from 1 through 3600',
          )
        }
        const resource = yield* inspect(sandboxRef)
        const result = yield* input.runtime.command({
          principal: input.principal,
          resource,
          command: body.command,
          cwd,
          timeoutSeconds,
        })
        return yield* decode(BoxV1CommandResponseSchema, {
          ok: true,
          type: 'command.finished',
          ...result,
          cwd,
        })
      }),
    artifact: (sandboxRef: string, path: string) =>
      Effect.gen(function* () {
        const safePath = yield* assertPath(path)
        const resource = yield* inspect(sandboxRef)
        return yield* input.runtime.artifact({
          principal: input.principal,
          resource,
          path: safePath,
        })
      }),
  }
}

export const boxCompatibilityServiceLayer = (input: {
  principal: BoxV1Principal
  policy: BoxV1Policy
  store: BoxV1NativeStore
  runtime: BoxV1Runtime
  now: () => Date
}) => Layer.succeed(BoxCompatibilityService, makeBoxCompatibilityService(input))

type RouteMatch =
  | { operation: 'me' | 'limits' | 'boxes' | 'create' }
  | {
      operation:
        | 'get'
        | 'update'
        | 'remove'
        | 'stop'
        | 'resume'
        | 'prompt'
        | 'events'
        | 'interrupt'
        | 'readFile'
        | 'writeFile'
        | 'command'
        | 'artifact'
      boxId: string
    }
  | { operation: 'promptRunStatus'; boxId: string; promptId: string }
  | { operation: 'unsupported'; sdkMethod: string }

const unsupportedPath = (pathname: string): string | undefined => {
  if (pathname === '/v1/api-keys') return 'apiKeys'
  if (pathname === '/v1/repos') return 'repos'
  if (pathname === '/v1/secrets') return 'secrets'
  if (pathname === '/v1/snapshots') return 'listSnapshots'
  if (/^\/v1\/snapshots\/[^/]+\/download$/.test(pathname))
    return 'getSnapshotDownload'
  if (/^\/v1\/snapshots\/[^/]+\/files$/.test(pathname)) return 'getSnapshotFile'
  if (/^\/v1\/snapshots\/[^/]+\/tree$/.test(pathname)) return 'getSnapshotTree'
  if (/^\/v1\/boxes\/[^/]+\/desktop$/.test(pathname)) return 'desktop'
  if (/^\/v1\/boxes\/[^/]+\/fork$/.test(pathname)) return 'fork'
  if (/^\/v1\/boxes\/[^/]+\/sshkey$/.test(pathname)) return 'sshKey'
  if (/^\/v1\/boxes\/[^/]+\/snapshots\/latest$/.test(pathname)) {
    return 'getLatestBoxSnapshot'
  }
  if (/^\/v1\/boxes\/[^/]+\/snapshots$/.test(pathname))
    return 'listBoxSnapshots'
  return undefined
}

const matchRoute = (request: Request): RouteMatch | undefined => {
  const url = new URL(request.url)
  const pathname = url.pathname
  const unsupported = unsupportedPath(pathname)
  if (unsupported !== undefined)
    return { operation: 'unsupported', sdkMethod: unsupported }
  if (pathname === `${BASE_PATH}/me` && request.method === 'GET')
    return { operation: 'me' }
  if (pathname === `${BASE_PATH}/limits` && request.method === 'GET')
    return { operation: 'limits' }
  if (pathname === `${BASE_PATH}/boxes`) {
    if (request.method === 'GET') return { operation: 'boxes' }
    if (request.method === 'POST') return { operation: 'create' }
  }
  const promptStatus = /^\/v1\/boxes\/([^/]+)\/prompts\/([^/]+)$/.exec(pathname)
  if (promptStatus !== null && request.method === 'GET') {
    return {
      operation: 'promptRunStatus',
      boxId: decodeURIComponent(promptStatus[1]!),
      promptId: decodeURIComponent(promptStatus[2]!),
    }
  }
  const nested =
    /^\/v1\/boxes\/([^/]+)\/(stop|resume|prompt|events|interrupt|files|commands|artifacts)$/.exec(
      pathname,
    )
  if (nested !== null) {
    const boxId = decodeURIComponent(nested[1]!)
    const segment = nested[2]!
    const operation =
      segment === 'files'
        ? request.method === 'GET'
          ? 'readFile'
          : request.method === 'PUT'
            ? 'writeFile'
            : undefined
        : segment === 'commands' && request.method === 'POST'
          ? 'command'
          : segment === 'artifacts' && request.method === 'GET'
            ? 'artifact'
            : segment === 'events' && request.method === 'GET'
              ? 'events'
              : segment === 'stop' && request.method === 'POST'
                ? 'stop'
                : segment === 'resume' && request.method === 'POST'
                  ? 'resume'
                  : segment === 'prompt' && request.method === 'POST'
                    ? 'prompt'
                    : segment === 'interrupt' && request.method === 'POST'
                      ? 'interrupt'
                      : undefined
    return operation === undefined ? undefined : { operation, boxId }
  }
  const single = /^\/v1\/boxes\/([^/]+)$/.exec(pathname)
  if (single !== null) {
    const boxId = decodeURIComponent(single[1]!)
    if (request.method === 'GET') return { operation: 'get', boxId }
    if (request.method === 'PATCH') return { operation: 'update', boxId }
    if (request.method === 'DELETE') return { operation: 'remove', boxId }
  }
  return undefined
}

const noStoreHeaders = {
  'cache-control': 'no-store',
  'content-type': 'application/json',
}

const jsonResponse = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: noStoreHeaders })

const errorResponse = (error: BoxV1FacadeError) => {
  const envelope = S.decodeUnknownSync(BoxV1ErrorEnvelopeSchema)({
    ok: false,
    type: 'error',
    status: error.status,
    code: error.code,
    message: error.message,
    requestId: `request.box.${randomUuid()}`,
    error: {
      code: error.code,
      message: error.message,
      status: error.status,
      ...(error.details === undefined ? {} : { details: error.details }),
    },
  })
  return jsonResponse(envelope, error.status)
}

const notFoundResponse = () =>
  errorResponse(
    new BoxV1FacadeError({
      code: 'resource_not_found',
      status: 404,
      message: 'Box-v1 compatibility route is not enabled',
      retryable: false,
    }),
  )

export type BoxV1RouteDependencies<Bindings> = Readonly<{
  enabled: (bindings: Bindings) => boolean
  authenticate: (
    request: Request,
    bindings: Bindings,
  ) => Effect.Effect<BoxV1Principal, BoxV1FacadeError>
  policy: (bindings: Bindings) => Effect.Effect<BoxV1Policy, BoxV1FacadeError>
  store: BoxV1StoreFactory<Bindings>
  runtime: BoxV1RuntimeFactory<Bindings>
  now?: () => Date
}>

export const makeBoxV1Routes = <Bindings>(
  dependencies: BoxV1RouteDependencies<Bindings>,
) => ({
  routeBoxV1Request: (request: Request, bindings: Bindings) => {
    const matched = matchRoute(request)
    if (matched === undefined) return undefined
    if (!dependencies.enabled(bindings))
      return Effect.succeed(notFoundResponse())
    const requestRef = `request.box.${randomUuid()}`

    const program = Effect.gen(function* () {
      const principal = yield* dependencies.authenticate(request, bindings)
      const policy = yield* dependencies.policy(bindings)
      const store = yield* dependencies.store(bindings, principal)
      const runtime = yield* dependencies.runtime(bindings, principal)
      const serviceLayer = boxCompatibilityServiceLayer({
        principal,
        policy,
        store,
        runtime,
        now: dependencies.now ?? currentDate,
      })
      const url = new URL(request.url)
      const run = Effect.gen(function* () {
        const service = yield* BoxCompatibilityService
        switch (matched.operation) {
          case 'me':
            return jsonResponse(yield* service.me())
          case 'limits':
            return jsonResponse(yield* service.limits())
          case 'boxes': {
            const limit = yield* boundedLimit(url.searchParams.get('limit'))
            const sort = url.searchParams.get('sort') ?? 'desc'
            if (sort !== 'desc') {
              return yield* validationError(
                'box listing supports sort=desc only',
              )
            }
            return jsonResponse(
              yield* service.list({
                limit,
                cursor: url.searchParams.get('cursor'),
                state: url.searchParams.get('state'),
              }),
            )
          }
          case 'create':
            return jsonResponse(
              yield* service.create(
                yield* optionalJsonBody(request),
                idempotencyKey(request),
              ),
              202,
            )
          case 'get':
            return jsonResponse(
              yield* service.get(yield* decode(SandboxRef, matched.boxId)),
            )
          case 'update':
            return jsonResponse(
              yield* service.update(
                yield* decode(SandboxRef, matched.boxId),
                yield* jsonBody(request),
                idempotencyKey(request),
              ),
            )
          case 'remove':
            return jsonResponse(
              yield* service.remove(
                yield* decode(SandboxRef, matched.boxId),
                idempotencyKey(request),
              ),
              202,
            )
          case 'stop':
            return jsonResponse(
              yield* service.stop(
                yield* decode(SandboxRef, matched.boxId),
                idempotencyKey(request),
              ),
              202,
            )
          case 'resume':
            return jsonResponse(
              yield* service.resume(
                yield* decode(SandboxRef, matched.boxId),
                idempotencyKey(request),
              ),
              202,
            )
          case 'prompt':
            return jsonResponse(
              yield* service.prompt(
                yield* decode(SandboxRef, matched.boxId),
                yield* jsonBody(request),
                idempotencyKey(request),
              ),
              202,
            )
          case 'promptRunStatus':
            return jsonResponse(
              yield* service.promptStatus(
                yield* decode(SandboxRef, matched.boxId),
                yield* decode(SandboxRef, matched.promptId),
              ),
            )
          case 'events':
            return jsonResponse(
              yield* service.events(yield* decode(SandboxRef, matched.boxId), {
                limit: yield* boundedLimit(url.searchParams.get('limit')),
                cursor: url.searchParams.get('cursor'),
                sort: url.searchParams.get('sort') ?? 'asc',
              }),
            )
          case 'interrupt':
            return jsonResponse(
              yield* service.interrupt(
                yield* decode(SandboxRef, matched.boxId),
                idempotencyKey(request),
              ),
              202,
            )
          case 'readFile': {
            const encoding = url.searchParams.get('encoding') ?? 'utf8'
            if (encoding !== 'utf8' && encoding !== 'base64') {
              return yield* validationError('encoding must be utf8 or base64')
            }
            const path = url.searchParams.get('path')
            if (path === null) return yield* validationError('path is required')
            return jsonResponse(
              yield* service.readFile(
                yield* decode(SandboxRef, matched.boxId),
                path,
                encoding,
              ),
            )
          }
          case 'writeFile':
            return jsonResponse(
              yield* service.writeFile(
                yield* decode(SandboxRef, matched.boxId),
                yield* jsonBody(request),
              ),
            )
          case 'command':
            return jsonResponse(
              yield* service.command(
                yield* decode(SandboxRef, matched.boxId),
                yield* jsonBody(request),
              ),
            )
          case 'artifact': {
            const path = url.searchParams.get('path')
            if (path === null) return yield* validationError('path is required')
            const artifact = yield* service.artifact(
              yield* decode(SandboxRef, matched.boxId),
              path,
            )
            return new Response(artifact.bytes as BodyInit, {
              status: 200,
              headers: {
                'cache-control': 'no-store',
                'content-type': artifact.contentType,
              },
            })
          }
          case 'unsupported':
            return jsonResponse(
              {
                ...capabilityNotImplemented(matched.sdkMethod),
                requestId: requestRef,
              },
              501,
            )
        }
      }).pipe(Effect.provide(serviceLayer))
      return yield* run
    }).pipe(Effect.catch(error => Effect.succeed(errorResponse(error))))

    return program
  },
})
