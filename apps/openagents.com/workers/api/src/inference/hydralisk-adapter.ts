// Hydralisk vLLM provider adapter for the inference gateway.
//
// Hydralisk is the OpenAgents-owned Python/NVIDIA lane. It exposes an
// OpenAI-compatible API, but it is not a generic partner passthrough: the
// gateway routes only the bounded GPT-OSS model id to this adapter and arms it
// only when public-safe preflight + receipt refs are configured. This adapter
// keeps the existing provider seam receipt-first by requiring terminal usage on
// non-streaming responses and by parsing terminal usage from streaming SSE.
import { Effect, Redacted } from 'effect'

import { parseJsonRecord, recordFromUnknown } from '../json-boundary'
import {
  inferenceToolCallDeltasFromUnknown,
  inferenceToolCallsFromUnknown,
  openAiWireMessageFromInferenceMessage,
} from './openai-chat-compat'
import {
  InferenceAdapterError,
  type InferenceAdapterRouteMetadata,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

type HydraliskResponse = Response

export type HydraliskFetch = (
  input: string,
  init: RequestInit,
) => Promise<HydraliskResponse>

export type HydraliskAdapterConfig = Readonly<{
  id: string
  apiKey: Redacted.Redacted<string>
  baseUrl: string
  fetchImpl?: HydraliskFetch | undefined
  upstreamModel?: string | undefined
}>

export type HydraliskReplicaAdapterConfig = HydraliskAdapterConfig &
  Readonly<{
    replicaId: string
    profileRef: string
    evidenceRefs: ReadonlyArray<string>
    costProfileRef: string
    maxInflight: number
    benchmarkReserved: boolean
    draining: boolean
  }>

export type GlmReplicaHealth = 'healthy' | 'degraded' | 'unhealthy'

export type GlmReplicaCapacityClass = 'spot' | 'on_demand' | 'unknown'

export type GlmSaturationPolicy =
  | 'overflow_immediately'
  | 'queue_then_overflow'
  | 'queue_then_429'

export type GlmReplicaRoutingState = Readonly<{
  replicaId: string
  health: GlmReplicaHealth
  warmState: 'cold' | 'unknown' | 'warm'
  warmAtEpochMs?: number | undefined
  inflightCount: number
  maxInflight: number
  queueDepth: number
  last429AtEpochMs?: number | undefined
  observedTtftMs?: number | undefined
  observedTps?: number | undefined
  region?: string | undefined
  capacityClass: GlmReplicaCapacityClass
  benchmarkReserved: boolean
  draining: boolean
}>

export type GlmReplicaRoutingStateOverride = Partial<
  Omit<
    GlmReplicaRoutingState,
    | 'benchmarkReserved'
    | 'draining'
    | 'inflightCount'
    | 'maxInflight'
    | 'replicaId'
    | 'warmState'
  >
> &
  Readonly<{
    benchmarkReserved?: boolean | undefined
    draining?: boolean | undefined
    health?: GlmReplicaHealth | undefined
    inflightCount?: number | undefined
    maxInflight?: number | undefined
    warmState?: 'cold' | 'unknown' | 'warm' | undefined
  }>

export type GlmReplicaRoutingStateOracle = (
  replicaId: string,
) => GlmReplicaRoutingStateOverride | undefined

export type GlmReplicaAffinityOracle = (affinity: string) => string | undefined

export type HydraliskPoolAdapterConfig = Readonly<{
  id: string
  replicas: ReadonlyArray<HydraliskReplicaAdapterConfig>
  affinityOracle?: GlmReplicaAffinityOracle | undefined
  routingStateOracle?: GlmReplicaRoutingStateOracle | undefined
  saturationPolicy?: GlmSaturationPolicy | undefined
  maxQueueWaitMs?: number | undefined
  sleep?: ((ms: number) => Effect.Effect<void>) | undefined
  nowEpochMs?: (() => number) | undefined
  upstreamModel?: string | undefined
}>

export type HydraliskPoolRouteAdmissionSnapshot = Readonly<{
  reason: string
  reservedExternalHeadroomAvailable: boolean
}>

export type HydraliskVllmPoolRuntime = Readonly<{
  adapter: InferenceProviderAdapter
  routeAdmission: () => HydraliskPoolRouteAdmissionSnapshot
}>

const DEFAULT_GLM_ASYNC_QUEUE_WAIT_MS = 250
const MAX_GLM_EDGE_QUEUE_WAIT_MS = 1_000
const DEFAULT_RESERVED_EXTERNAL_HEADROOM_SLOTS = 1

const requestUrl = (config: HydraliskAdapterConfig): string =>
  `${config.baseUrl.replace(/\/+$/u, '')}/v1/chat/completions`

const classifyStatus = (
  status: number,
): Readonly<{ kind: string; retryable: boolean }> => {
  if (status === 429) {
    return { kind: 'rate_limited', retryable: true }
  }
  if (status === 503) {
    return { kind: 'service_overloaded', retryable: true }
  }
  if (status >= 500) {
    return { kind: 'upstream_error', retryable: true }
  }
  // Lane-level auth/routing failures (401 unauthorized, 403 forbidden, 404 not
  // found) mean THIS serving lane is unavailable or misconfigured — not that the
  // request itself is bad. Treat them as retryable so `dispatchWithOverflow`
  // overflows to the next lane (e.g. the Khala Vertex Gemini fallback) instead of
  // failing the whole request. Genuine client errors (400/422) stay non-retryable.
  if (status === 401 || status === 403 || status === 404) {
    return { kind: 'lane_unavailable', retryable: true }
  }
  return { kind: 'request_rejected', retryable: false }
}

const adapterError = (
  config: HydraliskAdapterConfig,
  input: Readonly<{
    reason: string
    httpStatus?: number | undefined
    retryable?: boolean | undefined
    kind?: string | undefined
  }>,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: config.id,
    httpStatus: input.httpStatus,
    kind: input.kind,
    reason: input.reason,
    retryable: input.retryable,
  })

