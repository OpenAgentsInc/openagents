import { notFound } from '@openagents/sync-worker'
import { Effect, Match as M, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  sha256Hex,
} from './agent-registration'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import { PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION } from './public-pylon-stats'
import {
  PylonApiArtifactProofMetadataRequest,
  PylonApiAssignmentAcceptanceRequest,
  PylonApiAssignmentCloseoutRequest,
  PylonApiAssignmentProgressRequest,
  type PylonApiAssignmentRecord,
  type PylonApiAssignmentState,
  PylonApiCreateAssignmentRequest,
  type PylonApiEventKind,
  PylonApiHeartbeatRequest,
  PylonApiPaymentReceiptRequest,
  PylonApiPayoutTargetAdmissionRequest,
  type PylonApiRegistrationRecord,
  PylonApiRegistrationRequest,
  PylonApiSettlementStatusRequest,
  type PylonApiStore,
  PylonApiStoreError,
  PylonApiWalletReadinessRequest,
  buildPylonApiAssignmentRecord,
  buildPylonApiEventRecord,
  buildPylonApiRegistrationRecord,
  closeoutPylonApiAssignmentRecord,
  nextAssignmentForEvent,
  nextRegistrationForEvent,
  publicPylonApiAssignmentProjection,
  publicPylonApiEventProjection,
  publicPylonApiRegistrationProjection,
  pylonApiStoreErrorFromUnknown,
  pylonClientVersionMeetsMinimum,
} from './pylon-api'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

type HttpResponse = globalThis.Response

type PylonApiRouteDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  makeId?: () => string
  makeStore: (env: Bindings) => PylonApiStore
  nowIso?: () => string
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
}>

type PylonApiRouteEnv = Readonly<Record<string, unknown>>

class PylonApiUnauthorized extends S.TaggedErrorClass<PylonApiUnauthorized>()(
  'PylonApiUnauthorized',
  {},
) {}

type PylonApiRouteError = PylonApiStoreError | PylonApiUnauthorized

const routeErrorResponse = (error: PylonApiRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      PylonApiStoreError: storeError =>
        noStoreJsonResponse(
          { error: `pylon_api_${storeError.kind}`, reason: storeError.reason },
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
      PylonApiUnauthorized: () => unauthorized(),
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

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('Idempotency-Key')?.trim()

  return value === undefined || value === '' ? undefined : value
}

const requireIdempotencyHash = (
  request: Request,
): Effect.Effect<string, PylonApiStoreError> => {
  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey === undefined) {
    return Effect.fail(
      new PylonApiStoreError({
        kind: 'validation_error',
        reason: 'Idempotency-Key header is required.',
      }),
    )
  }

  return Effect.promise(() => sha256Hex(idempotencyKey))
}

const decodeBody = <A>(
  request: Request,
  schema: S.Decoder<A>,
): Effect.Effect<A, PylonApiStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new PylonApiStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      decodeUnknownWithSchema(schema, await readJsonObject(request)),
  })

const routeStore = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
): PylonApiStore => dependencies.makeStore(env)

const routeAgentStore = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
): AgentRegistrationStore => dependencies.agentStore(env)

