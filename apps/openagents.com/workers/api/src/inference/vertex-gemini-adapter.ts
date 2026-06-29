// Vertex Gemini provider adapter for the inference gateway
// (EPIC #5474, free-tier enablement). This is the Gemini lane: it serves
// Google's own Gemini models (default: Gemini 3.5 Flash, model id
// `gemini-3.5-flash`) from our first-party Google Cloud Vertex AI quota
// (project `openagentsgemini`). Unlike the Claude lane, Gemini is Google's OWN
// model on Vertex, so there is NO Anthropic partner quota involved — it is a
// plain Vertex `publishers/google` model.
//
// It implements the InferenceProviderAdapter seam (`complete` + `stream`),
// mapping our normalized InferenceRequest onto the Vertex AI Gemini
// `generateContent` wire format and returning receipt-first `usage` from the
// provider's `usageMetadata` — never an estimate (INVARIANTS.md "Canonical
// Token Usage Ledger" + gateway business doc §4). It owns ONLY request
// translation + transport; it never touches credits, payment, routing, free
// allowance, or public projection.
//
// Wire contract (Vertex AI Gemini, verified against the Vertex Gemini REST
// reference 2026-06-19):
//   - Endpoint:
//       POST https://{host}/v1/projects/{project}/locations/{location}/
//            publishers/google/models/{modelId}:generateContent
//       (and :streamGenerateContent?alt=sse for SSE)
//     where {host} is `aiplatform.googleapis.com` for the `global` location and
//     `{location}-aiplatform.googleapis.com` for regional/multi-region ones —
//     the SAME host rule as the Vertex Anthropic lane.
//   - `model` is NOT in the body — it is the {modelId} path segment.
//   - Auth: `Authorization: Bearer <GCP access token>`. We REUSE the exact
//     SA-key->OAuth token path the Vertex Anthropic adapter built
//     (`vertex-token.ts`, Worker secret VERTEX_SA_KEY); Gemini needs the same
//     `cloud-platform` scope.
//   - Body shape: `contents: [{ role, parts: [{ text }] }]`, optional
//     `systemInstruction: { parts: [{ text }] }`, and `generationConfig`
//     ({ maxOutputTokens, temperature, topP, topK }). Gemini roles are
//     `user` / `model` (NOT `assistant`); we map our normalized roles.
//   - Response: `candidates[].content.parts[].text`, `candidates[].finishReason`,
//     and `usageMetadata.{promptTokenCount, candidatesTokenCount,
//     totalTokenCount, cachedContentTokenCount}`.
//
// AUTH NOTE: identical to the Anthropic lane — a Cloudflare Worker cannot use
// gcloud ADC, so we mint a short-lived GCP access token from VERTEX_SA_KEY via
// the shared `vertex-token.ts` path. A pre-minted token may also be supplied
// directly (for tests / a sidecar that already holds ADC).
//
// INERT by default: index.ts constructs this adapter from env and registers it
// once; with no VERTEX_SA_KEY (and no pre-minted token) the adapter is
// unconfigured and surfaces a typed error rather than calling out. The whole
// gateway route stays flag-gated off via INFERENCE_GATEWAY_ENABLED regardless.
import { Effect } from 'effect'

import { parseJsonRecord } from '../json-boundary'
import { AUTOPILOT_CONCIERGE_MODEL_ID, KHALA_MINI_MODEL_ID } from './pricing'
import {
  InferenceAdapterError,
  type InferenceMessage,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceToolCall,
  type InferenceUsage,
} from './provider-adapter'
import {
  type VertexFetch,
  type VertexTokenProvider,
} from './vertex-anthropic-adapter'

export const VERTEX_GEMINI_ADAPTER_ID = 'vertex-gemini'

// The default Gemini model id the free tier serves. The gateway's default model
// when a caller specifies none (gateway free-tier enablement §3).
export const DEFAULT_GEMINI_MODEL_ID = 'gemini-3.5-flash'

// Default Vertex location — `global` uses the bare `aiplatform.googleapis.com`
// host (no region subdomain prefix). Same default as the Anthropic lane.
const DEFAULT_VERTEX_LOCATION = 'global'