const streamOptions = (
  passthroughParams: Readonly<Record<string, unknown>>,
): Record<string, unknown> => ({
  ...(recordFromUnknown(passthroughParams['stream_options']) ?? {}),
  include_usage: true,
})

const toRequestBody = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Record<string, unknown> => {
  const { 'x-session-affinity': _affinity, ...params } =
    request.passthroughParams
  return {
    ...params,
    messages: request.messages.map(openAiWireMessageFromInferenceMessage),
    model: config.upstreamModel ?? request.model,
    stream: request.stream,
    ...(request.stream
      ? { stream_options: streamOptions(request.passthroughParams) }
      : {}),
  }
}

const headersFor = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Record<string, string> => ({
  accept: request.stream ? 'text/event-stream' : 'application/json',
  authorization: `Bearer ${Redacted.value(config.apiKey)}`,
  'content-type': 'application/json',
})

const postChatCompletions = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<HydraliskResponse, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: error =>
      adapterError(config, {
        kind: 'transport_error',
        reason: `retryable: hydralisk transport error (${
          error instanceof Error ? error.name : 'unknown'
        })`,
        retryable: true,
      }),
    try: () => {
      const fetcher = config.fetchImpl ?? globalThis.fetch
      return fetcher(requestUrl(config), {
        body: JSON.stringify(toRequestBody(config, request)),
        headers: headersFor(config, request),
        method: 'POST',
        ...(request.abortSignal === undefined
          ? {}
          : { signal: request.abortSignal }),
      })
    },
  })

const failForStatus = (
  config: HydraliskAdapterConfig,
  response: HydraliskResponse,
): Effect.Effect<never, InferenceAdapterError> => {
  const classified = classifyStatus(response.status)
  return Effect.fail(
    adapterError(config, {
      httpStatus: response.status,
      kind: classified.kind,
      reason:
        response.status === 429
          ? 'hydralisk rate limited request (429)'
          : `hydralisk rejected request (${response.status})`,
      retryable: classified.retryable,
    }),
  )
}

const parseResponseJson = (
  config: HydraliskAdapterConfig,
  response: HydraliskResponse,
): Effect.Effect<Record<string, unknown>, InferenceAdapterError> =>
  Effect.gen(function* () {
    const value = yield* Effect.tryPromise({
      catch: () =>
        adapterError(config, {
          kind: 'malformed_response',
          reason: 'hydralisk returned a non-JSON response',
          retryable: false,
        }),
      try: () => response.json(),
    })
    const parsed = recordFromUnknown(value)
    if (parsed === undefined) {
      return yield* Effect.fail(
        adapterError(config, {
          kind: 'malformed_response',
          reason: 'hydralisk returned a non-JSON response',
          retryable: false,
        }),
      )
    }
    return parsed
  })

const extractUsage = (raw: unknown): InferenceUsage | undefined => {
  const record = recordFromUnknown(raw)
  const usage = recordFromUnknown(record?.['usage'])
  if (usage === undefined) {
    return undefined
  }
  const promptTokens = Number(usage['prompt_tokens'])
  const completionTokens = Number(usage['completion_tokens'])
  const totalTokens = Number(usage['total_tokens'])
  if (
    !Number.isFinite(promptTokens) ||
    !Number.isFinite(completionTokens) ||
    !Number.isFinite(totalTokens)
  ) {
    return undefined
  }
  const details = recordFromUnknown(usage['prompt_tokens_details'])
  const cached =
    details === undefined ? Number.NaN : Number(details['cached_tokens'])
  return {
    completionTokens,
    promptTokens,
    totalTokens,
    ...(Number.isFinite(cached) ? { cachedPromptTokens: cached } : {}),
  }
}

const firstChoice = (raw: unknown): Record<string, unknown> | undefined => {
  const record = recordFromUnknown(raw)
  const choices = record?.['choices']
  if (!Array.isArray(choices) || choices.length === 0) {
    return undefined
  }
  return recordFromUnknown(choices[0])
}

