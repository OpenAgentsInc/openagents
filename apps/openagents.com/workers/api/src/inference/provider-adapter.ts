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
  // OpenAI-compatible tool-call replay metadata. Tool-using clients (OpenCode,
  // AI SDK, etc.) send prior assistant tool calls and tool results back through
  // the next request; adapters that speak OpenAI-compatible chat must preserve
  // these fields so the provider can associate each tool result with the call
  // that requested it.
  name?: string | undefined
  toolCallId?: string | undefined
  toolCalls?: ReadonlyArray<InferenceToolCall> | undefined
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

// Public-safe routing metadata a provider adapter may know only after its own
// internal selection runs. The lane router knows "Hydralisk GLM pool"; the pool
// adapter knows which replica actually served. All fields must be refs, coarse
// measurements, or neutral reason strings: never raw URLs, private IPs, tokens,
// prompts, or responses.
export type InferenceAdapterRouteMetadata = Readonly<{
  selectedReplicaId?: string | undefined
  selectedReplicaRef?: string | undefined
  replicaFallbackReason?: string | null | undefined
  replicaHealthScore?: number | undefined
  replicaRegion?: string | undefined
  replicaBusyReason?: string | null | undefined
  queueWaitMs?: number | undefined
  glmSaturationPolicy?: string | undefined
  replicaCapacityClass?: string | undefined
  replicaCostProfileRef?: string | undefined
  replicaInflightCount?: number | undefined
  replicaMaxInflight?: number | undefined
  replicaQueueDepth?: number | undefined
  replicaWarmState?: 'cold' | 'unknown' | 'warm' | undefined
  glmAggregateInflightCount?: number | undefined
  glmAggregateMaxInflight?: number | undefined
  glmAggregateExternalHeadroom?: number | undefined
}>

export type InferenceToolCall = Readonly<{
  id: string
  type: 'function'
  function: Readonly<{
    name: string
    arguments: string
  }>
}>

export type InferenceToolCallDelta = Readonly<{
  index: number
  id?: string | undefined
  type?: 'function' | undefined
  function?:
    | Readonly<{
        name?: string | undefined
        arguments?: string | undefined
      }>
    | undefined
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
  // OpenAI-compatible assistant tool calls, present when `finishReason` is
  // `tool_calls` or a provider returns an assistant message that requests tools.
  toolCalls?: ReadonlyArray<InferenceToolCall> | undefined
  // Optional public-safe metadata about adapter-internal route selection. The
  // route folds this into the OpenAgents receipt block when present.
  adapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined
}>

// One normalized SSE frame as it is parsed off the upstream byte stream, plus
// the running terminal state (finishReason / usage / servedModel). This is what
// a TRUE pass-through stream yields incrementally: every content delta as it
// arrives (so the edge idle-timer resets and a multi-minute generation never
// 524s), and the terminal usage frame when the upstream emits it. The route
// re-frames these into OpenAI-compatible `data:` SSE and settles metering from
// the terminal usage frame once the upstream stream closes (receipt-first).
export type InferenceStreamEvent = Readonly<{
  // Incremental content for this frame (may be empty on the terminal frame).
  contentDelta: string
  // Incremental provider-labeled reasoning/thinking for this frame. This is
  // deliberately separate from content so clients can render or hide reasoning
  // differently and never have to infer the channel from prose.
  reasoningDelta?: string | undefined
  // Incremental OpenAI-compatible tool-call deltas. Arguments may arrive in many
  // partial frames; the route must forward them as-is instead of trying to
  // assemble or validate the provider's JSON argument stream.
  toolCallDeltas?: ReadonlyArray<InferenceToolCallDelta> | undefined
  // Set when the upstream reports a finish reason (terminal frame).
  finishReason?: string | undefined
  // Set when the upstream emits a usage frame (terminal, receipt-first).
  usage?: InferenceUsage | undefined
  // Provider-native model id when the upstream reports one.
  servedModel?: string | undefined
}>