const routeNowIso = <Bindings>(
  dependencies: PylonApiRouteDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: PylonApiRouteDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

const CONTROLLED_PYLON_ASSIGNMENT_DISPATCH_GATE_REF =
  'gate.public.pylon.assignment_dispatch.controlled.v1'
const CONTROLLED_PYLON_ASSIGNMENT_ONLINE_WINDOW_MS = 5 * 60 * 1000
const controlledDispatchOnlineStatuses = new Set([
  'available',
  'healthy',
  'idle',
  'online',
  'ready',
])
const duplicateBlockingAssignmentStates = new Set<PylonApiAssignmentState>([
  'accepted',
  'blocked',
  'offered',
  'proof_submitted',
  'running',
])

type ControlledPylonAssignmentDispatchGate = Readonly<{
  assignmentRef: string | null
  blockerRefs: ReadonlyArray<string>
  campaignRef: string | null
  caveatRefs: ReadonlyArray<string>
  dispatchAllowed: boolean
  forumAutoPublishAllowed: false
  gateRef: string
  noSpendDispatch: boolean
  paymentMode: string | null
  pylonRef: string
  settlementMutationAllowed: false
  sourceRefs: ReadonlyArray<string>
  state: 'blocked' | 'ready'
  stateLabel: string
  walletSpendAllowed: false
}>

const uniqueRouteRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const gateRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => uniqueRouteRefs(refs ?? [])

const hasGateRefs = (refs: ReadonlyArray<string> | undefined): boolean =>
  gateRefs(refs).length > 0

const paidAssignmentPaymentModes = new Set([
  'operator_credit',
  'payable_pending_settlement',
  'settled_bitcoin',
])

const pylonAssignmentHasActiveLease = (
  assignment: PylonApiAssignmentRecord,
  nowIso: string,
): boolean =>
  duplicateBlockingAssignmentStates.has(assignment.state) &&
  Date.parse(assignment.leaseExpiresAt) > Date.parse(nowIso)

const activeDuplicateAssignmentRefs = (
  assignments: ReadonlyArray<PylonApiAssignmentRecord>,
  nowIso: string,
): ReadonlyArray<string> =>
  assignments
    .filter(assignment => pylonAssignmentHasActiveLease(assignment, nowIso))
    .map(assignment => assignment.assignmentRef)

const hasControlledDispatchOnlineStatus = (
  registration: PylonApiRegistrationRecord,
): boolean =>
  controlledDispatchOnlineStatuses.has(
    (registration.latestHeartbeatStatus ?? '').trim().toLowerCase(),
  )

const heartbeatAgeMs = (
  registration: PylonApiRegistrationRecord,
  nowIso: string,
): number | null => {
  if (registration.latestHeartbeatAt === null) {
    return null
  }

  const latest = Date.parse(registration.latestHeartbeatAt)
  const now = Date.parse(nowIso)

  return Number.isFinite(latest) && Number.isFinite(now) ? now - latest : null
}

const controlledPylonAssignmentDispatchGate = (
  input: Readonly<{
    activeAssignments: ReadonlyArray<PylonApiAssignmentRecord>
    assignmentRef: string | null
    body: PylonApiCreateAssignmentRequest
    nowIso: string
    registration: PylonApiRegistrationRecord | undefined
  }>,
): ControlledPylonAssignmentDispatchGate => {
  const body = input.body
  const registration = input.registration
  const missingRefBlockers = [
    hasGateRefs(body.campaignPolicyRefs)
      ? null
      : 'blocker.public.pylon_dispatch.campaign_policy_missing',
    hasGateRefs(body.selectionPolicyRefs)
      ? null
      : 'blocker.public.pylon_dispatch.selection_policy_missing',
    body.paymentMode === undefined
      ? 'blocker.public.pylon_dispatch.payment_mode_missing'
      : null,
    hasGateRefs(body.idempotencyRefs)
      ? null
      : 'blocker.public.pylon_dispatch.idempotency_policy_missing',
    hasGateRefs(body.operatorPauseRefs)
      ? null
      : 'blocker.public.pylon_dispatch.pause_policy_missing',
    hasGateRefs(body.rollbackRefs)
      ? null
      : 'blocker.public.pylon_dispatch.rollback_path_missing',
    hasGateRefs(body.closeoutPathRefs)
      ? null
      : 'blocker.public.pylon_dispatch.closeout_path_missing',
    hasGateRefs(body.noDuplicateAssignmentRefs)
      ? null
      : 'blocker.public.pylon_dispatch.no_duplicate_policy_missing',
    hasGateRefs(body.noForumAutoPublishRefs)
      ? null
      : 'blocker.public.pylon_dispatch.no_forum_auto_publish_policy_missing',
    hasGateRefs(body.requiredCapabilityRefs)
      ? null
      : 'blocker.public.pylon_dispatch.required_capability_missing',
    body.campaignRef === undefined
      ? 'blocker.public.pylon_dispatch.campaign_ref_missing'
      : null,
    body.campaignPaused === undefined
      ? 'blocker.public.pylon_dispatch.pause_state_missing'
      : null,
    body.forumAutoPublishAllowed === undefined
      ? 'blocker.public.pylon_dispatch.forum_auto_publish_flag_missing'
      : null,
  ].filter((ref): ref is string => ref !== null)
  const paymentMode = body.paymentMode ?? null
  const paidModeMissingSpendCap =
    paymentMode !== null &&
    paidAssignmentPaymentModes.has(paymentMode) &&
    !hasGateRefs(body.spendCapRefs)
  const duplicateRefs = activeDuplicateAssignmentRefs(
    input.activeAssignments,
    input.nowIso,
  )
  const heartbeatAge =
    registration === undefined
      ? null
      : heartbeatAgeMs(registration, input.nowIso)
  const staleHeartbeat =
    heartbeatAge !== null &&
    (heartbeatAge < 0 ||
      heartbeatAge > CONTROLLED_PYLON_ASSIGNMENT_ONLINE_WINDOW_MS)
  const missingCapabilityRefs =
    registration === undefined
      ? []
      : gateRefs(body.requiredCapabilityRefs).filter(
          capabilityRef => !registration.capabilityRefs.includes(capabilityRef),
        )
  const blockers = uniqueRouteRefs([
    ...missingRefBlockers,
    ...(body.campaignPaused === true
      ? ['blocker.public.pylon_dispatch.campaign_paused']
      : []),
    ...(body.forumAutoPublishAllowed === true
      ? ['blocker.public.pylon_dispatch.forum_auto_publish_requested']
      : []),
    ...(paidModeMissingSpendCap
      ? ['blocker.public.pylon_dispatch.paid_mode_missing_spend_cap']
      : []),
    ...(registration === undefined
      ? ['blocker.public.pylon_dispatch.pylon_missing']
      : []),
    ...(registration !== undefined && registration.status !== 'active'
      ? ['blocker.public.pylon_dispatch.pylon_not_active']
      : []),
    ...(registration !== undefined && !registration.walletReady
      ? ['blocker.public.pylon_dispatch.wallet_not_ready']
      : []),
    ...(registration !== undefined &&
    !hasControlledDispatchOnlineStatus(registration)
      ? ['blocker.public.pylon_dispatch.pylon_offline']
      : []),
    ...(registration !== undefined && staleHeartbeat
      ? ['blocker.public.pylon_dispatch.pylon_stale']
      : []),
    ...(registration !== undefined &&
    !pylonClientVersionMeetsMinimum(
      registration.clientVersion,
      PUBLIC_PYLON_STATS_MINIMUM_CLIENT_VERSION,
    )
      ? ['blocker.public.pylon_dispatch.client_version_below_minimum']
      : []),
    ...(missingCapabilityRefs.length > 0
      ? ['blocker.public.pylon_dispatch.wrong_capability']
      : []),
    ...(duplicateRefs.length > 0
      ? ['blocker.public.pylon_dispatch.duplicate_active_assignment']
      : []),
  ])
  const dispatchAllowed = blockers.length === 0

  return {
    assignmentRef: input.assignmentRef,
    blockerRefs: blockers,
    campaignRef: body.campaignRef ?? null,
    caveatRefs: uniqueRouteRefs([
      'caveat.public.pylon_dispatch.assignment_only_no_wallet_spend',
      'caveat.public.pylon_dispatch.settlement_requires_separate_closeout_and_payout',
      'caveat.public.pylon_dispatch.forum_publish_disabled',
      ...(paymentMode === 'unpaid_smoke'
        ? ['caveat.public.pylon_dispatch.no_spend_smoke']
        : []),
    ]),
    dispatchAllowed,
    forumAutoPublishAllowed: false,
    gateRef: CONTROLLED_PYLON_ASSIGNMENT_DISPATCH_GATE_REF,
    noSpendDispatch: paymentMode === 'unpaid_smoke',
    paymentMode,
    pylonRef: body.pylonRef,
    settlementMutationAllowed: false,
    sourceRefs: uniqueRouteRefs([
      CONTROLLED_PYLON_ASSIGNMENT_DISPATCH_GATE_REF,
      'route:/api/operator/pylons/assignments',
      ...(body.campaignRef === undefined ? [] : [body.campaignRef]),
      ...gateRefs(body.campaignPolicyRefs),
      ...gateRefs(body.selectionPolicyRefs),
      ...gateRefs(body.idempotencyRefs),
      ...gateRefs(body.operatorPauseRefs),
      ...gateRefs(body.rollbackRefs),
      ...gateRefs(body.closeoutPathRefs),
      ...gateRefs(body.noDuplicateAssignmentRefs),
      ...gateRefs(body.noForumAutoPublishRefs),
      ...gateRefs(body.requiredCapabilityRefs),
      ...gateRefs(body.spendCapRefs),
    ]),
    state: dispatchAllowed ? 'ready' : 'blocked',
    stateLabel: dispatchAllowed
      ? 'Ready for controlled no-authority assignment dispatch'
      : `Blocked by ${blockers.length} controlled assignment gate blocker${
          blockers.length === 1 ? '' : 's'
        }`,
    walletSpendAllowed: false,
  }
}

const controlledDispatchGateBlockedResponse = (
  dispatchGate: ControlledPylonAssignmentDispatchGate,
): HttpResponse =>
  noStoreJsonResponse(
    {
      dispatchGate,
      error: 'controlled_dispatch_gate_blocked',
      reason: 'Controlled Pylon assignment dispatch gate is blocked.',
    },
    { status: 409 },
  )

const requireAgent = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<ProgrammaticAgentSession, PylonApiUnauthorized> => {
  const token = bearerTokenFromRequest(request)

  if (token === undefined) {
    return Effect.fail(new PylonApiUnauthorized({}))
  }

  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new PylonApiUnauthorized({}),
      try: () =>
        authenticateProgrammaticAgent(
          routeAgentStore(dependencies, env),
          token,
          dependencies.nowIso,
        ),
    }),
    session =>
      session === undefined
        ? Effect.fail(new PylonApiUnauthorized({}))
        : Effect.succeed(session),
  )
}

