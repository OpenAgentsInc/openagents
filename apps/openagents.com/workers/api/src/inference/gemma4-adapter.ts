// Gemma 4 provider adapter for the inference gateway — the primary conversational
// Khala lane on our own Google Cloud (owner decision 2026-07-09: "we are going to
// use Gemma 4 via our gcloud primarily").
//
// Unlike the Vertex Gemini lane (`vertex-gemini-adapter.ts`, Google's Gemini on
// the Vertex `aiplatform` publishers/google endpoint with an SA-minted OAuth
// token), this lane targets the **Generative Language API**:
//   POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key=GEMINI_API_KEY
//   POST https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse&key=GEMINI_API_KEY
// This is the EXACT path proved live by `apps/sarah/src/services/google-inference.ts`
// (#8594). Auth is the `GEMINI_API_KEY` Worker secret in the URL query string —
// the SAME key sarah uses; we reuse it, never mint a parallel one. The key rides
// the URL, so this adapter NEVER surfaces the request URL in an error string.
//
// Gemma 4 specifics honored here:
//   - THINKING MODEL: candidates carry scratchpad parts flagged `thought: true`
//     before the answer parts. Thought parts are filtered out of user-visible
//     content (buffered `complete`) / routed to the separate `reasoningDelta`
//     channel (streaming), never into `contentDelta`.
//   - `usageMetadata.thoughtsTokenCount` maps to `usage.reasoningTokens` — exact
//     only: mapped verbatim from the provider field, never invented. Google's
//     `totalTokenCount` already INCLUDES thoughts, so we trust it as the total.
//   - NO TOOL CALLING. Gemma exposes only generateContent/countTokens. This
//     adapter is a NO-TOOLS lane: a tool-bearing request typed-fails RETRYABLY
//     (`tool_calls_unsupported`) so `dispatchWithOverflow` moves it to a
//     tool-capable lane (Vertex Gemini / Fireworks / GLM) instead of silently
//     dropping the tools. The router ALSO keeps this id out of every tool plan
//     (model-router.ts `selectAdapterPlanForKhalaToolRequest`); the adapter guard
//     is defense-in-depth so the no-tools guarantee holds on every routing path.
//   - TINY OUTPUT BUDGET GUARD: thoughts draw from the same output budget, so a
//     tiny `max_tokens` (e.g. the canary's 8) is entirely consumed by thoughts
//     and the model emits ZERO visible text — which dispatch treats as an empty
//     lane and overflows off the primary (the 2026-07-08 AAR caveat). We floor
//     the effective `maxOutputTokens` to `minOutputTokens` so tiny budgets keep
//     headroom for a visible answer and the canary stays on the primary lane.
//
// It implements the InferenceProviderAdapter seam (`complete` + `stream` +
// `streamSse`), returning receipt-first `usage` from `usageMetadata` — never an
// estimate (INVARIANTS.md "Canonical Token Usage Ledger"). It owns ONLY request
// translation + transport; it never touches credits, payment, routing, free
// allowance, or public projection.
//
// INERT by default: index.ts constructs this adapter from env and registers it
// once; with no GEMINI_API_KEY the adapter is unconfigured and surfaces a typed
// non-retryable error rather than calling out. The gateway route stays flag-gated
// off via INFERENCE_GATEWAY_ENABLED regardless.
import { Effect, Redacted } from 'effect'

import { parseJsonRecord } from '../json-boundary'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

export const GEMMA4_ADAPTER_ID = 'google-gemma4'

// The default Gemma 4 model id served from the Generative Language API. Catalog
// slug verified live under #8594 (dense, default). The MoE variant
// `gemma-4-26b-a4b-it` is available via the SAME path if a future override needs
// it (GEMMA4_MODEL env), but the dense model leads.
export const DEFAULT_GEMMA4_MODEL_ID = 'gemma-4-31b-it'

export const GENERATIVE_LANGUAGE_BASE_URL =
  'https://generativelanguage.googleapis.com/v1beta'

