// Generic public Khala chat route (the `/khala` chat demo).
//
//   POST /api/khala/chat
//
// PUBLIC, UNAUTHENTICATED, STREAMING. Mirrors the onboarding streaming pattern
// (`autopilot-onboarding-routes.ts`): it reaches Khala over the SAME internal
// provider-adapter program path the onboarding route uses (no auth/credit gate,
// no external HTTP hop), emits incremental `event: delta` frames, optional
// provider-labeled `event: reasoning` frames, then a terminal `event: done`,
// and a terminal `event: error` on failure. The difference is
// that this is GENERIC and STATELESS: the client sends the running message list
// each turn, there is no server session row, no durable resume, and no
// persistence. The system prompt (Khala identity + generic chat instruction) is
// rebuilt server-side every turn — NOT the onboarding/concierge intake program.
//
// Abuse guard: a cheap best-effort per-IP token bucket (per-isolate, in memory)
// plus the program's message-count / per-message / total-character bounds. This
// is intentionally lightweight for a public demo; a durable cross-isolate limit
// would need a Durable Object and is out of scope here.
import { Cause, Effect, Match as M, Schema as S } from 'effect'

import {
  OnboardingInferenceError,
  type OnboardingStreamDelta,
  type OnboardingStreamMetadata,
} from './autopilot-onboarding-program'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { KHALA_STANDARD_GREETING } from './inference/khala-identity'
import {
  KHALA_CHAT_MODEL,
  KhalaChatRequest,
  KhalaChatValidationError,
  KHALA_ARTANIS_PUBLIC_READ_ONLY_ANSWER,
  buildKhalaChatRequest,
  isKhalaArtanisPublicReadOnlyTurn,
  isKhalaFastGreetingTurn,
  validateKhalaChatRequest,
} from './khala-chat-program'
import type {
  KhalaChatMessage,
  KhalaChatStreamClient,
  KhalaChatStreamSource,
} from './khala-chat-program'
import {
  type KhalaChatPylonContext,
  answerKhalaChatPylonQuestion,
} from './khala-chat-pylon-context'
import { logWorkerRouteError } from './observability'
import { compactRandomId, currentEpochMillis } from './runtime-primitives'

type HttpResponse = globalThis.Response
type KhalaChatRouteEffect = Effect.Effect<HttpResponse>

class KhalaChatBadRequest extends S.TaggedErrorClass<KhalaChatBadRequest>()(
  'KhalaChatBadRequest',
  {
    reason: S.String,
  },
) {}

class KhalaChatRateLimited extends S.TaggedErrorClass<KhalaChatRateLimited>()(
  'KhalaChatRateLimited',
  {},
) {}

type KhalaChatRouteError =
  | KhalaChatBadRequest
  | KhalaChatRateLimited
  | KhalaChatValidationError
  | OnboardingInferenceError

export type KhalaChatRouteDependencies = Readonly<{
  // Resolves the STREAMING inference client for the request env. Production wires
  // this to the same provider-adapter registry + overflow dispatch the gateway
  // and the onboarding route use (the internal Khala program path); tests inject
  // a deterministic stub.
  makeStreamClient: (env: unknown) => KhalaChatStreamClient
  loadPylonContext?:
    | ((env: unknown) => Effect.Effect<KhalaChatPylonContext, unknown>)
    | undefined
  // Overridable for tests; defaults to a per-IP token bucket. Returns false when
  // the caller is over budget.
  rateLimit?: ((request: Request) => boolean) | undefined
  recordServedTokens?:
    | ((input: {
        env: unknown
        metadata: OnboardingStreamMetadata
        traceRef: string
      }) => Effect.Effect<void, unknown>)
    | undefined
}>

// A narrow, self-describing SSE wire (same shape as the onboarding stream):
//   event: delta  data: { "text": "…" }   (one per content increment)
//   event: reasoning data: { "text": "…" } (provider-labeled reasoning only)
//   event: done   data: { "done": true }  (terminal, once)
//   event: error  data: { "error": "…" }  (terminal, on failure)
const sseFrame = (event: string, payload: unknown): string =>
  `event: ${event}\ndata: ${JSON.stringify(payload)}\n\n`