const parseSseData = (line: string): Record<string, unknown> | undefined => {
  const trimmed = line.trim()
  if (!trimmed.startsWith('data:')) {
    return undefined
  }
  const payload = trimmed.slice('data:'.length).trim()
  if (payload === '' || payload === '[DONE]') {
    return undefined
  }
  return parseJsonRecord(payload)
}

const deltaContentOf = (frame: Record<string, unknown>): string => {
  const delta = recordFromUnknown(firstChoice(frame)?.['delta'])
  const content = delta?.['content']
  return typeof content === 'string' ? content : ''
}

const deltaReasoningOf = (frame: Record<string, unknown>): string => {
  const delta = recordFromUnknown(firstChoice(frame)?.['delta'])
  const direct =
    delta?.['reasoning_content'] ??
    delta?.['reasoning'] ??
    delta?.['reasoning_delta'] ??
    delta?.['reasoningContent']
  return typeof direct === 'string' ? direct : ''
}

const toolCallDeltasOf = (
  frame: Record<string, unknown>,
): InferenceStreamEvent['toolCallDeltas'] => {
  const delta = recordFromUnknown(firstChoice(frame)?.['delta'])
  const toolCallDeltas = inferenceToolCallDeltasFromUnknown(
    delta?.['tool_calls'],
  )
  return toolCallDeltas === undefined || toolCallDeltas.length === 0
    ? undefined
    : toolCallDeltas
}

const finishReasonOf = (frame: Record<string, unknown>): string | undefined => {
  const reason = firstChoice(frame)?.['finish_reason']
  return typeof reason === 'string' && reason.length > 0 ? reason : undefined
}

const eventForFrame = (
  frame: Record<string, unknown>,
): InferenceStreamEvent => {
  const event: {
    contentDelta: string
    reasoningDelta?: string
    toolCallDeltas?: InferenceStreamEvent['toolCallDeltas']
    finishReason?: string
    usage?: InferenceUsage
  } = { contentDelta: deltaContentOf(frame) }
  const reasoningDelta = deltaReasoningOf(frame)
  if (reasoningDelta !== '') {
    event.reasoningDelta = reasoningDelta
  }
  const toolCallDeltas = toolCallDeltasOf(frame)
  if (toolCallDeltas !== undefined) {
    event.toolCallDeltas = toolCallDeltas
  }
  const reason = finishReasonOf(frame)
  if (reason !== undefined) {
    event.finishReason = reason
  }
  const usage = extractUsage(frame)
  if (usage !== undefined) {
    event.usage = usage
  }
  return event
}

const makeSseSource = (
  body: ReadableStream<Uint8Array>,
  fallbackModel: string,
  adapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined,
): InferenceStreamSource => {
  let finishReason: string | undefined
  let usage: InferenceUsage | undefined
  // The terminal served-model disclosure is the canonical lane id passed in as
  // `fallbackModel`, never the internal vLLM served-model-name in the frames
  // (#6259). Frame events are still yielded unchanged for the client SSE.
  const servedModel: string | undefined = fallbackModel

  const frames = (async function* (): AsyncIterable<InferenceStreamEvent> {
    const reader = body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    try {
      for (;;) {
        const { done, value } = await reader.read()
        if (value !== undefined) {
          buffer += decoder.decode(value, { stream: true })
        }
        let newlineIndex = buffer.indexOf('\n')
        while (newlineIndex !== -1) {
          const line = buffer.slice(0, newlineIndex)
          buffer = buffer.slice(newlineIndex + 1)
          const frame = parseSseData(line)
          if (frame !== undefined) {
            const event = eventForFrame(frame)
            if (event.finishReason !== undefined) {
              finishReason = event.finishReason
            }
            if (event.usage !== undefined) {
              usage = event.usage
            }
            yield event
          }
          newlineIndex = buffer.indexOf('\n')
        }
        if (done) {
          const tail = parseSseData(buffer)
          if (tail !== undefined) {
            const event = eventForFrame(tail)
            if (event.finishReason !== undefined) {
              finishReason = event.finishReason
            }
            if (event.usage !== undefined) {
              usage = event.usage
            }
            yield event
          }
          break
        }
      }
    } finally {
      reader.releaseLock()
    }
  })()

  return {
    frames,
    terminal: () => ({
      ...(adapterRouteMetadata === undefined ? {} : { adapterRouteMetadata }),
      finishReason,
      servedModel,
      usage,
    }),
  }
}

const toResult = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
  raw: Record<string, unknown>,
): Effect.Effect<InferenceResult, InferenceAdapterError> => {
  const usage = extractUsage(raw)
  if (usage === undefined) {
    return Effect.fail(
      adapterError(config, {
        kind: 'malformed_response',
        reason: 'hydralisk response missing terminal usage',
        retryable: false,
      }),
    )
  }
  const choice = firstChoice(raw)
  const message = recordFromUnknown(choice?.['message'])
  const toolCalls = inferenceToolCallsFromUnknown(message?.['tool_calls'])
  const content =
    typeof message?.['content'] === 'string'
      ? (message['content'] as string)
      : ''
  const finishReason =
    typeof choice?.['finish_reason'] === 'string'
      ? (choice['finish_reason'] as string)
      : 'stop'
  return Effect.succeed({
    content,
    finishReason,
    // Disclose the canonical OpenAgents lane id (e.g. `openagents/glm-5.2-reap-504b`),
    // not the internal vLLM served-model-name (e.g. `glm-5.2-reap-504b-g4`). The
    // public served-model disclosure (#6259) and usage receipts must name our
    // model id, never the private infra name. `config.upstreamModel` is exactly
    // the id we sent upstream (line ~181).
    servedModel: config.upstreamModel ?? request.model,
    ...(toolCalls === undefined || toolCalls.length === 0 ? {} : { toolCalls }),
    usage,
  })
}

