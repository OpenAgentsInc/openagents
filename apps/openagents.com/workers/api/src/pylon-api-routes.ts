import { notFound } from '@openagentsinc/sync-worker'
import { Effect, Match as M, Schema as S } from 'effect'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
  type OpenAuthAgentLinkRecord,
  type ProgrammaticAgentSession,
  authenticateProgrammaticAgent,
  sha256Hex,
} from './agent-registration'
import type { AutopilotWorkerCloseoutIngestionInput } from './autopilot-work-routes'
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
  PylonApiAssignmentWorkerCloseoutRequest,
  PylonApiCreateAssignmentRequest,
  type PylonApiEventKind,
  PylonApiHeartbeatRequest,
  PylonApiPaymentReceiptRequest,
  PylonApiPayoutTargetAdmissionRequest,
  type PylonApiRegistrationRecord,
  PylonApiRegistrationRequest,
  PylonApiSettlementStatusRequest,
  PylonApiSparkPayoutTargetRegisterRequest,
  type PylonApiStore,
  PylonApiStoreError,
  PylonApiWalletReadinessRequest,
  type PylonSparkPayoutTargetReadiness,
  type PylonSparkPayoutTargetStore,
  SPARK_PAYOUT_TARGET_NOT_READY,
  buildPylonApiAssignmentRecord,
  buildPylonApiEventRecord,
  buildPylonApiRegistrationRecord,
  closeoutPylonApiAssignmentRecord,
  nextAssignmentForEvent,
  nextRegistrationForEvent,
  publicPylonApiAssignmentProjection,
  publicPylonApiEventProjection,
  publicPylonApiRegistrationProjection,
  pylonCodingServiceCapacityProjection,
  pylonApiStoreErrorFromUnknown,
  pylonClientVersionMeetsMinimum,
  resolveSparkPayoutTargetReadiness,
} from './pylon-api'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  TASSADAR_DISPATCH_CAPABILITY_UNRECEIPTED_BLOCKER_REF,
  admitTassadarExecutorCapabilityClaim,
  tassadarDispatchCapabilityUnreceipted,
} from './tassadar-capability-admission'

type HttpResponse = globalThis.Response

type PylonApiRouteDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  makeId?: () => string
  makeStore: (env: Bindings) => PylonApiStore
  // #5252: private operator-only store for raw Spark payout targets. Optional so
  // existing route wiring/tests stay valid; the spark-payout-target route fails
  // closed (501) when it is not wired.
  makeSparkPayoutTargetStore?: (env: Bindings) => PylonSparkPayoutTargetStore
  nowIso?: () => string
  recordAutopilotWorkerCloseout?: (
    env: Bindings,
    input: AutopilotWorkerCloseoutIngestionInput,
  ) => Promise<unknown>
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<PylonApiBrowserSession | undefined>
}>

type PylonApiRouteEnv = Readonly<Record<string, unknown>>
type PylonApiBrowserSession = Readonly<{
  user: Readonly<{
    email?: string
    name?: string
    userId: string
  }>
}>

const LinkOpenAuthAgentRequest = S.Struct({
  agentToken: S.Trim.check(
    S.isMinLength(AGENT_TOKEN_PREFIX.length + 8),
    S.isMaxLength(2048),
    S.isPattern(/^oa_agent_[A-Za-z0-9_-]+$/),
  ),
})

type LinkOpenAuthAgentRequest = typeof LinkOpenAuthAgentRequest.Type

// Presence contract (#5058): Pylon presence and lifecycle writes are
// agent-token authenticated. A node's self-held Nostr key proves Nostr
// identity, not ownership of a Pylon registration: registrations are bound
// to `ownerAgentUserId` from the bearer-token session, and the registry does
// not bind a verified Nostr pubkey to that owner. The Worker therefore does
// not accept a NIP-98 self-signed heartbeat as presence authority. When a
// request arrives with a Nostr/NIP-98 `Authorization` scheme we return an
// explanatory 401 that names the token-only contract and points the node at
// the bearer-token path, instead of a bare `unauthorized`.
const PYLON_API_PRESENCE_REQUIRES_AGENT_TOKEN =
  'pylon_api_presence_requires_agent_token'

