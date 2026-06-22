// Vertex Anthropic provider adapter for the inference gateway
// (EPIC #5474, #5480). This is the Claude lane: it serves shared-lineage
// Claude models (Opus / Sonnet / Haiku) from our first-party Google Cloud
// Vertex AI quota (project `openagentsgemini`, see the gateway business doc §3a
// and `../../../../docs/cloud/quotas/2026-06-19-vertex-ai-anthropic-opus-quota-request.md`).
//
// It implements the InferenceProviderAdapter seam (`complete` + `stream`),
// mapping our normalized InferenceRequest onto the Vertex AI Anthropic Messages
// wire format and returning receipt-first `usage` from the provider response —
// never an estimate (INVARIANTS.md "Canonical Token Usage Ledger" + gateway
// business doc §4). It owns ONLY request translation + transport; it never
// touches credits, payment, routing, or public projection.
//
// Wire contract (Vertex AI Anthropic, verified against the Claude-on-Vertex
// docs 2026-06-19):
//   - Endpoint:
//       POST https://{host}/v1/projects/{project}/locations/{location}/
//            publishers/anthropic/models/{modelId}:rawPredict
//       (and :streamRawPredict for SSE)
//     where {host} is `aiplatform.googleapis.com` for the `global` location and
//     `{location}-aiplatform.googleapis.com` for regional/multi-region ones.
//   - `model` is NOT in the body — it is the {modelId} path segment.
//   - `anthropic_version` is in the BODY and must be `vertex-2023-10-16`.
//   - Auth: `Authorization: Bearer <GCP access token>`.
//   - Response is the standard Anthropic Messages shape: `content[].text`,
//     `stop_reason`, and `usage.{input_tokens,output_tokens,
//     cache_read_input_tokens,cache_creation_input_tokens}`.
//
// AUTH NOTE (important): a Cloudflare Worker cannot use gcloud Application
// Default Credentials. We mint a short-lived GCP access token from a
// service-account key (stored as the Worker secret VERTEX_SA_KEY — a JSON SA
// key, never committed) via a signed JWT -> Google OAuth token exchange
// (RS256, Web Crypto). A pre-minted token may also be supplied directly (for
// tests / a sidecar that already holds ADC). See `tokenProvider` below.
//
// INERT by default: index.ts constructs this adapter from env and registers it
// once; with no VERTEX_SA_KEY (and no pre-minted token) the adapter is
// unconfigured and surfaces a typed error rather than calling out. The whole
// gateway route stays flag-gated off via INFERENCE_GATEWAY_ENABLED regardless.

import { Effect } from 'effect'

import { parseJsonRecord } from '../json-boundary'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceUsage,
} from './provider-adapter'

export const VERTEX_ANTHROPIC_ADAPTER_ID = 'vertex-anthropic'

// The Vertex body version string — fixed by the platform contract.
const VERTEX_ANTHROPIC_VERSION = 'vertex-2023-10-16'

// Default Vertex location. `global` uses the bare `aiplatform.googleapis.com`
// host (no region subdomain prefix). Matches the gateway business doc default
// (the global endpoint has the broadest shared-lineage quota and no regional
// pricing premium).
const DEFAULT_VERTEX_LOCATION = 'global'

// Default max output tokens when the caller does not pass one. Vertex/Anthropic
// requires `max_tokens`; the route forwards a caller value via passthroughParams
// when present, otherwise we apply this floor so a request is never malformed.
const DEFAULT_MAX_TOKENS = 1024

// Pluggable token source. Returns a GCP OAuth access token string. The
// SA-key->token implementation is `makeServiceAccountTokenProvider`; tests pass
// a fake that returns a fixed token without any network call.
export type VertexTokenProvider = () => Effect.Effect<string, InferenceAdapterError>

// Pluggable fetch (defaults to global fetch; tests inject a mock).
export type VertexFetch = typeof fetch

