import { Effect, Match as M, Schema as S } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import { liveAtReadStaleness } from './public-projection-staleness'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  type TrainingVerificationChallengeRecord,
  TrainingVerificationChallengeCreateRequest,
  TrainingVerificationChallengeFinalizeRequest,
  TrainingVerificationChallengeLeaseRequest,
  TrainingVerificationChallengeRetryRequest,
  type TrainingVerificationClass,
  type TrainingVerificationRegistration,
  type TrainingVerificationStore,
  TrainingVerificationStoreError,
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
  publicTrainingVerificationChallengeProjection,
  retryTrainingVerificationChallengeRecord,
  runTrainingVerificationClass,
  timeOutTrainingVerificationChallengeRecord,
  trainingVerificationStoreErrorFromUnknown,
} from './training-verification'

type HttpResponse = globalThis.Response

type TrainingVerificationRouteEnv = Readonly<Record<string, unknown>>

type TrainingVerificationRouteDependencies<Bindings> = Readonly<{
  makeId?: () => string
  makeStore: (env: Bindings) => TrainingVerificationStore
  nowIso?: () => string
  registry?: ReadonlyMap<
    TrainingVerificationClass,
    TrainingVerificationRegistration
  >
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
}>

class TrainingVerificationUnauthorized extends S.TaggedErrorClass<TrainingVerificationUnauthorized>()(
  'TrainingVerificationUnauthorized',
  {},
) {}

type TrainingVerificationRouteError =
  | TrainingVerificationStoreError
  | TrainingVerificationUnauthorized

const routeErrorResponse = (
  error: TrainingVerificationRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      TrainingVerificationStoreError: storeError =>
        noStoreJsonResponse(
          {
            error: `training_verification_${storeError.kind}`,
            reason: storeError.reason,
          },
          {
            status:
              storeError.kind === 'conflict'
                ? 409
                : storeError.kind === 'forbidden'
                  ? 403
                  : storeError.kind === 'not_found'
                    ? 404
                    : storeError.kind === 'storage_error'
                      ? 500
                      : 400,
          },
        ),
      TrainingVerificationUnauthorized: () => unauthorized(),
    }),
    M.exhaustive,
  )

const decodeBody = <A>(
  request: Request,
  schema: S.Decoder<A>,
): Effect.Effect<A, TrainingVerificationStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new TrainingVerificationStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      decodeUnknownWithSchema(schema, await readJsonObject(request)),
  })

const routeNowIso = <Bindings>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

const requireAdmin = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, TrainingVerificationUnauthorized> =>
  Effect.tryPromise({
    catch: () => new TrainingVerificationUnauthorized({}),
    try: () =>
      dependencies.requireAdminApiToken?.(request, env) ??
      Promise.resolve(false),
  }).pipe(
    Effect.flatMap(isAdmin =>
      isAdmin
        ? Effect.void
        : Effect.fail(new TrainingVerificationUnauthorized({})),
    ),
  )

const routeCreateChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingVerificationRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(
      request,
      TrainingVerificationChallengeCreateRequest,
    )
    const nowIso = routeNowIso(dependencies)
    const built = buildTrainingVerificationChallengeRecord({
      makeId: () => routeMakeId(dependencies),
      nowIso,
      request: body,
      ...(dependencies.registry === undefined
        ? {}
        : { registry: dependencies.registry }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingVerificationStoreErrorFromUnknown,
      try: () =>
        dependencies.makeStore(env).createChallenge(
          built.challenge,
          built.event,
        ),
    })

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(stored, nowIso),
    })
  })

const selectChallenge = (
  challenges: ReadonlyArray<TrainingVerificationChallengeRecord>,
): TrainingVerificationChallengeRecord | undefined =>
  [...challenges].sort((left, right) =>
    left.createdAt.localeCompare(right.createdAt),
  )[0]

const routeLeaseChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingVerificationRouteError> =>
  Effect.gen(function* () {
    const body = yield* decodeBody(
      request,
      TrainingVerificationChallengeLeaseRequest,
    )
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const candidate = selectChallenge(
      yield* Effect.tryPromise({
        catch: trainingVerificationStoreErrorFromUnknown,
        try: () =>
          store.listLeaseCandidates(nowIso, 25, body.verificationClass),
      }),
    )

    if (candidate === undefined) {
      return yield* new TrainingVerificationStoreError({
        kind: 'not_found',
        reason: 'No training verification challenge is currently claimable.',
      })
    }

    const leased = leaseTrainingVerificationChallengeRecord({
      challenge: candidate,
      eventId: routeMakeId(dependencies),
      nowIso,
      request: body,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingVerificationStoreErrorFromUnknown,
      try: () => store.leaseChallenge(leased.challenge, leased.event),
    })

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(stored, nowIso),
    })
  })

const readChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  env: Bindings,
  challengeRef: string,
): Effect.Effect<
  TrainingVerificationChallengeRecord,
  TrainingVerificationStoreError
> =>
  Effect.gen(function* () {
    const challenge = yield* Effect.tryPromise({
      catch: trainingVerificationStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readChallenge(challengeRef),
    })

    if (challenge === undefined) {
      return yield* new TrainingVerificationStoreError({
        kind: 'not_found',
        reason: 'Training verification challenge not found.',
      })
    }

    return challenge
  })

const routeRetryChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  challengeRef: string,
): Effect.Effect<HttpResponse, TrainingVerificationRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(
      request,
      TrainingVerificationChallengeRetryRequest,
    )
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const challenge = yield* readChallenge(dependencies, env, challengeRef)
    const retried = retryTrainingVerificationChallengeRecord({
      challenge,
      eventId: routeMakeId(dependencies),
      nowIso,
      request: body,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingVerificationStoreErrorFromUnknown,
      try: () => store.transitionChallenge(retried.challenge, retried.event),
    })

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(stored, nowIso),
    })
  })

const routeFinalizeChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  challengeRef: string,
): Effect.Effect<HttpResponse, TrainingVerificationRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(
      request,
      TrainingVerificationChallengeFinalizeRequest,
    )
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const challenge = yield* readChallenge(dependencies, env, challengeRef)
    const verdict = yield* Effect.tryPromise({
      catch: trainingVerificationStoreErrorFromUnknown,
      try: () =>
        runTrainingVerificationClass({
          challenge,
          ...(dependencies.registry === undefined
            ? {}
            : { registry: dependencies.registry }),
        }),
    })
    const finalized = finalizeTrainingVerificationChallengeRecord({
      challenge,
      eventId: routeMakeId(dependencies),
      nowIso,
      request: body,
      verdict,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingVerificationStoreErrorFromUnknown,
      try: () =>
        store.transitionChallenge(finalized.challenge, finalized.event),
    })

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(stored, nowIso),
    })
  })

const routeTimeoutChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  challengeRef: string,
): Effect.Effect<HttpResponse, TrainingVerificationRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const challenge = yield* readChallenge(dependencies, env, challengeRef)
    const timedOut = timeOutTrainingVerificationChallengeRecord({
      challenge,
      eventId: routeMakeId(dependencies),
      nowIso,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingVerificationStoreErrorFromUnknown,
      try: () => store.transitionChallenge(timedOut.challenge, timedOut.event),
    })

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(stored, nowIso),
    })
  })

const routeReadChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  env: Bindings,
  challengeRef: string,
): Effect.Effect<HttpResponse, TrainingVerificationRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const challenge = yield* readChallenge(dependencies, env, challengeRef)

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(
        challenge,
        nowIso,
      ),
    })
  })

export const PublicTrainingVerificationChallengeSchemaVersion =
  'openagents.public_training_verification_challenge.v1'

// Public per-challenge read (#5403 gap 3). The verification model is already
// dereferenceable inside the run summary (`realGradient.verifiedReplayPairs` /
// `rejectedReplayPairs`) and via each settlement's `verificationChallengeRef`,
// but a skeptic could not dereference ONE worker->validator replay pair
// directly. This standalone public read serves the SAME public-safe
// projection (`publicTrainingVerificationChallengeProjection`: challenge,
// worker/validator/verdict refs, the two compared sha256 digests, public-safe
// failure codes — never seeds, payloads, payment hashes, or raw traces) and,
// because it is a public projection, carries `generatedAt` plus the shared
// staleness contract per the projection-staleness invariant. Live at read: the
// challenge row is the source of truth, so the payload can never be older than
// the request; it rebuilds on the verification challenge transitions.
const publicTrainingVerificationChallengeStaleness = () =>
  liveAtReadStaleness([
    'training_verification_challenge_created',
    'training_verification_challenge_leased',
    'training_verification_challenge_finalized',
    'training_verification_challenge_timed_out',
  ])

const routeReadPublicChallenge = <Bindings extends TrainingVerificationRouteEnv>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
  env: Bindings,
  challengeRef: string,
): Effect.Effect<HttpResponse, TrainingVerificationRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const challenge = yield* readChallenge(dependencies, env, challengeRef)

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(
        challenge,
        nowIso,
      ),
      generatedAt: nowIso,
      schemaVersion: PublicTrainingVerificationChallengeSchemaVersion,
      sourceRefs: [
        `route:/api/public/training/verification-challenges/${challengeRef}`,
        `route:/api/public/training/runs/${challenge.trainingRunRef}/settlements`,
        'route:/api/public/tassadar-run-summary',
      ],
      staleness: publicTrainingVerificationChallengeStaleness(),
    })
  })

export const makeTrainingVerificationRoutes = <
  Bindings extends TrainingVerificationRouteEnv,
>(
  dependencies: TrainingVerificationRouteDependencies<Bindings>,
) => ({
  routeTrainingVerificationRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/training/verification/challenges') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeCreateChallenge(dependencies, request, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/verification/challenges/claim') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeLeaseChallenge(dependencies, request, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    const actionMatch =
      /^\/api\/training\/verification\/challenges\/([^/]+)\/(retry|finalize|timeout)$/.exec(
        url.pathname,
      )

    if (actionMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      const challengeRef = decodeURIComponent(actionMatch[1]!)
      const action = actionMatch[2]!

      if (action === 'retry') {
        return routeRetryChallenge(
          dependencies,
          request,
          env,
          challengeRef,
        ).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        )
      }

      if (action === 'finalize') {
        return routeFinalizeChallenge(
          dependencies,
          request,
          env,
          challengeRef,
        ).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        )
      }

      return routeTimeoutChallenge(
        dependencies,
        request,
        env,
        challengeRef,
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const publicReadMatch =
      /^\/api\/public\/training\/verification-challenges\/([^/]+)$/.exec(
        url.pathname,
      )

    if (publicReadMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeReadPublicChallenge(
        dependencies,
        env,
        decodeURIComponent(publicReadMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const readMatch =
      /^\/api\/training\/verification\/challenges\/([^/]+)$/.exec(
        url.pathname,
      )

    if (readMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeReadChallenge(
        dependencies,
        env,
        decodeURIComponent(readMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    return undefined
  },
})