const requireAdmin = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, PylonApiUnauthorized> => {
  const requireAdminApiToken = dependencies.requireAdminApiToken

  if (requireAdminApiToken === undefined) {
    return Effect.fail(new PylonApiUnauthorized({}))
  }

  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new PylonApiUnauthorized({}),
      try: () => requireAdminApiToken(request, env),
    }),
    allowed =>
      allowed ? Effect.void : Effect.fail(new PylonApiUnauthorized({})),
  )
}

const requireOwnedRegistration = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
  pylonRef: string,
  session: ProgrammaticAgentSession,
): Effect.Effect<PylonApiRegistrationRecord, PylonApiStoreError> =>
  Effect.flatMap(
    Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => routeStore(dependencies, env).readRegistration(pylonRef),
    }),
    registration => {
      if (registration === undefined) {
        return Effect.fail(
          new PylonApiStoreError({
            kind: 'not_found',
            reason: 'Pylon registration was not found.',
          }),
        )
      }

      if (registration.ownerAgentUserId !== session.user.id) {
        return Effect.fail(
          new PylonApiStoreError({
            kind: 'forbidden',
            reason: 'Pylon registration belongs to another agent.',
          }),
        )
      }

      return Effect.succeed(registration)
    },
  )

const requireOwnedAssignment = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
  pylonRef: string,
  assignmentRef: string,
  session: ProgrammaticAgentSession,
) =>
  Effect.flatMap(
    Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => routeStore(dependencies, env).readAssignment(assignmentRef),
    }),
    assignment => {
      if (assignment === undefined || assignment.pylonRef !== pylonRef) {
        return Effect.fail(
          new PylonApiStoreError({
            kind: 'not_found',
            reason: 'Pylon assignment was not found for this Pylon.',
          }),
        )
      }

      if (assignment.ownerAgentUserId !== session.user.id) {
        return Effect.fail(
          new PylonApiStoreError({
            kind: 'forbidden',
            reason: 'Pylon assignment belongs to another agent.',
          }),
        )
      }

      return Effect.succeed(assignment)
    },
  )