// Default max output tokens when the caller does not pass one. Gemini does not
// strictly require maxOutputTokens, but we apply a sane floor for parity with
// the Anthropic lane and to bound a runaway generation.
const DEFAULT_MAX_TOKENS = 1024

export type VertexGeminiAdapterConfig = Readonly<{
  // GCP project id that holds the Vertex Gemini quota (e.g. "openagentsgemini").
  project: string
  // Vertex location ("global" | "us" | "eu" | a region like "us-east1").
  location?: string | undefined
  // Mints a GCP access token (SHARED with the Anthropic lane: same SA-key path).
  // When undefined the adapter is unconfigured and every call fails with a
  // typed, non-retryable error.
  tokenProvider?: VertexTokenProvider | undefined
  // Maps a requested model alias to the Vertex-native Gemini model id. Defaults
  // to stripping a leading `vertex/` / `google/` / `gemini/` provider hint and
  // mapping the bare alias `gemini` -> DEFAULT_GEMINI_MODEL_ID. Routing owns
  // alias policy; this is only a last-mile id normalizer.
  resolveModelId?: ((requestedModel: string) => string) | undefined
  // Injected for tests; defaults to global fetch.
  fetchImpl?: VertexFetch | undefined
}>

// Known Gemini model ids served from this Vertex lane. Exported as the
// last-mile reference of the served lane; the router decides WHICH alias maps
// here.
export const KNOWN_VERTEX_GEMINI_MODEL_IDS: ReadonlyArray<string> = [
  'gemini-3.5-flash',
  'gemini-3.5-pro',
  'gemini-2.5-flash',
  'gemini-2.5-pro',
]

const defaultResolveModelId = (requestedModel: string): string => {
  const requested = requestedModel.trim()
  if (
    requested.toLowerCase() === KHALA_MINI_MODEL_ID ||
    requested.toLowerCase() === AUTOPILOT_CONCIERGE_MODEL_ID
  ) {
    return DEFAULT_GEMINI_MODEL_ID
  }
  // Strip a leading provider hint (`vertex/`, `google/`, `gemini/`) if present.
  const stripped = requested.replace(/^(?:vertex|google|gemini)\//u, '')
  // A bare `gemini` alias maps to the default Flash model; an empty id likewise.
  if (stripped === '' || stripped.toLowerCase() === 'gemini') {
    return DEFAULT_GEMINI_MODEL_ID
  }
  return stripped
}

const vertexHost = (location: string): string =>
  location === 'global'
    ? 'aiplatform.googleapis.com'
    : `${location}-aiplatform.googleapis.com`

const vertexUrl = (
  config: Readonly<{ project: string; location: string }>,
  modelId: string,
  method: 'generateContent' | 'streamGenerateContent',
): string => {
  const base =
    `https://${vertexHost(config.location)}/v1/projects/${config.project}` +
    `/locations/${config.location}/publishers/google/models/${modelId}:${method}`
  // Streaming uses SSE framing so we can split on `data:` lines like the
  // Anthropic lane; without `?alt=sse` Vertex returns a single JSON array.
  return method === 'streamGenerateContent' ? `${base}?alt=sse` : base
}

// Map our normalized message role onto a Gemini `contents[].role`. Gemini uses
// `user` and `model`; `assistant` maps to `model`, `system` is hoisted into the
// top-level `systemInstruction` (handled in buildGeminiBody), and any other
// role defaults to `user` (a safe, non-throwing default).
const toGeminiRole = (role: string): 'user' | 'model' =>
  role === 'assistant' || role === 'model' ? 'model' : 'user'

// Generation-config sampling params Gemini accepts that the route may forward
// verbatim via passthroughParams. We copy only recognized, type-checked fields
// (Gemini names them differently from OpenAI/Anthropic, so we translate).
const buildGenerationConfig = (
  request: InferenceRequest,
): Record<string, unknown> => {
  const passthrough = request.passthroughParams
  const config: Record<string, unknown> = {}

  const rawMax = passthrough['max_tokens']
  config['maxOutputTokens'] =
    typeof rawMax === 'number' && Number.isInteger(rawMax) && rawMax > 0
      ? rawMax
      : DEFAULT_MAX_TOKENS

  const rawThinkingBudget =
    passthrough['thinking_budget'] ?? passthrough['thinkingBudget']
  if (
    typeof rawThinkingBudget === 'number' &&
    Number.isInteger(rawThinkingBudget) &&
    rawThinkingBudget >= 0
  ) {
    config['thinkingConfig'] = { thinkingBudget: rawThinkingBudget }
  }

  const temperature = passthrough['temperature']
  if (typeof temperature === 'number') {
    config['temperature'] = temperature
  }
  const topP = passthrough['top_p']
  if (typeof topP === 'number') {
    config['topP'] = topP
  }
  const topK = passthrough['top_k']
  if (typeof topK === 'number') {
    config['topK'] = topK
  }
  return config
}

// ---------------------------------------------------------------------------
// Function calling (#6364): translate OpenAI-style `tools` / `tool_choice` and
// the tool-call/tool-result message history into Gemini's wire format, and
// parse Gemini `functionCall` parts back into OpenAI-compatible `toolCalls`.
// This is what lets the Khala operator tool-calling loop fire when the Gemini
// lane serves it.
// ---------------------------------------------------------------------------

// Gemini function-declaration parameter schemas reject some JSON-Schema keywords
// our tool definitions carry (e.g. `additionalProperties`, `$schema`). Strip
// them recursively so a standard OpenAI `parameters` schema is accepted.
const sanitizeGeminiSchema = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map(sanitizeGeminiSchema)
  }
  if (typeof value === 'object' && value !== null) {
    const out: Record<string, unknown> = {}
    for (const [key, child] of Object.entries(value)) {
      if (key === 'additionalProperties' || key === '$schema') {
        continue
      }
      out[key] = sanitizeGeminiSchema(child)
    }
    return out
  }
  return value
}

