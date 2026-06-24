// Onboarding turn route (EPIC #6123, issue #6126).
//
//   POST /api/autopilot/onboarding/{sessionId}/turn
//
// Owns a persisted onboarding SESSION in D1 and advances the productized intake
// interview one turn at a time over the Khala inference orchestrator (the
// OpenAI-compatible `/v1/chat/completions` gateway, model
// `openagents/khala-mini`). The handler is transport-agnostic: it decodes the
// text turn from JSON today and hands a typed `OnboardingTurnInput` to the pure
// driver; a voice front-end (STT -> this route -> TTS) can reuse the same driver
// without changing it.

import { Effect, Match as M, Schema as S } from 'effect'

import {
  AUTOPILOT_CONCIERGE_VERTICALS,
  type AutopilotConciergeVertical,
} from './inference/autopilot-concierge-model'
import {
  type DurableStreamNamespace,
  replayFromOffsetDO,
  teeUpstreamToDurableDO,
} from './inference/durable-inference-do-transport'
import { resolveOnboardingPromptVertical } from './autopilot-onboarding-system-prompt'
import {
  type OnboardingInferenceClient,
  OnboardingInferenceError,
  type OnboardingSessionStore,
  type OnboardingStreamClient,
  type OnboardingStreamTurn,
  OnboardingStorageError,
  OnboardingTurnRequest,
  type OnboardingTurnResponse,
  OnboardingValidationError,
  finalizeOnboardingStreamTurn,
  makeD1OnboardingSessionStore,
  prepareOnboardingStreamTurn,
  runOnboardingTurn,
} from './autopilot-onboarding-program'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'

type OnboardingRouteEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

// Alias the env behind a generic indirection so this route module stays off the
// raw Cloudflare-Env zero-debt ratchet (mirrors agent-goal-routes.ts WorkerEnv).
type WorkerEnv<Env extends OnboardingRouteEnv> = Env

type HttpResponse = globalThis.Response
type OnboardingRouteEffect = Effect.Effect<HttpResponse>

class OnboardingBadRequest extends S.TaggedErrorClass<OnboardingBadRequest>()(
  'OnboardingBadRequest',
  {
    reason: S.String,
  },
) {}

type OnboardingRouteError =
  | OnboardingBadRequest
  | OnboardingInferenceError
  | OnboardingStorageError
  | OnboardingValidationError

export type OnboardingRouteDependencies<Env extends OnboardingRouteEnv> =
  Readonly<{
    // Resolves the inference client for the request env. Production wires this to
    // the provider-adapter registry + overflow dispatch (no external HTTP hop);
    // tests inject a stub.
    makeInferenceClient: (env: WorkerEnv<Env>) => OnboardingInferenceClient
    // Resolves the STREAMING inference client for the request env (the SSE path).
    // Optional: when absent, the route serves the buffered JSON path even for an
    // `Accept: text/event-stream` request (fail-safe — no behaviour regression).
    makeStreamClient?:
      | ((env: WorkerEnv<Env>) => OnboardingStreamClient)
      | undefined
    // Overridable for tests; defaults to the D1-backed store.
    makeStore?: ((env: WorkerEnv<Env>) => OnboardingSessionStore) | undefined
    // DURABLE ONBOARDING STREAM (#6154 item 4). Resolves the per-request Durable
    // Object namespace when the durable-stream flag is on AND the binding is
    // wired; absent => the onboarding stream is the non-durable SSE (fail-safe).
    // When present, each onboarding turn stream is teed into a durable log keyed
    // by the STABLE id `onboarding:{sessionId}:{turnIndex}`, so an unauthenticated
    // browser holding only `sessionId` + last offset can resume a dropped turn.
    resolveDurableStream?:
      | ((env: WorkerEnv<Env>) => DurableStreamNamespace | undefined)
      | undefined
    nowIso?: (() => string) | undefined
  }>

const decodeJsonBody = <Schema extends S.Top>(
  request: Request,
  schema: Schema,
) =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: error =>
        new OnboardingBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
    })

    return yield* S.decodeUnknownEffect(schema)(payload).pipe(
      Effect.mapError(error => new OnboardingBadRequest({ reason: String(error) })),
    )
  })

