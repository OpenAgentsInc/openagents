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
// A SECOND registration of the Fireworks adapter, reserved for the strongest
// tool-capable coding lane. index.ts registers an overflow-safe alias adapter
// under this id whose model is rewritten (by the chat route's
// `requestForAdapter`) to the full frontier GLM coding model. It exists so a
// frontier coding lane can sit AHEAD of the normal Fireworks Khala backing in
// the plan while still overflowing to that proven backing — the same adapter id
// cannot otherwise appear twice with two different backing models.
export const FIREWORKS_STRONG_CODING_ADAPTER_ID = 'fireworks-strong-coding'
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

const KHALA_CONVERSATIONAL_ADAPTER_PLAN: ReadonlyArray<string> = [
  VERTEX_GEMINI_ADAPTER_ID,
  FIREWORKS_ADAPTER_ID,
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
]

const KHALA_FIREWORKS_DEEPSEEK_ADAPTER_PLAN: ReadonlyArray<string> = [
  FIREWORKS_ADAPTER_ID,
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
]

const KHALA_AGENT_TOOL_ADAPTER_PLAN: ReadonlyArray<string> = [
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  FIREWORKS_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
]

// Strongest tool-capable coding plan for Khala. Used ONLY for internal,
// honestly-tagged frontier-coding eval load (e.g. the MirrorCode gym rung) where
// we deliberately want the best coding model Khala can serve rather than the
// latency-first conversational backing. The frontier GLM coding lane leads;
// it overflows to the proven Fireworks Khala backing, then Vertex Gemini, then
// the hidden OpenRouter free lane. The owned GLM-5.2-REAP lane is intentionally
// EXCLUDED here because its tool-calling is unreliable for agentic coding loops
// (see #6310), which is exactly what dumps these runs onto a weak fallback.
const KHALA_STRONG_CODING_ADAPTER_PLAN: ReadonlyArray<string> = [
  FIREWORKS_STRONG_CODING_ADAPTER_ID,
  FIREWORKS_ADAPTER_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
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
      : KHALA_CONVERSATIONAL_ADAPTER_PLAN
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
    ...KHALA_AGENT_TOOL_ADAPTER_PLAN,
    ...basePlan.filter(
      adapterId =>
        adapterId !== HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID &&
        adapterId !== FIREWORKS_ADAPTER_ID &&
        adapterId !== VERTEX_GEMINI_ADAPTER_ID &&
        adapterId !== OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
    ),
  ])
}

// Resolve the STRONGEST tool-capable coding plan for a Khala request. Only the
// public Khala alias is upgraded; any other id keeps its base plan. Deterministic
// + pure; the caller gates this on internal frontier-coding attribution so it
// never changes normal conversational or external Khala routing.
export const selectAdapterPlanForKhalaStrongCodingRequest = (
  model: string,
  basePlan: ReadonlyArray<string> = selectAdapterPlan(model),
): ReadonlyArray<string> =>
  normalizeKhalaModelId(model) === KHALA_MODEL_ID
    ? KHALA_STRONG_CODING_ADAPTER_PLAN
    : basePlan

export const makeKhalaBackedAdapterPlan =
  (khalaBacking: KhalaBackingModel | undefined) =>
  (model: string): ReadonlyArray<string> =>
    selectAdapterPlanForKhalaBacking(
      model,
      khalaBacking ?? KHALA_BACKING_HYDRALISK_GPT_OSS,
    )