type OpenAiToolDefinition = Readonly<{
  type?: unknown
  function?:
    | Readonly<{ name?: unknown; description?: unknown; parameters?: unknown }>
    | undefined
}>

// Build Gemini `tools` (functionDeclarations) + `toolConfig` from the OpenAI
// `tools` / `tool_choice` passthrough params. Returns undefined when no usable
// function tools were supplied.
const buildGeminiTools = (
  passthrough: Readonly<Record<string, unknown>>,
):
  | Readonly<{ tools: ReadonlyArray<unknown>; toolConfig?: unknown }>
  | undefined => {
  const rawTools = passthrough['tools']
  if (!Array.isArray(rawTools) || rawTools.length === 0) {
    return undefined
  }
  const declarations = rawTools
    .map((tool): Record<string, unknown> | null => {
      const fn = (tool as OpenAiToolDefinition).function
      const name = typeof fn?.name === 'string' ? fn.name : null
      if (name === null) return null
      const declaration: Record<string, unknown> = { name }
      if (typeof fn?.description === 'string') {
        declaration['description'] = fn.description
      }
      if (
        typeof fn?.parameters === 'object' &&
        fn.parameters !== null
      ) {
        declaration['parameters'] = sanitizeGeminiSchema(fn.parameters)
      }
      return declaration
    })
    .filter((value): value is Record<string, unknown> => value !== null)

  if (declarations.length === 0) {
    return undefined
  }

  // Map OpenAI tool_choice -> Gemini functionCallingConfig.mode.
  const choice = passthrough['tool_choice']
  let mode: 'AUTO' | 'ANY' | 'NONE' = 'AUTO'
  if (choice === 'none') {
    mode = 'NONE'
  } else if (choice === 'required' || choice === 'any') {
    mode = 'ANY'
  }

  return {
    toolConfig: { functionCallingConfig: { mode } },
    tools: [{ functionDeclarations: declarations }],
  }
}