const routeRegister = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.gen(function* () {
    const session = yield* requireAgent(dependencies, request, env)
    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const body = yield* decodeBody(request, PylonApiRegistrationRequest)
    const nowIso = routeNowIso(dependencies)
    const store = routeStore(dependencies, env)
    const existingEvent = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.readEventByIdempotencyKeyHash(idempotencyKeyHash),
    })

    if (existingEvent !== undefined) {
      if (existingEvent.ownerAgentUserId !== session.user.id) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'forbidden',
            reason: 'Idempotency key is already bound to another agent.',
          }),
        )
      }

      if (existingEvent.eventKind !== 'registration') {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'conflict',
            reason:
              'Idempotency key is already bound to a different Pylon event.',
          }),
        )
      }

      const existingRegistration = yield* Effect.tryPromise({
        catch: pylonApiStoreErrorFromUnknown,
        try: () => store.readRegistration(existingEvent.pylonRef),
      })

      if (existingRegistration === undefined) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'storage_error',
            reason:
              'Pylon registration event replay could not find its registration.',
          }),
        )
      }

      return noStoreJsonResponse(
        {
          event: publicPylonApiEventProjection(existingEvent, nowIso),
          idempotent: true,
          pylon: publicPylonApiRegistrationProjection(
            existingRegistration,
            nowIso,
          ),
        },
        { status: 200 },
      )
    }

    const registration = yield* Effect.try({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        buildPylonApiRegistrationRecord({
          credentialId: session.credential.id,
          displayName: session.user.displayName,
          makeId: () => routeMakeId(dependencies),
          nowIso,
          ownerAgentTokenPrefix: session.credential.tokenPrefix,
          ownerAgentUserId: session.user.id,
          request: body,
        }),
    })
    const event = yield* Effect.try({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        buildPylonApiEventRecord({
          body: {
            capabilityRefs: registration.capabilityRefs,
            clientProtocolVersion: registration.clientProtocolVersion,
            clientVersion: registration.clientVersion,
            resourceMode: registration.resourceMode,
            statusRefs: body.statusRefs ?? [],
            walletRef: registration.walletRef,
          },
          eventKind: 'registration',
          idempotencyKeyHash,
          makeId: () => routeMakeId(dependencies),
          nowIso,
          ownerAgentUserId: session.user.id,
          pylonRef: registration.pylonRef,
          status: registration.status,
        }),
    })
    const storedRegistration = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.upsertRegistration(registration),
    })
    const eventResult = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.createEvent(event),
    })

    return noStoreJsonResponse(
      {
        event: publicPylonApiEventProjection(eventResult.record, nowIso),
        idempotent: eventResult.idempotent,
        pylon: publicPylonApiRegistrationProjection(storedRegistration, nowIso),
      },
      { status: eventResult.idempotent ? 200 : 201 },
    )
  })