// ----------------------------------------------------------------------------
// MULTI-LANE FAN-OUT (throughput unlock)
// ----------------------------------------------------------------------------
//
// The normal Khala plan is an ORDERED overflow chain: every request starts on
// the same primary lane (Vertex Gemini today) and only moves to the next lane
// when the primary typed-fails retryably (429 / 503 / transport). When the
// primary lane serves at its own rate WITHOUT 429ing (exactly Vertex's
// behavior), every request pins to that one lane and aggregate throughput is
// capped at a single lane's serve rate — overflow never triggers, so the other
// healthy paid lanes sit idle.
//
// For honestly-tagged internal continual-learning / stress burn demand we want
// the opposite: spread successive concurrent requests ACROSS the healthy paid
// lanes so aggregate throughput becomes the SUM of the lanes' capacities. This
// is a pure ROTATION of the lane plan by a per-request round-robin index: request
// N starts on lane (N mod laneCount), and each request still keeps the full
// remaining plan behind its chosen primary, so a chosen lane that rate-limits or
// is quarantined still overflows down the rest of the chain (fail-closed posture
// and the no-fallback guarantees of other modes are untouched — this only
// activates for the new burn demand-source on the Khala model).
//
// Round-robin (rotate the plan) rather than random so a bounded number of
// concurrent requests provably touches every lane, and so tests are
// deterministic given the injected index sequence.

// Demand-source tokens (lowercased `x-openagents-demand-source` values) that
// opt an `internal_stress` Khala request into multi-lane fan-out. Both spellings
// are accepted so callers can label the lane by intent (a generic multi-lane
// burn, or a continual-learning saturation run) without a code change.
export const MULTILANE_BURN_DEMAND_SOURCES: ReadonlyArray<string> = [
  'multilane-burn',
  'cl-saturation',
]

export const isMultiLaneBurnDemandSource = (
  demandSource: string | undefined,
): boolean =>
  demandSource !== undefined &&
  MULTILANE_BURN_DEMAND_SOURCES.includes(demandSource.trim().toLowerCase())

// Rotate an ordered adapter plan left by `rotationIndex` positions. Pure +
// deterministic: a non-finite / fractional / negative index is normalized into
// `[0, length)`. The rotated list is a permutation of the input — no lane is
// dropped or added — so the chosen primary still overflows down the full
// remaining chain. A 0/1-element plan is returned unchanged.
export const rotateAdapterPlan = (
  adapterIds: ReadonlyArray<string>,
  rotationIndex: number,
): ReadonlyArray<string> => {
  const length = adapterIds.length
  if (length <= 1) {
    return adapterIds
  }
  const safeIndex = Number.isFinite(rotationIndex)
    ? Math.trunc(rotationIndex)
    : 0
  const offset = ((safeIndex % length) + length) % length
  if (offset === 0) {
    return adapterIds
  }
  return [...adapterIds.slice(offset), ...adapterIds.slice(0, offset)]
}

// Resolve the MULTI-LANE FAN-OUT plan for a Khala burn request. Only the public
// Khala alias fans out; any other model id keeps its base plan unchanged. The
// caller passes the already health/registration-eligible base plan (so rotation
// spreads only across lanes that can actually serve) plus the per-request
// round-robin index. Deterministic + pure; the caller gates this on the
// internal burn demand-source so normal conversational, tool, and external Khala
// routing is never affected.
export const selectAdapterPlanForKhalaMultiLaneBurnRequest = (
  model: string,
  basePlan: ReadonlyArray<string>,
  rotationIndex: number,
): ReadonlyArray<string> =>
  normalizeKhalaModelId(model) === KHALA_MODEL_ID
    ? rotateAdapterPlan(basePlan, rotationIndex)
    : basePlan