const complete = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<InferenceResult, InferenceAdapterError> =>
  Effect.gen(function* () {
    const response = yield* postChatCompletions(config, {
      ...request,
      stream: false,
    })
    if (!response.ok) {
      return yield* failForStatus(config, response)
    }
    const raw = yield* parseResponseJson(config, response)
    return yield* toResult(config, request, raw)
  })

const streamChunks = (
  config: HydraliskAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<ReadonlyArray<InferenceStreamChunk>, InferenceAdapterError> =>
  Effect.gen(function* () {
    const response = yield* postChatCompletions(config, {
      ...request,
      stream: true,
    })
    if (!response.ok) {
      return yield* failForStatus(config, response)
    }
    const body = yield* Effect.tryPromise({
      catch: () =>
        adapterError(config, {
          kind: 'malformed_response',
          reason: 'hydralisk returned an unreadable stream',
          retryable: false,
        }),
      try: () => response.text(),
    })
    const frames = body
      .split('\n')
      .map(parseSseData)
      .filter((frame): frame is Record<string, unknown> => frame !== undefined)

    const contentChunks: Array<InferenceStreamChunk> = []
    let finishReason: string | undefined
    // Public disclosure uses our canonical lane id, not the raw vLLM
    // served-model-name returned in the stream frames (#6259).
    const servedModel = config.upstreamModel ?? request.model
    let usage: InferenceUsage | undefined

    for (const frame of frames) {
      const event = eventForFrame(frame)
      if (
        event.contentDelta !== '' ||
        event.reasoningDelta !== undefined ||
        (event.toolCallDeltas !== undefined && event.toolCallDeltas.length > 0)
      ) {
        contentChunks.push({
          contentDelta: event.contentDelta,
          ...(event.reasoningDelta === undefined
            ? {}
            : { reasoningDelta: event.reasoningDelta }),
          ...(event.toolCallDeltas === undefined
            ? {}
            : { toolCallDeltas: event.toolCallDeltas }),
        })
      }
      if (event.finishReason !== undefined) {
        finishReason = event.finishReason
      }
      if (event.usage !== undefined) {
        usage = event.usage
      }
    }

    return [
      ...contentChunks,
      {
        contentDelta: '',
        finishReason: finishReason ?? 'stop',
        servedModel,
        ...(usage === undefined ? {} : { usage }),
      },
    ]
  })

export const makeHydraliskVllmAdapter = (
  config: HydraliskAdapterConfig,
): InferenceProviderAdapter => ({
  complete: request => complete(config, request),
  id: config.id,
  stream: request => streamChunks(config, request),
  streamSse: request =>
    Effect.gen(function* () {
      const response = yield* postChatCompletions(config, {
        ...request,
        stream: true,
      })
      if (!response.ok) {
        return yield* failForStatus(config, response)
      }
      if (response.body === null) {
        return yield* Effect.fail(
          adapterError(config, {
            kind: 'malformed_response',
            reason: 'hydralisk stream had no response body',
            retryable: false,
          }),
        )
      }
      return makeSseSource(response.body, config.upstreamModel ?? request.model)
    }),
})

const poolAdapterError = (
  config: HydraliskPoolAdapterConfig,
  reason: string,
  input: Readonly<{
    kind?: string | undefined
    retryable?: boolean | undefined
    httpStatus?: number | undefined
    adapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined
  }> = {},
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterRouteMetadata: input.adapterRouteMetadata,
    adapterId: config.id,
    httpStatus: input.httpStatus,
    kind: input.kind ?? 'configuration_error',
    reason,
    retryable: input.retryable ?? false,
  })

const finiteNonNegative = (value: number | undefined): number | undefined =>
  value === undefined || !Number.isFinite(value) || value < 0
    ? undefined
    : value

const positiveInteger = (value: number | undefined): number | undefined =>
  value === undefined || !Number.isFinite(value) || value < 1
    ? undefined
    : Math.floor(value)

const healthScore = (health: GlmReplicaHealth): number => {
  switch (health) {
    case 'healthy':
      return 1
    case 'degraded':
      return 0.5
    case 'unhealthy':
      return 0
  }
}

const replicaRefFor = (replicaId: string): string =>
  `replica.hydralisk.glm_52_reap_504b.${replicaId}`

const requestAffinity = (request: InferenceRequest): string | undefined => {
  const affinity = request.passthroughParams['x-session-affinity']
  return typeof affinity === 'string' && affinity.trim() !== ''
    ? affinity.trim()
    : undefined
}