// Translate one normalized message into the Gemini content parts that represent
// it. Assistant tool-call messages become `functionCall` parts (role model);
// `tool` result messages become `functionResponse` parts (role user). Plain
// messages become a single text part.
const geminiContentForMessage = (
  message: InferenceMessage,
): Record<string, unknown> | null => {
  if (message.role === 'tool') {
    // A tool result. Gemini wants a functionResponse whose `name` matches the
    // earlier functionCall, with an object `response`.
    return {
      parts: [
        {
          functionResponse: {
            name: message.name ?? 'tool',
            response: { content: message.content },
          },
        },
      ],
      role: 'user',
    }
  }

  if (
    message.role === 'assistant' &&
    message.toolCalls !== undefined &&
    message.toolCalls.length > 0
  ) {
    const parts: Array<Record<string, unknown>> = []
    if (message.content !== '') {
      parts.push({ text: message.content })
    }
    for (const toolCall of message.toolCalls) {
      const parsed = parseJsonRecord(toolCall.function.arguments)
      parts.push({
        functionCall: {
          args: parsed ?? {},
          name: toolCall.function.name,
        },
        // Replay Gemini's thoughtSignature on the PART so the follow-up turn is
        // accepted (Gemini 3 requirement; see extractToolCalls).
        ...(toolCall.thoughtSignature === undefined
          ? {}
          : { thoughtSignature: toolCall.thoughtSignature }),
      })
    }
    return { parts, role: 'model' }
  }

  return {
    parts: [{ text: message.content }],
    role: toGeminiRole(message.role),
  }
}

const buildGeminiBody = (
  request: InferenceRequest,
): Record<string, unknown> => {
  // System messages (from a `system` role or the `system` passthrough param)
  // are hoisted into Gemini's top-level systemInstruction; everything else maps
  // into `contents`.
  const systemTexts: Array<string> = []
  const contents: Array<Record<string, unknown>> = []
  for (const message of request.messages) {
    if (message.role === 'system') {
      if (message.content !== '') {
        systemTexts.push(message.content)
      }
      continue
    }
    const content = geminiContentForMessage(message)
    if (content !== null) {
      contents.push(content)
    }
  }

  const passthroughSystem = request.passthroughParams['system']
  if (typeof passthroughSystem === 'string' && passthroughSystem !== '') {
    systemTexts.unshift(passthroughSystem)
  }

  const body: Record<string, unknown> = {
    contents,
    generationConfig: buildGenerationConfig(request),
  }
  if (systemTexts.length > 0) {
    body['systemInstruction'] = {
      parts: systemTexts.map(text => ({ text })),
    }
  }

  const toolingConfig = buildGeminiTools(request.passthroughParams)
  if (toolingConfig !== undefined) {
    body['tools'] = toolingConfig.tools
    if (toolingConfig.toolConfig !== undefined) {
      body['toolConfig'] = toolingConfig.toolConfig
    }
  }

  return body
}

// Parse the receipt-first usage object from a Vertex Gemini response. Maps
// Gemini `usageMetadata` field names onto our InferenceUsage. Missing fields
// default to 0 so a malformed-but-2xx response still yields a stable shape; the
// caller (metering) treats these as authoritative provider counts.
//
// Gemini's totalTokenCount can include tool-use + thoughts tokens beyond
// prompt+candidates, so we trust the provider's totalTokenCount when present
// rather than recomputing (prompt + candidates would under-count thinking
// tokens we are still billed for).
const parseUsage = (raw: unknown): InferenceUsage => {
  const usage =
    typeof raw === 'object' && raw !== null
      ? (raw as Record<string, unknown>)
      : {}
  const num = (value: unknown): number =>
    typeof value === 'number' && Number.isFinite(value) ? value : 0
  const promptTokens = num(usage['promptTokenCount'])
  const completionTokens = num(usage['candidatesTokenCount'])
  const reportedTotal = num(usage['totalTokenCount'])
  const totalTokens =
    reportedTotal > 0 ? reportedTotal : promptTokens + completionTokens
  const cached = usage['cachedContentTokenCount']
  return {
    completionTokens,
    promptTokens,
    totalTokens,
    ...(typeof cached === 'number' && Number.isFinite(cached) && cached > 0
      ? { cachedPromptTokens: cached }
      : {}),
  }
}