export const GEMMA4_DEFAULT_TIMEOUT_MS = 60_000

// Default output budget when the caller passes none. Thoughts draw from this
// budget, so keep healthy headroom above the visible answer we actually want
// (mirrors sarah's 2048 default).
export const GEMMA4_DEFAULT_MAX_OUTPUT_TOKENS = 2048

// Floor for the effective output budget. Tiny budgets (the canary's max_tokens=8)
// would otherwise be entirely consumed by thoughts and emit no visible text,
// which dispatch treats as an empty lane and overflows off the primary. Flooring
// keeps headroom for a visible answer so tiny requests stay on this lane.
export const GEMMA4_DEFAULT_MIN_OUTPUT_TOKENS = 512

export type Gemma4Fetch = (
  input: string,
  init: Readonly<{
    body: string
    headers: Record<string, string>
    method: string
    signal?: AbortSignal | undefined
  }>,
) => Promise<Response>

export type Gemma4AdapterConfig = Readonly<{
  // Resolves the GEMINI_API_KEY Worker secret at CALL time (the registry is
  // constructed at module load, before the live env is captured — same lazy
  // pattern as the Vertex lanes' tokenProvider). Returns undefined when the key
  // is absent, in which case every call fails with a typed non-retryable error.
  apiKey?: (() => Redacted.Redacted<string> | undefined) | undefined
  // Generative Language API base URL (no trailing slash needed). Default is the
  // public origin; overridable for tests.
  baseUrl?: string | undefined
  // The Gemma model id path segment. Default DEFAULT_GEMMA4_MODEL_ID.
  model?: string | undefined
  // Effective output-budget floor (see GEMMA4_DEFAULT_MIN_OUTPUT_TOKENS).
  minOutputTokens?: number | undefined
  fetchImpl?: Gemma4Fetch | undefined
  timeoutMs?: number | undefined
}>

type GemmaPart = {
  text?: string | undefined
  thought?: boolean | undefined
}

// Whether a request carries tool/function material. Gemma cannot serve it, so the
// adapter refuses (retryably) and the router keeps such requests off this lane.
// Mirrors chat-completions-routes `isToolBearingKhalaRequest` so the two agree.
const requestCarriesTools = (request: InferenceRequest): boolean => {
  const params = request.passthroughParams
  const declaredTools = params['tools']
  if (Array.isArray(declaredTools) && declaredTools.length > 0) {
    return true
  }
  const declaredFunctions = params['functions']
  if (Array.isArray(declaredFunctions) && declaredFunctions.length > 0) {
    return true
  }
  return request.messages.some(
    message =>
      message.role === 'tool' ||
      message.toolCallId !== undefined ||
      (message.toolCalls !== undefined && message.toolCalls.length > 0),
  )
}

// Map our normalized role onto a Gemma `contents[].role` (`user` | `model`).
// `assistant` -> `model`; `system` is hoisted into `systemInstruction`; anything
// else defaults to `user` (a safe, non-throwing default).
const toGemmaRole = (role: string): 'user' | 'model' =>
  role === 'assistant' || role === 'model' ? 'model' : 'user'