const routeErrorResponse = (error: OnboardingRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      OnboardingBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      OnboardingValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'validation_error', reason },
          { status: 400 },
        ),
      OnboardingInferenceError: () =>
        noStoreJsonResponse(
          { error: 'inference_unavailable' },
          { status: 502 },
        ),
      OnboardingStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
    }),
    M.exhaustive,
  )

// A narrow SSE frame for the onboarding stream. We do NOT reuse the
// OpenAI-compatible chat-completions wire here: the onboarding client only needs
// prose deltas + a terminal payload, so the wire is intentionally minimal and
// self-describing (the client's `parseComponentFrames`-style splitter reads it):
//   event: stream  data: { "streamId", "sessionId", "turnIndex" }  (handshake, first)
//   event: delta   data: { "text": "…" }     (one per content increment)
//   event: done    data: <OnboardingTurnResponse JSON>   (terminal, once)
//   event: error   data: { "error": "…" }     (terminal, on failure)
const sseFrame = (event: string, payload: unknown): string =>
  `event: ${event}\ndata: ${JSON.stringify(payload)}\n\n`

// The STABLE durable log key for an onboarding turn (#6154 item 4). Derivable by
// the browser from `sessionId` + the turn index (the session's `turnCount` BEFORE
// this turn finalizes), so a returning unauthenticated browser only needs to
// persist `sessionId` + last `offset` — it never has to invent a requestId.
export const onboardingDurableStreamId = (
  sessionId: string,
  turnIndex: number,
): string => `onboarding:${sessionId}:${turnIndex}`

// The public resume read URL for an in-flight onboarding turn (#6154 item 4).
// Keyed by (sessionId, turnIndex) — both of which the browser holds — plus the
// last `offset`. No prompt/credential material is in the path.
export const onboardingResumeUrl = (
  sessionId: string,
  turnIndex: number,
): string =>
  `/api/autopilot/onboarding/${encodeURIComponent(sessionId)}/turn/${turnIndex}/stream`

const STREAM_HEADERS: Readonly<Record<string, string>> = {
  'content-type': 'text/event-stream; charset=utf-8',
  'cache-control': 'no-store',
  connection: 'keep-alive',
  'x-accel-buffering': 'no',
}

const wantsEventStream = (request: Request): boolean => {
  const accept = request.headers.get('accept') ?? ''
  return accept.toLowerCase().includes('text/event-stream')
}

// The durable substrate for one prepared onboarding turn: the per-request DO
// namespace + the STABLE stream id derived from `sessionId` + turn index. Absent
// => the stream is the non-durable SSE (today's behaviour, fail-safe).
type OnboardingDurable = Readonly<{
  namespace: DurableStreamNamespace
  streamId: string
  sessionId: string
  turnIndex: number
}>