const stateForReplica = (
  replica: HydraliskReplicaAdapterConfig,
  internalInflight: number,
  override: GlmReplicaRoutingStateOverride | undefined,
): GlmReplicaRoutingState => {
  const maxInflight =
    positiveInteger(override?.maxInflight) ??
    positiveInteger(replica.maxInflight) ??
    1
  const externalInflight = finiteNonNegative(override?.inflightCount) ?? 0
  const warmAtEpochMs = finiteNonNegative(override?.warmAtEpochMs)
  const warmState =
    override?.warmState ?? (warmAtEpochMs === undefined ? 'unknown' : 'warm')
  return {
    benchmarkReserved: override?.benchmarkReserved ?? replica.benchmarkReserved,
    capacityClass: override?.capacityClass ?? 'unknown',
    draining: override?.draining ?? replica.draining,
    health: override?.health ?? 'healthy',
    inflightCount: Math.max(internalInflight, externalInflight),
    maxInflight,
    queueDepth: finiteNonNegative(override?.queueDepth) ?? 0,
    replicaId: replica.replicaId,
    warmState,
    ...(finiteNonNegative(override?.last429AtEpochMs) === undefined
      ? {}
      : { last429AtEpochMs: finiteNonNegative(override?.last429AtEpochMs)! }),
    ...(finiteNonNegative(override?.observedTps) === undefined
      ? {}
      : { observedTps: finiteNonNegative(override?.observedTps)! }),
    ...(finiteNonNegative(override?.observedTtftMs) === undefined
      ? {}
      : { observedTtftMs: finiteNonNegative(override?.observedTtftMs)! }),
    ...(typeof override?.region === 'string' && override.region.trim() !== ''
      ? { region: override.region.trim() }
      : {}),
    ...(warmAtEpochMs === undefined ? {} : { warmAtEpochMs }),
  }
}

type ReplicaCandidate = Readonly<{
  index: number
  replica: HydraliskReplicaAdapterConfig
  routingStateObserved: boolean
  state: GlmReplicaRoutingState
}>

type ReplicaSelection = Readonly<{
  adapterConfig: HydraliskAdapterConfig
  metadata: InferenceAdapterRouteMetadata
  release: () => void
}>

type GlmAggregateLiveHeadroom = Readonly<{
  aggregateExternalHeadroom: number
  aggregateInflightCount: number
  aggregateMaxInflight: number
}>

const busyReasonFor = (state: GlmReplicaRoutingState): string | null => {
  if (state.benchmarkReserved) {
    return 'benchmark_reserved'
  }
  if (state.draining) {
    return 'draining'
  }
  if (state.health === 'unhealthy') {
    return `health_${state.health}`
  }
  if (state.inflightCount >= state.maxInflight) {
    return 'inflight_full'
  }
  return null
}

const rankEligibleReplicas = (
  a: ReplicaCandidate,
  b: ReplicaCandidate,
): number => {
  const warmRank = (state: GlmReplicaRoutingState): number => {
    switch (state.warmState) {
      case 'warm':
        return 2
      case 'unknown':
        return 1
      case 'cold':
        return 0
    }
  }
  const aWarmRank = warmRank(a.state)
  const bWarmRank = warmRank(b.state)
  const aHealthScore = healthScore(a.state.health)
  const bHealthScore = healthScore(b.state.health)
  if (aHealthScore !== bHealthScore) {
    return bHealthScore - aHealthScore
  }
  if (aWarmRank !== bWarmRank) {
    return bWarmRank - aWarmRank
  }
  const aWarm = a.state.warmAtEpochMs ?? -1
  const bWarm = b.state.warmAtEpochMs ?? -1
  if (aWarm !== bWarm) {
    return bWarm - aWarm
  }
  if (a.state.queueDepth !== b.state.queueDepth) {
    return a.state.queueDepth - b.state.queueDepth
  }
  if (a.state.inflightCount !== b.state.inflightCount) {
    return a.state.inflightCount - b.state.inflightCount
  }
  const aTtft = a.state.observedTtftMs ?? Number.POSITIVE_INFINITY
  const bTtft = b.state.observedTtftMs ?? Number.POSITIVE_INFINITY
  if (aTtft !== bTtft) {
    return aTtft - bTtft
  }
  const aTps = a.state.observedTps ?? -1
  const bTps = b.state.observedTps ?? -1
  if (aTps !== bTps) {
    return bTps - aTps
  }
  return a.index - b.index
}