const STREAM_HEADERS: Readonly<Record<string, string>> = {
  'content-type': 'text/event-stream; charset=utf-8',
  'cache-control': 'no-store',
  connection: 'keep-alive',
  'x-accel-buffering': 'no',
}

const decodeJsonBody = (request: Request) =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: error =>
        new KhalaChatBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
    })

    return yield* S.decodeUnknownEffect(KhalaChatRequest)(payload).pipe(
      Effect.mapError(
        error => new KhalaChatBadRequest({ reason: String(error) }),
      ),
    )
  })

const routeErrorResponse = (error: KhalaChatRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      KhalaChatBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      KhalaChatValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'validation_error', reason },
          { status: 400 },
        ),
      KhalaChatRateLimited: () =>
        noStoreJsonResponse({ error: 'rate_limited' }, { status: 429 }),
      OnboardingInferenceError: ({ reason }) =>
        noStoreJsonResponse(
          {
            code: 'inference_unavailable',
            error: 'inference_unavailable',
            reason,
          },
          { status: 502 },
        ),
    }),
    M.exhaustive,
  )

const withTraceHeader = (
  response: HttpResponse,
  traceRef: string,
): HttpResponse => {
  const headers = new Headers(response.headers)
  headers.set('x-openagents-trace-ref', traceRef)
  return new Response(response.body, {
    headers,
    status: response.status,
    statusText: response.statusText,
  })
}

const traceRef = (): string => `trace.khala_chat.${compactRandomId('req')}`

const logKhalaChatFailure = (
  trace: string,
  stage: string,
  error: unknown,
): void => {
  const reason =
    typeof error === 'object' && error !== null && 'reason' in error
      ? String((error as { reason?: unknown }).reason)
      : error instanceof Error
        ? error.message
        : String(error)
  logWorkerRouteError('khala_chat_inference_failure', error, {
    reason,
    stage,
    traceRef: trace,
  })
}

const streamDeltaEvent = (
  delta: OnboardingStreamDelta,
): Readonly<{ event: 'delta' | 'reasoning'; text: string }> => {
  if (typeof delta === 'string') {
    return { event: 'delta', text: delta }
  }
  return {
    event: delta.kind === 'reasoning' ? 'reasoning' : 'delta',
    text: delta.text,
  }
}

const fastGreetingSource = (): KhalaChatStreamSource => ({
  deltas: (async function* () {
    yield KHALA_STANDARD_GREETING
  })(),
  final: () => KHALA_STANDARD_GREETING,
  metadata: () => ({
    finishReason: 'fast_greeting',
    requestedModel: KHALA_CHAT_MODEL,
    servedAdapterId: 'khala-fast-path',
    servedModel: 'khala-fast-greeting',
    usage: {
      completionTokens: Math.ceil(KHALA_STANDARD_GREETING.length / 4),
      promptTokens: 0,
      totalTokens: Math.ceil(KHALA_STANDARD_GREETING.length / 4),
    },
  }),
})

const staticTextSource = (
  text: string,
  finishReason: string,
  servedAdapterId: string,
  servedModel: string,
): KhalaChatStreamSource => ({
  deltas: (async function* () {
    yield text
  })(),
  final: () => text,
  metadata: () => ({
    finishReason,
    requestedModel: KHALA_CHAT_MODEL,
    servedAdapterId,
    servedModel,
    usage: {
      completionTokens: Math.ceil(text.length / 4),
      promptTokens: 0,
      totalTokens: Math.ceil(text.length / 4),
    },
  }),
})

