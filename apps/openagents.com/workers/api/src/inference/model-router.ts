// Routing & supply selection for the OpenAgents inference gateway
// (EPIC #5474, child #5482).
//
// This module replaces the #5476 `stubModelRouter` with cheapest-viable-supply
// selection across the registered provider lanes:
//   - Vertex Anthropic (#5480)  -> the Claude lane (claude-* / opus / sonnet /
//                                  haiku shared-lineage models from our Vertex
//                                  quota; best margin — gateway business doc §3a)
//   - Fireworks (#5479)         -> the managed open-model lane (deepseek / kimi /
//                                  glm / qwen / minimax / gpt-oss / nemotron —
//                                  doc §3b / fireworks-provider doc)
//   - partner passthrough (#5481) -> breadth + burst coverage for everything
//                                  else (doc §3c); also the OVERFLOW target when
//                                  an owned lane rate-limits (429) or sheds load
//                                  (503).
//
// Two concerns live here, both deterministic + table-driven:
//
//   1. SELECTION. Given a requested model, resolve an ORDERED lane plan: the
//      cheapest viable adapter first, then the overflow fallbacks. Selection is a
//      bounded model-id -> provider config map + a per-class default — never an
//      ad-hoc string match on user intent (CLAUDE.md "Semantic Routing": a
//      bounded model-id -> provider map is the allowed deterministic case).
//      Cheapest-viable ordering within the open class is taken from `pricing.ts`
//      (blended $/Mtok), so the table re-solves when costs change.
//
//   2. DISPATCH WITH OVERFLOW. Try the planned lanes in order. On a typed
//      RETRYABLE failure (429 / 503 / 5xx / transport — `error.retryable`),
//      back off (bounded) and overflow to the next viable lane rather than
//      failing the request. A NON-retryable failure surfaces immediately. If
//      every lane is exhausted, the last failure surfaces.
//
// Boundaries: this module owns selection + overflow only. It never touches
// credits, payment, projection, or provider wire formats — it asks the registry
// for an adapter by id and runs the adapter's own Effect. It is pure config +
// Effect orchestration; the live env is plumbed into the env-dependent adapters
// by index.ts (`setInferenceAdapterEnv`) before dispatch, not here.
import { Effect } from 'effect'

import {
  KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH,
  KHALA_BACKING_HYDRALISK_GPT_OSS,
  type KhalaBackingModel,
} from './model-serving-policy'
import {
  HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  KHALA_MODEL_ID,
  KHALA_PYLON_MINI_MODEL_ID,
  MODEL_PRICING_TABLE,
  type SupplyLane,
  blendedCostPerMtok,
  lookupModel,
  normalizeKhalaModelId,
} from './pricing'
import {
  InferenceAdapterError,
  type InferenceAdapterRouteMetadata,
  type InferenceProviderAdapter,
  type InferenceProviderRegistry,
  type InferenceRequest,
  type InferenceResult,
} from './provider-adapter'

// Registered adapter ids the router can select. Kept as string constants here
// (rather than importing each adapter module) so routing stays a pure config
// layer with no dependency on adapter construction. index.ts registers adapters
// under exactly these ids.
export const VERTEX_ANTHROPIC_ADAPTER_ID = 'vertex-anthropic'
// The Vertex Gemini lane (Google's own model). Serves Gemini 3.5 Flash + other
// gemini-* ids from our first-party Vertex quota — the default/free-tier lane.
export const VERTEX_GEMINI_ADAPTER_ID = 'vertex-gemini'
export const FIREWORKS_ADAPTER_ID = 'fireworks'
export const HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID =
  'hydralisk-vllm-glm-5p2-reap-504b'
export const OPENROUTER_KHALA_FALLBACK_ADAPTER_ID =
  'openrouter-khala-glm-fallback'
export const HYDRALISK_ADAPTER_ID = 'hydralisk-vllm'
export const HYDRALISK_GPT_OSS_120B_ADAPTER_ID = 'hydralisk-vllm-gpt-oss-120b'
export const PASSTHROUGH_ANTHROPIC_ADAPTER_ID = 'passthrough-anthropic'
export const PASSTHROUGH_OPENAI_ADAPTER_ID = 'passthrough-openai'
// The OpenAgents serving-fabric lane (#5483). Maps to the network adapter id.
// Routing keeps it AHEAD of passthrough (our own compute is preferred over a
// pure-passthrough partner; gateway doc §4 "our Vertex quota first ..., then
// network nodes ..., then partner passthrough"). Until a live Psionic fabric
// dispatch seam is wired, index.ts registers NO adapter under this id, so
// `dispatchWithOverflow` (which filters the plan to registered adapters) simply
// SKIPS the lane and falls through to passthrough — the lane is a real,
// selectable insert point but never a faked serve. When the fabric dispatch lands,
// registering a configured `openagents-network` adapter activates the lane with no
// routing change.
export const OPENAGENTS_NETWORK_ADAPTER_ID = 'openagents-network'