export type VertexAnthropicAdapterConfig = Readonly<{
  // GCP project id that holds the Claude Vertex quota (e.g. "openagentsgemini").
  project: string
  // Vertex location ("global" | "us" | "eu" | a region like "us-east1").
  location?: string | undefined
  // Mints a GCP access token. When undefined the adapter is unconfigured and
  // every call fails with a typed, non-retryable error.
  tokenProvider?: VertexTokenProvider | undefined
  // Maps a requested model alias to the Vertex-native model id. Defaults to an
  // identity map over the known shared-lineage Claude ids; routing (#5482) owns
  // alias policy, this is only a last-mile id normalizer.
  resolveModelId?: ((requestedModel: string) => string) | undefined
  // Injected for tests; defaults to global fetch.
  fetchImpl?: VertexFetch | undefined
}>

// Shared-lineage Claude model ids we serve from Vertex (gateway business doc
// §3a). The router (#5482) decides WHICH alias maps here; exported as the
// last-mile reference of the served lane (Opus/Sonnet/Haiku shared lineage).
export const KNOWN_VERTEX_MODEL_IDS: ReadonlyArray<string> = [
  'claude-opus-4-8',
  'claude-opus-4-7',
  'claude-opus-4-6',
  'claude-sonnet-4-6',
  'claude-haiku-4-5',
]

const defaultResolveModelId = (requestedModel: string): string => {
  // Strip a leading "vertex/" or "anthropic/" provider hint if present, then
  // pass the bare id through. Unknown ids are forwarded as-is; Vertex returns a
  // 4xx the adapter maps to a typed error (not retryable).
  const stripped = requestedModel.replace(/^(?:vertex|anthropic)\//u, '')
  return stripped
}

const vertexHost = (location: string): string =>
  location === 'global'
    ? 'aiplatform.googleapis.com'
    : `${location}-aiplatform.googleapis.com`

const vertexUrl = (
  config: Readonly<{ project: string; location: string }>,
  modelId: string,
  method: 'rawPredict' | 'streamRawPredict',
): string =>
  `https://${vertexHost(config.location)}/v1/projects/${config.project}` +
  `/locations/${config.location}/publishers/anthropic/models/${modelId}:${method}`

// Number sampling params Vertex/Anthropic accepts that the route may forward
// verbatim via passthroughParams. We copy only recognized, type-checked fields
// so an arbitrary client cannot inject unsupported keys.
const PASSTHROUGH_NUMBER_PARAMS: ReadonlyArray<string> = [
  'temperature',
  'top_p',
  'top_k',
]

const buildVertexBody = (
  request: InferenceRequest,
  stream: boolean,
): Record<string, unknown> => {
  const passthrough = request.passthroughParams
  const rawMax = passthrough['max_tokens']
  const maxTokens =
    typeof rawMax === 'number' && Number.isInteger(rawMax) && rawMax > 0
      ? rawMax
      : DEFAULT_MAX_TOKENS

  const body: Record<string, unknown> = {
    anthropic_version: VERTEX_ANTHROPIC_VERSION,
    max_tokens: maxTokens,
    // Anthropic Messages shape mirrors our normalized {role, content}.
    messages: request.messages.map(message => ({
      content: message.content,
      role: message.role,
    })),
    stream,
  }

  for (const key of PASSTHROUGH_NUMBER_PARAMS) {
    const value = passthrough[key]
    if (typeof value === 'number') {
      body[key] = value
    }
  }
  // A `system` string param (the Anthropic top-level system prompt) is
  // forwarded when present, since our normalized messages keep role names but
  // some clients send system separately.
  const system = passthrough['system']
  if (typeof system === 'string' && system.length > 0) {
    body['system'] = system
  }

  return body
}

// Parse the receipt-first usage object from a Vertex Anthropic Messages
// response. Maps Anthropic field names onto our InferenceUsage. Missing fields
// default to 0 so a malformed-but-2xx response still yields a stable shape; the
// caller (metering) treats these as authoritative provider counts.
const parseUsage = (raw: unknown): InferenceUsage => {
  const usage =
    typeof raw === 'object' && raw !== null
      ? (raw as Record<string, unknown>)
      : {}
  const num = (value: unknown): number =>
    typeof value === 'number' && Number.isFinite(value) ? value : 0
  const promptTokens = num(usage['input_tokens'])
  const completionTokens = num(usage['output_tokens'])
  const cacheRead = usage['cache_read_input_tokens']
  return {
    completionTokens,
    promptTokens,
    totalTokens: promptTokens + completionTokens,
    ...(typeof cacheRead === 'number' && Number.isFinite(cacheRead)
      ? { cachedPromptTokens: cacheRead }
      : {}),
  }
}

// Concatenate the text from an Anthropic `content` array (text blocks only).
const extractText = (content: unknown): string => {
  if (!Array.isArray(content)) {
    return ''
  }
  return content
    .map(block => {
      if (
        typeof block === 'object' &&
        block !== null &&
        (block as Record<string, unknown>)['type'] === 'text'
      ) {
        const text = (block as Record<string, unknown>)['text']
        return typeof text === 'string' ? text : ''
      }
      return ''
    })
    .join('')
}

// Whether an HTTP status from Vertex should be a retryable (overflow-eligible)
// failure: 429 (rate limit / quota) and 5xx transient errors. Routing/overflow
// (#5482) reads `error.retryable` to fail over to other supply on quota
// pressure, per the issue's 429/quota -> typed retryable error requirement.
const isRetryableStatus = (status: number): boolean =>
  status === 429 || (status >= 500 && status <= 599)

const adapterError = (
  reason: string,
  retryable: boolean,
): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: VERTEX_ANTHROPIC_ADAPTER_ID,
    retryable,
    reason,
  })