// Resolve a requested model to its ORDERED list of candidate adapter ids
// (cheapest viable first, then overflow fallbacks). Deterministic + pure.
export const selectAdapterPlan = (model: string): ReadonlyArray<string> => {
  const normalizedModel = normalizeKhalaModelId(model)
  if (normalizedModel === KHALA_MODEL_ID) {
    // Conversational Khala is latency-first: start with fast Vertex Gemini,
    // then Fireworks, then the owned GLM lane once warm, with OpenRouter free as
    // the final hidden overflow. Tool/agentic Khala requests use
    // `selectAdapterPlanForKhalaToolRequest`, which flips the plan to
    // self-hosted GLM-first.
    // Explicit raw Hydralisk model ids below keep NO Gemini/Fireworks fallback —
    // they are deliberate supply-lane requests, not the generic Khala lane.
    return KHALA_CONVERSATIONAL_ADAPTER_PLAN
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

export type DispatchSchedulerPreemptionEvidence = Readonly<{
  evidenceRef: string
  reason: string
  targetDemandClass: 'internal_stress'
  targetOutcome: 'preempted_yielded'
}>

export type DispatchSchedulerPreemptionPolicy = Readonly<{
  demandClass: InferenceDemandClass
  reservedExternalHeadroomAvailable: boolean
  reason: string
  preempt: () => Effect.Effect<
    DispatchSchedulerPreemptionEvidence | undefined
  >
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

export type GlmOwnCapacityFailoverEvent = Readonly<{
  adapterId: string
  consecutiveFailures: number
  message: string
  reason: 'glm_recovered' | 'reserved_headroom_unavailable'
  threshold: number
  type: 'activated' | 'cleared'
}>

export type DispatchGlmOwnCapacityFailover = Readonly<{
  adapterId: string
  isActive: () => boolean
  isRecovered: () => boolean
  recordFailure: (error: InferenceAdapterError) => void
  recordSuccess: (adapterId: string) => void
}>

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
  // Optional last-mile request mapper. Use this when a virtual public model
  // routes across heterogeneous provider lanes whose native model ids differ.
  requestForAdapter?:
    | ((
        request: InferenceRequest,
        adapterId: string,
      ) => InferenceRequest)
    | undefined
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
  // Optional typed route admission metadata. It is carried for observability and
  // external preemption decisions, but dispatch does not hard-reject
  // internal_stress from a coarse reservation snapshot. The GLM adapter remains
  // the authority on real pool capacity and can still return typed saturation.
  admission?: DispatchRouteAdmissionPolicy | undefined
  // Optional typed scheduler hook. It can preempt one in-flight internal_stress
  // request only when external demand arrives and reserved external headroom is
  // unavailable. The hook is inert for every non-external demand class.
  preemption?: DispatchSchedulerPreemptionPolicy | undefined
  // Optional bounded hedge. When enabled for external demand, a quarantined or
  // P99-breaching primary can be skipped once to a different warm eligible lane.
  hedging?: DispatchHedgingPolicy | undefined
  // Optional public-safe failure telemetry. The event shape carries counts and
  // neutral classifiers only: never prompts, completions, URLs, IPs, or tokens.
  failureTelemetry?: DispatchFailureTelemetry | undefined
  // Optional GLM own-capacity failover breaker. When the self-hosted GLM lane
  // returns repeated public-safe no-headroom saturation signals, external demand
  // skips that lane until the control-plane recovery predicate clears the
  // breaker.
  glmOwnCapacityFailover?: DispatchGlmOwnCapacityFailover | undefined
}>

export type DispatchRouteMetadata = Readonly<{
  primaryAdapterId: string
  servedAdapterId: string
  fallbackReason: string | null
  laneHealth?: ProviderLaneHealth | undefined
  providerHealthScore?: number | undefined
  region?: string | undefined
  fallbackAdapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined
  schedulerPreemption?: DispatchSchedulerPreemptionEvidence | undefined
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
    ...(shedding.demandClass === 'internal_stress'
      ? { httpStatus: 429, kind: 'internal_stress_yielded' }
      : { kind: `slo_shed_${shedding.demandClass}` }),
    reason:
      shedding.demandClass === 'internal_stress'
        ? `internal_stress yielded because external SLO is breached: ${shedding.slo.reason}`
        : `request shed because SLO is breached: ${shedding.slo.reason}`,
    retryable: true,
  })

const internalStressPreemptedError = (
  request: InferenceRequest,
): InferenceAdapterError => {
  const reason =
    typeof request.abortSignal?.reason === 'string' &&
    request.abortSignal.reason.trim() !== ''
      ? request.abortSignal.reason.trim()
      : 'external_preemption'
  return new InferenceAdapterError({
    adapterId: 'router',
    httpStatus: 429,
    kind: 'internal_stress_yielded',
    reason: `internal_stress yielded because external demand preempted it: ${reason}`,
    retryable: true,
  })
}

const wasInternalStressPreempted = (request: InferenceRequest): boolean =>
  request.priority === 'internal_stress' && request.abortSignal?.aborted === true

const shouldPreemptInternalStress = (
  preemption: DispatchSchedulerPreemptionPolicy | undefined,
): boolean =>
  preemption !== undefined &&
  preemption.demandClass === 'external' &&
  !preemption.reservedExternalHeadroomAvailable

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

const GLM_OWN_CAPACITY_DOWN_ALERT =
  'GLM own-capacity down — failover active'

const GLM_ROUTE_HEADROOM_FAILURE_KINDS = new Set([
  'route_admission_reserved_headroom_unavailable',
  'glm_reserved_external_headroom_unavailable',
])

const GLM_POOL_HEADROOM_FAILURE_REASONS = new Set([
  'glm_aggregate_external_headroom_zero',
  'glm_reserved_external_headroom_unavailable',
  'reserved_headroom_unavailable',
])

const isGlmOwnCapacityUnavailableFailure = (
  error: InferenceAdapterError,
): boolean => {
  if (GLM_ROUTE_HEADROOM_FAILURE_KINDS.has(error.kind ?? '')) {
    return true
  }
  if (error.kind !== 'glm_pool_saturated') {
    return false
  }
  const metadata = error.adapterRouteMetadata
  if (metadata?.glmAggregateExternalHeadroom === 0) {
    return true
  }
  const replicaBusyReason = metadata?.replicaBusyReason
  return (
    typeof replicaBusyReason === 'string' &&
    GLM_POOL_HEADROOM_FAILURE_REASONS.has(replicaBusyReason)
  )
}

export const makeGlmOwnCapacityFailover = (
  input: Readonly<{
    adapterId?: string | undefined
    failureThreshold?: number | undefined
    isRecovered?: (() => boolean) | undefined
    onAlert?: ((event: GlmOwnCapacityFailoverEvent) => void) | undefined
  }> = {},
): DispatchGlmOwnCapacityFailover => {
  const adapterId = input.adapterId ?? HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID
  const threshold =
    input.failureThreshold === undefined ||
    !Number.isFinite(input.failureThreshold) ||
    input.failureThreshold <= 0
      ? 3
      : Math.floor(input.failureThreshold)
  let consecutiveFailures = 0
  let active = false
  const alert = (event: GlmOwnCapacityFailoverEvent): void => {
    input.onAlert?.(event)
  }
  const clear = (): void => {
    if (!active && consecutiveFailures === 0) {
      return
    }
    const wasActive = active
    active = false
    consecutiveFailures = 0
    if (!wasActive) {
      return
    }
    alert({
      adapterId,
      consecutiveFailures,
      message: 'GLM own-capacity recovered - failover cleared',
      reason: 'glm_recovered',
      threshold,
      type: 'cleared',
    })
  }
  return {
    adapterId,
    isActive: () => active,
    isRecovered: () => input.isRecovered?.() === true,
    recordFailure: error => {
      if (
        error.adapterId !== adapterId ||
        !isGlmOwnCapacityUnavailableFailure(error)
      ) {
        return
      }
      consecutiveFailures += 1
      if (!active && consecutiveFailures >= threshold) {
        active = true
        alert({
          adapterId,
          consecutiveFailures,
          message: GLM_OWN_CAPACITY_DOWN_ALERT,
          reason: 'reserved_headroom_unavailable',
          threshold,
          type: 'activated',
        })
      }
    },
    recordSuccess: servedAdapterId => {
      if (servedAdapterId === adapterId) {
        clear()
      }
    },
  }
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

const shouldRetryPrimaryValidationAfterPreemption = (
  input: Readonly<{
    adapterId: string
    error: InferenceAdapterError
    primaryAdapterId: string
    schedulerPreemption: DispatchSchedulerPreemptionEvidence | undefined
  }>,
): boolean =>
  input.schedulerPreemption !== undefined &&
  input.adapterId === input.primaryAdapterId &&
  input.error.retryable &&
  input.error.kind === 'empty_assistant_content'

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

    const shedding = deps.shedding
    if (shouldShedRequest(shedding) && shedding !== undefined) {
      const error = shedError(shedding)
      recordFailureTelemetry(
        deps.failureTelemetry,
        telemetryForError(error, 'load_shed'),
      )
      return yield* Effect.fail(error)
    }

    const schedulerPreemption = shouldPreemptInternalStress(deps.preemption)
      ? yield* deps.preemption!.preempt()
      : undefined

    const adapterIds = planFor(request.model)
    let healthQuarantinedLaneCount = 0
    const glmOwnCapacityFailover = deps.glmOwnCapacityFailover
    const recoveredGlmOwnCapacity =
      glmOwnCapacityFailover?.isActive() === true &&
      glmOwnCapacityFailover.isRecovered()
    if (recoveredGlmOwnCapacity) {
      glmOwnCapacityFailover?.recordSuccess(glmOwnCapacityFailover.adapterId)
    }
    // Resolve to the lanes that are actually registered (skip absent partners).
    const adapters = adapterIds.flatMap(id => {
      const adapter = deps.registry.resolve(id)
      if (adapter === undefined) {
        return []
      }
      if (
        id === glmOwnCapacityFailover?.adapterId &&
        glmOwnCapacityFailover.isActive()
      ) {
        recordFailureTelemetry(deps.failureTelemetry, {
          adapterId: id,
          classifier: 'fallback',
          kind: 'glm_own_capacity_failover_active',
          retryable: true,
          stage: 'fallback',
        })
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

    adapterLoop: for (
      let index = startIndex;
      index < adapters.length;
      index += 1
    ) {
      const adapter = adapters[index]!
      const adapterRequest =
        deps.requestForAdapter?.(request, adapter.id) ?? request
      // Back off before an overflow attempt (never before the first attempt).
      if (index > startIndex) {
        yield* sleep(backoffDelayMs(backoff, overflowCount))
        overflowCount += 1
      }

      let validationRetryAttempt = 0
      // #6318: after external demand preempts internal stress, an empty primary
      // assistant response gets one same-lane validation retry before overflow.
      const validationRetryLimit =
        schedulerPreemption === undefined ? retryCount : Math.max(1, retryCount)

      while (true) {
        const outcome = yield* attemptAdapterWithRetry({
          adapter,
          backoff,
          failureTelemetry: deps.failureTelemetry,
          operation,
          request: adapterRequest,
          retryCount,
          sleep,
        })

        if (outcome.ok) {
          const validation =
            validateSuccess?.({
              adapter,
              request: adapterRequest,
              value: outcome.value,
            }) ??
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
            if (
              validationRetryAttempt < validationRetryLimit &&
              shouldRetryPrimaryValidationAfterPreemption({
                adapterId: adapter.id,
                error: validation.error,
                primaryAdapterId,
                schedulerPreemption,
              })
            ) {
              yield* sleep(backoffDelayMs(backoff, validationRetryAttempt))
              validationRetryAttempt += 1
              continue
            }
            fallbackReason = fallbackReasonFor(validation.error)
            fallbackAdapterRouteMetadata = validation.error.adapterRouteMetadata
            continue adapterLoop
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
          deps.glmOwnCapacityFailover?.recordSuccess(adapter.id)
          return {
            route: {
              fallbackReason,
              ...(fallbackAdapterRouteMetadata === undefined
                ? {}
                : { fallbackAdapterRouteMetadata }),
              ...(schedulerPreemption === undefined
                ? {}
                : { schedulerPreemption }),
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
        deps.glmOwnCapacityFailover?.recordFailure(outcome.error)
        if (wasInternalStressPreempted(adapterRequest)) {
          return yield* Effect.fail(internalStressPreemptedError(adapterRequest))
        }
        // Non-retryable: surface immediately, no overflow.
        if (!outcome.error.retryable) {
          return yield* Effect.fail(outcome.error)
        }
        fallbackReason = fallbackReasonFor(outcome.error)
        fallbackAdapterRouteMetadata = outcome.error.adapterRouteMetadata
        // Retryable: continue to the next viable lane (loop), backing off above.
        continue adapterLoop
      }
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