// The provider lane a model is routed to. Mirrors `pricing.ts` `SupplyLane`
// (cost provenance) plus the passthrough breadth lane that pricing folds into
// the unknown-model fallback.
export type RouterLane = SupplyLane | 'passthrough'

// ----------------------------------------------------------------------------
// Capability: which adapter id serves a lane
// ----------------------------------------------------------------------------

// The adapter that serves each lane. The passthrough lane resolves to whichever
// partner adapter is registered (Anthropic first for Claude-shaped overflow,
// then OpenAI); routing tries both ids so an absent partner secret simply means
// that fallback is skipped at dispatch time.
const LANE_ADAPTER_IDS: Readonly<Record<RouterLane, ReadonlyArray<string>>> = {
  'vertex-anthropic': [VERTEX_ANTHROPIC_ADAPTER_ID],
  'vertex-gemini': [VERTEX_GEMINI_ADAPTER_ID],
  fireworks: [FIREWORKS_ADAPTER_ID],
  hydralisk: [HYDRALISK_ADAPTER_ID],
  openrouter: [OPENROUTER_KHALA_FALLBACK_ADAPTER_ID],
  'openagents-network': [OPENAGENTS_NETWORK_ADAPTER_ID],
  passthrough: [
    PASSTHROUGH_ANTHROPIC_ADAPTER_ID,
    PASSTHROUGH_OPENAI_ADAPTER_ID,
  ],
}

// ----------------------------------------------------------------------------
// Selection: requested model -> ordered lane plan
// ----------------------------------------------------------------------------

// Model classes we route by. A bounded enum (not free-form intent): the only
// classification we do is "which provider family is this model id?".
export type ModelClass = 'claude' | 'gemini' | 'open' | 'unknown'

const classForPricedLane = (lane: SupplyLane): ModelClass => {
  switch (lane) {
    case 'vertex-anthropic':
      return 'claude'
    case 'vertex-gemini':
      return 'gemini'
    case 'fireworks':
    case 'hydralisk':
    case 'openrouter':
    case 'openagents-network':
      return 'open'
  }
}