// Concatenate the text from a Gemini candidate's content.parts (text parts
// only). Reads the FIRST candidate (we request a single completion).
const extractText = (candidates: unknown): string => {
  if (!Array.isArray(candidates) || candidates.length === 0) {
    return ''
  }
  const first = candidates[0]
  if (typeof first !== 'object' || first === null) {
    return ''
  }
  const content = (first as Record<string, unknown>)['content']
  if (typeof content !== 'object' || content === null) {
    return ''
  }
  const parts = (content as Record<string, unknown>)['parts']
  if (!Array.isArray(parts)) {
    return ''
  }
  return parts
    .map(part => {
      if (typeof part === 'object' && part !== null) {
        const text = (part as Record<string, unknown>)['text']
        return typeof text === 'string' ? text : ''
      }
      return ''
    })
    .join('')
}

// Extract OpenAI-compatible tool calls from a Gemini candidate's
// `content.parts[].functionCall` (#6364). Gemini does not return a call id, so
// we synthesize a stable one per call. `arguments` is JSON-stringified to match
// the OpenAI shape the operator loop + other adapters consume.
const extractToolCalls = (
  candidates: unknown,
): ReadonlyArray<InferenceToolCall> | undefined => {
  if (!Array.isArray(candidates) || candidates.length === 0) {
    return undefined
  }
  const first = candidates[0]
  if (typeof first !== 'object' || first === null) {
    return undefined
  }
  const content = (first as Record<string, unknown>)['content']
  if (typeof content !== 'object' || content === null) {
    return undefined
  }
  const parts = (content as Record<string, unknown>)['parts']
  if (!Array.isArray(parts)) {
    return undefined
  }
  const calls: Array<InferenceToolCall> = []
  parts.forEach((part, index) => {
    if (typeof part !== 'object' || part === null) return
    const functionCall = (part as Record<string, unknown>)['functionCall']
    if (typeof functionCall !== 'object' || functionCall === null) return
    const record = functionCall as Record<string, unknown>
    const name = typeof record['name'] === 'string' ? record['name'] : null
    if (name === null) return
    const args = record['args']
    // Gemini 3 returns a `thoughtSignature` on the PART alongside `functionCall`;
    // it MUST be replayed with the call or the next turn 400s. Capture it.
    const signature = (part as Record<string, unknown>)['thoughtSignature']
    calls.push({
      function: {
        arguments: JSON.stringify(args ?? {}),
        name,
      },
      id: `gemini_call_${index}_${name}`,
      type: 'function',
      ...(typeof signature === 'string' && signature !== ''
        ? { thoughtSignature: signature }
        : {}),
    })
  })
  return calls.length === 0 ? undefined : calls
}

// Pull the finishReason from the first candidate (Gemini values: "STOP",
// "MAX_TOKENS", "SAFETY", ...). Defaults to 'stop' when absent.
const extractFinishReason = (candidates: unknown): string => {
  if (!Array.isArray(candidates) || candidates.length === 0) {
    return 'stop'
  }
  const first = candidates[0]
  if (typeof first === 'object' && first !== null) {
    const reason = (first as Record<string, unknown>)['finishReason']
    if (typeof reason === 'string' && reason !== '') {
      return reason
    }
  }
  return 'stop'
}

// Whether an HTTP status from Vertex should be a retryable (overflow-eligible)
// failure: 429 (rate limit / quota) and 5xx transient errors. Routing reads
// `error.retryable` to fail over to other supply on quota pressure.
const isRetryableStatus = (status: number): boolean =>
  status === 429 || (status >= 500 && status <= 599)

const adapterError = (
  reason: string,
  retryable: boolean,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: VERTEX_GEMINI_ADAPTER_ID,
    retryable,
    reason,
  })