// Build the adapter from config. With no token provider it is "unconfigured"
// and every call yields a typed, non-retryable error (so the route maps it to a
// stable provider_error rather than a crash). This keeps the registry seeding
// in index.ts inert until the VERTEX_SA_KEY secret is present.
export const makeVertexAnthropicAdapter = (
  config: VertexAnthropicAdapterConfig,
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
            'Vertex Anthropic adapter is not configured (missing VERTEX_SA_KEY / token provider).',
            false,
          ),
        )
      : Effect.succeed(tokenProvider)

  // Issue a Vertex request and return the raw response body text. Reading the
  // body here keeps the Cloudflare Response type fully encapsulated in this
  // helper (no Response-returning surface leaks into the adapter contract).
  const call = (request: InferenceRequest, method: 'rawPredict' | 'streamRawPredict') =>
    Effect.gen(function* () {
      const provide = yield* ensureConfigured()
      const token = yield* provide()
      const modelId = resolveModelId(request.model)
      const url = vertexUrl({ location, project: config.project }, modelId, method)
      const body = buildVertexBody(request, method === 'streamRawPredict')

      const { ok, status, text } = yield* Effect.tryPromise({
        catch: error =>
          adapterError(
            `Vertex request transport error: ${
              error instanceof Error ? error.message : String(error)
            }`,
            true,
          ),
        try: async () => {
          const response = await fetchImpl(url, {
            body: JSON.stringify(body),
            headers: {
              authorization: `Bearer ${token}`,
              'content-type': 'application/json',
            },
            method: 'POST',
          })
          return {
            ok: response.ok,
            status: response.status,
            text: await response.text(),
          }
        },
      })

      if (!ok) {
        return yield* Effect.fail(
          adapterError(
            `Vertex returned HTTP ${status}${
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
        const text = yield* call(request, 'rawPredict')
        const json = parseJsonRecord(text)
        if (json === undefined) {
          return yield* Effect.fail(
            adapterError('Vertex response was not valid JSON.', false),
          )
        }
        const stopReason = json['stop_reason']
        const result: InferenceResult = {
          content: extractText(json['content']),
          finishReason: typeof stopReason === 'string' ? stopReason : 'stop',
          servedModel:
            typeof json['model'] === 'string'
              ? (json['model'] as string)
              : resolveModelId(request.model),
          usage: parseUsage(json['usage']),
        }
        return result
      }),
    id: VERTEX_ANTHROPIC_ADAPTER_ID,
    stream: (request: InferenceRequest) =>
      Effect.gen(function* () {
        const text = yield* call(request, 'streamRawPredict')
        return parseVertexSseChunks(text, request.model, resolveModelId)
      }),
  }
}

// Parse a Vertex Anthropic streamRawPredict SSE body into our normalized
// stream chunks. Vertex streams the Anthropic Messages event protocol:
//   message_start (carries usage.input_tokens),
//   content_block_delta (delta.text deltas),
//   message_delta (delta.stop_reason + usage.output_tokens),
//   message_stop.
// We collapse this into content deltas plus one terminal frame carrying the
// receipt-first usage so the route's metering settles from real counts. (The
// route currently consumes a batched array; this preserves that contract.)
const parseVertexSseChunks = (
  body: string,
  requestedModel: string,
  resolveModelId: (model: string) => string,
): ReadonlyArray<InferenceStreamChunk> => {
  let promptTokens = 0
  let completionTokens = 0
  let cachedPromptTokens: number | undefined
  let finishReason: string | undefined
  let servedModel = resolveModelId(requestedModel)
  const contentDeltas: Array<string> = []

  const num = (value: unknown): number | undefined =>
    typeof value === 'number' && Number.isFinite(value) ? value : undefined

  for (const line of body.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed.startsWith('data:')) {
      continue
    }
    const payload = trimmed.slice('data:'.length).trim()
    if (payload === '' || payload === '[DONE]') {
      continue
    }
    const event = parseJsonRecord(payload)
    if (event === undefined) {
      continue
    }
    const type = event['type']
    if (type === 'message_start') {
      const message = event['message']
      if (
        typeof message === 'object' &&
        message !== null &&
        typeof (message as Record<string, unknown>)['model'] === 'string'
      ) {
        servedModel = (message as Record<string, unknown>)['model'] as string
      }
      const usage =
        typeof message === 'object' && message !== null
          ? (message as Record<string, unknown>)['usage']
          : undefined
      if (typeof usage === 'object' && usage !== null) {
        const u = usage as Record<string, unknown>
        promptTokens = num(u['input_tokens']) ?? promptTokens
        cachedPromptTokens = num(u['cache_read_input_tokens']) ?? cachedPromptTokens
        // message_start usage may already carry partial output_tokens.
        completionTokens = num(u['output_tokens']) ?? completionTokens
      }
    } else if (type === 'content_block_delta') {
      const delta = event['delta']
      if (typeof delta === 'object' && delta !== null) {
        const d = delta as Record<string, unknown>
        if (d['type'] === 'text_delta' && typeof d['text'] === 'string') {
          contentDeltas.push(d['text'])
        }
      }
    } else if (type === 'message_delta') {
      const delta = event['delta']
      if (typeof delta === 'object' && delta !== null) {
        const stop = (delta as Record<string, unknown>)['stop_reason']
        if (typeof stop === 'string') {
          finishReason = stop
        }
      }
      const usage = event['usage']
      if (typeof usage === 'object' && usage !== null) {
        completionTokens =
          num((usage as Record<string, unknown>)['output_tokens']) ??
          completionTokens
      }
    }
  }

  const usage: InferenceUsage = {
    completionTokens,
    promptTokens,
    totalTokens: promptTokens + completionTokens,
    ...(cachedPromptTokens === undefined ? {} : { cachedPromptTokens }),
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
    usage,
  })
  return chunks
}