// Gemini-family model ids served from the Vertex Gemini lane. Matched against
// the LOWERCASED requested id. The bare `gemini` alias and the canonical
// `gemini-*` ids both route here, as do `google/` / `vertex/` provider-prefixed
// forms. This is a bounded model-id classifier, not an intent parser.
const isGeminiModel = (model: string): boolean => {
  const id = model
    .trim()
    .toLowerCase()
    .replace(/^(?:vertex|google)\//u, '')
  return id === 'gemini' || id.startsWith('gemini-') || id.startsWith('gemini.')
}

// Claude-family model ids served from the Vertex lane. Matched against the
// LOWERCASED requested id. Bare aliases (`opus` / `sonnet` / `haiku` / `fable`)
// and the canonical `claude-*` ids both route here, as do `anthropic/` /
// `vertex/` provider-prefixed forms. This is a bounded model-id classifier, not
// an intent parser.
const isClaudeModel = (model: string): boolean => {
  const id = model
    .trim()
    .toLowerCase()
    .replace(/^(?:vertex|anthropic)\//u, '')
  if (id.startsWith('claude-') || id.startsWith('claude.')) {
    return true
  }
  return id === 'opus' || id === 'sonnet' || id === 'haiku' || id === 'fable'
}

// The open-model set served from Fireworks (doc §3b). Membership is by family
// prefix so the bounded set in the pricing table plus future same-family ids
// (e.g. a new `deepseek-*`) route to Fireworks without a code change. Still a
// bounded provider-family classifier.
const OPEN_MODEL_PREFIXES: ReadonlyArray<string> = [
  'deepseek',
  'kimi',
  'glm',
  'qwen',
  'minimax',
  'gpt-oss',
  'nemotron',
]

const isOpenModel = (model: string): boolean => {
  const id = model
    .trim()
    .toLowerCase()
    .replace(/^fireworks\//u, '')
  // A model priced on the Fireworks lane is open by definition.
  if (lookupModel(id)?.lane === 'fireworks') {
    return true
  }
  return OPEN_MODEL_PREFIXES.some(prefix => id.startsWith(prefix))
}

export const classifyModel = (model: string): ModelClass => {
  const normalizedModel = normalizeKhalaModelId(model)
  if (isClaudeModel(normalizedModel)) {
    return 'claude'
  }
  if (isGeminiModel(normalizedModel)) {
    return 'gemini'
  }
  if (isOpenModel(normalizedModel)) {
    return 'open'
  }
  const priced = lookupModel(normalizedModel)
  if (priced !== undefined) {
    return classForPricedLane(priced.lane)
  }
  return 'unknown'
}

// Cheapest-viable ordering of the Fireworks open models, by blended $/Mtok from
// the pricing table (so the ordering re-solves when costs change). Exposed for
// callers/tests that want the cheapest open model id; the dispatch path routes
// the whole open class to the single Fireworks adapter, then overflows to
// passthrough.
export const openModelsByCost: ReadonlyArray<string> =
  MODEL_PRICING_TABLE.filter(entry => entry.lane === 'fireworks')
    .map(entry => ({
      cost: blendedCostPerMtok(entry.cost),
      model: entry.model,
    }))
    .sort((a, b) => a.cost - b.cost)
    .map(entry => entry.model)

// The ordered lane plan for a model class. Cheapest viable owned lane first,
// then overflow lanes (gateway business doc §4: "our Vertex quota first (best
// margin), then network nodes ..., then partner passthrough (coverage)").
//
//   - claude  : Vertex (owned, best margin) -> passthrough (Anthropic breadth)
//   - open    : Fireworks (managed open) -> OpenAgents serving fabric (#5483)
//               -> passthrough (OpenAI breadth)
//   - unknown : passthrough only (coverage for models we hold no quota for)
//
// The OpenAgents serving-fabric lane (#5483) is placed in the OPEN class plan,
// AHEAD of partner passthrough — per the shard-WAN doc §6 the near-term start
// routes the cheap/open tier to our own Pylon serving (the margin lives there and
// it is the only lane whose margin fans to contributors), keeping large frontier
// (claude) on Vertex + passthrough. The lane is INERT today: its adapter
// typed-fails `network_dispatch_unavailable` (non-retryable), so `dispatchWithOverflow`
// falls straight through to passthrough until a live Psionic fabric dispatch is
// wired — it is a real, selectable lane but never a faked serve.
// The gemini class routes to our first-party Vertex Gemini lane only. There is
// no partner-passthrough overflow for Gemini today (we hold the quota directly;
// no partner Gemini key is wired), so a Vertex Gemini failure surfaces rather
// than overflowing to an unconfigured lane.
const LANE_PLAN_BY_CLASS: Readonly<
  Record<ModelClass, ReadonlyArray<RouterLane>>
> = {
  claude: ['vertex-anthropic', 'passthrough'],
  gemini: ['vertex-gemini'],
  open: ['fireworks', 'openagents-network', 'passthrough'],
  unknown: ['passthrough'],
}

const KHALA_HYDRALISK_ADAPTER_PLAN: ReadonlyArray<string> = [
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  FIREWORKS_ADAPTER_ID,
]

const KHALA_FIREWORKS_DEEPSEEK_ADAPTER_PLAN: ReadonlyArray<string> = [
  FIREWORKS_ADAPTER_ID,
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
]

const KHALA_TOOL_SAFE_ADAPTER_PLAN: ReadonlyArray<string> = [
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  FIREWORKS_ADAPTER_ID,
]

const dedupeAdapterPlan = (
  adapterIds: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const seen = new Set<string>()
  return adapterIds.filter(adapterId => {
    if (seen.has(adapterId)) {
      return false
    }
    seen.add(adapterId)
    return true
  })
}

export const selectAdapterPlanForKhalaBacking = (
  model: string,
  khalaBacking: KhalaBackingModel = KHALA_BACKING_HYDRALISK_GPT_OSS,
): ReadonlyArray<string> => {
  const normalizedModel = normalizeKhalaModelId(model)
  if (normalizedModel === KHALA_MODEL_ID) {
    return khalaBacking === KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH
      ? KHALA_FIREWORKS_DEEPSEEK_ADAPTER_PLAN
      : KHALA_HYDRALISK_ADAPTER_PLAN
  }
  return selectAdapterPlan(model)
}

export const selectAdapterPlanForKhalaToolRequest = (
  model: string,
  basePlan: ReadonlyArray<string> = selectAdapterPlan(model),
): ReadonlyArray<string> => {
  const normalizedModel = normalizeKhalaModelId(model)
  if (normalizedModel !== KHALA_MODEL_ID) {
    return basePlan
  }
  return dedupeAdapterPlan([
    ...KHALA_TOOL_SAFE_ADAPTER_PLAN,
    ...basePlan.filter(
      adapterId => adapterId !== HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
    ),
  ])
}

export const makeKhalaBackedAdapterPlan =
  (khalaBacking: KhalaBackingModel | undefined) =>
  (model: string): ReadonlyArray<string> =>
    selectAdapterPlanForKhalaBacking(
      model,
      khalaBacking ?? KHALA_BACKING_HYDRALISK_GPT_OSS,
    )

// Resolve a requested model to its ORDERED list of candidate adapter ids
// (cheapest viable first, then overflow fallbacks). Deterministic + pure.
export const selectAdapterPlan = (model: string): ReadonlyArray<string> => {
  const normalizedModel = normalizeKhalaModelId(model)
  if (normalizedModel === KHALA_MODEL_ID) {
    // Khala-first: the Hydralisk owned lanes serve the collapsed public Khala
    // model, with GLM-5.2 REAP first when that private G4 route is armed, then
    // the hidden OpenRouter free fallback, Vertex Gemini, then Fireworks as the
    // final graceful-degradation overflow so a full GLM/OpenRouter/Gemini outage
    // degrades instead of failing the whole product with `inference_unavailable`.
    // Explicit raw Hydralisk model ids below keep NO Gemini/Fireworks fallback —
    // they are deliberate supply-lane requests, not the generic Khala lane.
    return KHALA_HYDRALISK_ADAPTER_PLAN
  }
  if (normalizedModel === HYDRALISK_GLM_52_REAP_504B_MODEL_ID) {
    return [HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID]
  }
  if (normalizedModel === HYDRALISK_GPT_OSS_20B_MODEL_ID) {
    return [HYDRALISK_ADAPTER_ID]
  }
  if (normalizedModel === HYDRALISK_GPT_OSS_120B_MODEL_ID) {
    return [HYDRALISK_GPT_OSS_120B_ADAPTER_ID]
  }
  if (normalizedModel === KHALA_PYLON_MINI_MODEL_ID) {
    return [OPENAGENTS_NETWORK_ADAPTER_ID]
  }
  const plan = LANE_PLAN_BY_CLASS[classifyModel(normalizedModel)]
  return plan.flatMap(lane => LANE_ADAPTER_IDS[lane])
}

// Backward-compatible single-id resolver satisfying the route's existing
// `ModelRouter` seam (`(model) => string | undefined`). Returns the PRIMARY
// (cheapest viable) adapter id, or undefined when no lane is configured for the
// class. The route uses `dispatchWithOverflow` for the real multi-lane path;
// this keeps the simple seam working for callers that only need the primary id.
export const selectPrimaryAdapterId = (model: string): string | undefined =>
  selectAdapterPlan(model)[0]

// ----------------------------------------------------------------------------
// Dispatch with bounded backoff + overflow
// ----------------------------------------------------------------------------

// Bounded backoff config for overflow. Deterministic: delay before the Nth
// overflow attempt = baseDelayMs × 2^(attemptIndex), capped at maxDelayMs. Kept
// tiny by default; tests inject `sleep: () => Effect.void` so they never wait.
export type OverflowBackoff = Readonly<{
  baseDelayMs: number
  maxDelayMs: number
}>

export const DEFAULT_OVERFLOW_BACKOFF: OverflowBackoff = {
  baseDelayMs: 50,
  maxDelayMs: 1_000,
}

const backoffDelayMs = (
  backoff: OverflowBackoff,
  attemptIndex: number,
): number =>
  Math.min(backoff.maxDelayMs, backoff.baseDelayMs * 2 ** attemptIndex)

// A single adapter operation (complete or stream) the dispatcher runs against a
// resolved adapter. Generic over the adapter's success type so the same overflow
// loop drives both `complete` and `stream`.
export type AdapterOperation<A> = (
  adapter: InferenceProviderAdapter,
  request: InferenceRequest,
) => Effect.Effect<A, InferenceAdapterError>

export type DispatchLaneValidation =
  | Readonly<{ _tag: 'accepted' }>
  | Readonly<{ _tag: 'failed'; error: InferenceAdapterError }>

export type DispatchSuccessValidator<A> = (
  input: Readonly<{
    adapter: InferenceProviderAdapter
    request: InferenceRequest
    value: A
  }>,
) => DispatchLaneValidation

export const acceptDispatchLane: DispatchLaneValidation = { _tag: 'accepted' }

const recordFromUnknown = (
  value: unknown,
): Readonly<Record<string, unknown>> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Readonly<Record<string, unknown>>)
    : undefined

const inferenceResultFromUnknown = (
  value: unknown,
): InferenceResult | undefined => {
  const record = recordFromUnknown(value)
  if (record === undefined) {
    return undefined
  }
  if (
    typeof record['content'] === 'string' &&
    typeof record['finishReason'] === 'string' &&
    typeof record['servedModel'] === 'string'
  ) {
    return value as InferenceResult
  }
  return inferenceResultFromUnknown(record['value'])
}

const validateDefaultDispatchSuccess = <A>(
  input: Readonly<{
    adapter: InferenceProviderAdapter
    value: A
  }>,
): DispatchLaneValidation => {
  const result = inferenceResultFromUnknown(input.value)
  if (result === undefined) {
    return acceptDispatchLane
  }
  return result.content.trim() === '' &&
    (result.toolCalls === undefined || result.toolCalls.length === 0)
    ? {
        _tag: 'failed',
        error: new InferenceAdapterError({
          adapterId: input.adapter.id,
          kind: 'empty_assistant_content',
          reason: 'adapter returned empty assistant content',
          retryable: true,
        }),
      }
    : acceptDispatchLane
}

export type ProviderRoutingSignals = Readonly<{
  // Coarse circuit-breaker state. `unhealthy` and `quarantined` remove the lane
  // from eligibility just like a draining GLM replica; `degraded` remains
  // eligible so a lane-wide breaker only trips when the control plane says the
  // lane/quorum is no longer safe to serve.
  laneHealth?: ProviderLaneHealth | undefined
  // Public-safe provider health score in [0, 1], where 1 means the lane is fully
  // healthy according to the injected control-plane snapshot. Undefined means
  // the gateway has no measured score for this lane.
  providerHealthScore?: number | undefined
  // Coarse serving region exposed by the lane/control plane. Undefined means the
  // provider did not disclose a region to the gateway.
  region?: string | undefined
  // Public-safe warm/cold state used only for bounded external hedging.
  warmState?: ProviderWarmState | undefined
  // Measured first-token latency for this lane/replica class. Undefined means
  // hedging has no evidence and stays inert.
  ttftP99Ms?: number | undefined
}>

export type ProviderRoutingSignalsOracle = (
  adapterId: string,
) => ProviderRoutingSignals | undefined

export type ProviderLaneHealth =
  | 'degraded'
  | 'healthy'
  | 'quarantined'
  | 'unhealthy'

export type ProviderWarmState = 'cold' | 'unknown' | 'warm'

export type InferenceDemandClass =
  | 'batch'
  | 'external'
  | 'internal_stress'
  | 'keep_warm'

export type DispatchRetryPolicy = Readonly<{
  maxRetriesPerLane: number
}>

export type DispatchSloSnapshot = Readonly<{
  breached: boolean
  reason: string
}>

export type DispatchLoadSheddingPolicy = Readonly<{
  demandClass: InferenceDemandClass
  slo: DispatchSloSnapshot
}>

export type DispatchRouteAdmissionPolicy = Readonly<{
  demandClass: InferenceDemandClass
  reservedExternalHeadroomAvailable: boolean
  reason: string
}>

export type DispatchHedgingPolicy = Readonly<{
  demandClass: InferenceDemandClass
  enabled: boolean
  ttftP99ThresholdMs: number
}>

export type DispatchFailureTelemetryEvent = Readonly<{
  adapterId: string
  classifier: DispatchFailureTelemetryClassifier
  stage:
    | 'adapter_error'
    | 'fallback'
    | 'health_quarantine'
    | 'hedged'
    | 'load_shed'
    | 'validation_failure'
  kind: string
  retryable: boolean
  httpStatus?: number | undefined
}>

export type DispatchFailureTelemetry = (
  event: DispatchFailureTelemetryEvent,
) => void

export type DispatchFailureTelemetryClassifier =
  | 'empty_content'
  | 'fallback'
  | 'invalid_tool'
  | 'provider_error'
  | 'rate_limited_429'

export type DispatchFailureTelemetrySnapshot = Readonly<{
  counts: Readonly<Record<DispatchFailureTelemetryClassifier, number>>
  events: ReadonlyArray<DispatchFailureTelemetryEvent>
  windowMs: number
}>

export type BoundedDispatchFailureTelemetry = Readonly<{
  record: DispatchFailureTelemetry
  snapshot: (nowMs?: number) => DispatchFailureTelemetrySnapshot
}>

export type DispatchDeps = Readonly<{
  registry: InferenceProviderRegistry
  // Defaults to the pure selector; injectable for tests.
  plan?: ((model: string) => ReadonlyArray<string>) | undefined
  backoff?: OverflowBackoff | undefined
  // Injected delay (defaults to Effect.sleep). Tests pass `() => Effect.void`.
  sleep?: ((ms: number) => Effect.Effect<void>) | undefined
  // Optional inert-by-default control-plane snapshot. When absent, dispatch
  // behavior is unchanged and receipts use honest not_measured sentinels.
  routingSignals?: ProviderRoutingSignalsOracle | undefined
  // Optional typed same-lane retry. Defaults to zero retries, preserving the
  // existing overflow-on-first-retryable-failure behavior.
  retry?: DispatchRetryPolicy | undefined
  // Optional SLO shedding. Only non-external demand can be shed; external demand
  // continues through normal dispatch even when the SLO snapshot is breached.
  shedding?: DispatchLoadSheddingPolicy | undefined
  // Optional typed route admission guard. It only refuses internal_stress when
  // the control-plane snapshot says reserved external headroom is unavailable;
  // external demand still dispatches through the normal lane plan.
  admission?: DispatchRouteAdmissionPolicy | undefined
  // Optional bounded hedge. When enabled for external demand, a quarantined or
  // P99-breaching primary can be skipped once to a different warm eligible lane.
  hedging?: DispatchHedgingPolicy | undefined
  // Optional public-safe failure telemetry. The event shape carries counts and
  // neutral classifiers only: never prompts, completions, URLs, IPs, or tokens.
  failureTelemetry?: DispatchFailureTelemetry | undefined
}>

export type DispatchRouteMetadata = Readonly<{
  primaryAdapterId: string
  servedAdapterId: string
  fallbackReason: string | null
  laneHealth?: ProviderLaneHealth | undefined
  providerHealthScore?: number | undefined
  region?: string | undefined
  fallbackAdapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined
  ttftP99Ms?: number | undefined
  warmState?: ProviderWarmState | undefined
}>

export type DispatchWithOverflowResult<A> = Readonly<{
  value: A
  route: DispatchRouteMetadata
}>

const normalizeProviderHealthScore = (
  value: number | undefined,
): number | undefined =>
  value === undefined || !Number.isFinite(value) || value < 0 || value > 1
    ? undefined
    : value

const normalizeRegion = (value: string | undefined): string | undefined =>
  value === undefined || value.trim() === '' ? undefined : value.trim()

const normalizeTtftP99Ms = (value: number | undefined): number | undefined =>
  value === undefined || !Number.isFinite(value) || value < 0
    ? undefined
    : value

const fallbackReasonFor = (error: InferenceAdapterError): string =>
  error.kind ??
  (error.httpStatus === undefined
    ? 'retryable_provider_error'
    : `http_${error.httpStatus}`)

export const classifyDispatchFailureTelemetry = (
  input: Readonly<{
    httpStatus?: number | undefined
    kind: string
    stage: DispatchFailureTelemetryEvent['stage']
  }>,
): DispatchFailureTelemetryClassifier => {
  if (input.stage === 'fallback' || input.stage === 'hedged') {
    return 'fallback'
  }
  if (input.httpStatus === 429 || input.kind === 'rate_limited') {
    return 'rate_limited_429'
  }
  if (input.kind === 'empty_assistant_content') {
    return 'empty_content'
  }
  if (
    input.kind === 'tool_required_no_tool_calls' ||
    input.kind === 'invalid_tool' ||
    input.kind.startsWith('invalid_tool_')
  ) {
    return 'invalid_tool'
  }
  return 'provider_error'
}

const emptyTelemetryCounts = (): Record<
  DispatchFailureTelemetryClassifier,
  number
> => ({
  empty_content: 0,
  fallback: 0,
  invalid_tool: 0,
  provider_error: 0,
  rate_limited_429: 0,
})

export const makeBoundedDispatchFailureTelemetry = (
  input: Readonly<{
    maxEvents?: number | undefined
    nowMs: () => number
    windowMs: number
  }>,
): BoundedDispatchFailureTelemetry => {
  const maxEvents =
    input.maxEvents === undefined ||
    !Number.isFinite(input.maxEvents) ||
    input.maxEvents <= 0
      ? 128
      : Math.floor(input.maxEvents)
  const now = input.nowMs
  let entries: Array<
    Readonly<{ atMs: number; event: DispatchFailureTelemetryEvent }>
  > = []
  const prune = (nowMs: number): void => {
    const floor = nowMs - input.windowMs
    const freshEntries = entries.filter(entry => entry.atMs >= floor)
    entries = freshEntries.slice(Math.max(0, freshEntries.length - maxEvents))
  }
  return {
    record: event => {
      entries.push({ atMs: now(), event })
      prune(now())
    },
    snapshot: nowMs => {
      const effectiveNowMs = nowMs ?? now()
      prune(effectiveNowMs)
      const counts = emptyTelemetryCounts()
      for (const entry of entries) {
        counts[entry.event.classifier] += 1
      }
      return {
        counts,
        events: entries.map(entry => entry.event),
        windowMs: input.windowMs,
      }
    },
  }
}

const isLaneHealthEligible = (
  signals: ProviderRoutingSignals | undefined,
): boolean =>
  signals?.laneHealth !== 'quarantined' && signals?.laneHealth !== 'unhealthy'

const shouldShedRequest = (
  shedding: DispatchLoadSheddingPolicy | undefined,
): boolean =>
  shedding !== undefined &&
  shedding.slo.breached &&
  shedding.demandClass !== 'external'

const shedError = (
  shedding: DispatchLoadSheddingPolicy,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: 'router',
    kind: `slo_shed_${shedding.demandClass}`,
    reason: `request shed because SLO is breached: ${shedding.slo.reason}`,
    retryable: true,
  })

const shouldRejectAdmission = (
  admission: DispatchRouteAdmissionPolicy | undefined,
): boolean =>
  admission !== undefined &&
  admission.demandClass === 'internal_stress' &&
  !admission.reservedExternalHeadroomAvailable

const admissionError = (
  admission: DispatchRouteAdmissionPolicy,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: 'router',
    httpStatus: 429,
    kind: 'route_admission_reserved_headroom_unavailable',
    reason: `internal_stress rejected because reserved external headroom is unavailable: ${admission.reason}`,
    retryable: true,
  })