// Build the adapter from config. With no token provider it is "unconfigured"
// and every call yields a typed, non-retryable error (so the route maps it to a
// stable provider_error rather than a crash). This keeps the registry seeding
// in index.ts inert until the VERTEX_SA_KEY secret is present.
export const makeVertexGeminiAdapter = (
  config: VertexGeminiAdapterConfig,
): InferenceProviderAdapter => {
  const location = config.location ?? DEFAULT_VERTEX_LOCATION
  const resolveModelId = config.resolveModelId ?? defaultResolveModelId
  const fetchImpl = config.fetchImpl ?? fetch
  const tokenProvider = config.tokenProvider

  const ensureConfigured = (): Effect.Effect<
    VertexTokenProvider,
    InferenceAdapterError
  > =>
    tokenProvider === undefined
      ? Effect.fail(
          adapterError(
            'Vertex Gemini adapter is not configured (missing VERTEX_SA_KEY / token provider).',
            false,
          ),
        )
      : Effect.succeed(tokenProvider)

  // Issue a Vertex request and return the raw Cloudflare `Response`. Callers that
  // want a buffered body call `.text()`; the incremental `streamSse` path reads
  // `response.body` as bytes arrive instead. Status validation that needs the
  // body (the non-ok error slice) is left to the caller so the streaming path
  // never drains a successful body up front.
  const callResponse = (
    request: InferenceRequest,
    method: 'generateContent' | 'streamGenerateContent',
  ) =>
    Effect.gen(function* () {
      const provide = yield* ensureConfigured()
      const token = yield* provide()
      const modelId = resolveModelId(request.model)
      const url = vertexUrl(
        { location, project: config.project },
        modelId,
        method,
      )
      const body = buildGeminiBody(request)

      return yield* Effect.tryPromise({
        catch: error =>
          adapterError(
            `Vertex Gemini request transport error: ${
              error instanceof Error ? error.message : String(error)
            }`,
            true,
          ),
        try: () =>
          fetchImpl(url, {
            body: JSON.stringify(body),
            headers: {
              authorization: `Bearer ${token}`,
              'content-type': 'application/json',
            },
            method: 'POST',
          }),
      })
    })

  // Issue a Vertex request and return the raw response body text (buffered). Used
  // by `complete` and the buffered `stream` path. Reading the body here keeps the
  // Cloudflare Response type fully encapsulated.
  const call = (
    request: InferenceRequest,
    method: 'generateContent' | 'streamGenerateContent',
  ) =>
    Effect.gen(function* () {
      const response = yield* callResponse(request, method)
      const { ok, status, text } = yield* Effect.tryPromise({
        catch: error =>
          adapterError(
            `Vertex Gemini response read error: ${
              error instanceof Error ? error.message : String(error)
            }`,
            true,
          ),
        try: async () => ({
          ok: response.ok,
          status: response.status,
          text: await response.text(),
        }),
      })

      if (!ok) {
        return yield* Effect.fail(
          adapterError(
            `Vertex Gemini returned HTTP ${status}${
              text === '' ? '' : `: ${text.slice(0, 500)}`
            }`,
            isRetryableStatus(status),
          ),
        )
      }

      return text
    })

  return {
    complete: (request: InferenceRequest) =>
      Effect.gen(function* () {
        const text = yield* call(request, 'generateContent')
        const json = parseJsonRecord(text)
        if (json === undefined) {
          return yield* Effect.fail(
            adapterError('Vertex Gemini response was not valid JSON.', false),
          )
        }
        const candidates = json['candidates']
        const toolCalls = extractToolCalls(candidates)
        const result: InferenceResult = {
          content: extractText(candidates),
          // When Gemini returns function calls, normalize the finish reason to
          // the OpenAI `tool_calls` value the operator loop + route key off
          // (Gemini reports "STOP" even when it emits a functionCall).
          finishReason:
            toolCalls !== undefined
              ? 'tool_calls'
              : extractFinishReason(candidates),
          servedModel:
            typeof json['modelVersion'] === 'string'
              ? (json['modelVersion'] as string)
              : resolveModelId(request.model),
          usage: parseUsage(json['usageMetadata']),
          ...(toolCalls === undefined ? {} : { toolCalls }),
        }
        return result
      }),
    id: VERTEX_GEMINI_ADAPTER_ID,
    stream: (request: InferenceRequest) =>
      Effect.gen(function* () {
        const text = yield* call(request, 'streamGenerateContent')
        return parseGeminiSseChunks(text, request.model, resolveModelId)
      }),
    // TRUE PASS-THROUGH STREAM (the long-generation 524 fix + onboarding
    // token-by-token). Returns a lazily consumed SSE source over the upstream
    // `response.body` so the caller can pump each Gemini SSE fragment as Google
    // produces it — every fragment is a separate `event: delta`, so onboarding
    // (khala-mini) streams incrementally instead of one buffered reply. The
    // buffered `stream` above stays for tests/metering reconstruction/overflow.
    streamSse: (request: InferenceRequest) =>
      Effect.gen(function* () {
        const response = yield* callResponse(request, 'streamGenerateContent')
        if (!response.ok) {
          const errorText = yield* Effect.tryPromise({
            catch: () => adapterError('Vertex Gemini stream read error.', true),
            try: () => response.text(),
          })
          return yield* Effect.fail(
            adapterError(
              `Vertex Gemini returned HTTP ${response.status}${
                errorText === '' ? '' : `: ${errorText.slice(0, 500)}`
              }`,
              isRetryableStatus(response.status),
            ),
          )
        }
        const body = response.body
        if (body === null) {
          return yield* Effect.fail(
            adapterError('Vertex Gemini stream had no response body.', false),
          )
        }
        return makeGeminiSseSource(body, request.model, resolveModelId)
      }),
  }
}