class PylonApiUnauthorized extends S.TaggedErrorClass<PylonApiUnauthorized>()(
  'PylonApiUnauthorized',
  { presenceContract: S.optionalKey(S.Boolean) },
) {}

type PylonApiRouteError = PylonApiStoreError | PylonApiUnauthorized

const presenceContractUnauthorized = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: PYLON_API_PRESENCE_REQUIRES_AGENT_TOKEN,
      reason:
        'Pylon presence and lifecycle writes are authenticated with an OpenAgents agent bearer token. A self-signed Nostr (NIP-98) signature proves Nostr identity but is not accepted as presence authority, because Pylon registrations are bound to the owning agent token, not to a Nostr pubkey. Send this request with `Authorization: Bearer <agent token>`.',
    },
    { headers: { 'www-authenticate': 'Bearer' }, status: 401 },
  )

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
      PylonApiUnauthorized: unauthorizedError =>
        unauthorizedError.presenceContract === true
          ? presenceContractUnauthorized()
          : unauthorized(),
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

// A NIP-98 self-signed request uses the `Nostr` authorization scheme
// (see apps/pylon `encodeNip98Authorization`). Detecting it lets the
// presence routes return the documented token-only contract (#5058)
// instead of a bare 401.
const isNostrSignedRequest = (request: Request): boolean => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return false
  }

  const [scheme] = authorization.split(' ')

  return scheme?.toLowerCase() === 'nostr'
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

const requireBrowser = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<PylonApiBrowserSession, PylonApiUnauthorized> => {
  if (dependencies.requireBrowserSession === undefined) {
    return Effect.fail(new PylonApiUnauthorized({}))
  }

  return Effect.flatMap(
    Effect.tryPromise({
      catch: () => new PylonApiUnauthorized({}),
      try: () => dependencies.requireBrowserSession!(request, env, ctx),
    }),
    session =>
      session === undefined
        ? Effect.fail(new PylonApiUnauthorized({}))
        : Effect.succeed(session),
  )
}

// #5306 onboarding backstop: resolve the node's Spark payout-target readiness
// from the private operator store keyed by pylonRef. Fails closed — when the
// store dependency is not wired or the read errors, readiness is not-ready, so
// the public projection shows a visible, self-healing gap rather than a
// fabricated target. Because this is recomputed on every register/heartbeat/read,
// the flag flips to ready with no manual step once the node (#5305) auto-registers.
const resolveRouteSparkPayoutTargetReadiness = <
  Bindings extends PylonApiRouteEnv,
>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
  pylonRef: string,
): Effect.Effect<PylonSparkPayoutTargetReadiness> => {
  const makeSparkStore = dependencies.makeSparkPayoutTargetStore
  if (makeSparkStore === undefined) {
    return Effect.succeed(SPARK_PAYOUT_TARGET_NOT_READY)
  }

  return Effect.promise(() =>
    resolveSparkPayoutTargetReadiness(makeSparkStore(env), pylonRef),
  )
}

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