const normalizedRetryCount = (
  retry: DispatchRetryPolicy | undefined,
): number => {
  const value = retry?.maxRetriesPerLane
  return value === undefined || !Number.isFinite(value) || value <= 0
    ? 0
    : Math.floor(value)
}

const recordFailureTelemetry = (
  telemetry: DispatchFailureTelemetry | undefined,
  event: DispatchFailureTelemetryEvent,
): void => {
  telemetry?.(event)
}

const telemetryForError = (
  error: InferenceAdapterError,
  stage: DispatchFailureTelemetryEvent['stage'],
): DispatchFailureTelemetryEvent => {
  const kind = fallbackReasonFor(error)
  return {
    adapterId: error.adapterId,
    classifier: classifyDispatchFailureTelemetry({
      httpStatus: error.httpStatus,
      kind,
      stage,
    }),
    httpStatus: error.httpStatus,
    kind,
    retryable: error.retryable,
    stage,
  }
}

const shouldHedgeToWarmLane = (
  input: Readonly<{
    hedging: DispatchHedgingPolicy | undefined
    primarySignals: ProviderRoutingSignals | undefined
    hedgeSignals: ProviderRoutingSignals | undefined
  }>,
): boolean => {
  if (
    input.hedging?.enabled !== true ||
    input.hedging.demandClass !== 'external'
  ) {
    return false
  }
  const primaryTtft = input.primarySignals?.ttftP99Ms
  return (
    primaryTtft !== undefined &&
    Number.isFinite(primaryTtft) &&
    primaryTtft > input.hedging.ttftP99ThresholdMs &&
    input.hedgeSignals?.warmState === 'warm' &&
    isLaneHealthEligible(input.hedgeSignals)
  )
}

