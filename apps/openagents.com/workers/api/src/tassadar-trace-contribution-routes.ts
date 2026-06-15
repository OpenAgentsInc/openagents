import { Effect, Match as M, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
} from './agent-registration'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import { tassadarExecutorTraceVerificationChallengeRequest } from './tassadar-executor-trace-homework'
import {
  TrainingReplayVerdictRequest,
  type TrainingTraceContributionStore,
  TrainingTraceContributionStoreError,
  TrainingTraceSubmissionRequest,
  buildTrainingTraceContributionRecord,
  closeoutFromPairedContribution,
  pairedContributionProjectionJson,
  trainingTraceContributionStoreErrorFromUnknown,
} from './tassadar-trace-contribution-authority'
import {
  type TrainingAuthorityStore,
  TrainingAuthorityStoreError,
  type TrainingWindowLeaseRecord,
  trainingAuthorityStoreErrorFromUnknown,
} from './training-run-window-authority'
import {
  type TrainingVerificationChallengeCreateRequest,
  type TrainingVerificationChallengeRecord,
  publicTrainingVerificationChallengeProjection,
} from './training-verification'

type HttpResponse = globalThis.Response

/**
 * Agent-gated worker -> validator executor-trace completion routes (#5052, epic
 * #5051). These two routes are the contributor-callable submit/verify path that
 * was previously missing: every training write except lease-claim is
 * requireAdmin, so a contributor could claim work but never finish it.
 *
 * - §4.1 POST /api/training/leases/{leaseRef}/trace-submission (requireAgent):
 *   the lease must belong to the caller's Pylon. Records a pending worker trace
 *   contribution; idempotent by lease + workload.
 * - §4.2 POST /api/training/leases/{leaseRef}/replay-verdict (requireAgent):
 *   the validator device must be DISTINCT from the worker Pylon device. Pairs
 *   with the pending contribution and builds the existing exact_trace_replay
 *   verification challenge (digest match -> Verified -> verifiedWorkCount;
 *   mismatch -> Rejected -> rejectedWorkCount).
 *
 * These routes are INERT with respect to existing behavior until the pairing
 * orchestration (#5053) and Pylon client (#5054) wire them: they add no admin
 * authority, do not relax any requireAdmin, and touch no settlement/payout path.
 * Replay is the trust anchor — the submitter's digest is never trusted; the
 * verdict is the separate-device replay match.
 */
type TassadarTraceContributionRouteDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  createVerificationChallenge?: (
    env: Bindings,
    request: TrainingVerificationChallengeCreateRequest,
  ) => Promise<TrainingVerificationChallengeRecord>
  makeContributionStore: (env: Bindings) => TrainingTraceContributionStore
  makeId?: () => string
  makeStore: (env: Bindings) => TrainingAuthorityStore
  nowIso?: () => string
  // Resolves the owning agent user id for a Pylon ref (the lease pylon_ref).
  // Wired in index.ts to the Pylon registry; the lease ownership check compares
  // it against the authenticated session's user id.
  resolvePylonOwnerUserId: (
    env: Bindings,
    pylonRef: string,
  ) => Promise<string | undefined>
}>

type TassadarTraceContributionRouteEnv = Readonly<Record<string, unknown>>

class TassadarTraceContributionUnauthorized extends S.TaggedErrorClass<TassadarTraceContributionUnauthorized>()(
  'TassadarTraceContributionUnauthorized',
  {},
) {}

type TassadarTraceContributionRouteError =
  | TrainingAuthorityStoreError
  | TrainingTraceContributionStoreError
  | TassadarTraceContributionUnauthorized

const storeErrorStatus = (
  kind:
    | 'conflict'
    | 'forbidden'
    | 'not_found'
    | 'storage_error'
    | 'validation_error',
): number =>
  kind === 'conflict'
    ? 409
    : kind === 'forbidden'
      ? 403
      : kind === 'not_found'
        ? 404
        : kind === 'storage_error'
          ? 500
          : 400

const routeErrorResponse = (
  error: TassadarTraceContributionRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      TassadarTraceContributionUnauthorized: () => unauthorized(),
      TrainingAuthorityStoreError: storeError =>
        noStoreJsonResponse(
          {
            error: `training_authority_${storeError.kind}`,
            reason: storeError.reason,
          },
          { status: storeErrorStatus(storeError.kind) },
        ),
      TrainingTraceContributionStoreError: storeError =>
        noStoreJsonResponse(
          {
            error: `training_trace_contribution_${storeError.kind}`,
            reason: storeError.reason,
          },
          { status: storeErrorStatus(storeError.kind) },
        ),
    }),
    M.exhaustive,
  )