const metadataForSelection = (
  selected: ReplicaCandidate,
  replicaFallbackReason: string | null,
  replicaBusyReason: string | null,
  queueWaitMs: number,
  saturationPolicy: GlmSaturationPolicy,
  aggregateHeadroom: GlmAggregateLiveHeadroom,
): InferenceAdapterRouteMetadata => ({
  glmSaturationPolicy: saturationPolicy,
  glmAggregateExternalHeadroom: aggregateHeadroom.aggregateExternalHeadroom,
  glmAggregateInflightCount: aggregateHeadroom.aggregateInflightCount,
  glmAggregateMaxInflight: aggregateHeadroom.aggregateMaxInflight,
  queueWaitMs,
  replicaBusyReason,
  replicaCapacityClass: selected.state.capacityClass,
  replicaCostProfileRef: selected.replica.costProfileRef,
  replicaFallbackReason,
  replicaHealthScore: healthScore(selected.state.health),
  replicaInflightCount: selected.state.inflightCount,
  replicaMaxInflight: selected.state.maxInflight,
  replicaQueueDepth: selected.state.queueDepth,
  replicaWarmState: selected.state.warmState,
  ...(selected.state.region === undefined
    ? {}
    : { replicaRegion: selected.state.region }),
  selectedReplicaId: selected.replica.replicaId,
  selectedReplicaRef: replicaRefFor(selected.replica.replicaId),
})

const boundedQueueWaitMs = (value: number | undefined): number =>
  Math.min(
    MAX_GLM_EDGE_QUEUE_WAIT_MS,
    Math.max(
      0,
      Math.floor(
        value === undefined || !Number.isFinite(value)
          ? DEFAULT_GLM_ASYNC_QUEUE_WAIT_MS
          : value,
      ),
    ),
  )

const defaultSaturationPolicyFor = (
  request: InferenceRequest,
): GlmSaturationPolicy =>
  request.stream ? 'overflow_immediately' : 'queue_then_overflow'

const saturationError = (
  config: HydraliskPoolAdapterConfig,
  input: Readonly<{
    aggregateHeadroom: GlmAggregateLiveHeadroom
    policy: GlmSaturationPolicy
    reason: string
    queueWaitMs: number
  }>,
): InferenceAdapterError =>
  poolAdapterError(
    config,
    input.policy === 'queue_then_429'
      ? `hydralisk GLM pool saturated (${input.reason}); retry later or use the async batch lane`
      : `hydralisk GLM pool saturated (${input.reason}); overflowing to the next Khala lane`,
    {
      httpStatus: 429,
      kind: 'glm_pool_saturated',
      retryable: input.policy !== 'queue_then_429',
      adapterRouteMetadata: {
        glmSaturationPolicy: input.policy,
        glmAggregateExternalHeadroom:
          input.aggregateHeadroom.aggregateExternalHeadroom,
        glmAggregateInflightCount:
          input.aggregateHeadroom.aggregateInflightCount,
        glmAggregateMaxInflight: input.aggregateHeadroom.aggregateMaxInflight,
        queueWaitMs: input.queueWaitMs,
        replicaBusyReason: input.reason,
        replicaFallbackReason: input.reason,
      },
    },
  )

const laneQuorumUnhealthyError = (
  config: HydraliskPoolAdapterConfig,
  input: Readonly<{
    aggregateHeadroom: GlmAggregateLiveHeadroom
    policy: GlmSaturationPolicy
    queueWaitMs: number
  }>,
): InferenceAdapterError =>
  poolAdapterError(
    config,
    'hydralisk GLM lane quorum unhealthy; overflowing to the next Khala lane',
    {
      httpStatus: 503,
      kind: 'lane_quorum_unhealthy',
      retryable: true,
      adapterRouteMetadata: {
        glmSaturationPolicy: input.policy,
        glmAggregateExternalHeadroom:
          input.aggregateHeadroom.aggregateExternalHeadroom,
        glmAggregateInflightCount:
          input.aggregateHeadroom.aggregateInflightCount,
        glmAggregateMaxInflight: input.aggregateHeadroom.aggregateMaxInflight,
        queueWaitMs: input.queueWaitMs,
        replicaBusyReason: 'lane_quorum_unhealthy',
        replicaFallbackReason: 'lane_quorum_unhealthy',
        replicaHealthScore: 0,
      },
    },
  )

const isLaneQuorumUnhealthy = (
  candidates: ReadonlyArray<ReplicaCandidate>,
): boolean => {
  const active = candidates.filter(
    candidate => !candidate.state.benchmarkReserved && !candidate.state.draining,
  )
  if (active.length === 0) {
    return false
  }
  const quorum = Math.floor(active.length / 2) + 1
  const unhealthyObservedCount = active.filter(
    candidate =>
      candidate.routingStateObserved && candidate.state.health === 'unhealthy',
  ).length
  return unhealthyObservedCount >= quorum
}

const aggregateLiveHeadroomFor = (
  candidates: ReadonlyArray<ReplicaCandidate>,
): GlmAggregateLiveHeadroom => {
  const eligibleForExternal = candidates.filter(
    candidate =>
      !candidate.state.benchmarkReserved &&
      !candidate.state.draining &&
      candidate.state.health !== 'unhealthy',
  )
  const aggregateMaxInflight = eligibleForExternal.reduce(
    (sum, candidate) => sum + candidate.state.maxInflight,
    0,
  )
  const aggregateInflightCount = eligibleForExternal.reduce(
    (sum, candidate) => sum + candidate.state.inflightCount,
    0,
  )
  return {
    aggregateExternalHeadroom: Math.max(
      0,
      aggregateMaxInflight - aggregateInflightCount,
    ),
    aggregateInflightCount,
    aggregateMaxInflight,
  }
}