const attemptAdapterWithRetry = <A>(
  input: Readonly<{
    adapter: InferenceProviderAdapter
    request: InferenceRequest
    operation: AdapterOperation<A>
    retryCount: number
    backoff: OverflowBackoff
    sleep: (ms: number) => Effect.Effect<void>
    failureTelemetry?: DispatchFailureTelemetry | undefined
  }>,
): Effect.Effect<
  | Readonly<{ ok: true; value: A }>
  | Readonly<{ error: InferenceAdapterError; ok: false }>,
  never
> =>
  Effect.gen(function* () {
    let latestError: InferenceAdapterError | undefined
    for (let attempt = 0; attempt <= input.retryCount; attempt += 1) {
      if (attempt > 0) {
        yield* input.sleep(backoffDelayMs(input.backoff, attempt - 1))
      }
      const outcome = yield* input
        .operation(input.adapter, input.request)
        .pipe(
          Effect.map(value => ({ ok: true as const, value })),
          Effect.catch(error =>
            Effect.succeed({ error, ok: false as const }),
          ),
        )
      if (outcome.ok) {
        return outcome
      }
      latestError = outcome.error
      recordFailureTelemetry(
        input.failureTelemetry,
        telemetryForError(outcome.error, 'adapter_error'),
      )
      if (!outcome.error.retryable || attempt >= input.retryCount) {
        return outcome
      }
    }
    return {
      error:
        latestError ??
        new InferenceAdapterError({
          adapterId: input.adapter.id,
          kind: 'upstream_error',
          reason: 'provider lane retry exhausted without a terminal error',
          retryable: true,
        }),
      ok: false as const,
    }
  })