const numberParam = (
  params: Readonly<Record<string, unknown>>,
  key: string,
): number | undefined => {
  const value = params[key]
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

const buildGenerationConfig = (
  request: InferenceRequest,
  minOutputTokens: number,
): Record<string, unknown> => {
  const passthrough = request.passthroughParams
  const config: Record<string, unknown> = {}

  const rawMax = passthrough['max_tokens']
  const requestedMax =
    typeof rawMax === 'number' && Number.isInteger(rawMax) && rawMax > 0
      ? rawMax
      : GEMMA4_DEFAULT_MAX_OUTPUT_TOKENS
  // Floor the effective budget so a tiny request keeps headroom for visible text
  // after the thinking budget (see GEMMA4_DEFAULT_MIN_OUTPUT_TOKENS).
  config['maxOutputTokens'] = Math.max(requestedMax, minOutputTokens)

  const temperature = numberParam(passthrough, 'temperature')
  if (temperature !== undefined) {
    config['temperature'] = temperature
  }
  const topP = numberParam(passthrough, 'top_p')
  if (topP !== undefined) {
    config['topP'] = topP
  }
  const topK = numberParam(passthrough, 'top_k')
  if (topK !== undefined) {
    config['topK'] = topK
  }
  return config
}

const buildGemmaBody = (
  request: InferenceRequest,
  minOutputTokens: number,
): Record<string, unknown> => {
  const systemTexts: Array<string> = []
  const contents: Array<Record<string, unknown>> = []
  for (const message of request.messages) {
    if (message.role === 'system') {
      if (message.content !== '') {
        systemTexts.push(message.content)
      }
      continue
    }
    contents.push({
      parts: [{ text: message.content }],
      role: toGemmaRole(message.role),
    })
  }

  const passthroughSystem = request.passthroughParams['system']
  if (typeof passthroughSystem === 'string' && passthroughSystem !== '') {
    systemTexts.unshift(passthroughSystem)
  }

  const body: Record<string, unknown> = {
    contents,
    generationConfig: buildGenerationConfig(request, minOutputTokens),
  }
  if (systemTexts.length > 0) {
    body['systemInstruction'] = { parts: systemTexts.map(text => ({ text })) }
  }
  return body
}

// Classify a Generative Language API HTTP status into a routing failure class.
// 402 (billing) / 429 (rate/quota) / 503 (overloaded) / 5xx are LANE-unviable, so
// they overflow to the next lane; a 4xx request rejection surfaces non-retryably.
// (Matches the openrouter-adapter classification pattern from the #8565 fix.)
const classifyStatus = (
  status: number,
): Readonly<{ kind: string; retryable: boolean }> => {
  if (status === 429) {
    return { kind: 'rate_limited', retryable: true }
  }
  if (status === 402) {
    return { kind: 'quota_exhausted', retryable: true }
  }
  if (status === 503) {
    return { kind: 'service_overloaded', retryable: true }
  }
  if (status >= 500) {
    return { kind: 'upstream_error', retryable: true }
  }
  return { kind: 'request_rejected', retryable: false }
}

const adapterError = (
  input: Readonly<{
    reason: string
    retryable: boolean
    httpStatus?: number | undefined
    kind?: string | undefined
  }>,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: GEMMA4_ADAPTER_ID,
    httpStatus: input.httpStatus,
    kind: input.kind,
    reason: input.reason,
    retryable: input.retryable,
  })

const safeSignal = (timeoutMs: number): AbortSignal | undefined => {
  try {
    return AbortSignal.timeout(timeoutMs)
  } catch {
    return undefined
  }
}

// Parse the receipt-first usage. Maps Gemma `usageMetadata` field names onto our
// InferenceUsage. `thoughtsTokenCount` -> `reasoningTokens` (exact, only when the
// field is a real finite number). Google's `totalTokenCount` already includes
// thoughts, so we trust it; a missing/zero total falls back to the component sum
// (prompt + candidates + thoughts) so a malformed-but-2xx response is still exact
// to whatever fields were present.
const parseUsage = (raw: unknown): InferenceUsage => {
  const usage =
    typeof raw === 'object' && raw !== null
      ? (raw as Record<string, unknown>)
      : {}
  const num = (value: unknown): number =>
    typeof value === 'number' && Number.isFinite(value) ? value : 0
  const promptTokens = num(usage['promptTokenCount'])
  const completionTokens = num(usage['candidatesTokenCount'])
  const reasoningTokens = num(usage['thoughtsTokenCount'])
  const reportedTotal = num(usage['totalTokenCount'])
  const totalTokens =
    reportedTotal > 0
      ? reportedTotal
      : promptTokens + completionTokens + reasoningTokens
  const cached = usage['cachedContentTokenCount']
  return {
    completionTokens,
    promptTokens,
    totalTokens,
    ...(reasoningTokens > 0 ? { reasoningTokens } : {}),
    ...(typeof cached === 'number' && Number.isFinite(cached) && cached > 0
      ? { cachedPromptTokens: cached }
      : {}),
  }
}