export type ControlledPylonAssignmentDispatchGate = Readonly<{
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

const codingServiceByCapabilityRef = new Map<string, 'claude' | 'codex'>([
  ['capability.pylon.local_claude_agent', 'claude'],
  ['capability.pylon.local_codex', 'codex'],
])

const activeDuplicateCapacitySlots = (
  input: Readonly<{
    body: PylonApiCreateAssignmentRequest
    registration: PylonApiRegistrationRecord | undefined
  }>,
): number => {
  if (input.registration === undefined) {
    return 1
  }

  const requestedServices = new Set(
    gateRefs(input.body.requiredCapabilityRefs)
      .map(ref => codingServiceByCapabilityRef.get(ref))
      .filter((service): service is 'claude' | 'codex' => service !== undefined),
  )

  if (requestedServices.size !== 1) {
    return 1
  }

  const [requestedService] = requestedServices
  const capacity = pylonCodingServiceCapacityProjection(input.registration).find(
    item => item.service === requestedService,
  )
  const advertisedSlots =
    capacity === undefined
      ? 1
      : capacity.ready > 0
        ? capacity.ready
        : capacity.available

  return Math.max(1, advertisedSlots)
}

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

export const controlledPylonAssignmentDispatchGate = (
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
  const duplicateCapacitySlots = activeDuplicateCapacitySlots({
    body,
    registration,
  })
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
    ...(registration !== undefined &&
    tassadarDispatchCapabilityUnreceipted(
      gateRefs(body.requiredCapabilityRefs),
      registration.capabilityRefs,
    )
      ? [TASSADAR_DISPATCH_CAPABILITY_UNRECEIPTED_BLOCKER_REF]
      : []),
    ...(duplicateRefs.length >= duplicateCapacitySlots
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
    return Effect.fail(
      new PylonApiUnauthorized(
        isNostrSignedRequest(request) ? { presenceContract: true } : {},
      ),
    )
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

      const existingSparkReadiness =
        yield* resolveRouteSparkPayoutTargetReadiness(
          dependencies,
          env,
          existingRegistration.pylonRef,
        )

      return noStoreJsonResponse(
        {
          event: publicPylonApiEventProjection(existingEvent, nowIso),
          idempotent: true,
          pylon: publicPylonApiRegistrationProjection(
            existingRegistration,
            nowIso,
            existingSparkReadiness,
          ),
        },
        { status: 200 },
      )
    }

    // W4.1 (#4750): a Tassadar executor-capability claim is admitted
    // only with its self-test receipt ref. Refused claims are stripped
    // before the registration row is built, so unreceipted executor
    // capacity never becomes dispatchable registry state.
    const tassadarAdmission = admitTassadarExecutorCapabilityClaim(
      body.capabilityRefs ?? [],
    )
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
          request: {
            ...body,
            capabilityRefs: tassadarAdmission.admittedCapabilityRefs,
          },
        }),
    })
    const event = yield* Effect.try({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        buildPylonApiEventRecord({
          body: {
            capabilityRefs: registration.capabilityRefs,
            capabilityRefusalRefs: tassadarAdmission.refusalRefs,
            clientProtocolVersion: registration.clientProtocolVersion,
            clientVersion: registration.clientVersion,
            providerMarketRelayRefs: registration.providerMarketRelayRefs,
            providerNip90LaneRefs: registration.providerNip90LaneRefs,
            providerNostrNpub: registration.providerNostrNpub,
            providerNostrPubkey: registration.providerNostrPubkey,
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
    const sparkReadiness = yield* resolveRouteSparkPayoutTargetReadiness(
      dependencies,
      env,
      storedRegistration.pylonRef,
    )

    return noStoreJsonResponse(
      {
        event: publicPylonApiEventProjection(eventResult.record, nowIso),
        idempotent: eventResult.idempotent,
        pylon: publicPylonApiRegistrationProjection(
          storedRegistration,
          nowIso,
          sparkReadiness,
        ),
        tassadarCapabilityAdmission: {
          refusalRefs: tassadarAdmission.refusalRefs,
          selfTestReceiptRefs: tassadarAdmission.selfTestReceiptRefs,
          state: tassadarAdmission.state,
        },
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

const assignmentAcceptanceCanClaim = (
  eventKind: PylonApiEventKind,
  assignmentState: PylonApiAssignmentState,
): boolean => eventKind !== 'assignment_acceptance' || assignmentState === 'offered'

const maybeRecordAutopilotWorkerCloseout = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    assignment: PylonApiAssignmentRecord
    body: Record<string, unknown>
    eventKind: PylonApiEventKind
    nowIso: string
  }>,
): Effect.Effect<void, PylonApiStoreError> => {
  const recordAutopilotWorkerCloseout =
    dependencies.recordAutopilotWorkerCloseout

  return input.eventKind !== 'worker_closeout' ||
    recordAutopilotWorkerCloseout === undefined
    ? Effect.void
    : Effect.tryPromise({
        catch: pylonApiStoreErrorFromUnknown,
        try: () =>
          recordAutopilotWorkerCloseout(env, {
            assignment: input.assignment,
            body: input.body,
            nowIso: input.nowIso,
          }),
      }).pipe(Effect.asVoid)
}

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
      const replaySparkReadiness =
        yield* resolveRouteSparkPayoutTargetReadiness(
          dependencies,
          env,
          existingRegistration.pylonRef,
        )

      return noStoreJsonResponse(
        {
          event: publicPylonApiEventProjection(existingEvent, replayNowIso),
          idempotent: true,
          pylon: publicPylonApiRegistrationProjection(
            existingRegistration,
            replayNowIso,
            replaySparkReadiness,
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

      if (
        !assignmentAcceptanceCanClaim(input.eventKind, assignment.state)
      ) {
        return routeErrorResponse(
          new PylonApiStoreError({
            kind: 'conflict',
            reason: 'Pylon assignment was already claimed.',
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
    const claimedAcceptanceAssignment =
      assignment !== undefined && input.eventKind === 'assignment_acceptance'
        ? yield* Effect.flatMap(
            Effect.tryPromise({
              catch: pylonApiStoreErrorFromUnknown,
              try: () =>
                store.updateAssignmentIfState(
                  nextAssignmentForEvent(assignment, event, nowIso),
                  'offered',
                ),
            }),
            record =>
              record === undefined
                ? Effect.fail(
                    new PylonApiStoreError({
                      kind: 'conflict',
                      reason: 'Pylon assignment was already claimed.',
                    }),
                  )
                : Effect.succeed(record),
          )
        : undefined
    const eventResult = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.createEvent(event),
    })
    const storedAssignment =
      assignment === undefined
        ? undefined
        : input.eventKind === 'assignment_acceptance'
          ? claimedAcceptanceAssignment
          : yield* Effect.tryPromise({
              catch: pylonApiStoreErrorFromUnknown,
              try: () =>
                store.updateAssignment(
                  nextAssignmentForEvent(
                    assignment,
                    eventResult.record,
                    nowIso,
                  ),
                ),
            })
    if (storedAssignment !== undefined) {
      yield* maybeRecordAutopilotWorkerCloseout(dependencies, env, {
        assignment: storedAssignment,
        body,
        eventKind: input.eventKind,
        nowIso,
      })
    }
    const nextRegistration = nextRegistrationForEvent(
      registration,
      eventResult.record,
      nowIso,
    )
    const storedRegistration = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.upsertRegistration(nextRegistration),
    })
    const sparkReadiness = yield* resolveRouteSparkPayoutTargetReadiness(
      dependencies,
      env,
      storedRegistration.pylonRef,
    )

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
        pylon: publicPylonApiRegistrationProjection(
          storedRegistration,
          nowIso,
          sparkReadiness,
        ),
      },
      { status: eventResult.idempotent ? 200 : 201 },
    )
  })

// #5252: derive the public-safe redacted Spark payout-target ref from the raw
// address. Mirrors the Pylon `sparkPayoutTargetRef` (sha256(raw).slice(0,24)) so
// the server can verify the client's declared digest actually corresponds to
// the raw address it sent, rather than trusting an arbitrary ref.
const deriveSparkPayoutTargetRef = (
  rawSparkAddress: string,
): Effect.Effect<string> =>
  Effect.map(
    Effect.promise(() => sha256Hex(rawSparkAddress.trim())),
    digest => `payout.spark.${digest.slice(0, 24)}`,
  )

// #5252: register a raw Spark address as the agent's OWN payout target. The raw
// `spark1…` rides ONLY this authenticated request body, is stored in the private
// operator store keyed to pylonRef + owning agent, and is NEVER projected,
// logged, or persisted into a public event. The public projection — including
// the emitted `payout_target_admission` event body — carries ONLY the redacted
// `payout.spark.<digest>` ref. Auth is the agent's own bearer token, so a node
// can only set its own target. The private upsert and the public event are both
// idempotency-keyed, so re-registering the same address is a no-op update.
const routeRegisterSparkPayoutTarget = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  pylonRef: string,
) =>
  Effect.gen(function* () {
    const session = yield* requireAgent(dependencies, request, env)

    const makeSparkStore = dependencies.makeSparkPayoutTargetStore
    if (makeSparkStore === undefined) {
      return noStoreJsonResponse(
        {
          error: 'pylon_api_spark_payout_target_unavailable',
          reason:
            'Raw Spark payout-target registration is not wired in this deployment.',
        },
        { status: 501 },
      )
    }

    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const store = routeStore(dependencies, env)
    const sparkStore = makeSparkStore(env)

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

      if (existingEvent.eventKind !== 'payout_target_admission') {
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
      const replaySparkReadiness = yield* Effect.promise(() =>
        resolveSparkPayoutTargetReadiness(
          sparkStore,
          existingRegistration.pylonRef,
        ),
      )

      return noStoreJsonResponse(
        {
          event: publicPylonApiEventProjection(existingEvent, replayNowIso),
          idempotent: true,
          pylon: publicPylonApiRegistrationProjection(
            existingRegistration,
            replayNowIso,
            replaySparkReadiness,
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
    // The schema validates the raw `spark1…` shape and the redacted digest ref
    // shape at the JSON boundary; the raw address never reaches here unvalidated.
    const body = yield* decodeBody(
      request,
      PylonApiSparkPayoutTargetRegisterRequest,
    )

    // Verify the declared redacted ref actually corresponds to the raw address.
    // A mismatch fails closed: we never store a raw address under a ref a
    // resolver would not derive for it.
    const derivedRef = yield* deriveSparkPayoutTargetRef(body.rawSparkAddress)
    if (derivedRef !== body.payoutTargetRef) {
      return routeErrorResponse(
        new PylonApiStoreError({
          kind: 'validation_error',
          reason:
            'payoutTargetRef does not match the digest of the provided Spark address.',
        }),
      )
    }

    const nowIso = routeNowIso(dependencies)

    // PRIVATE write FIRST: store the raw address operator-only, keyed to this
    // pylon and bound to the owning agent. Idempotent upsert.
    yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        sparkStore.upsert({
          pylonRef: registration.pylonRef,
          ownerAgentUserId: session.user.id,
          payoutTargetRef: derivedRef,
          rawSparkAddress: body.rawSparkAddress,
          createdAt: nowIso,
          updatedAt: nowIso,
        }),
    })

    // PUBLIC event: carries ONLY the redacted digest ref — never the raw address.
    const event = yield* Effect.try({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        buildPylonApiEventRecord({
          body: {
            admissionRefs: ['admission.public.pylon.payout_target.registered'],
            payoutTargetRef: derivedRef,
            policyRefs: [
              'policy.public.pylon.redacted_payout_target_only',
              'policy.private.pylon.spark_payout_target_raw_stored_operator_only',
            ],
            status: 'registered',
          },
          eventKind: 'payout_target_admission',
          idempotencyKeyHash,
          makeId: () => routeMakeId(dependencies),
          nowIso,
          ownerAgentUserId: session.user.id,
          pylonRef: registration.pylonRef,
          status: 'registered',
        }),
    })
    const eventResult = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => store.createEvent(event),
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
    // Recompute readiness from the private store right after the upsert: this
    // is the moment a node-registered target becomes visible as ready (#5306).
    const sparkReadiness = yield* Effect.promise(() =>
      resolveSparkPayoutTargetReadiness(
        sparkStore,
        storedRegistration.pylonRef,
      ),
    )

    return noStoreJsonResponse(
      {
        event: publicPylonApiEventProjection(eventResult.record, nowIso),
        idempotent: eventResult.idempotent,
        pylon: publicPylonApiRegistrationProjection(
          storedRegistration,
          nowIso,
          sparkReadiness,
        ),
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

    const pylons = yield* Effect.forEach(
      registrations,
      registration =>
        resolveRouteSparkPayoutTargetReadiness(
          dependencies,
          env,
          registration.pylonRef,
        ).pipe(
          Effect.map(readiness =>
            publicPylonApiRegistrationProjection(
              registration,
              nowIso,
              readiness,
            ),
          ),
        ),
      { concurrency: 8 },
    )

    return noStoreJsonResponse({ pylons })
  })

const routeListAccountPylons = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireBrowser(dependencies, request, env, ctx)
    const nowIso = routeNowIso(dependencies)
    const agentStore = routeAgentStore(dependencies, env)
    if (agentStore.listLinkedAgentsForOpenAuthUser === undefined) {
      return routeErrorResponse(
        new PylonApiStoreError({
          kind: 'storage_error',
          reason: 'OpenAuth agent link store is not wired.',
        }),
      )
    }
    const linkedAgents = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        agentStore.listLinkedAgentsForOpenAuthUser!(session.user.userId, 100),
    })
    const ownerAgentUserIds = linkedAgents.map(agent => agent.agentUserId)
    const store = routeStore(dependencies, env)
    const registrations = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () =>
        store.listRegistrationsForOwnerAgentUserIds === undefined
          ? store
              .listRegistrations(200)
              .then(rows =>
                rows.filter(row =>
                  ownerAgentUserIds.includes(row.ownerAgentUserId),
                ),
              )
          : store.listRegistrationsForOwnerAgentUserIds(ownerAgentUserIds, 200),
    })
    const pylonRefs = registrations.map(registration => registration.pylonRef)
    const assignments = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: async () =>
        store.listAssignmentsForPylons === undefined
          ? (
              await Promise.all(
                pylonRefs.map(pylonRef =>
                  store.listAssignmentsForPylon(pylonRef, 25),
                ),
              )
            ).flat()
          : store.listAssignmentsForPylons(pylonRefs, 100),
    })
    const events = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: async () =>
        (
          await Promise.all(
            pylonRefs.map(pylonRef => store.listEventsForPylon(pylonRef, 10)),
          )
        )
          .flat()
          .sort((left, right) => right.createdAt.localeCompare(left.createdAt))
          .slice(0, 100),
    })
    const pylons = yield* Effect.forEach(
      registrations,
      registration =>
        resolveRouteSparkPayoutTargetReadiness(
          dependencies,
          env,
          registration.pylonRef,
        ).pipe(
          Effect.map(readiness =>
            publicPylonApiRegistrationProjection(
              registration,
              nowIso,
              readiness,
            ),
          ),
        ),
      { concurrency: 8 },
    )

    return noStoreJsonResponse({
      activity: {
        assignments: [...assignments]
          .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
          .slice(0, 100)
          .map(assignment =>
            publicPylonApiAssignmentProjection(assignment, nowIso),
          ),
        events: events.map(event =>
          publicPylonApiEventProjection(event, nowIso),
        ),
      },
      linkedAgents: linkedAgents.map(agent => ({
        agentRef: `agent:${agent.agentUserId}`,
        displayName: agent.displayName,
        linkKind: agent.linkKind,
        tokenPrefix: agent.tokenPrefix,
      })),
      pylons,
      summary: {
        linkedAgentCount: linkedAgents.length,
        onlineCodingPylonCount: pylons.filter(pylon =>
          pylon.codingCapacity.some(capacity => capacity.available > 0),
        ).length,
        pylonCount: pylons.length,
      },
    })
  })