const replicaCandidatesFor = (
  config: HydraliskPoolAdapterConfig,
  inflight: ReadonlyMap<string, number>,
): ReadonlyArray<ReplicaCandidate> =>
  config.replicas.map((replica, index) => {
    const routingStateOverride = config.routingStateOracle?.(replica.replicaId)
    return {
      index,
      replica,
      routingStateObserved: routingStateOverride !== undefined,
      state: stateForReplica(
        replica,
        inflight.get(replica.replicaId) ?? 0,
        routingStateOverride,
      ),
    }
  })

const routeAdmissionForHeadroom = (
  headroom: GlmAggregateLiveHeadroom,
): HydraliskPoolRouteAdmissionSnapshot => {
  if (headroom.aggregateMaxInflight <= 0) {
    return {
      reason: 'glm_pool_no_external_capacity',
      reservedExternalHeadroomAvailable: false,
    }
  }
  if (headroom.aggregateExternalHeadroom <= 0) {
    return {
      reason: 'glm_aggregate_external_headroom_zero',
      reservedExternalHeadroomAvailable: false,
    }
  }
  if (
    headroom.aggregateExternalHeadroom <=
    DEFAULT_RESERVED_EXTERNAL_HEADROOM_SLOTS
  ) {
    return {
      reason: 'glm_reserved_external_headroom_unavailable',
      reservedExternalHeadroomAvailable: false,
    }
  }
  return {
    reason: 'glm_reserved_external_headroom_available',
    reservedExternalHeadroomAvailable: true,
  }
}

export const readHydraliskPoolRouteAdmission = (
  config: HydraliskPoolAdapterConfig,
  inflight: ReadonlyMap<string, number> = new Map(),
): HydraliskPoolRouteAdmissionSnapshot =>
  routeAdmissionForHeadroom(
    aggregateLiveHeadroomFor(replicaCandidatesFor(config, inflight)),
  )

const selectedReplicaConfigOnce = (
  config: HydraliskPoolAdapterConfig,
  request: InferenceRequest,
  inflight: Map<string, number>,
  queueWaitMs: number,
  saturationPolicy: GlmSaturationPolicy,
): Effect.Effect<ReplicaSelection, InferenceAdapterError> =>
  Effect.try({
    catch: error =>
      error instanceof InferenceAdapterError
        ? error
        : poolAdapterError(
            config,
            'hydralisk GLM replica pool selection failed unexpectedly',
          ),
    try: () => {
      const candidates = replicaCandidatesFor(config, inflight)
      const eligible = candidates.filter(
        candidate => busyReasonFor(candidate.state) === null,
      )
      const aggregateHeadroom = aggregateLiveHeadroomFor(candidates)

      if (candidates.length === 0) {
        throw poolAdapterError(
          config,
          'hydralisk GLM replica pool has no configured replicas',
        )
      }

      if (isLaneQuorumUnhealthy(candidates)) {
        throw laneQuorumUnhealthyError(config, {
          aggregateHeadroom,
          policy: saturationPolicy,
          queueWaitMs,
        })
      }

      if (eligible.length === 0) {
        const reason =
          candidates
            .map(candidate => busyReasonFor(candidate.state))
            .find(Boolean) ?? 'no_eligible_replica'
        throw saturationError(config, {
          aggregateHeadroom,
          policy: saturationPolicy,
          queueWaitMs,
          reason,
        })
      }

      const affinity = requestAffinity(request)
      const affinityReplicaId =
        affinity === undefined ? undefined : config.affinityOracle?.(affinity)
      const affinityCandidate =
        affinityReplicaId === undefined
          ? undefined
          : candidates.find(
              candidate => candidate.replica.replicaId === affinityReplicaId,
            )
      const affinityBusy =
        affinityCandidate === undefined
          ? affinityReplicaId === undefined
            ? null
            : 'affinity_replica_missing'
          : busyReasonFor(affinityCandidate.state)
      const selected =
        affinityCandidate !== undefined && affinityBusy === null
          ? affinityCandidate
          : [...eligible].sort(rankEligibleReplicas)[0]!
      const replicaFallbackReason =
        affinityReplicaId === undefined
          ? null
          : selected.replica.replicaId === affinityReplicaId
            ? 'cache_affinity_hit'
            : (affinityBusy ?? 'affinity_replica_not_selected')
      const replicaBusyReason =
        selected.state.inflightCount > 0 ? 'shared_capacity_in_use' : null

      inflight.set(selected.replica.replicaId, selected.state.inflightCount + 1)
      let released = false
      const release = () => {
        if (released) {
          return
        }
        released = true
        const current = inflight.get(selected.replica.replicaId) ?? 0
        if (current <= 1) {
          inflight.delete(selected.replica.replicaId)
        } else {
          inflight.set(selected.replica.replicaId, current - 1)
        }
      }

      return {
        adapterConfig: {
          apiKey: selected.replica.apiKey,
          baseUrl: selected.replica.baseUrl,
          fetchImpl: selected.replica.fetchImpl,
          id: config.id,
          upstreamModel: config.upstreamModel ?? selected.replica.upstreamModel,
        },
        metadata: metadataForSelection(
          selected,
          replicaFallbackReason,
          replicaBusyReason,
          queueWaitMs,
          saturationPolicy,
          aggregateHeadroom,
        ),
        release,
      }
    },
  })