const partsOfFirstCandidate = (candidates: unknown): ReadonlyArray<GemmaPart> => {
  if (!Array.isArray(candidates) || candidates.length === 0) {
    return []
  }
  const first = candidates[0]
  if (typeof first !== 'object' || first === null) {
    return []
  }
  const content = (first as Record<string, unknown>)['content']
  if (typeof content !== 'object' || content === null) {
    return []
  }
  const parts = (content as Record<string, unknown>)['parts']
  if (!Array.isArray(parts)) {
    return []
  }
  return parts.filter(
    (part): part is GemmaPart => typeof part === 'object' && part !== null,
  )
}

// Visible answer text: concatenate NON-thought text parts. `thought: true`
// scratchpad parts are dropped from user-visible content.
const extractVisibleText = (candidates: unknown): string =>
  partsOfFirstCandidate(candidates)
    .filter(part => part.thought !== true && typeof part.text === 'string')
    .map(part => part.text ?? '')
    .join('')

// Reasoning/thinking text: concatenate `thought: true` text parts. Surfaced on
// the separate `reasoningDelta` channel (streaming) so clients can hide it; never
// mixed into user-visible content.
const extractThoughtText = (candidates: unknown): string =>
  partsOfFirstCandidate(candidates)
    .filter(part => part.thought === true && typeof part.text === 'string')
    .map(part => part.text ?? '')
    .join('')

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

const servedModelFrom = (raw: Record<string, unknown>, fallback: string): string =>
  typeof raw['modelVersion'] === 'string' && raw['modelVersion'] !== ''
    ? (raw['modelVersion'] as string)
    : fallback

const num = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

// Fold a single SSE fragment's `usageMetadata` over the running cumulative usage.
// Gemma reports usage CUMULATIVELY across the stream, so a later fragment
// supersedes the earlier one; we carry prior values forward and overwrite only the
// fields this fragment actually reports.
const foldGemmaUsage = (
  fragment: Record<string, unknown>,
  prior: InferenceUsage | undefined,
): InferenceUsage | undefined => {
  const usage = fragment['usageMetadata']
  if (typeof usage !== 'object' || usage === null) {
    return prior
  }
  const u = usage as Record<string, unknown>
  const promptTokens = num(u['promptTokenCount']) ?? prior?.promptTokens ?? 0
  const completionTokens =
    num(u['candidatesTokenCount']) ?? prior?.completionTokens ?? 0
  const reasoningTokens =
    num(u['thoughtsTokenCount']) ?? prior?.reasoningTokens ?? 0
  const reportedTotal = num(u['totalTokenCount']) ?? prior?.totalTokens ?? 0
  const cached = num(u['cachedContentTokenCount']) ?? prior?.cachedPromptTokens
  return {
    completionTokens,
    promptTokens,
    totalTokens:
      reportedTotal > 0
        ? reportedTotal
        : promptTokens + completionTokens + reasoningTokens,
    ...(reasoningTokens > 0 ? { reasoningTokens } : {}),
    ...(cached === undefined || cached <= 0 ? {} : { cachedPromptTokens: cached }),
  }
}