const num = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

// Pull the `usageMetadata` (if any) off a single Gemini SSE fragment, layered on
// the running cumulative usage. Gemini reports usage CUMULATIVELY across the
// stream — each later fragment supersedes the earlier one — so we carry the prior
// values forward and only overwrite the fields this fragment actually reports.
const foldGeminiUsage = (
  fragment: Record<string, unknown>,
  prior: InferenceUsage | undefined,
): InferenceUsage | undefined => {
  const usage = fragment['usageMetadata']
  if (typeof usage !== 'object' || usage === null) {
    return prior
  }
  const u = usage as Record<string, unknown>
  const promptTokens =
    num(u['promptTokenCount']) ?? prior?.promptTokens ?? 0
  const completionTokens =
    num(u['candidatesTokenCount']) ?? prior?.completionTokens ?? 0
  const reportedTotal = num(u['totalTokenCount']) ?? prior?.totalTokens ?? 0
  const cached =
    num(u['cachedContentTokenCount']) ?? prior?.cachedPromptTokens
  return {
    completionTokens,
    promptTokens,
    totalTokens:
      reportedTotal > 0 ? reportedTotal : promptTokens + completionTokens,
    ...(cached === undefined || cached <= 0 ? {} : { cachedPromptTokens: cached }),
  }
}

// Normalize one parsed Gemini SSE fragment into the route-facing event shape.
// Shared by the buffered `stream` array path and the incremental `streamSse`
// pass-through so both produce identical normalization. `usage` carries the
// cumulative usage VISIBLE SO FAR (Gemini cumulates), folded over `priorUsage`.
const eventForGeminiFragment = (
  fragment: Record<string, unknown>,
  priorUsage: InferenceUsage | undefined,
): InferenceStreamEvent => {
  const event: {
    contentDelta: string
    finishReason?: string
    usage?: InferenceUsage
    servedModel?: string
  } = { contentDelta: '' }

  const candidates = fragment['candidates']
  if (Array.isArray(candidates) && candidates.length > 0) {
    event.contentDelta = extractText(candidates)
    // extractFinishReason defaults to 'stop'; only adopt a real terminal reason
    // when the fragment actually carries one.
    const first = candidates[0]
    if (
      typeof first === 'object' &&
      first !== null &&
      typeof (first as Record<string, unknown>)['finishReason'] === 'string'
    ) {
      event.finishReason = extractFinishReason(candidates)
    }
  }

  if (typeof fragment['modelVersion'] === 'string') {
    event.servedModel = fragment['modelVersion']
  }

  const usage = foldGeminiUsage(fragment, priorUsage)
  if (usage !== undefined) {
    event.usage = usage
  }
  return event
}