// Build the SSE response body for a prepared streaming turn. Pumps prose deltas,
// then calls `finalize` (a plain Promise so the effect boundary stays OUT of the
// Effect.gen request pipeline) to append + persist and resolve the terminal
// payload, then emits the `done` frame. A failure at any point emits a terminal
// `error` frame and closes — the client never hangs.
//
// DURABLE (#6154 item 4): when `durable` is present, the turn is teed into the
// per-request Durable Object via the SAME `teeUpstreamToDurableDO` the gateway
// uses, persisting the onboarding `delta`/`done` frames so a mid-turn disconnect
// can resume by offset. An `event: stream` handshake frame is emitted FIRST (not
// persisted — it is metadata the resuming browser already holds) carrying the
// streamId + sessionId + turnIndex so the browser can persist them.
const makeOnboardingStreamBody = (
  turn: OnboardingStreamTurn,
  finalize: (reply: string) => Promise<OnboardingTurnResponse>,
  durable: OnboardingDurable | undefined,
): ReadableStream<Uint8Array> => {
  const encoder = new TextEncoder()
  return new ReadableStream<Uint8Array>({
    async start(controller) {
      const emit = (frame: string): void =>
        controller.enqueue(encoder.encode(frame))

      // HANDSHAKE: tell the client the durable stream id + keys FIRST, before any
      // delta, so a refresh/navigate-away can resume by (sessionId, turnIndex,
      // offset). Emitted live only — not part of the durable log.
      if (durable !== undefined) {
        emit(
          sseFrame('stream', {
            streamId: durable.streamId,
            sessionId: durable.sessionId,
            turnIndex: durable.turnIndex,
          }),
        )
      }

      // The non-durable path: pump deltas straight to the client (today's wire).
      if (durable === undefined) {
        let accumulated = ''
        try {
          for await (const delta of turn.source.deltas) {
            if (delta !== '') {
              accumulated += delta
              emit(sseFrame('delta', { text: delta }))
            }
          }
          const final = turn.source.final()
          const reply = final === '' ? accumulated : final
          const result = await finalize(reply)
          emit(sseFrame('done', result))
        } catch {
          emit(sseFrame('error', { error: 'stream_failed' }))
        } finally {
          controller.close()
        }
        return
      }

      // The durable path: adapt the onboarding source into an `InferenceStreamSource`
      // and tee through the DO transport. `frameForDelta` renders the onboarding
      // `delta` wire; `onEof` finalizes (append + persist the session) and renders
      // the terminal `done` wire. Both the delta frames and the `done` frame are
      // persisted to the durable log, so a resume replays the byte-identical wire.
      // The session transcript append happens ONCE here in `onEof` (the live
      // producer drain), never on a resume read.
      let accumulated = ''
      const adapted = {
        frames: (async function* () {
          for await (const delta of turn.source.deltas) {
            if (delta !== '') {
              accumulated += delta
              yield { contentDelta: delta }
            }
          }
        })(),
        terminal: () => ({
          finishReason: 'stop' as string | undefined,
          servedModel: undefined,
          usage: undefined,
        }),
      }

      try {
        await teeUpstreamToDurableDO({
          emit,
          frameForDelta: delta => sseFrame('delta', { text: delta }),
          namespace: durable.namespace,
          onEof: async () => {
            const final = turn.source.final()
            const reply = final === '' ? accumulated : final
            const result = await finalize(reply)
            return sseFrame('done', result)
          },
          requestId: durable.streamId,
          source: adapted,
        })
      } catch {
        emit(sseFrame('error', { error: 'stream_failed' }))
      } finally {
        controller.close()
      }
    },
  })
}

// Construct the streaming `Response` for a prepared turn. The finalize step
// (append + persist) is a plain Promise driven by `Effect.runPromise` HERE — at
// the streaming boundary, outside any request Effect pipeline — so the SSE pump
// owns its own effect run.
const buildStreamResponse = (
  turn: OnboardingStreamTurn,
  store: OnboardingSessionStore,
  nowIso: () => string,
  durable: OnboardingDurable | undefined,
): globalThis.Response => {
  const finalize = (reply: string): Promise<OnboardingTurnResponse> =>
    Effect.runPromise(finalizeOnboardingStreamTurn(turn, reply, { store, nowIso }))

  return new Response(makeOnboardingStreamBody(turn, finalize, durable), {
    headers:
      durable === undefined
        ? STREAM_HEADERS
        : {
            ...STREAM_HEADERS,
            // Advertise the durable resume read URL + the stable stream id so the
            // browser can resume even if it missed the `event: stream` frame.
            'openagents-onboarding-stream-id': durable.streamId,
            'openagents-onboarding-resume-url': onboardingResumeUrl(
              durable.sessionId,
              durable.turnIndex,
            ),
          },
  })
}

const errorReason = (error: OnboardingRouteError): string =>
  M.value(error).pipe(
    M.tags({
      OnboardingBadRequest: () => 'bad_request',
      OnboardingValidationError: () => 'validation_error',
      OnboardingInferenceError: () => 'inference_unavailable',
      OnboardingStorageError: () => 'storage_error',
    }),
    M.exhaustive,
  )

const resolveRequestedVertical = (
  body: OnboardingTurnRequest,
): Effect.Effect<AutopilotConciergeVertical, OnboardingBadRequest> => {
  const explicit = body.vertical?.trim().toLowerCase()
  if (explicit !== undefined && explicit !== '') {
    if (
      AUTOPILOT_CONCIERGE_VERTICALS.includes(
        explicit as AutopilotConciergeVertical,
      )
    ) {
      return Effect.succeed(explicit as AutopilotConciergeVertical)
    }
    return Effect.fail(
      new OnboardingBadRequest({
        reason: `invalid vertical; allowed: ${AUTOPILOT_CONCIERGE_VERTICALS.join(', ')}`,
      }),
    )
  }

  return Effect.succeed(
    resolveOnboardingPromptVertical(body.verticalOverlay ?? null),
  )
}