const routeCreateAssignment = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
) =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const body = yield* decodeBody(request, PylonApiCreateAssignmentRequest)
    const nowIso = routeNowIso(dependencies)
    const store = routeStore(dependencies, env)
    const existing = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.readAssignmentByIdempotencyKeyHash(idempotencyKeyHash),
    })

    if (existing !== undefined) {
      if (
        existing.pylonRef !== body.pylonRef ||
        (body.assignmentRef !== undefined &&
          existing.assignmentRef !== body.assignmentRef)
      ) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'conflict',
            reason:
              'Idempotency key is already bound to a different assignment.',
          }),
        )
      }

      return noStoreJsonResponse(
        {
          assignment: publicPylonApiAssignmentProjection(existing, nowIso),
          idempotent: true,
        },
        { status: 200 },
      )
    }

    const requestedAssignmentRef = body.assignmentRef

    if (requestedAssignmentRef !== undefined) {
      const sameRefAssignment = yield* Effect.tryPromise({
        catch: pylonApiStoreErrorFromUnknown,
        try: () => store.readAssignment(requestedAssignmentRef),
      })

      if (sameRefAssignment !== undefined) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'conflict',
            reason: 'Assignment ref is already in use.',
          }),
        )
      }
    }

    const registration = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.readRegistration(body.pylonRef),
    })

    const activeAssignments =
      registration === undefined
        ? []
        : yield* Effect.tryPromise({
            catch: pylonApiStoreErrorFromUnknown,
            try: () => store.listAssignmentsForPylon(body.pylonRef, 100),
          })
    const dispatchGate = controlledPylonAssignmentDispatchGate({
      activeAssignments,
      assignmentRef: requestedAssignmentRef ?? null,
      body,
      nowIso,
      registration,
    })

    if (!dispatchGate.dispatchAllowed) {
      return controlledDispatchGateBlockedResponse(dispatchGate)
    }

    const dispatchRegistration = registration

    if (dispatchRegistration === undefined) {
      return controlledDispatchGateBlockedResponse(dispatchGate)
    }

    const assignment = yield* Effect.try({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        buildPylonApiAssignmentRecord({
          idempotencyKeyHash,
          makeId: () => routeMakeId(dependencies),
          nowIso,
          ownerAgentUserId: dispatchRegistration.ownerAgentUserId,
          request: body,
        }),
    })
    const result = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.createAssignment(assignment),
    })

    return noStoreJsonResponse(
      {
        assignment: publicPylonApiAssignmentProjection(result.record, nowIso),
        dispatchGate: {
          ...dispatchGate,
          assignmentRef: result.record.assignmentRef,
        },
        idempotent: result.idempotent,
      },
      { status: result.idempotent ? 200 : 201 },
    )
  })

