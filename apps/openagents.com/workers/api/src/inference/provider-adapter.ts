// Provider-adapter seam for the OpenAgents inference gateway (EPIC #5474, #5476).
//
// This is the typed interface + registry that the rest of the inference build
// plugs into. Each provider lane registers exactly ONE adapter by id:
//   - #5479 Fireworks (open models)        -> registerProviderAdapter(fireworksAdapter)
//   - #5480 Vertex Anthropic (Claude)      -> registerProviderAdapter(vertexAdapter)
//   - #5481 partner passthrough            -> registerProviderAdapter(passthroughAdapter)
// Routing (#5482) decides WHICH adapter id handles a given model; this module
// owns only the contract + lookup. Adapters never touch credits, payment, or
// public projection — they translate a normalized request into provider tokens
// and return a normalized result + the provider `usage` object (receipt-first).
//
// The shape is intentionally provider-neutral and OpenAI-Chat-Completions-like
// so a real client works by changing only base URL + key. Anthropic Messages is
// a parallel surface (#5476 leaves a clean spot); both normalize into the same
// adapter request/result here.

import { Effect } from 'effect'

// A single chat message in the normalized request. `role`/`content` mirror the
// OpenAI Chat Completions shape so adapters can pass through with minimal
// translation.
export type InferenceMessage = Readonly<{
  role: string
  content: string
}>

// Normalized inference request handed to an adapter. Adapter implementations
// translate this into their provider's wire format. `passthroughParams` carries
// standard sampling params (temperature, top_p, max_tokens, ...) that the route
// forwards verbatim; adapters apply only the ones their provider supports.
export type InferenceRequest = Readonly<{
  model: string
  messages: ReadonlyArray<InferenceMessage>
  stream: boolean
  passthroughParams: Readonly<Record<string, unknown>>
}>

// Provider `usage` object — the receipt-first source of truth for metering.
// Adapters MUST populate this from the provider response, never an estimate
// (INVARIANTS.md "Canonical Token Usage Ledger" + the gateway business doc §4).
export type InferenceUsage = Readonly<{
  promptTokens: number
  completionTokens: number
  totalTokens: number
  // Optional cached-input dimension (e.g. Fireworks prompt-cache hits, billed
  // ~50% of input). Present when the provider reports it; left undefined
  // otherwise so the metering hook can decide how to price the difference.
  cachedPromptTokens?: number | undefined
}>

// Non-streaming adapter result.
export type InferenceResult = Readonly<{
  // The assistant message content for the (single) completion choice.
  content: string
  // Provider-reported finish reason ("stop", "length", ...).
  finishReason: string
  usage: InferenceUsage
  // Provider-native model id actually served (may differ from the requested
  // alias once routing/aliasing lands).
  servedModel: string
}>

// A single streamed delta. The route serializes these into OpenAI-compatible
// `data:` SSE frames. The final chunk of a stream carries `usage` so the
// metering hook can settle from real counts.
export type InferenceStreamChunk = Readonly<{
  // Incremental content delta for this frame (may be empty on the final frame).
  contentDelta: string
  // Set on the terminal frame only.
  finishReason?: string | undefined
  // Set on the terminal frame only (receipt-first usage).
  usage?: InferenceUsage | undefined
  // Provider-native model id actually served. Set on the terminal frame when
  // the adapter can resolve it; the route falls back to the requested id.
  servedModel?: string | undefined
}>

// Typed adapter failure. Adapters surface provider/transport problems as this
// rather than throwing, so the route can map them to a stable JSON error.
//
// The retry classification fields (`retryable`, `httpStatus`, `kind`) are the
// typed signal routing (#5482) consumes to decide backoff + overflow to another
// supply lane. They are OPTIONAL and default to a non-retryable, unclassified
// failure so existing `{ adapterId, reason }` constructions stay valid and the
// route can keep reading `error.reason` unchanged.
export class InferenceAdapterError extends Error {
  readonly _tag = 'InferenceAdapterError'
  readonly adapterId: string
  readonly reason: string
  // Whether routing may safely retry the request or overflow it to another
  // supply lane (e.g. provider 429/503 or a transport fault). Defaults false.
  readonly retryable: boolean
  // The upstream HTTP status when the failure came from a provider response
  // (e.g. 429, 503, 500); undefined for transport/config/parse failures.
  readonly httpStatus: number | undefined
  // Stable, neutral failure classification for routing/metrics, e.g.
  // "rate_limited", "service_overloaded", "upstream_error", "transport_error",
  // "configuration_error", "malformed_response", "request_rejected".
  readonly kind: string | undefined

  constructor(
    input: Readonly<{
      adapterId: string
      reason: string
      retryable?: boolean | undefined
      httpStatus?: number | undefined
      kind?: string | undefined
    }>,
  ) {
    super(`[${input.adapterId}] ${input.reason}`)
    this.name = 'InferenceAdapterError'
    this.adapterId = input.adapterId
    this.reason = input.reason
    this.retryable = input.retryable ?? false
    this.httpStatus = input.httpStatus
    this.kind = input.kind
  }
}

// The provider-adapter contract. Phase-2 adapters implement this once each.
export type InferenceProviderAdapter = Readonly<{
  // Stable adapter id, e.g. "stub-echo", "fireworks", "vertex-anthropic",
  // "passthrough". Used by the registry and routing.
  id: string
  // Non-streaming completion.
  complete: (
    request: InferenceRequest,
  ) => Effect.Effect<InferenceResult, InferenceAdapterError>
  // Streaming completion. Implementations yield deltas; the terminal chunk
  // carries `finishReason` + `usage`.
  stream: (
    request: InferenceRequest,
  ) => Effect.Effect<ReadonlyArray<InferenceStreamChunk>, InferenceAdapterError>
}>

// Registry: maps adapter id -> adapter. The registry is the single seam the
// rest of the EPIC plugs into. Routing resolves a model to an adapter id and
// asks the registry for the adapter; this module does not decide routing.
export class InferenceProviderRegistry {
  private readonly adapters = new Map<string, InferenceProviderAdapter>()

  // Register (or replace) an adapter by its id. Each provider child issue calls
  // this exactly once at wiring time.
  register(adapter: InferenceProviderAdapter): void {
    this.adapters.set(adapter.id, adapter)
  }

  // Resolve an adapter by id. Returns undefined when no adapter is registered
  // for the id (the route maps that to a stable model_unavailable error).
  resolve(adapterId: string): InferenceProviderAdapter | undefined {
    return this.adapters.get(adapterId)
  }

  // Snapshot of registered adapter ids (stable order), for diagnostics/tests.
  ids(): ReadonlyArray<string> {
    return Array.from(this.adapters.keys()).sort()
  }
}