const eventForGemmaFragment = (
  fragment: Record<string, unknown>,
  priorUsage: InferenceUsage | undefined,
  fallbackModel: string,
): InferenceStreamEvent => {
  const event: {
    contentDelta: string
    reasoningDelta?: string
    finishReason?: string
    usage?: InferenceUsage
    servedModel?: string
  } = { contentDelta: '' }

  const candidates = fragment['candidates']
  if (Array.isArray(candidates) && candidates.length > 0) {
    event.contentDelta = extractVisibleText(candidates)
    const thought = extractThoughtText(candidates)
    if (thought !== '') {
      event.reasoningDelta = thought
    }
    const first = candidates[0]
    if (
      typeof first === 'object' &&
      first !== null &&
      typeof (first as Record<string, unknown>)['finishReason'] === 'string'
    ) {
      event.finishReason = extractFinishReason(candidates)
    }
  }

  event.servedModel = servedModelFrom(fragment, fallbackModel)

  const usage = foldGemmaUsage(fragment, priorUsage)
  if (usage !== undefined) {
    event.usage = usage
  }
  return event
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

const makeGemmaSseSource = (
  body: ReadableStream<Uint8Array>,
  fallbackModel: string,
): InferenceStreamSource => {
  let finishReason: string | undefined
  let usage: InferenceUsage | undefined
  let servedModel: string | undefined = fallbackModel

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
          const fragment = parseSseData(line)
          if (fragment !== undefined) {
            const event = eventForGemmaFragment(fragment, usage, fallbackModel)
            captureTerminalState(event)
            yield event
          }
          newlineIndex = buffer.indexOf('\n')
        }
        if (done) {
          const tail = parseSseData(buffer)
          if (tail !== undefined) {
            const event = eventForGemmaFragment(tail, usage, fallbackModel)
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

export const makeGemma4Adapter = (
  config: Gemma4AdapterConfig,
): InferenceProviderAdapter => {
  const baseUrl = (config.baseUrl ?? GENERATIVE_LANGUAGE_BASE_URL).replace(
    /\/+$/u,
    '',
  )
  const model = config.model?.trim() || DEFAULT_GEMMA4_MODEL_ID
  const minOutputTokens =
    typeof config.minOutputTokens === 'number' &&
    Number.isInteger(config.minOutputTokens) &&
    config.minOutputTokens > 0
      ? config.minOutputTokens
      : GEMMA4_DEFAULT_MIN_OUTPUT_TOKENS
  const timeoutMs = config.timeoutMs ?? GEMMA4_DEFAULT_TIMEOUT_MS
  const fetchImpl = config.fetchImpl ?? (globalThis.fetch as Gemma4Fetch)

  // The endpoint carries the API key in the query string, so it must NEVER be
  // surfaced in an error message or log.
  const endpoint = (
    method: 'generateContent' | 'streamGenerateContent',
    apiKey: Redacted.Redacted<string>,
  ): string => {
    const base = `${baseUrl}/models/${model}:${method}`
    const sse = method === 'streamGenerateContent' ? '&alt=sse' : ''
    return `${base}?key=${Redacted.value(apiKey)}${sse}`
  }

  const ensureConfigured = (): Effect.Effect<
    Redacted.Redacted<string>,
    InferenceAdapterError
  > => {
    const resolved = config.apiKey?.()
    return resolved === undefined
      ? Effect.fail(
          adapterError({
            reason:
              'Gemma 4 adapter is not configured (missing GEMINI_API_KEY).',
            retryable: false,
          }),
        )
      : Effect.succeed(resolved)
  }

  // The NO-TOOLS guard. A tool-bearing request cannot be served by Gemma; fail
  // RETRYABLY so dispatch overflows to a tool-capable lane rather than dropping
  // the tools. Defense-in-depth with the router's tool-plan exclusion.
  const ensureNoTools = (
    request: InferenceRequest,
  ): Effect.Effect<void, InferenceAdapterError> =>
    requestCarriesTools(request)
      ? Effect.fail(
          adapterError({
            kind: 'tool_calls_unsupported',
            reason:
              'retryable: Gemma 4 has no tool calling — overflowing tool-bearing request to a tool-capable lane',
            retryable: true,
          }),
        )
      : Effect.void

  const postResponse = (
    request: InferenceRequest,
    method: 'generateContent' | 'streamGenerateContent',
  ): Effect.Effect<Response, InferenceAdapterError> =>
    Effect.gen(function* () {
      yield* ensureNoTools(request)
      const apiKey = yield* ensureConfigured()
      const signal = request.abortSignal ?? safeSignal(timeoutMs)
      return yield* Effect.tryPromise({
        catch: error =>
          adapterError({
            kind: 'transport_error',
            reason: `retryable: Gemma 4 transport error (${
              error instanceof Error ? error.name : 'unknown'
            })`,
            retryable: true,
          }),
        try: () =>
          fetchImpl(endpoint(method, apiKey), {
            body: JSON.stringify(buildGemmaBody(request, minOutputTokens)),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
            ...(signal === undefined ? {} : { signal }),
          }),
      })
    })

  const failForStatus = (
    status: number,
    surface: 'request' | 'stream request',
  ): InferenceAdapterError => {
    const classified = classifyStatus(status)
    // Never include the endpoint (it carries the key) in the surfaced reason.
    return adapterError({
      httpStatus: status,
      kind: classified.kind,
      reason: classified.retryable
        ? `retryable: Gemma 4 rejected ${surface} (${status})`
        : `Gemma 4 rejected ${surface} (${status})`,
      retryable: classified.retryable,
    })
  }

  const runCompletion = (
    request: InferenceRequest,
  ): Effect.Effect<InferenceResult, InferenceAdapterError> =>
    Effect.gen(function* () {
      const response = yield* postResponse(request, 'generateContent')
      const { ok, status, text } = yield* Effect.tryPromise({
        catch: () =>
          adapterError({
            kind: 'malformed_response',
            reason: 'Gemma 4 returned an unreadable response body',
            retryable: true,
          }),
        try: async () => ({
          ok: response.ok,
          status: response.status,
          text: await response.text(),
        }),
      })
      if (!ok) {
        return yield* Effect.fail(failForStatus(status, 'request'))
      }
      const raw = parseJsonRecord(text)
      if (raw === undefined) {
        return yield* Effect.fail(
          adapterError({
            kind: 'malformed_response',
            reason: 'Gemma 4 returned unparseable JSON',
            retryable: false,
          }),
        )
      }
      const candidates = raw['candidates']
      return {
        content: extractVisibleText(candidates),
        finishReason: extractFinishReason(candidates),
        servedModel: servedModelFrom(raw, model),
        usage: parseUsage(raw['usageMetadata']),
      } satisfies InferenceResult
    })

  return {
    complete: request => runCompletion({ ...request, stream: false }),
    id: GEMMA4_ADAPTER_ID,
    // Buffered stream: materialize the completion into a content chunk plus a
    // terminal receipt chunk. Kept for tests / metering reconstruction / the
    // overflow dispatcher; the route prefers `streamSse` for the hot path.
    stream: request =>
      runCompletion({ ...request, stream: false }).pipe(
        Effect.map((result): ReadonlyArray<InferenceStreamChunk> => {
          const chunks: Array<InferenceStreamChunk> = []
          if (result.content !== '') {
            chunks.push({ contentDelta: result.content })
          }
          chunks.push({
            contentDelta: '',
            finishReason: result.finishReason,
            servedModel: result.servedModel,
            usage: result.usage,
          })
          return chunks
        }),
      ),
    // TRUE PASS-THROUGH STREAM. Lazily consumed SSE source over the upstream
    // `streamGenerateContent?alt=sse` body: each Gemma fragment is decoded as
    // bytes arrive and yielded one at a time, so the route pumps every fragment to
    // the client and the edge idle-timer resets (long generations never 524).
    // Thought parts route to `reasoningDelta`; the terminal receipt state
    // (finishReason / cumulative usage / servedModel) is captured during
    // iteration and exposed via `terminal()`.
    streamSse: request =>
      Effect.gen(function* () {
        const response = yield* postResponse(request, 'streamGenerateContent')
        if (!response.ok) {
          return yield* Effect.fail(failForStatus(response.status, 'stream request'))
        }
        const body = response.body
        if (body === null) {
          return yield* Effect.fail(
            adapterError({
              kind: 'malformed_response',
              reason: 'Gemma 4 stream had no response body',
              retryable: false,
            }),
          )
        }
        return makeGemmaSseSource(body, model)
      }),
  }
}