const routeListAssignments = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pylonRef: string,
) =>
  Effect.gen(function* () {
    const session = yield* requireAgent(dependencies, request, env)
    yield* requireOwnedRegistration(dependencies, env, pylonRef, session)
    const nowIso = routeNowIso(dependencies)
    const assignments = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        routeStore(dependencies, env).listAssignmentsForPylon(pylonRef, 25),
    })

    return noStoreJsonResponse({
      assignments: assignments.map(assignment =>
        publicPylonApiAssignmentProjection(assignment, nowIso),
      ),
    })
  })

const routeCloseoutAssignment = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  assignmentRef: string,
) =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, PylonApiAssignmentCloseoutRequest)
    const nowIso = routeNowIso(dependencies)
    const store = routeStore(dependencies, env)
    const assignment = yield* Effect.flatMap(
      Effect.tryPromise({
        catch: pylonApiStoreErrorFromUnknown,
        try: () => store.readAssignment(assignmentRef),
      }),
      record =>
        record === undefined
          ? Effect.fail(
              new PylonApiStoreError({
                kind: 'not_found',
                reason: 'Pylon assignment was not found.',
              }),
            )
          : Effect.succeed(record),
    )
    const nextAssignment = yield* Effect.try({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        closeoutPylonApiAssignmentRecord({
          assignment,
          nowIso,
          request: body,
        }),
    })
    const storedAssignment = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.updateAssignment(nextAssignment),
    })

    return noStoreJsonResponse({
      assignment: publicPylonApiAssignmentProjection(storedAssignment, nowIso),
    })
  })

const eventStatusFromBody = (
  body: Record<string, unknown>,
  fallback: string,
): string => (typeof body.status === 'string' ? body.status : fallback)

const terminalAssignmentEventAllowed = (
  eventKind: PylonApiEventKind,
  assignmentState: string,
): boolean =>
  assignmentState === 'accepted_work' &&
  (eventKind === 'payment_receipt' || eventKind === 'settlement_status')