const artanisPublicReadOnlySource = (): KhalaChatStreamSource => ({
  deltas: (async function* () {
    yield KHALA_ARTANIS_PUBLIC_READ_ONLY_ANSWER
  })(),
  final: () => KHALA_ARTANIS_PUBLIC_READ_ONLY_ANSWER,
  metadata: () => ({
    finishReason: 'artanis_public_read_only',
    requestedModel: KHALA_CHAT_MODEL,
    servedAdapterId: 'khala-artanis-read-only-signature',
    servedModel: 'khala-artanis-public-signature',
    usage: {
      completionTokens: Math.ceil(KHALA_ARTANIS_PUBLIC_READ_ONLY_ANSWER.length / 4),
      promptTokens: 0,
      totalTokens: Math.ceil(KHALA_ARTANIS_PUBLIC_READ_ONLY_ANSWER.length / 4),
    },
  }),
})

const latestUserContent = (messages: ReadonlyArray<KhalaChatMessage>): string =>
  messages[messages.length - 1]?.content ?? ''

const loadPylonContext = (
  dependencies: KhalaChatRouteDependencies,
  env: unknown,
  trace: string,
): Effect.Effect<KhalaChatPylonContext | undefined> => {
  if (dependencies.loadPylonContext === undefined) {
    return Effect.sync((): KhalaChatPylonContext | undefined => undefined)
  }

  return dependencies
    .loadPylonContext(env)
    .pipe(
      Effect.catch(error =>
        Effect.sync(() =>
          logKhalaChatFailure(trace, 'pylon_context', error),
        ).pipe(Effect.as(undefined)),
      ),
    )
}

// Build the SSE response body for a prepared streaming turn. Pumps prose deltas,
// then emits the terminal `done` frame. A failure mid-stream emits a terminal
// `error` frame and closes — the client never hangs. Stateless: there is no
// finalize/persist step (unlike onboarding); the client owns the transcript.
const makeKhalaChatStreamBody = (
  source: {
    readonly deltas: AsyncIterable<OnboardingStreamDelta>
    readonly metadata?: (() => OnboardingStreamMetadata | undefined) | undefined
  },
  trace: string,
  recordServedTokens:
    | ((input: {
        env: unknown
        metadata: OnboardingStreamMetadata
        traceRef: string
      }) => Effect.Effect<void, unknown>)
    | undefined,
  env: unknown,
): ReadableStream<Uint8Array> => {
  const encoder = new TextEncoder()
  return new ReadableStream<Uint8Array>({
    async start(controller) {
      const emit = (frame: string): void =>
        controller.enqueue(encoder.encode(frame))

      try {
        for await (const delta of source.deltas) {
          const payload = streamDeltaEvent(delta)
          if (payload.text !== '') {
            emit(sseFrame(payload.event, { text: payload.text }))
          }
        }
        const metadata = source.metadata?.()
        if (metadata !== undefined) {
          const recordEffect = recordServedTokens?.({
            env,
            metadata,
            traceRef: trace,
          })
          if (recordEffect !== undefined) {
            await Effect.runPromise(
              recordEffect.pipe(
                Effect.catchCause(cause =>
                  Effect.sync(() =>
                    logKhalaChatFailure(
                      trace,
                      'served_tokens_record',
                      Cause.pretty(cause),
                    ),
                  ),
                ),
              ),
            )
          }
          emit(sseFrame('meta', { ...metadata, traceRef: trace }))
        }
        emit(sseFrame('done', { done: true }))
      } catch (error) {
        logKhalaChatFailure(trace, 'stream', error)
        emit(
          sseFrame('error', {
            code: 'stream_failed',
            error: 'stream_failed',
            reason: error instanceof Error ? error.message : String(error),
            traceRef: trace,
          }),
        )
      } finally {
        controller.close()
      }
    },
  })
}

// A cheap, best-effort per-IP token bucket. In-memory and per-isolate (Workers
// spins up many isolates, so this is NOT a hard global limit — it only blunts a
// single hot isolate). A durable global limit would need a Durable Object; out
// of scope for a public demo. Refills `CAPACITY` tokens over `WINDOW_MS`.
const RATE_CAPACITY = 20
const RATE_WINDOW_MS = 60_000

type Bucket = { tokens: number; updatedAt: number }
const buckets = new Map<string, Bucket>()