const bearerTokenFromRequest = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  return scheme?.toLowerCase() === 'bearer' &&
    token !== undefined &&
    token.startsWith(AGENT_TOKEN_PREFIX)
    ? token
    : undefined
}

const routeNowIso = <Bindings>(
  dependencies: TassadarTraceContributionRouteDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: TassadarTraceContributionRouteDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

const decodeBody = <A>(
  request: Request,
  schema: S.Decoder<A>,
): Effect.Effect<A, TrainingTraceContributionStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new TrainingTraceContributionStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      decodeUnknownWithSchema(schema, await readJsonObject(request)),
  })

const requireAgent = <Bindings extends TassadarTraceContributionRouteEnv>(
  dependencies: TassadarTraceContributionRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<
  ProgrammaticAgentSession,
  TassadarTraceContributionUnauthorized
> => {
  const token = bearerTokenFromRequest(request)

  if (token === undefined) {
    return Effect.fail(new TassadarTraceContributionUnauthorized({}))
  }

  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new TassadarTraceContributionUnauthorized({}),
      try: () =>
        authenticateProgrammaticAgent(
          dependencies.agentStore(env),
          token,
          dependencies.nowIso,
        ),
    }),
    session =>
      session === undefined
        ? Effect.fail(new TassadarTraceContributionUnauthorized({}))
        : Effect.succeed(session),
  )
}

/**
 * Read the lease by ref and enforce that it belongs to the caller's Pylon: the
 * lease pylon_ref must resolve to a Pylon registration owned by the session
 * user. A missing lease is a 404; a lease owned by another agent is a 403.
 */
const requireOwnedLease = <Bindings extends TassadarTraceContributionRouteEnv>(
  dependencies: TassadarTraceContributionRouteDependencies<Bindings>,
  env: Bindings,
  leaseRef: string,
  session: ProgrammaticAgentSession,
): Effect.Effect<TrainingWindowLeaseRecord, TrainingAuthorityStoreError> =>
  Effect.gen(function* () {
    const lease = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readWindowLease(leaseRef),
    })

    if (lease === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training window lease not found.',
      })
    }

    const ownerUserId = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => dependencies.resolvePylonOwnerUserId(env, lease.pylonRef),
    })

    if (ownerUserId === undefined || ownerUserId !== session.user.id) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'forbidden',
        reason: 'Training window lease belongs to another Pylon.',
      })
    }

    return lease
  })

const routeTraceSubmission = <
  Bindings extends TassadarTraceContributionRouteEnv,
>(
  dependencies: TassadarTraceContributionRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  leaseRef: string,
): Effect.Effect<HttpResponse, TassadarTraceContributionRouteError> =>
  Effect.gen(function* () {
    const session = yield* requireAgent(dependencies, request, env)
    const body = yield* decodeBody(request, TrainingTraceSubmissionRequest)
    const lease = yield* requireOwnedLease(dependencies, env, leaseRef, session)
    const nowIso = routeNowIso(dependencies)
    const record = buildTrainingTraceContributionRecord({
      leaseRef: lease.leaseRef,
      makeId: () => routeMakeId(dependencies),
      nowIso,
      pylonRef: lease.pylonRef,
      request: body,
      trainingRunRef: lease.trainingRunRef,
      windowRef: lease.windowRef,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingTraceContributionStoreErrorFromUnknown,
      try: () =>
        dependencies
          .makeContributionStore(env)
          .recordWorkerContribution(record),
    })

    return noStoreJsonResponse({
      contribution: {
        assignmentRef: stored.assignmentRef,
        contributionRef: stored.contributionRef,
        leaseRef: stored.leaseRef,
        pylonRef: stored.pylonRef,
        state: stored.state,
        submittedAt: stored.submittedAt,
        trainingRunRef: stored.trainingRunRef,
        windowRef: stored.windowRef,
        workloadFamily: stored.workloadFamily,
      },
    })
  })