// Parse a single SSE `data:` line into a Gemini fragment record, or undefined for
// blank/comment/`[DONE]` lines.
const parseGeminiSseData = (
  line: string,
): Record<string, unknown> | undefined => {
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

// Parse a Vertex Gemini streamGenerateContent (?alt=sse) body into our
// normalized stream chunks. Each `data:` line is a GenerateContentResponse
// fragment carrying incremental `candidates[].content.parts[].text`; the final
// fragments carry `finishReason` and the cumulative `usageMetadata`. We collapse
// this into content deltas plus one terminal frame carrying the receipt-first
// usage so the route's metering settles from real counts. (The route consumes a
// batched array; this preserves that contract.)
const parseGeminiSseChunks = (
  body: string,
  requestedModel: string,
  resolveModelId: (model: string) => string,
): ReadonlyArray<InferenceStreamChunk> => {
  let finishReason: string | undefined
  let servedModel = resolveModelId(requestedModel)
  let usage: InferenceUsage | undefined
  const contentDeltas: Array<string> = []

  for (const line of body.split('\n')) {
    const fragment = parseGeminiSseData(line)
    if (fragment === undefined) {
      continue
    }
    const event = eventForGeminiFragment(fragment, usage)
    if (event.contentDelta !== '') {
      contentDeltas.push(event.contentDelta)
    }
    if (event.finishReason !== undefined) {
      finishReason = event.finishReason
    }
    if (event.servedModel !== undefined) {
      servedModel = event.servedModel
    }
    if (event.usage !== undefined) {
      usage = event.usage
    }
  }

  const terminalUsage: InferenceUsage = usage ?? {
    completionTokens: 0,
    promptTokens: 0,
    totalTokens: 0,
  }

  const chunks: Array<InferenceStreamChunk> = []
  const joined = contentDeltas.join('')
  if (joined !== '') {
    chunks.push({ contentDelta: joined })
  }
  chunks.push({
    contentDelta: '',
    finishReason: finishReason ?? 'stop',
    servedModel,
    usage: terminalUsage,
  })
  return chunks
}

// Build a true incremental SSE source over the upstream Gemini `ReadableStream`.
// Fragments are decoded + parsed AS BYTES ARRIVE (partial lines buffered across
// reads) and yielded one at a time, so the caller can pump each to the client and
// reset the edge idle-timer — and onboarding emits one `event: delta` per Gemini
// fragment instead of one buffered reply. The running terminal state
// (finishReason / cumulative usage / servedModel) is captured during iteration
// and exposed via `terminal()` once the stream is drained — receipt-first, no
// re-buffering of content. Blank/comment/`[DONE]` lines are skipped.
const makeGeminiSseSource = (
  body: ReadableStream<Uint8Array>,
  requestedModel: string,
  resolveModelId: (model: string) => string,
): InferenceStreamSource => {
  let finishReason: string | undefined
  let usage: InferenceUsage | undefined
  let servedModel: string | undefined = resolveModelId(requestedModel)

  const captureTerminalState = (event: InferenceStreamEvent): void => {
    if (event.finishReason !== undefined) {
      finishReason = event.finishReason
    }
    if (event.usage !== undefined) {
      usage = event.usage
    }
    if (event.servedModel !== undefined) {
      servedModel = event.servedModel
    }
  }

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
          const fragment = parseGeminiSseData(line)
          if (fragment !== undefined) {
            const event = eventForGeminiFragment(fragment, usage)
            captureTerminalState(event)
            yield event
          }
          newlineIndex = buffer.indexOf('\n')
        }
        if (done) {
          const tail = parseGeminiSseData(buffer)
          if (tail !== undefined) {
            const event = eventForGeminiFragment(tail, usage)
            captureTerminalState(event)
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
    terminal: () => ({ finishReason, servedModel, usage }),
  }
}
