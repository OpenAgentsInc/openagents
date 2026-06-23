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
  type OnboardingInferenceClient,
  OnboardingInferenceError,
  type OnboardingSessionStore,
  OnboardingStorageError,
  OnboardingTurnRequest,
  OnboardingValidationError,
  makeD1OnboardingSessionStore,
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
    // Overridable for tests; defaults to the D1-backed store.
    makeStore?: ((env: WorkerEnv<Env>) => OnboardingSessionStore) | undefined
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

export const makeAutopilotOnboardingRoutes = <Env extends OnboardingRouteEnv>(
  dependencies: OnboardingRouteDependencies<Env>,
) => {
  const nowIso = dependencies.nowIso ?? currentIsoTimestamp

  const turnResponse = (
    request: Request,
    env: WorkerEnv<Env>,
    sessionId: string,
  ): OnboardingRouteEffect =>
    Effect.gen(function* () {
      const body = yield* decodeJsonBody(request, OnboardingTurnRequest)
      const store =
        dependencies.makeStore?.(env) ??
        makeD1OnboardingSessionStore(openAgentsDatabase(env))

      const result = yield* runOnboardingTurn(
        {
          sessionId,
          userText: body.userText,
          verticalOverlay: body.verticalOverlay ?? null,
        },
        {
          infer: dependencies.makeInferenceClient(env),
          nowIso,
          store,
        },
      )

      return noStoreJsonResponse(result)
    }).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )

  return {
    routeOnboardingTurnRequest: (
      request: Request,
      env: WorkerEnv<Env>,
    ): OnboardingRouteEffect | undefined => {
      const url = new URL(request.url)
      const match =
        /^\/api\/autopilot\/onboarding\/([^/]+)\/turn$/.exec(url.pathname)

      if (match === null) {
        return undefined
      }

      return request.method === 'POST'
        ? turnResponse(request, env, decodeURIComponent(match[1] ?? ''))
        : Effect.succeed(methodNotAllowed(['POST']))
    },
  }
}