export const makeAutopilotOnboardingRoutes = <Env extends OnboardingRouteEnv>(
  dependencies: OnboardingRouteDependencies<Env>,
) => {
  const nowIso = dependencies.nowIso ?? currentIsoTimestamp

  const resolveStore = (env: WorkerEnv<Env>): OnboardingSessionStore =>
    dependencies.makeStore?.(env) ??
    makeD1OnboardingSessionStore(openAgentsDatabase(env))

  const turnResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    sessionId: string,
  ): OnboardingRouteEffect =>
    Effect.gen(function* () {
      const body = yield* decodeJsonBody(request, OnboardingTurnRequest)
      const vertical = yield* resolveRequestedVertical(body)

      const result = yield* runOnboardingTurn(
        {
          sessionId,
          userText: body.userText,
          vertical,
        },
        {
          infer: dependencies.makeInferenceClient(env),
          nowIso,
          store: resolveStore(env),
        },
      )

      return noStoreJsonResponse(result)
    }).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )

  // The SSE streaming path. Prepares the turn (validate + read/create session +
  // open the stream) on the request path so validation/storage failures map to a
  // clean error BEFORE any stream byte is committed; then returns a ReadableStream
  // that pumps prose deltas, finalizes (append + persist), and emits the terminal
  // `done` payload. A failure mid-prepare returns a normal JSON error response; a
  // failure mid-stream emits a terminal `error` SSE frame and closes.
  const streamResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    sessionId: string,
    makeStreamClient: (env: WorkerEnv<Env>) => OnboardingStreamClient,
  ): OnboardingRouteEffect => {
    const store = resolveStore(env)
    const streamClient = makeStreamClient(env)
    const namespace = dependencies.resolveDurableStream?.(env)

    return decodeJsonBody(request, OnboardingTurnRequest).pipe(
      Effect.flatMap(body =>
        Effect.map(resolveRequestedVertical(body), vertical => ({
          body,
          vertical,
        })),
      ),
      Effect.flatMap(({ body, vertical }) =>
        prepareOnboardingStreamTurn(
          {
            sessionId,
            userText: body.userText,
            vertical,
          },
          { nowIso, store, stream: streamClient },
        ),
      ),
      // The Response construction is a PURE transform of the prepared turn; the
      // SSE pump + finalize (a plain Promise via `Effect.runPromise`) run AFTER
      // this request Effect resolves, so the effect boundary stays out of the
      // request pipeline. The durable substrate (when wired) keys the per-turn
      // log by the STABLE id `onboarding:{sessionId}:{turnIndex}`, where the turn
      // index is the session's PRE-turn `turnCount` (so a resume read can derive
      // it from `sessionId` + the count it last saw).
      Effect.map(turn =>
        buildStreamResponse(
          turn,
          store,
          nowIso,
          namespace === undefined
            ? undefined
            : {
                namespace,
                sessionId,
                streamId: onboardingDurableStreamId(
                  sessionId,
                  turn.session.turnCount,
                ),
                turnIndex: turn.session.turnCount,
              },
        ),
      ),
      // A prepare-time failure (validation / storage / inference open) becomes a
      // clean JSON error — the stream was never started.
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  }

  // RESUME READ (#6154 item 4): replay an in-flight (or completed) onboarding
  // turn's durable log from `?offset=`, keyed by (sessionId, turnIndex). The
  // browser calls this on reload with only `sessionId` + last `offset`. NEVER
  // meters (onboarding is free; this only reads stored bytes). Returns 404 when
  // the durable substrate is unwired or the turn has no durable log.
  const resumeReadResponse = (
    env: WorkerEnv<Env>,
    sessionId: string,
    turnIndex: number,
    offset: string | undefined,
  ): OnboardingRouteEffect => {
    const namespace = dependencies.resolveDurableStream?.(env)
    if (namespace === undefined) {
      return Effect.succeed(
        noStoreJsonResponse({ error: 'not_found' }, { status: 404 }),
      )
    }
    return Effect.promise(async () => {
      // A missing/empty durable log resolves to `undefined` (→ 404), NOT a throw
      // (the DO SQL adapter reads its single metadata row tolerantly). A throw
      // here is therefore a genuine DO transport fault; map it to a clean 502
      // rather than an unhandled 500 defect. Onboarding is free, so this read
      // never meters regardless.
      let replay: Awaited<ReturnType<typeof replayFromOffsetDO>>
      try {
        replay = await replayFromOffsetDO({
          namespace,
          offset,
          requestId: onboardingDurableStreamId(sessionId, turnIndex),
        })
      } catch {
        return noStoreJsonResponse(
          { error: 'durable_stream_unavailable' },
          { status: 502 },
        )
      }
      if (replay === undefined) {
        return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
      }
      if (replay.status === 400) {
        return noStoreJsonResponse(
          { error: 'invalid_offset' },
          { status: 400 },
        )
      }
      const headers: Record<string, string> = {
        'cache-control': 'no-store',
        'content-type': replay.contentType,
        'stream-next-offset': replay.nextOffset,
      }
      if (replay.upToDate) {
        headers['stream-up-to-date'] = 'true'
      }
      if (replay.streamClosed) {
        headers['stream-closed'] = 'true'
      }
      return new Response(replay.body, { headers, status: 200 })
    })
  }

  // GET SESSION (#6154 item 4): the persisted transcript + status + outputSpec +
  // turnCount for a `sessionId`, so a returning browser can rehydrate past
  // messages (even if a turn completed server-side while the tab was gone). Read
  // only — no mutation, no metering. 404 when the session does not exist.
  const sessionResponse = (
    env: WorkerEnv<Env>,
    sessionId: string,
  ): OnboardingRouteEffect =>
    resolveStore(env)
      .read(sessionId)
      .pipe(
        Effect.map(session =>
          session === undefined
            ? noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
            : noStoreJsonResponse({
                sessionId: session.id,
                status: session.status,
                turnCount: session.turnCount,
                transcript: session.transcript,
                outputSpec: session.outputSpec,
              }),
        ),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )

  return {
    routeOnboardingTurnRequest: (
      request: Request,
      env: WorkerEnv<Env>,
    ): OnboardingRouteEffect | undefined => {
      const url = new URL(request.url)

      // RESUME READ: GET /api/autopilot/onboarding/{sessionId}/turn/{turnIndex}/stream?offset=
      const resumeMatch =
        /^\/api\/autopilot\/onboarding\/([^/]+)\/turn\/(\d+)\/stream$/.exec(
          url.pathname,
        )
      if (resumeMatch !== null) {
        if (request.method !== 'GET') {
          return Effect.succeed(methodNotAllowed(['GET']))
        }
        return resumeReadResponse(
          env,
          decodeURIComponent(resumeMatch[1] ?? ''),
          Number(resumeMatch[2] ?? '0'),
          url.searchParams.get('offset') ?? undefined,
        )
      }

      // TURN: POST /api/autopilot/onboarding/{sessionId}/turn
      const match =
        /^\/api\/autopilot\/onboarding\/([^/]+)\/turn$/.exec(url.pathname)

      if (match !== null) {
        if (request.method !== 'POST') {
          return Effect.succeed(methodNotAllowed(['POST']))
        }

        const sessionId = decodeURIComponent(match[1] ?? '')
        const makeStreamClient = dependencies.makeStreamClient

        // Content-negotiated: an `Accept: text/event-stream` request streams when
        // a stream client is wired; otherwise the buffered JSON path serves (and
        // is the fail-safe default whenever streaming is unavailable).
        return wantsEventStream(request) && makeStreamClient !== undefined
          ? streamResponse(request, env, sessionId, makeStreamClient)
          : turnResponse(request, env, sessionId)
      }

      // GET SESSION: GET /api/autopilot/onboarding/{sessionId}
      const sessionMatch =
        /^\/api\/autopilot\/onboarding\/([^/]+)$/.exec(url.pathname)
      if (sessionMatch !== null) {
        if (request.method !== 'GET') {
          return Effect.succeed(methodNotAllowed(['GET']))
        }
        return sessionResponse(env, decodeURIComponent(sessionMatch[1] ?? ''))
      }

      return undefined
    },
  }
}

// Re-exported for tests that assert the SSE wire shape directly.
export const onboardingSseFrame = sseFrame
export const onboardingStreamError = errorReason