const routeEvent = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  input: Readonly<{
    assignmentRef?: string | undefined
    eventKind: PylonApiEventKind
    fallbackStatus: string
    pylonRef: string
    schema: S.Decoder<Record<string, unknown>>
  }>,
) =>
  Effect.gen(function* () {
    const session = yield* requireAgent(dependencies, request, env)
    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const pylonRef = input.pylonRef
    const assignmentRef = input.assignmentRef ?? null
    const store = routeStore(dependencies, env)
    const existingEvent = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.readEventByIdempotencyKeyHash(idempotencyKeyHash),
    })

    if (existingEvent !== undefined) {
      if (
        existingEvent.ownerAgentUserId !== session.user.id ||
        existingEvent.pylonRef !== pylonRef
      ) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'forbidden',
            reason:
              'Idempotency key is already bound to another agent or Pylon.',
          }),
        )
      }

      if (
        existingEvent.eventKind !== input.eventKind ||
        existingEvent.assignmentRef !== assignmentRef
      ) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'conflict',
            reason:
              'Idempotency key is already bound to a different Pylon event.',
          }),
        )
      }

      const existingRegistration = yield* requireOwnedRegistration(
        dependencies,
        env,
        pylonRef,
        session,
      )
      const replayNowIso = routeNowIso(dependencies)

      return noStoreJsonResponse(
        {
          event: publicPylonApiEventProjection(existingEvent, replayNowIso),
          idempotent: true,
          pylon: publicPylonApiRegistrationProjection(
            existingRegistration,
            replayNowIso,
          ),
        },
        { status: 200 },
      )
    }

    const registration = yield* requireOwnedRegistration(
      dependencies,
      env,
      pylonRef,
      session,
    )
    const nowIso = routeNowIso(dependencies)
    const assignment =
      input.assignmentRef === undefined
        ? undefined
        : yield* requireOwnedAssignment(
            dependencies,
            env,
            pylonRef,
            input.assignmentRef,
            session,
          )

    if (assignment !== undefined) {
      const leaseState = publicPylonApiAssignmentProjection(
        assignment,
        nowIso,
      ).leaseState

      if (leaseState === 'expired') {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'conflict',
            reason: 'Pylon assignment lease is stale.',
          }),
        )
      }

      if (
        leaseState === 'terminal' &&
        !terminalAssignmentEventAllowed(input.eventKind, assignment.state)
      ) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'conflict',
            reason: 'Pylon assignment is already closed.',
          }),
        )
      }
    }

    const body = yield* decodeBody(request, input.schema)
    const event = yield* Effect.try({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        buildPylonApiEventRecord({
          assignmentRef: input.assignmentRef,
          body,
          eventKind: input.eventKind,
          idempotencyKeyHash,
          makeId: () => routeMakeId(dependencies),
          nowIso,
          ownerAgentUserId: session.user.id,
          pylonRef: registration.pylonRef,
          status: eventStatusFromBody(body, input.fallbackStatus),
        }),
    })
    const eventResult = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.createEvent(event),
    })
    const storedAssignment =
      assignment === undefined
        ? undefined
        : yield* Effect.tryPromise({
            catch: pylonApiStoreErrorFromUnknown,
            try: () =>
              store.updateAssignment(
                nextAssignmentForEvent(assignment, eventResult.record, nowIso),
              ),
          })
    const nextRegistration = nextRegistrationForEvent(
      registration,
      eventResult.record,
      nowIso,
    )
    const storedRegistration = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.upsertRegistration(nextRegistration),
    })

    return noStoreJsonResponse(
      {
        ...(storedAssignment === undefined
          ? {}
          : {
              assignment: publicPylonApiAssignmentProjection(
                storedAssignment,
                nowIso,
              ),
            }),
        event: publicPylonApiEventProjection(eventResult.record, nowIso),
        idempotent: eventResult.idempotent,
        pylon: publicPylonApiRegistrationProjection(storedRegistration, nowIso),
      },
      { status: eventResult.idempotent ? 200 : 201 },
    )
  })

const routeList = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
) =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const registrations = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => routeStore(dependencies, env).listRegistrations(100),
    })

    return noStoreJsonResponse({
      pylons: registrations.map(registration =>
        publicPylonApiRegistrationProjection(registration, nowIso),
      ),
    })
  })

const routeRead = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
  pylonRef: string,
) =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const registration = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => routeStore(dependencies, env).readRegistration(pylonRef),
    })

    if (registration === undefined) {
      return notFound()
    }

    const events = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => routeStore(dependencies, env).listEventsForPylon(pylonRef, 25),
    })

    return noStoreJsonResponse({
      events: events.map(event => publicPylonApiEventProjection(event, nowIso)),
      pylon: publicPylonApiRegistrationProjection(registration, nowIso),
    })
  })