const clientIp = (request: Request): string =>
  request.headers.get('cf-connecting-ip') ??
  request.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ??
  'unknown'

const defaultRateLimit = (request: Request): boolean => {
  const ip = clientIp(request)
  const now = currentEpochMillis()
  const existing = buckets.get(ip)
  const refillRate = RATE_CAPACITY / RATE_WINDOW_MS

  if (existing === undefined) {
    buckets.set(ip, { tokens: RATE_CAPACITY - 1, updatedAt: now })
    return true
  }

  const refilled = Math.min(
    RATE_CAPACITY,
    existing.tokens + (now - existing.updatedAt) * refillRate,
  )
  if (refilled < 1) {
    buckets.set(ip, { tokens: refilled, updatedAt: now })
    return false
  }
  buckets.set(ip, { tokens: refilled - 1, updatedAt: now })
  return true
}

export const makeKhalaChatRoutes = (
  dependencies: KhalaChatRouteDependencies,
) => {
  const rateLimit = dependencies.rateLimit ?? defaultRateLimit

  // The streaming turn: decode + validate + rate-limit on the request path so a
  // bad/over-budget request maps to a clean JSON error BEFORE any stream byte is
  // committed; then return a ReadableStream that pumps prose deltas and emits
  // the terminal `done`. A failure mid-prepare returns a JSON error; a failure
  // mid-stream emits a terminal `error` SSE frame.
  const streamResponse = (
    request: Request,
    env: unknown,
  ): KhalaChatRouteEffect => {
    const requestTraceRef = traceRef()
    if (!rateLimit(request)) {
      return Effect.succeed(
        withTraceHeader(
          routeErrorResponse(new KhalaChatRateLimited({})),
          requestTraceRef,
        ),
      )
    }

    const streamClient = dependencies.makeStreamClient(env)

    return decodeJsonBody(request).pipe(
      Effect.flatMap(validateKhalaChatRequest),
      Effect.flatMap(messages =>
        isKhalaFastGreetingTurn(messages)
          ? Effect.succeed(fastGreetingSource())
          : isKhalaArtanisPublicReadOnlyTurn(messages)
            ? Effect.succeed(artanisPublicReadOnlySource())
          : loadPylonContext(dependencies, env, requestTraceRef).pipe(
              Effect.flatMap(pylonContext => {
                const pylonAnswer = answerKhalaChatPylonQuestion(
                  latestUserContent(messages),
                  pylonContext,
                )

                return pylonAnswer !== null
                  ? Effect.succeed(
                      staticTextSource(
                        pylonAnswer,
                        'pylon_context',
                        'khala-pylon-context',
                        'openagents-pylon-registry',
                      ),
                    )
                  : Effect.succeed(
                      buildKhalaChatRequest(messages, { pylonContext }),
                    ).pipe(
                      Effect.flatMap(inferenceRequest =>
                        streamClient(inferenceRequest),
                      ),
                    )
              }),
            ),
      ),
      Effect.map(
        source =>
          new Response(
            makeKhalaChatStreamBody(
              source,
              requestTraceRef,
              dependencies.recordServedTokens,
              env,
            ),
            {
              headers: {
                ...STREAM_HEADERS,
                'x-openagents-trace-ref': requestTraceRef,
              },
            },
          ),
      ),
      Effect.catch(error => {
        if (error instanceof OnboardingInferenceError) {
          logKhalaChatFailure(requestTraceRef, 'open', error)
        }
        return Effect.succeed(
          withTraceHeader(routeErrorResponse(error), requestTraceRef),
        )
      }),
    )
  }

  return {
    routeKhalaChatRequest: (
      request: Request,
      env: unknown,
    ): KhalaChatRouteEffect | undefined => {
      const url = new URL(request.url)
      if (url.pathname !== '/api/khala/chat') {
        return undefined
      }
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }
      return streamResponse(request, env)
    },
  }
}

// Re-exported for tests that assert the SSE wire shape directly.
export const khalaChatSseFrame = sseFrame