// A true incremental stream of normalized SSE events. Unlike `stream` (which
// returns a fully-materialized array — convenient for tests/metering but it
// buffers the WHOLE upstream completion before the first byte reaches the
// client, which is exactly what trips the Cloudflare edge ~100s idle timeout on
// a long generation), `streamSse` hands back a lazily-consumed source that emits
// each frame as the upstream produces it. The route pumps these straight to the
// client so bytes flow continuously and a 3-minute generation never 524s.
//
// `terminal()` resolves AFTER the source is fully drained, with the captured
// terminal state (finishReason / usage / servedModel) so the route can settle
// metering receipt-first and attach the `openagents` disclosure block — without
// re-buffering the content.
export type InferenceStreamSource = Readonly<{
  // Async-iterable of normalized frames, consumed once.
  frames: AsyncIterable<InferenceStreamEvent>
  // Resolves once `frames` is exhausted, with the terminal metering state.
  terminal: () => Readonly<{
    finishReason: string | undefined
    usage: InferenceUsage | undefined
    servedModel: string | undefined
    adapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined
  }>
}>

// A single streamed delta. The route serializes these into OpenAI-compatible
// `data:` SSE frames. The final chunk of a stream carries `usage` so the
// metering hook can settle from real counts.
export type InferenceStreamChunk = Readonly<{
  // Incremental content delta for this frame (may be empty on the final frame).
  contentDelta: string
  // Incremental provider-labeled reasoning/thinking for this frame.
  reasoningDelta?: string | undefined
  // Incremental OpenAI-compatible tool-call deltas.
  toolCallDeltas?: ReadonlyArray<InferenceToolCallDelta> | undefined
  // Set on the terminal frame only.
  finishReason?: string | undefined
  // Set on the terminal frame only (receipt-first usage).
  usage?: InferenceUsage | undefined
  // Provider-native model id actually served. Set on the terminal frame when
  // the adapter can resolve it; the route falls back to the requested id.
  servedModel?: string | undefined
  // Optional public-safe metadata about adapter-internal route selection. Set on
  // the terminal chunk when the adapter can disclose it.
  adapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined
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
  // Optional public-safe routing metadata about the failed adapter attempt. Used
  // when a lane refuses work before serving (for example GLM saturation) so the
  // overflow receipt can still expose queue wait and busy reason without leaking
  // private endpoint details.
  readonly adapterRouteMetadata: InferenceAdapterRouteMetadata | undefined

  constructor(
    input: Readonly<{
      adapterId: string
      reason: string
      retryable?: boolean | undefined
      httpStatus?: number | undefined
      kind?: string | undefined
      adapterRouteMetadata?: InferenceAdapterRouteMetadata | undefined
    }>,
  ) {
    super(`[${input.adapterId}] ${input.reason}`)
    this.name = 'InferenceAdapterError'
    this.adapterId = input.adapterId
    this.reason = input.reason
    this.retryable = input.retryable ?? false
    this.httpStatus = input.httpStatus
    this.kind = input.kind
    this.adapterRouteMetadata = input.adapterRouteMetadata
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
  // carries `finishReason` + `usage`. NOTE: this returns a fully-materialized
  // array, so it BUFFERS the whole upstream completion before any byte reaches
  // the client. It stays for tests, metering reconstruction, and the overflow
  // dispatcher; the route prefers `streamSse` (below) for the live hot path so a
  // long generation streams through and never trips the edge idle timeout.
  stream: (
    request: InferenceRequest,
  ) => Effect.Effect<ReadonlyArray<InferenceStreamChunk>, InferenceAdapterError>
  // OPTIONAL true incremental pass-through stream. When an adapter implements
  // this, the route pumps the upstream SSE to the client frame-by-frame (no
  // server-side buffering of the whole stream), so the edge idle-timer resets on
  // every chunk and long generations never 524. Adapters that cannot stream
  // incrementally (stub/echo, simple test adapters) omit it and the route falls
  // back to the buffered `stream` path.
  streamSse?: (
    request: InferenceRequest,
  ) => Effect.Effect<InferenceStreamSource, InferenceAdapterError>
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