const selectedReplicaConfig = (
  config: HydraliskPoolAdapterConfig,
  request: InferenceRequest,
  inflight: Map<string, number>,
): Effect.Effect<ReplicaSelection, InferenceAdapterError> =>
  Effect.gen(function* () {
    const policy =
      config.saturationPolicy ?? defaultSaturationPolicyFor(request)
    const queueWaitLimitMs =
      policy === 'overflow_immediately'
        ? 0
        : boundedQueueWaitMs(config.maxQueueWaitMs)
    const first = yield* selectedReplicaConfigOnce(
      config,
      request,
      inflight,
      0,
      policy,
    ).pipe(Effect.result)

    if (first._tag === 'Success') {
      return first.success
    }
    if (first.failure.kind !== 'glm_pool_saturated' || queueWaitLimitMs <= 0) {
      return yield* Effect.fail(first.failure)
    }

    const now = config.nowEpochMs ?? Date.now
    const startedAt = now()
    yield* (config.sleep ?? Effect.sleep)(queueWaitLimitMs)
    const elapsed = Math.max(0, now() - startedAt)
    const measuredQueueWaitMs = Math.max(queueWaitLimitMs, elapsed)

    return yield* selectedReplicaConfigOnce(
      config,
      request,
      inflight,
      measuredQueueWaitMs,
      policy,
    )
  })

const terminalChunkIndex = (
  chunks: ReadonlyArray<InferenceStreamChunk>,
): number =>
  chunks.findIndex(
    chunk => chunk.finishReason !== undefined || chunk.usage !== undefined,
  )

const attachMetadataToStreamChunks = (
  chunks: ReadonlyArray<InferenceStreamChunk>,
  metadata: InferenceAdapterRouteMetadata,
): ReadonlyArray<InferenceStreamChunk> => {
  const terminalIndex = terminalChunkIndex(chunks)
  const targetIndex = terminalIndex === -1 ? chunks.length - 1 : terminalIndex
  return chunks.map((chunk, index) =>
    index === targetIndex
      ? { ...chunk, adapterRouteMetadata: metadata }
      : chunk,
  )
}

const attachMetadataToSource = (
  source: InferenceStreamSource,
  metadata: InferenceAdapterRouteMetadata,
  release: () => void,
): InferenceStreamSource => {
  let released = false
  const releaseOnce = () => {
    if (released) {
      return
    }
    released = true
    release()
  }
  const frames = (async function* (): AsyncIterable<InferenceStreamEvent> {
    try {
      for await (const frame of source.frames) {
        yield frame
      }
    } finally {
      releaseOnce()
    }
  })()
  return {
    frames,
    terminal: () => ({
      ...source.terminal(),
      adapterRouteMetadata: metadata,
    }),
  }
}

export const makeHydraliskVllmPoolRuntime = (
  config: HydraliskPoolAdapterConfig,
): HydraliskVllmPoolRuntime => {
  const inflight = new Map<string, number>()
  const adapter: InferenceProviderAdapter = {
    complete: request =>
      selectedReplicaConfig(config, request, inflight).pipe(
        Effect.flatMap(selection =>
          complete(selection.adapterConfig, request).pipe(
            Effect.map(result => ({
              ...result,
              adapterRouteMetadata: selection.metadata,
            })),
            Effect.ensuring(Effect.sync(selection.release)),
          ),
        ),
      ),
    id: config.id,
    stream: request =>
      selectedReplicaConfig(config, request, inflight).pipe(
        Effect.flatMap(selection =>
          streamChunks(selection.adapterConfig, request).pipe(
            Effect.map(chunks =>
              attachMetadataToStreamChunks(chunks, selection.metadata),
            ),
            Effect.ensuring(Effect.sync(selection.release)),
          ),
        ),
      ),
    streamSse: request =>
      selectedReplicaConfig(config, request, inflight).pipe(
        Effect.flatMap(selection =>
          makeHydraliskVllmAdapter(selection.adapterConfig).streamSse!(
            request,
          ).pipe(
            Effect.map(source =>
              attachMetadataToSource(
                source,
                selection.metadata,
                selection.release,
              ),
            ),
            Effect.catch((error: InferenceAdapterError) =>
              Effect.sync(selection.release).pipe(
                Effect.flatMap(() => Effect.fail(error)),
              ),
            ),
          ),
        ),
      ),
  }
  return {
    adapter,
    routeAdmission: () => readHydraliskPoolRouteAdmission(config, inflight),
  }
}

export const makeHydraliskVllmPoolAdapter = (
  config: HydraliskPoolAdapterConfig,
): InferenceProviderAdapter => makeHydraliskVllmPoolRuntime(config).adapter