// Run an adapter operation across the model's lane plan with bounded-backoff
// overflow. Lanes that are not registered (e.g. an absent partner secret) are
// skipped. A retryable failure backs off and overflows to the next lane; a
// non-retryable failure surfaces immediately. When every viable lane fails (or
// none is configured) the last/first failure surfaces as a typed error.
export const dispatchWithOverflowWithMetadata = <A>(
  request: InferenceRequest,
  operation: AdapterOperation<A>,
  deps: DispatchDeps,
  validateSuccess?: DispatchSuccessValidator<A>,
): Effect.Effect<DispatchWithOverflowResult<A>, InferenceAdapterError> =>
  Effect.gen(function* () {
    const planFor = deps.plan ?? selectAdapterPlan
    const backoff = deps.backoff ?? DEFAULT_OVERFLOW_BACKOFF
    const sleep = deps.sleep ?? Effect.sleep
    const retryCount = normalizedRetryCount(deps.retry)

    const admission = deps.admission
    if (shouldRejectAdmission(admission) && admission !== undefined) {
      const error = admissionError(admission)
      recordFailureTelemetry(
        deps.failureTelemetry,
        telemetryForError(error, 'load_shed'),
      )
      return yield* Effect.fail(error)
    }

    const shedding = deps.shedding
    if (shouldShedRequest(shedding) && shedding !== undefined) {
      const error = shedError(shedding)
      recordFailureTelemetry(
        deps.failureTelemetry,
        telemetryForError(error, 'load_shed'),
      )
      return yield* Effect.fail(error)
    }

    const adapterIds = planFor(request.model)
    let healthQuarantinedLaneCount = 0
    // Resolve to the lanes that are actually registered (skip absent partners).
    const adapters = adapterIds.flatMap(id => {
      const adapter = deps.registry.resolve(id)
      if (adapter === undefined) {
        return []
      }
      const signals = deps.routingSignals?.(id)
      if (isLaneHealthEligible(signals)) {
        return [adapter]
      }
      healthQuarantinedLaneCount += 1
      recordFailureTelemetry(deps.failureTelemetry, {
        adapterId: id,
        classifier: 'provider_error',
        kind: signals?.laneHealth ?? 'unhealthy',
        retryable: true,
        stage: 'health_quarantine',
      })
      return []
    })

    if (adapters.length === 0) {
      if (healthQuarantinedLaneCount > 0) {
        return yield* Effect.fail(
          new InferenceAdapterError({
            adapterId: 'router',
            kind: 'lane_quorum_unhealthy',
            reason:
              'all registered provider lanes are quarantined or unhealthy',
            retryable: true,
          }),
        )
      }
      return yield* Effect.fail(
        new InferenceAdapterError({
          adapterId: 'router',
          kind: 'configuration_error',
          reason: `no provider lane configured for model "${request.model}"`,
          retryable: false,
        }),
      )
    }

    let lastError: InferenceAdapterError | undefined
    let overflowCount = 0
    let fallbackReason: string | null = null
    let fallbackAdapterRouteMetadata: InferenceAdapterRouteMetadata | undefined
    let startIndex = 0
    const primaryAdapterId = adapters[0]!.id
    const primarySignals = deps.routingSignals?.(primaryAdapterId)
    const hedgeAdapter = adapters[1]
    if (
      hedgeAdapter !== undefined &&
      shouldHedgeToWarmLane({
        hedgeSignals: deps.routingSignals?.(hedgeAdapter.id),
        hedging: deps.hedging,
        primarySignals,
      })
    ) {
      startIndex = 1
      fallbackReason = 'hedged_ttft_p99_breach'
      recordFailureTelemetry(deps.failureTelemetry, {
        adapterId: primaryAdapterId,
        classifier: 'fallback',
        kind: 'hedged_ttft_p99_breach',
        retryable: true,
        stage: 'hedged',
      })
    }

    for (let index = startIndex; index < adapters.length; index += 1) {
      const adapter = adapters[index]!
      // Back off before an overflow attempt (never before the first attempt).
      if (index > startIndex) {
        yield* sleep(backoffDelayMs(backoff, overflowCount))
        overflowCount += 1
      }

      const outcome = yield* attemptAdapterWithRetry({
        adapter,
        backoff,
        failureTelemetry: deps.failureTelemetry,
        operation,
        request,
        retryCount,
        sleep,
      })

      if (outcome.ok) {
        const validation =
          validateSuccess?.({ adapter, request, value: outcome.value }) ??
          validateDefaultDispatchSuccess({ adapter, value: outcome.value })
        if (validation._tag === 'failed') {
          lastError = validation.error
          recordFailureTelemetry(
            deps.failureTelemetry,
            telemetryForError(validation.error, 'validation_failure'),
          )
          if (!validation.error.retryable) {
            return yield* Effect.fail(validation.error)
          }
          fallbackReason = fallbackReasonFor(validation.error)
          fallbackAdapterRouteMetadata = validation.error.adapterRouteMetadata
          continue
        }
        const signals = deps.routingSignals?.(adapter.id)
        const providerHealthScore = normalizeProviderHealthScore(
          signals?.providerHealthScore,
        )
        const region = normalizeRegion(signals?.region)
        const ttftP99Ms = normalizeTtftP99Ms(signals?.ttftP99Ms)
        if (fallbackReason !== null) {
          recordFailureTelemetry(deps.failureTelemetry, {
            adapterId: adapter.id,
            classifier: 'fallback',
            kind: fallbackReason,
            retryable: true,
            stage: 'fallback',
          })
        }
        return {
          route: {
            fallbackReason,
            ...(fallbackAdapterRouteMetadata === undefined
              ? {}
              : { fallbackAdapterRouteMetadata }),
            ...(signals?.laneHealth === undefined
              ? {}
              : { laneHealth: signals.laneHealth }),
            primaryAdapterId,
            servedAdapterId: adapter.id,
            ...(providerHealthScore === undefined
              ? {}
              : { providerHealthScore }),
            ...(region === undefined ? {} : { region }),
            ...(ttftP99Ms === undefined ? {} : { ttftP99Ms }),
            ...(signals?.warmState === undefined
              ? {}
              : { warmState: signals.warmState }),
          },
          value: outcome.value,
        }
      }

      lastError = outcome.error
      // Non-retryable: surface immediately, no overflow.
      if (!outcome.error.retryable) {
        return yield* Effect.fail(outcome.error)
      }
      fallbackReason = fallbackReasonFor(outcome.error)
      fallbackAdapterRouteMetadata = outcome.error.adapterRouteMetadata
      // Retryable: continue to the next viable lane (loop), backing off above.
    }

    // Every viable lane failed with a retryable error: surface the last one so
    // the route maps it to a stable provider_error rather than overflowing
    // forever.
    return yield* Effect.fail(
      lastError ??
        new InferenceAdapterError({
          adapterId: 'router',
          kind: 'upstream_error',
          reason: 'all provider lanes exhausted',
          retryable: true,
        }),
    )
  })

export const dispatchWithOverflow = <A>(
  request: InferenceRequest,
  operation: AdapterOperation<A>,
  deps: DispatchDeps,
  validateSuccess?: DispatchSuccessValidator<A>,
): Effect.Effect<A, InferenceAdapterError> =>
  dispatchWithOverflowWithMetadata(
    request,
    operation,
    deps,
    validateSuccess,
  ).pipe(Effect.map(result => result.value))