export const makePylonApiRoutes = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
) => ({
  routePylonApiRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/pylons') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeList(dependencies, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/pylons/register') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeRegister(dependencies, request, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/operator/pylons/assignments') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeCreateAssignment(dependencies, request, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    const operatorCloseoutMatch =
      /^\/api\/operator\/pylons\/assignments\/([^/]+)\/closeout$/.exec(
        url.pathname,
      )

    if (operatorCloseoutMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeCloseoutAssignment(
        dependencies,
        request,
        env,
        decodeURIComponent(operatorCloseoutMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const assignmentListMatch = /^\/api\/pylons\/([^/]+)\/assignments$/.exec(
      url.pathname,
    )

    if (assignmentListMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeListAssignments(
        dependencies,
        request,
        env,
        decodeURIComponent(assignmentListMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const readMatch = /^\/api\/pylons\/([^/]+)$/.exec(url.pathname)

    if (readMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeRead(
        dependencies,
        env,
        decodeURIComponent(readMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const heartbeatMatch = /^\/api\/pylons\/([^/]+)\/heartbeat$/.exec(
      url.pathname,
    )

    if (heartbeatMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeEvent(dependencies, request, env, {
        eventKind: 'heartbeat',
        fallbackStatus: 'online',
        pylonRef: decodeURIComponent(heartbeatMatch[1]!),
        schema: PylonApiHeartbeatRequest,
      }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const walletReadinessMatch =
      /^\/api\/pylons\/([^/]+)\/wallet-readiness$/.exec(url.pathname)

    if (walletReadinessMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeEvent(dependencies, request, env, {
        eventKind: 'wallet_readiness',
        fallbackStatus: 'reported',
        pylonRef: decodeURIComponent(walletReadinessMatch[1]!),
        schema: PylonApiWalletReadinessRequest,
      }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const payoutTargetMatch =
      /^\/api\/pylons\/([^/]+)\/payout-target-admission$/.exec(url.pathname)

    if (payoutTargetMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeEvent(dependencies, request, env, {
        eventKind: 'payout_target_admission',
        fallbackStatus: 'requested',
        pylonRef: decodeURIComponent(payoutTargetMatch[1]!),
        schema: PylonApiPayoutTargetAdmissionRequest,
      }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const assignmentMatch =
      /^\/api\/pylons\/([^/]+)\/assignments\/([^/]+)\/(accept|progress|artifacts|payment-receipts|settlement-status)$/.exec(
        url.pathname,
      )

    if (assignmentMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      const action = assignmentMatch[3]!
      const route =
        action === 'accept'
          ? {
              eventKind: 'assignment_acceptance' as const,
              fallbackStatus: 'accepted',
              schema: PylonApiAssignmentAcceptanceRequest,
            }
          : action === 'progress'
            ? {
                eventKind: 'assignment_progress' as const,
                fallbackStatus: 'running',
                schema: PylonApiAssignmentProgressRequest,
              }
            : action === 'artifacts'
              ? {
                  eventKind: 'artifact_proof_metadata' as const,
                  fallbackStatus: 'submitted',
                  schema: PylonApiArtifactProofMetadataRequest,
                }
              : action === 'payment-receipts'
                ? {
                    eventKind: 'payment_receipt' as const,
                    fallbackStatus: 'reported',
                    schema: PylonApiPaymentReceiptRequest,
                  }
                : {
                    eventKind: 'settlement_status' as const,
                    fallbackStatus: 'reported',
                    schema: PylonApiSettlementStatusRequest,
                  }

      return routeEvent(dependencies, request, env, {
        assignmentRef: decodeURIComponent(assignmentMatch[2]!),
        eventKind: route.eventKind,
        fallbackStatus: route.fallbackStatus,
        pylonRef: decodeURIComponent(assignmentMatch[1]!),
        schema: route.schema,
      }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    return undefined
  },
})