const routeLinkOpenAuthAgent = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* requireBrowser(dependencies, request, env, ctx)
    const body = yield* decodeBody(request, LinkOpenAuthAgentRequest)
    const agentStore = routeAgentStore(dependencies, env)
    if (agentStore.linkOpenAuthAgent === undefined) {
      return routeErrorResponse(
        new PylonApiStoreError({
          kind: 'storage_error',
          reason: 'OpenAuth agent link store is not wired.',
        }),
      )
    }
    const nowIso = routeNowIso(dependencies)
    const agentSession = yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => authenticateProgrammaticAgent(agentStore, body.agentToken),
    })

    if (agentSession === undefined) {
      return unauthorized()
    }

    if (
      agentSession.credential.openauthUserId !== null &&
      agentSession.credential.openauthUserId !== session.user.userId
    ) {
      return routeErrorResponse(
        new PylonApiStoreError({
          kind: 'forbidden',
          reason:
            'Agent credential is already linked to another OpenAuth user.',
        }),
      )
    }

    const linkRecord: OpenAuthAgentLinkRecord = {
      agentCredentialId: agentSession.credential.id,
      agentUserId: agentSession.user.id,
      createdAt: nowIso,
      id: `openauth_agent_link_${routeMakeId(dependencies)}`,
      linkKind: 'credential_anchor',
      openauthUserId: session.user.userId,
      revokedAt: null,
      status: 'active',
      updatedAt: nowIso,
    }

    yield* Effect.tryPromise({
      catch: pylonApiStoreErrorFromUnknown,
      try: () => agentStore.linkOpenAuthAgent!(linkRecord),
    })

    return noStoreJsonResponse(
      {
        linkedAgent: {
          agentRef: `agent:${agentSession.user.id}`,
          displayName: agentSession.user.displayName,
          linkKind: linkRecord.linkKind,
          tokenPrefix: agentSession.credential.tokenPrefix,
        },
      },
      { status: 201 },
    )
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

    const sparkReadiness = yield* resolveRouteSparkPayoutTargetReadiness(
      dependencies,
      env,
      registration.pylonRef,
    )

    return noStoreJsonResponse({
      events: events.map(event => publicPylonApiEventProjection(event, nowIso)),
      pylon: publicPylonApiRegistrationProjection(
        registration,
        nowIso,
        sparkReadiness,
      ),
    })
  })