const routeReplayVerdict = <Bindings extends TassadarTraceContributionRouteEnv>(
  dependencies: TassadarTraceContributionRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  leaseRef: string,
): Effect.Effect<HttpResponse, TassadarTraceContributionRouteError> =>
  Effect.gen(function* () {
    // The validator authenticates as an agent but, by design, is a DIFFERENT
    // party from the worker that owns the lease — so no lease-ownership check
    // here. The trust anchor is device-distinctness (validator device != worker
    // device) plus the separate-device replay match, both enforced below.
    yield* requireAgent(dependencies, request, env)
    const body = yield* decodeBody(request, TrainingReplayVerdictRequest)
    const lease = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readWindowLease(leaseRef),
    })

    if (lease === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training window lease not found.',
      })
    }

    const nowIso = routeNowIso(dependencies)
    const contributionStore = dependencies.makeContributionStore(env)
    const contribution = yield* Effect.tryPromise({
      catch: trainingTraceContributionStoreErrorFromUnknown,
      try: () =>
        contributionStore.readWorkerContribution(
          lease.leaseRef,
          body.workloadFamily,
        ),
    })

    if (contribution === undefined) {
      return yield* new TrainingTraceContributionStoreError({
        kind: 'not_found',
        reason:
          'No pending worker trace contribution for this lease and workload.',
      })
    }

    if (contribution.state !== 'pending') {
      return yield* new TrainingTraceContributionStoreError({
        kind: 'conflict',
        reason: 'Worker trace contribution has already been paired.',
      })
    }

    // Device-distinctness is enforced server-side: a single actor must not
    // self-verify across two of its own processes on one device. Reject before
    // touching the challenge builder so self-validation never creates a verdict.
    if (body.validatorDeviceRef === contribution.pylonDeviceRef) {
      return yield* new TrainingTraceContributionStoreError({
        kind: 'forbidden',
        reason:
          'exact_trace_replay requires a validator device distinct from the worker Pylon.',
      })
    }

    const createVerificationChallenge = dependencies.createVerificationChallenge

    if (createVerificationChallenge === undefined) {
      return yield* new TrainingTraceContributionStoreError({
        kind: 'storage_error',
        reason: 'Verification challenge creation is not configured.',
      })
    }

    const closeout = closeoutFromPairedContribution(contribution, {
      replayDigestRef: body.replayDigestRef,
      validatorDeviceRef: body.validatorDeviceRef,
    })
    // The builder re-enforces worker != validator device (a violation surfaces
    // as a validation error); the resulting exact_trace_replay challenge
    // computes Verified (digest match) or Rejected (mismatch).
    const challengeRequest = yield* Effect.try({
      catch: error =>
        new TrainingTraceContributionStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        tassadarExecutorTraceVerificationChallengeRequest({
          closeout,
          trainingRunRef: contribution.trainingRunRef,
          windowRef: contribution.windowRef,
        }),
    })
    const challenge = yield* Effect.tryPromise({
      catch: trainingTraceContributionStoreErrorFromUnknown,
      try: () => createVerificationChallenge(env, challengeRequest),
    })
    const paired = yield* Effect.tryPromise({
      catch: trainingTraceContributionStoreErrorFromUnknown,
      try: () =>
        contributionStore.pairValidatorVerdict({
          contributionRef: contribution.contributionRef,
          publicProjectionJson: pairedContributionProjectionJson(contribution, {
            replayDigestRef: body.replayDigestRef,
            validatorDeviceRef: body.validatorDeviceRef,
            verificationChallengeRef: challenge.challengeRef,
          }),
          replayDigestRef: body.replayDigestRef,
          updatedAt: nowIso,
          validatorDeviceRef: body.validatorDeviceRef,
          verificationChallengeRef: challenge.challengeRef,
        }),
    })

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(
        challenge,
        nowIso,
      ),
      contribution: {
        contributionRef: paired.contributionRef,
        leaseRef: paired.leaseRef,
        state: paired.state,
        trainingRunRef: paired.trainingRunRef,
        verificationChallengeRef: paired.verificationChallengeRef,
        windowRef: paired.windowRef,
        workloadFamily: paired.workloadFamily,
      },
    })
  })

export const makeTassadarTraceContributionRoutes = <
  Bindings extends TassadarTraceContributionRouteEnv,
>(
  dependencies: TassadarTraceContributionRouteDependencies<Bindings>,
) => ({
  routeTassadarTraceContributionRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    const traceSubmissionMatch =
      /^\/api\/training\/leases\/([^/]+)\/trace-submission$/.exec(url.pathname)

    if (traceSubmissionMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeTraceSubmission(
        dependencies,
        request,
        env,
        decodeURIComponent(traceSubmissionMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const replayVerdictMatch =
      /^\/api\/training\/leases\/([^/]+)\/replay-verdict$/.exec(url.pathname)

    if (replayVerdictMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeReplayVerdict(
        dependencies,
        request,
        env,
        decodeURIComponent(replayVerdictMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    return undefined
  },
})
