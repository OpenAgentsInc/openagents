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
  blendedCostPerMtok,
  lookupModel,
  MODEL_PRICING_TABLE,
  type SupplyLane,
} from './pricing'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceProviderRegistry,
  type InferenceRequest,
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
  const id = model.trim().toLowerCase().replace(/^(?:vertex|anthropic)\//u, '')
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
  const id = model.trim().toLowerCase().replace(/^fireworks\//u, '')
  // A model priced on the Fireworks lane is open by definition.
  if (lookupModel(id)?.lane === 'fireworks') {
    return true
  }
  return OPEN_MODEL_PREFIXES.some(prefix => id.startsWith(prefix))
}

export const classifyModel = (model: string): ModelClass => {
  if (isClaudeModel(model)) {
    return 'claude'
  }
  if (isGeminiModel(model)) {
    return 'gemini'
  }
  if (isOpenModel(model)) {
    return 'open'
  }
  const priced = lookupModel(model)
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
export const openModelsByCost: ReadonlyArray<string> = MODEL_PRICING_TABLE.filter(
  entry => entry.lane === 'fireworks',
)
  .map(entry => ({ cost: blendedCostPerMtok(entry.cost), model: entry.model }))
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

// Resolve a requested model to its ORDERED list of candidate adapter ids
// (cheapest viable first, then overflow fallbacks). Deterministic + pure.
export const selectAdapterPlan = (model: string): ReadonlyArray<string> => {
  const plan = LANE_PLAN_BY_CLASS[classifyModel(model)]
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

export type DispatchDeps = Readonly<{
  registry: InferenceProviderRegistry
  // Defaults to the pure selector; injectable for tests.
  plan?: ((model: string) => ReadonlyArray<string>) | undefined
  backoff?: OverflowBackoff | undefined
  // Injected delay (defaults to Effect.sleep). Tests pass `() => Effect.void`.
  sleep?: ((ms: number) => Effect.Effect<void>) | undefined
}>

// Run an adapter operation across the model's lane plan with bounded-backoff
// overflow. Lanes that are not registered (e.g. an absent partner secret) are
// skipped. A retryable failure backs off and overflows to the next lane; a
// non-retryable failure surfaces immediately. When every viable lane fails (or
// none is configured) the last/first failure surfaces as a typed error.
export const dispatchWithOverflow = <A>(
  request: InferenceRequest,
  operation: AdapterOperation<A>,
  deps: DispatchDeps,
): Effect.Effect<A, InferenceAdapterError> =>
  Effect.gen(function* () {
    const planFor = deps.plan ?? selectAdapterPlan
    const backoff = deps.backoff ?? DEFAULT_OVERFLOW_BACKOFF
    const sleep = deps.sleep ?? Effect.sleep

    const adapterIds = planFor(request.model)
    // Resolve to the lanes that are actually registered (skip absent partners).
    const adapters = adapterIds
      .map(id => deps.registry.resolve(id))
      .filter((a): a is InferenceProviderAdapter => a !== undefined)

    if (adapters.length === 0) {
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

    for (let index = 0; index < adapters.length; index += 1) {
      const adapter = adapters[index]!
      // Back off before an overflow attempt (never before the first attempt).
      if (index > 0) {
        yield* sleep(backoffDelayMs(backoff, overflowCount))
        overflowCount += 1
      }

      const outcome = yield* operation(adapter, request).pipe(
        Effect.map(value => ({ ok: true as const, value })),
        Effect.catch(error => Effect.succeed({ error, ok: false as const })),
      )

      if (outcome.ok) {
        return outcome.value
      }

      lastError = outcome.error
      // Non-retryable: surface immediately, no overflow.
      if (!outcome.error.retryable) {
        return yield* Effect.fail(outcome.error)
      }
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