export const makePylonApiRoutes = <Bindings extends PylonApiRouteEnv>(
  dependencies: PylonApiRouteDependencies<Bindings>,
) => ({
  routePylonApiRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/account/pylons') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeListAccountPylons(dependencies, request, env, ctx).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/account/pylon-agent-links') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeLinkOpenAuthAgent(dependencies, request, env, ctx).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

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

    // #5252: raw Spark address registration as a payout target. The raw address
    // rides only the authenticated request body and is stored privately; the
    // public projection carries only the redacted `payout.spark.<digest>` ref.
    const sparkPayoutTargetMatch =
      /^\/api\/pylons\/([^/]+)\/spark-payout-target$/.exec(url.pathname)

    if (sparkPayoutTargetMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeRegisterSparkPayoutTarget(
        dependencies,
        request,
        env,
        decodeURIComponent(sparkPayoutTargetMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const assignmentMatch =
      /^\/api\/pylons\/([^/]+)\/assignments\/([^/]+)\/(accept|progress|artifacts|closeout|payment-receipts|settlement-status)$/.exec(
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
              : action === 'closeout'
                ? {
                    eventKind: 'worker_closeout' as const,
                    fallbackStatus: 'closeout_submitted',
                    schema: PylonApiAssignmentWorkerCloseoutRequest,
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
