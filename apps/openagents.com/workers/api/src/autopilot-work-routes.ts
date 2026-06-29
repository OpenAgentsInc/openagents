import { Effect, Exit, Match as M, Schema as S } from 'effect'

import {
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  autopilotCodingAssignmentsForWork,
} from './autopilot-coding-assignment'
import {
  validateAutopilotDecisionCloseoutReceipt,
} from './autopilot-decision-closeout'
import { missionBriefingForWorkOrder } from './autopilot-mission-briefing'
import {
  assignmentIntentsForWorkOrder,
  type AutopilotWorkAssignmentIntentProjection,
} from './autopilot-work-assignment-planner'
import {
  fallbackLeaseIntentsForAutopilotWork,
  type AutopilotFallbackLeaseIntentProjection,
} from './autopilot-work-fallback-lease-adapter'
import {
  type AutopilotPlacementDecisionProjection,
  selectAutopilotPlacement,
} from './autopilot-work-placement-selector'
import {
  pylonAssignmentIntentsForAutopilotWork,
  type AutopilotPylonAssignmentIntentProjection,
} from './autopilot-work-pylon-assignment-synthesizer'
import {
  authenticateCustomerOrderAgentRequest,
  CustomerOrderAgentAuthFailure,
  type CustomerOrderAgentScope,
} from './customer-order-agent-auth'
import {
  evaluateLaneCFanoutForWorkOrder,
  laneCFanoutObjectiveRef,
} from './lane-c-fanout-bridge'
import { isMarketplaceWorkClassId } from './marketplace-work-class-catalog'
import { buildSelfServeFanoutPlan } from './self-serve-fanout'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  parseJsonStringArray,
  parseJsonUnknown,
  readJsonObject,
} from './json-boundary'
import {
  formatOpenAgentsL402WwwAuthenticate,
  parseOpenAgentsPaymentHeaders,
} from './l402-payment-headers'
import {
  type OpenAgentsL402CredentialPayload,
  type OpenAgentsL402SigningBoundary,
  l402PayloadFromBuyerPaymentChallenge,
  mintOpenAgentsL402Credential,
  verifyOpenAgentsL402Credential,
} from './l402-credential-service'
import {
  currentIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from './runtime-primitives'
import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentLedgerStore,
  BuyerPaymentLedgerAmount,
} from './buyer-payment-ledger'
import type {
  PylonApiAssignmentRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'
import { buildPylonApiAssignmentRecord } from './pylon-api'
import {
  type AutopilotWorkQuote,
  makeAutopilotWorkQuote,
} from './autopilot-work-quote'
import {
  type AutopilotWorkPricingLanePolicy,
  type AutopilotWorkPricingPolicy,
  autopilotWorkPricingPolicy,
  pricingLaneForRunnerKind,
} from './autopilot-work-pricing-policy'
import {
  type OpenAgentsAutopilotAccessRequestKind,
  type OpenAgentsAutopilotRequestedCodingAdapter,
  OpenAgentsAutopilotRunnerKind,
  type OpenAgentsAutopilotRunnerKind as OpenAgentsAutopilotRunnerKindType,
  OpenAgentsAutopilotWorkState,
  type OpenAgentsAutopilotWorkRequest,
  type OpenAgentsAutopilotWorkState as OpenAgentsAutopilotWorkStateType,
  decodeOpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'
import {
  type AutopilotWorkScheduledLaunchProjection,
  AutopilotWorkScheduledLaunchRecord,
  dispatchedScheduledLaunch,
  expiredScheduledLaunch,
  scheduledLaunchDue,
  scheduledLaunchHoldsDispatch,
  scheduledLaunchHorizonReason,
  scheduledLaunchProjection,
  scheduledLaunchRecordForRequest,
  scheduledLaunchRetryAfterSeconds,
  scheduledLaunchWindowExpired,
} from './autopilot-work-scheduled-launch'

type HttpResponse = globalThis.Response

const errorReason = (error: unknown): string =>
  typeof error === 'object' &&
  error !== null &&
  'reason' in error &&
  typeof error.reason === 'string'
    ? error.reason
    : error instanceof Error
      ? error.message
      : String(error)

export class AutopilotWorkStoreError extends S.TaggedErrorClass<AutopilotWorkStoreError>()(
  'AutopilotWorkStoreError',
  {
    kind: S.Literals([
      'conflict',
      'not_found',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export type AutopilotWorkStoreErrorKind = AutopilotWorkStoreError['kind']

export type AutopilotWorkAccessGrantAction =
  | 'authorize_github_branch'
  | 'authorize_github_pull_request'
  | 'connect_github_account'
  | 'connect_github_repository'
  | 'configure_secret_broker'
  | 'confirm_privacy_tier'
  | 'customer_review'
  | 'enroll_pylon'
  | 'operator_review'
  | 'select_repository'

export type AutopilotWorkAccessRequirementProjection = Readonly<{
  accessRequestRef: string
  grantAction: AutopilotWorkAccessGrantAction
  kind: OpenAgentsAutopilotAccessRequestKind
  ownerActionRef: string
  reasonRef: string
  requiredBeforeLaunch: true
  status: 'missing'
  taskRef: string
}>

export type AutopilotWorkRepositoryAuthorityProjection = Readonly<{
  branch: string
  deployAuthority: false
  fullName: string
  provider: 'github'
  pullRequestAuthority:
    | 'owner_grant_required'
    | 'not_requested'
  readAuthority:
    | 'owner_grant_required'
    | 'public_read_available'
  spendAuthority: false
  taskRef: string
  visibility: 'internal' | 'private' | 'public'
  writeAuthority:
    | 'owner_grant_required'
    | 'not_requested'
}>

export type AutopilotWorkBuyerPaymentProof = Readonly<{
  proofRef: string
  source: 'l402' | 'mdk_checkout'
}>

export type AutopilotWorkPaymentChallengeProjection = Readonly<{
  amountCents: number
  challengeRef: string
  checkoutIntentRef: string | null
  checkoutUrlRef: string | null
  expiresAt: string
  kind: 'l402' | 'mdk_checkout'
  l402CredentialRef: string | null
  l402HeaderRef: string | null
  quoteRef: string
  status: 'paid_ready' | 'payment_required'
}>

export type AutopilotWorkFundingProjection = Readonly<{
  buyerFundingState: 'funded' | 'not_required' | 'payment_required'
  buyerPaymentProofRef: string | null
  fundedAmountCents: number
  quoteRef: string
  settlementBlockedReasonRef:
    | 'settlement.accepted_work_required'
    | 'settlement.buyer_payment_required'
    | 'settlement.no_worker_payout_mode'
  settlementEligible: false
  workerPayoutEligible: false
}>

export type AutopilotWorkReviewAction =
  | 'accept'
  | 'reject'
  | 'request_changes'

const AutopilotWorkReviewDecisionRecord = S.Struct({
  action: S.Literals(['accept', 'reject', 'request_changes']),
  actorAgentCredentialId: S.String,
  actorAgentUserId: S.String,
  decisionRefs: S.Array(S.String),
  idempotencyKeyHash: S.String,
  recordedAt: S.String,
  rejectionRefs: S.Array(S.String),
  revisionRequestRefs: S.Array(S.String),
})
export type AutopilotWorkReviewDecisionRecord =
  typeof AutopilotWorkReviewDecisionRecord.Type

export type AutopilotWorkReviewDecisionProjection =
  AutopilotWorkReviewDecisionRecord & Readonly<{
    acceptedWorkAuthority: false
    deployAuthority: false
    forumAutoPublishAllowed: false
    publicSafe: true
    settlementAuthority: false
    workerPayoutAuthority: false
  }>

export type AutopilotWorkPlacementPolicyRecordProjection = Readonly<{
  allowedRunnerKinds: OpenAgentsAutopilotWorkRequest['placementPolicy']['allowedRunnerKinds']
  auditable: true
  disallowedRunnerKinds: OpenAgentsAutopilotWorkRequest['placementPolicy']['disallowedRunnerKinds']
  localOnlyAllowed: boolean
  placementPolicyRef: string
  preferredRunnerKinds: OpenAgentsAutopilotWorkRequest['placementPolicy']['preferredRunnerKinds']
  privacyTier: OpenAgentsAutopilotWorkRequest['placementPolicy']['privacyTier']
  promptKeywordRouting: false
  publicTraceAllowed: boolean
  reasonRefs: ReadonlyArray<string>
  requiresSecretBroker: boolean
}>

export type AutopilotWorkNextActionProjection = Readonly<{
  callerActionRefs: ReadonlyArray<string>
  reasonRefs: ReadonlyArray<string>
  retryAfterSeconds: number | null
  state:
    | 'accepted'
    | 'blocked'
    | 'delivered'
    | 'needs_input'
    | 'payment_required'
    | 'ready'
    | 'rejected'
    | 'revision_required'
    | 'retry_later'
}>

export type AutopilotWorkTaskAccessState =
  | 'missing_required_access'
  | 'satisfied'

export type AutopilotWorkTaskLifecycleState =
  | 'access_required'
  | 'accepted'
  | 'blocked'
  | 'delivered'
  | 'payment_required'
  | 'queued_or_running'
  | 'rejected'
  | 'ready_for_assignment'
  | 'revision_required'
  | 'scheduled'

export type AutopilotWorkTaskPlacementState =
  | 'accepted'
  | 'blocked'
  | 'blocked_on_access'
  | 'blocked_on_payment'
  | 'delivered'
  | 'queued_or_running'
  | 'rejected'
  | 'ready_for_assignment'
  | 'revision_required'
  | 'scheduled'

export type AutopilotWorkTaskRecordProjection = Readonly<{
  acceptanceCriteriaRefs: ReadonlyArray<string>
  accessRequirements: ReadonlyArray<AutopilotWorkAccessRequirementProjection>
  accessState: AutopilotWorkTaskAccessState
  checkout: OpenAgentsAutopilotWorkRequest['tasks'][number]['checkout'] | null
  kind: OpenAgentsAutopilotWorkRequest['tasks'][number]['kind']
  lifecycleState: AutopilotWorkTaskLifecycleState
  objective: string
  paymentState: AutopilotWorkFundingProjection['buyerFundingState']
  placementState: AutopilotWorkTaskPlacementState
  requestedAdapter?: OpenAgentsAutopilotRequestedCodingAdapter | null
  requestedAdapterProfileRef?: string | null
  repository: OpenAgentsAutopilotWorkRequest['tasks'][number]['repository'] | null
  taskRef: string
}>

export type AutopilotWorkOrderRecord = Readonly<{
  accessRequestRefs: ReadonlyArray<string>
  agentCredentialId: string
  agentUserId: string
  archivedAt: string | null
  buyerPaymentProofRef: string | null
  clientRequestRef: string
  createdAt: string
  eventStreamRef: string
  executionCloseout: AutopilotWorkExecutionCloseoutRecord | null
  id: string
  idempotencyKeyHash: string
  ownerUserId: string
  paymentChallengeRef: string | null
  request: OpenAgentsAutopilotWorkRequest
  reviewDecision: AutopilotWorkReviewDecisionRecord | null
  scheduledLaunch: AutopilotWorkScheduledLaunchRecord | null
  state: OpenAgentsAutopilotWorkStateType
  statusUrlRef: string
  taskRefs: ReadonlyArray<string>
  updatedAt: string
  workOrderRef: string
}>

const AutopilotWorkExecutionCloseoutRecord = S.Struct({
  assignmentRefs: S.Array(S.String),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  authorityReceiptRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  buildRefs: S.optionalKey(S.Array(S.String)),
  changeCaptureRefs: S.optionalKey(S.Array(S.String)),
  changeCaptureStatus: S.optionalKey(
    S.Literals(['blocked', 'review_ready', 'stale']),
  ),
  closeoutRefs: S.Array(S.String),
  deliveryReadinessFreshness: S.optionalKey(S.Literals(['fresh', 'stale'])),
  deliveryReadinessRefs: S.optionalKey(S.Array(S.String)),
  deliveryReadinessStatus: S.optionalKey(
    S.Literals(['blocked', 'ready', 'scoped_exception']),
  ),
  fileCount: S.optionalKey(S.Number),
  addedLineCount: S.optionalKey(S.Number),
  patchDigestRef: S.optionalKey(S.NullOr(S.String)),
  previewRefs: S.optionalKey(S.Array(S.String)),
  proofRefs: S.Array(S.String),
  removedLineCount: S.optionalKey(S.Number),
  resultRefs: S.Array(S.String),
  reviewCaveatRefs: S.optionalKey(S.Array(S.String)),
  runnerKind: OpenAgentsAutopilotRunnerKind,
  summaryRefs: S.optionalKey(S.Array(S.String)),
  testRefs: S.optionalKey(S.Array(S.String)),
  verificationRefs: S.optionalKey(S.Array(S.String)),
  worktreeIdentityStatus: S.optionalKey(S.Literals(['blocked', 'ready', 'stale'])),
  writebackRequired: S.optionalKey(S.Boolean),
})
export type AutopilotWorkExecutionCloseoutRecord =
  typeof AutopilotWorkExecutionCloseoutRecord.Type

export type AutopilotWorkerCloseoutIngestionInput = Readonly<{
  assignment: PylonApiAssignmentRecord
  body: Record<string, unknown>
  nowIso: string
}>

export type AutopilotWorkExecutionCloseoutProjection =
  AutopilotWorkExecutionCloseoutRecord & Readonly<{
    acceptedWorkAuthority: false
    forumAutoPublishAllowed: false
    publicSafe: true
    workerPayoutAuthority: false
  }>

export type AutopilotWorkOrderProjection = Readonly<{
  accessRequirements: ReadonlyArray<AutopilotWorkAccessRequirementProjection>
  accessRequestRefs: ReadonlyArray<string>
  assignmentIntents: ReadonlyArray<AutopilotWorkAssignmentIntentProjection>
  buyerPaymentProofRef: string | null
  clientRequestRef: string
  createdAt: string
  eventStreamRef: string
  executionCloseout: AutopilotWorkExecutionCloseoutProjection | null
  fallbackLeaseIntents: ReadonlyArray<AutopilotFallbackLeaseIntentProjection>
  funding: AutopilotWorkFundingProjection
  generatedAt: string
  idempotent: boolean
  nextAction: AutopilotWorkNextActionProjection
  paymentChallenge: AutopilotWorkPaymentChallengeProjection | null
  paymentChallengeRef: string | null
  placementDecision: AutopilotPlacementDecisionProjection
  placementPolicy: AutopilotWorkPlacementPolicyRecordProjection
  pricingPolicy: AutopilotWorkPricingPolicy & Readonly<{
    activeLane: AutopilotWorkPricingLanePolicy | null
  }>
  pylonAssignmentIntents: ReadonlyArray<AutopilotPylonAssignmentIntentProjection>
  promiseRef: Readonly<{
    blockerRefs: ReadonlyArray<string>
    promiseId: string
    registryVersion: string
  }> | null
  quote: AutopilotWorkQuote
  repositoryAuthorities: ReadonlyArray<AutopilotWorkRepositoryAuthorityProjection>
  reviewDecision: AutopilotWorkReviewDecisionProjection | null
  scheduledLaunch: AutopilotWorkScheduledLaunchProjection | null
  state: OpenAgentsAutopilotWorkStateType
  statusUrlRef: string
  taskRefs: ReadonlyArray<string>
  tasks: ReadonlyArray<AutopilotWorkTaskRecordProjection>
  updatedAt: string
  workOrderRef: string
}>

export type AutopilotWorkEventKind =
  | 'accepted'
  | 'blocked'
  | 'delivered'
  | 'needs_access'
  | 'payment_required'
  | 'queued'
  | 'rejected'
  | 'revision_required'
  | 'running'
  | 'scheduled'
  | 'settled'

export type AutopilotWorkEventProjection = Readonly<{
  eventKind: AutopilotWorkEventKind
  eventRef: string
  occurredAt: string
  publicSafe: true
  sequence: number
  state: OpenAgentsAutopilotWorkStateType
  taskRefs: ReadonlyArray<string>
  workOrderRef: string
}>

export type AutopilotWorkStore = Readonly<{
  createWorkOrder: (
    record: AutopilotWorkOrderRecord,
  ) => Promise<Readonly<{ idempotent: boolean; record: AutopilotWorkOrderRecord }>>
  listWorkOrdersForOwner: (
    input: Readonly<{ limit: number; ownerUserId: string }>,
  ) => Promise<ReadonlyArray<AutopilotWorkOrderRecord>>
  recordPylonAssignmentDispatch: (
    input: Readonly<{
      ownerUserId: string
      updatedAt: string
      workOrderRef: string
    }>,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
  recordExecutionCloseout: (
    input: Readonly<{
      executionCloseout: AutopilotWorkExecutionCloseoutRecord
      ownerUserId: string
      updatedAt: string
      workOrderRef: string
    }>,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
  recordReviewDecision: (
    input: Readonly<{
      ownerUserId: string
      reviewDecision: AutopilotWorkReviewDecisionRecord
      state: Extract<
        OpenAgentsAutopilotWorkStateType,
        'accepted' | 'rejected' | 'revision_required'
      >
      updatedAt: string
      workOrderRef: string
    }>,
  ) => Promise<
    | Readonly<{ idempotent: boolean; record: AutopilotWorkOrderRecord }>
    | undefined
  >
  recordDecisionCloseoutReceipt?: (
    receipt: import('./autopilot-decision-closeout').AutopilotDecisionCloseoutReceipt,
  ) => Promise<void>
  listDecisionCloseoutReceiptsForWorkOrder?: (
    input: Readonly<{ ownerUserId: string; workOrderRef: string }>,
  ) => Promise<
    ReadonlyArray<
      import('./autopilot-decision-closeout').AutopilotDecisionCloseoutReceipt
    >
  >
  readDecisionCloseoutReceipt?: (
    input: Readonly<{ closeoutRef: string; ownerUserId: string }>,
  ) => Promise<
    | import('./autopilot-decision-closeout').AutopilotDecisionCloseoutReceipt
    | undefined
  >
  recordBuyerPaymentProof: (
    input: Readonly<{
      buyerPaymentProofRef: string
      ownerUserId: string
      updatedAt: string
      workOrderRef: string
    }>,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
  readWorkOrder: (
    workOrderRef: string,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
  readWorkOrderByIdempotency: (
    ownerUserId: string,
    idempotencyKeyHash: string,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
  listPendingScheduledWorkOrders: (
    input: Readonly<{ limit: number }>,
  ) => Promise<ReadonlyArray<AutopilotWorkOrderRecord>>
  recordScheduledLaunchTransition: (
    input: Readonly<{
      ownerUserId: string
      scheduledLaunch: AutopilotWorkScheduledLaunchRecord
      state: OpenAgentsAutopilotWorkStateType
      updatedAt: string
      workOrderRef: string
    }>,
  ) => Promise<AutopilotWorkOrderRecord | undefined>
}>

export type AutopilotWorkExecutor = (
  input: Readonly<{
    nowIso: string
    work: AutopilotWorkOrderProjection
  }>,
) => Promise<AutopilotWorkExecutionCloseoutRecord | undefined>

type AutopilotPylonApiStore = Pick<PylonApiStore, 'listRegistrations'> &
  Partial<
    Pick<
      PylonApiStore,
      | 'createAssignment'
      | 'listAssignmentsForPylon'
      | 'readAssignment'
      | 'readAssignmentByIdempotencyKeyHash'
      | 'readRegistration'
    >
  >

type AutopilotPylonAssignmentLeaseStore =
  Pick<
    PylonApiStore,
    | 'createAssignment'
    | 'listAssignmentsForPylon'
    | 'readAssignment'
    | 'readAssignmentByIdempotencyKeyHash'
    | 'readRegistration'
  >

type AutopilotWorkRoutesDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  executeReadyWork?: (
    env: Bindings,
    input: Parameters<AutopilotWorkExecutor>[0],
  ) => ReturnType<AutopilotWorkExecutor>
  l402SigningBoundary?: (
    env: Bindings,
  ) => Promise<OpenAgentsL402SigningBoundary | null>
  makeBuyerPaymentLedgerStore?: (env: Bindings) => BuyerPaymentLedgerStore
  makeId?: () => string
  makePylonApiStore?: (env: Bindings) => AutopilotPylonApiStore
  makeStore: (env: Bindings) => AutopilotWorkStore
  nowIso?: () => string
  pylonRegistrations?: (
    env: Bindings,
  ) => Promise<ReadonlyArray<PylonApiRegistrationRecord>>
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: Readonly<{ userId: string }> }> | undefined>
  verifyL402PaymentProof?: (
    env: Bindings,
    input: AutopilotWorkL402PaymentVerificationInput,
  ) => Promise<AutopilotWorkL402PaymentVerificationResult | null>
}>

type AutopilotWorkRouteEnv = Readonly<Record<string, unknown>>

const routeErrorResponse = (error: AutopilotWorkStoreError): HttpResponse =>
  noStoreJsonResponse(
    { error: `autopilot_work_${error.kind}`, reason: error.reason },
    {
      status:
        error.kind === 'conflict'
          ? 409
          : error.kind === 'not_found'
            ? 404
            : error.kind === 'storage_error'
              ? 500
              : 400,
    },
  )

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('Idempotency-Key')?.trim()

  return value === undefined || value === '' ? undefined : value
}

const requireIdempotencyHash = (
  request: Request,
): Effect.Effect<string, AutopilotWorkStoreError> => {
  const idempotencyKey = idempotencyKeyFromRequest(request)

  if (idempotencyKey === undefined) {
    return Effect.fail(
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: 'Idempotency-Key header is required.',
      }),
    )
  }

  return Effect.promise(() => sha256Hex(idempotencyKey))
}

const idempotencyHashForBrowserRequest = <Bindings>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  ownerUserId: string,
): Effect.Effect<string, AutopilotWorkStoreError> => {
  const idempotencyKey = idempotencyKeyFromRequest(request)

  return Effect.promise(() =>
    sha256Hex(
      idempotencyKey ??
        `browser.autopilot_work.${ownerUserId}.${routeMakeId(dependencies)}`,
    ),
  )
}

const decodeWorkRequest = (
  request: Request,
): Effect.Effect<OpenAgentsAutopilotWorkRequest, AutopilotWorkStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      decodeOpenAgentsAutopilotWorkRequest(await readJsonObject(request)),
  })

const decodeReviewDecisionRequest = (
  request: Request,
): Effect.Effect<AutopilotWorkReviewDecisionRequest, AutopilotWorkStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      S.decodeUnknownSync(AutopilotWorkReviewDecisionRequest)(
        await readJsonObject(request),
      ),
  })

const decodeFallbackCloseoutRequest = (
  request: Request,
): Effect.Effect<AutopilotWorkFallbackCloseoutRequest, AutopilotWorkStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      S.decodeUnknownSync(AutopilotWorkFallbackCloseoutRequest)(
        await readJsonObject(request),
      ),
  })

const routeNowIso = <Bindings>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

export type AutopilotWorkRouteAuth = Readonly<{
  actorAgentCredentialId: string
  actorAgentUserId: string
  ownerUserId: string
}>

export type AutopilotWorkAuthDependencies<Bindings> = Readonly<{
  agentStore: (env: Bindings) => AgentRegistrationStore
  requireBrowserSession?: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: Readonly<{ userId: string }> }> | undefined>
}>

const hasBearerAuthorization = (request: Request): boolean =>
  request.headers.get('authorization')?.trim().toLowerCase().startsWith('bearer ') ===
  true

export const authenticateAutopilotWorkRequest = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkAuthDependencies<Bindings>,
  request: Request,
  env: Bindings,
  input: Readonly<{
    ctx: ExecutionContext
    nowIso: () => string
    requiredScope: CustomerOrderAgentScope
  }>,
): Effect.Effect<
  AutopilotWorkRouteAuth,
  AutopilotWorkStoreError | CustomerOrderAgentAuthFailure
> =>
  hasBearerAuthorization(request) || dependencies.requireBrowserSession === undefined
    ? authenticateCustomerOrderAgentRequest(
        request,
        dependencies.agentStore(env),
        {
          nowIso: input.nowIso,
          requiredScope: input.requiredScope,
        },
      ).pipe(
        Effect.map(auth => ({
          actorAgentCredentialId: auth.agent.credential.id,
          actorAgentUserId: auth.agent.user.id,
          ownerUserId: auth.ownerUserId,
        })),
      )
    : Effect.gen(function* () {
        const session = yield* Effect.tryPromise({
          catch: error =>
            new AutopilotWorkStoreError({
              kind: 'storage_error',
              reason: errorReason(error),
            }),
          try: () =>
            dependencies.requireBrowserSession?.(request, env, input.ctx) ??
            Promise.resolve(undefined),
        })

        if (session === undefined) {
          return yield* new CustomerOrderAgentAuthFailure({
            failureKind: 'missing_credentials',
            reason: 'Autopilot work browser session is required.',
          })
        }

        return {
          actorAgentCredentialId:
            `browser_session.${cleanRefSegment(session.user.userId)}`,
          actorAgentUserId: session.user.userId,
          ownerUserId: session.user.userId,
        }
      })

const routePylonRegistrations = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<
  ReadonlyArray<PylonApiRegistrationRecord>,
  AutopilotWorkStoreError
> =>
  dependencies.pylonRegistrations !== undefined
    ? Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'storage_error',
            reason: error instanceof Error ? error.message : String(error),
          }),
        try: () => dependencies.pylonRegistrations?.(env) ?? Promise.resolve([]),
      })
    : dependencies.makePylonApiStore === undefined
      ? Effect.succeed([])
      : Effect.tryPromise({
          catch: error =>
            new AutopilotWorkStoreError({
              kind: 'storage_error',
              reason: error instanceof Error ? error.message : String(error),
            }),
          try: () =>
            dependencies.makePylonApiStore?.(env).listRegistrations(1000) ??
            Promise.resolve([]),
        })

const pylonAssignmentLeaseStore = (
  store: AutopilotPylonApiStore | undefined,
): AutopilotPylonAssignmentLeaseStore | undefined =>
  store !== undefined &&
  store.createAssignment !== undefined &&
  store.listAssignmentsForPylon !== undefined &&
  store.readAssignment !== undefined &&
  store.readAssignmentByIdempotencyKeyHash !== undefined &&
  store.readRegistration !== undefined
    ? {
        createAssignment: store.createAssignment,
        listAssignmentsForPylon: store.listAssignmentsForPylon,
        readAssignment: store.readAssignment,
        readAssignmentByIdempotencyKeyHash:
          store.readAssignmentByIdempotencyKeyHash,
        readRegistration: store.readRegistration,
      }
    : undefined

const workOrderRefForId = (id: string): string =>
  id.startsWith('autopilot_work_order.')
    ? id
    : `autopilot_work_order.${id}`

const statusUrlRefForWorkOrder = (workOrderRef: string): string =>
  `status.${workOrderRef}`

const eventStreamRefForWorkOrder = (workOrderRef: string): string =>
  `events.${workOrderRef}`

const AutopilotWorkL402ChallengeTtlSeconds = 15 * 60
const AutopilotWorkL402EndpointRef = 'endpoint.autopilot.work'
const AutopilotWorkL402ProductId = 'product.autopilot.work'

const cleanRefSegment = (value: string): string =>
  value.replace(/[^A-Za-z0-9_-]+/g, '_').slice(0, 120)

const addSecondsIso = (iso: string, seconds: number): string =>
  isoTimestampAfterIso(iso, seconds * 1000)

const autopilotQuoteAmount = (
  quote: AutopilotWorkQuote,
): BuyerPaymentLedgerAmount => ({
  amountMinorUnits: quote.amountCents,
  asset: 'usd',
  denomination: 'usd_cent',
})

const autopilotWorkRequestBodyDigest = (
  record: AutopilotWorkOrderRecord,
): Promise<string> => sha256Hex(JSON.stringify(record.request))

const autopilotL402CredentialRef = (
  record: AutopilotWorkOrderRecord,
): string => `credential.autopilot_work.${cleanRefSegment(record.workOrderRef)}`

const autopilotL402PaymentHashRef = (
  record: AutopilotWorkOrderRecord,
): string => `payment_hash.redacted.autopilot_work.${cleanRefSegment(record.workOrderRef)}`

const autopilotL402ReplayNonceRef = (
  record: AutopilotWorkOrderRecord,
): string => `replay_nonce.autopilot_work.${cleanRefSegment(record.workOrderRef)}`

const autopilotL402EntitlementScopeRefs = (
  record: AutopilotWorkOrderRecord,
): ReadonlyArray<string> => [
  `scope.autopilot_work.${cleanRefSegment(record.workOrderRef)}`,
  `scope.autopilot_owner.${cleanRefSegment(record.ownerUserId)}`,
  `scope.autopilot_agent.${cleanRefSegment(record.agentUserId)}`,
  makeAutopilotWorkQuote(record.request).quoteRef,
]

const autopilotBuyerPaymentChallengeRecord = async (
  record: AutopilotWorkOrderRecord,
): Promise<BuyerPaymentChallengeRecord> => {
  const quote = makeAutopilotWorkQuote(record.request)
  const price = autopilotQuoteAmount(quote)

  return {
    actorRef: `agent.${cleanRefSegment(record.agentUserId)}`,
    archivedAt: null,
    challengeRef:
      record.paymentChallengeRef ?? `challenge.${quote.quoteRef}`,
    createdAt: record.createdAt,
    expiresAt: addSecondsIso(
      record.createdAt,
      AutopilotWorkL402ChallengeTtlSeconds,
    ),
    id: `buyer_payment_challenge.${cleanRefSegment(record.workOrderRef)}`,
    idempotencyKeyHash: record.idempotencyKeyHash,
    metadataRefs: [
      `metadata.autopilot_work.${cleanRefSegment(record.workOrderRef)}`,
      quote.quoteRef,
    ],
    method: 'POST',
    ownerUserId: record.ownerUserId,
    path: '/api/autopilot/work',
    price,
    productId: AutopilotWorkL402ProductId,
    publicProjectionJson: JSON.stringify({
      amountCents: quote.amountCents,
      challengeRef: record.paymentChallengeRef,
      quoteRef: quote.quoteRef,
      workOrderRef: record.workOrderRef,
    }),
    requestBodyDigest: await autopilotWorkRequestBodyDigest(record),
    spendCap: price,
    status: 'issued',
    surface: 'agent_api',
  }
}

export type AutopilotWorkL402PaymentVerificationInput = Readonly<{
  credentialPayload: OpenAgentsL402CredentialPayload
  paymentProofRef: string
  quote: AutopilotWorkQuote
  workOrderRef: string
}>

export type AutopilotWorkL402PaymentVerificationResult = Readonly<{
  paymentProofRef: string
  verifierRef: string
}>

const amountsMatch = (
  left: BuyerPaymentLedgerAmount,
  right: BuyerPaymentLedgerAmount,
): boolean =>
  left.amountMinorUnits === right.amountMinorUnits &&
  left.asset === right.asset &&
  left.denomination === right.denomination

export const verifyAutopilotL402PaymentProofFromBuyerLedger = async (
  store: BuyerPaymentLedgerStore,
  input: AutopilotWorkL402PaymentVerificationInput,
): Promise<AutopilotWorkL402PaymentVerificationResult | null> => {
  const redemption = await store.readRedemptionByChallengeRef(
    input.credentialPayload.challengeRef,
  )

  if (
    redemption === undefined ||
    redemption.status !== 'redeemed' ||
    redemption.replayed !== 0 ||
    redemption.proofRef !== input.paymentProofRef
  ) {
    return null
  }

  const [receipt, entitlement] = await Promise.all([
    store.readReceiptByRef(redemption.receiptRef),
    store.readEntitlementByRef(redemption.entitlementRef),
  ])

  if (
    receipt === undefined ||
    entitlement === undefined ||
    receipt.status !== 'issued' ||
    entitlement.status !== 'active' ||
    receipt.challengeRef !== input.credentialPayload.challengeRef ||
    entitlement.challengeRef !== input.credentialPayload.challengeRef ||
    receipt.receiptRef !== redemption.receiptRef ||
    entitlement.receiptRef !== redemption.receiptRef ||
    receipt.entitlementRef !== redemption.entitlementRef ||
    receipt.productId !== input.credentialPayload.productId ||
    entitlement.productId !== input.credentialPayload.productId ||
    !amountsMatch(receipt.amount, input.credentialPayload.amount) ||
    !amountsMatch(
      autopilotQuoteAmount(input.quote),
      input.credentialPayload.amount,
    )
  ) {
    return null
  }

  const entitlementScopes = new Set(entitlement.scopeRefs)

  if (
    !input.credentialPayload.entitlementScopeRefs.every(scopeRef =>
      entitlementScopes.has(scopeRef)
    )
  ) {
    return null
  }

  const reconciliation = await store.readReconciliationEventByReceiptRef(
    receipt.receiptRef,
  )

  if (
    reconciliation === undefined ||
    reconciliation.status !== 'matched' ||
    reconciliation.receiptRef !== receipt.receiptRef ||
    reconciliation.challengeRef !== input.credentialPayload.challengeRef ||
    reconciliation.productId !== input.credentialPayload.productId
  ) {
    return null
  }

  return {
    paymentProofRef: redemption.proofRef,
    verifierRef: reconciliation.eventRef,
  }
}

const accessRequestRefsForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): ReadonlyArray<string> =>
  accessRequirementsForRequest(request).map(
    requirement => requirement.accessRequestRef,
  )

const hasAccessRequestKind = (
  task: OpenAgentsAutopilotWorkRequest['tasks'][number],
  kind: OpenAgentsAutopilotAccessRequestKind,
): boolean =>
  task.accessRequests.some(accessRequest => accessRequest.kind === kind)

const isAccessRequestSatisfiedByRepositoryPolicy = (
  task: OpenAgentsAutopilotWorkRequest['tasks'][number],
  kind: OpenAgentsAutopilotAccessRequestKind,
): boolean =>
  kind === 'github_repo_read' && task.repository?.visibility === 'public'

const accessGrantActionForKind = (
  kind: OpenAgentsAutopilotAccessRequestKind,
): AutopilotWorkAccessGrantAction => {
  switch (kind) {
    case 'customer_review':
    case 'site_deploy_review':
      return 'customer_review'
    case 'github_account_link':
      return 'connect_github_account'
    case 'github_branch_write':
      return 'authorize_github_branch'
    case 'github_pull_request':
      return 'authorize_github_pull_request'
    case 'github_repo_read':
    case 'github_repo_write':
      return 'connect_github_repository'
    case 'operator_review':
      return 'operator_review'
    case 'privacy_tier_confirmation':
      return 'confirm_privacy_tier'
    case 'pylon_enrollment':
      return 'enroll_pylon'
    case 'repository_selection':
      return 'select_repository'
    case 'secret_broker':
      return 'configure_secret_broker'
  }
}

const accessRequirementsForTask = (
  task: OpenAgentsAutopilotWorkRequest['tasks'][number],
): ReadonlyArray<AutopilotWorkAccessRequirementProjection> =>
  task.accessRequests
    .filter(
      accessRequest =>
        !isAccessRequestSatisfiedByRepositoryPolicy(task, accessRequest.kind),
    )
    .map(accessRequest => {
      const accessRequestRef =
        `access_request.${task.taskRef}.${accessRequest.kind}`

      return {
        accessRequestRef,
        grantAction: accessGrantActionForKind(accessRequest.kind),
        kind: accessRequest.kind,
        ownerActionRef:
          `owner_action.${task.taskRef}.${accessRequest.kind}`,
        reasonRef: accessRequest.reasonRef,
        requiredBeforeLaunch: true,
        status: 'missing',
        taskRef: task.taskRef,
      }
    })

const accessRequirementsForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): ReadonlyArray<AutopilotWorkAccessRequirementProjection> =>
  request.tasks.flatMap(task => accessRequirementsForTask(task))

const repositoryAuthoritiesForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): ReadonlyArray<AutopilotWorkRepositoryAuthorityProjection> =>
  request.tasks.flatMap(task =>
    task.repository === undefined
      ? []
      : [
          {
            branch: task.repository.branch,
            deployAuthority: false,
            fullName: task.repository.fullName,
            provider: task.repository.provider,
            pullRequestAuthority: hasAccessRequestKind(
              task,
              'github_pull_request',
            )
              ? 'owner_grant_required'
              : 'not_requested',
            readAuthority:
              task.repository.visibility === 'public'
                ? 'public_read_available'
                : 'owner_grant_required',
            spendAuthority: false,
            taskRef: task.taskRef,
            visibility: task.repository.visibility,
            writeAuthority:
              hasAccessRequestKind(task, 'github_repo_write') ||
              hasAccessRequestKind(task, 'github_branch_write')
                ? 'owner_grant_required'
                : 'not_requested',
          },
        ]
  )

const paymentChallengeRefForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
): string | null => {
  const quote = makeAutopilotWorkQuote(request)

  return quote.paymentRequired &&
    (request.paymentPolicy.buyerPaymentMode === 'l402' ||
      request.paymentPolicy.buyerPaymentMode === 'mdk_checkout' ||
      request.paymentPolicy.buyerPaymentMode === 'paid_quote_required')
    ? `challenge.${quote.quoteRef}`
    : null
}

const safePaymentProofRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const safeExecutionCloseoutRefPattern =
  /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeExecutionCloseoutRefPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|repo)|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet[_-]?(home|material|mnemonic|path|private|secret|state)|webhook[_-]?secret)/iu
const AutopilotWorkReviewPublicSafeRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const AutopilotWorkReviewRefs = S.optionalKey(
  S.Array(AutopilotWorkReviewPublicSafeRef),
)
const AutopilotWorkReviewDecisionRequest = S.Struct({
  action: S.Literals(['accept', 'reject', 'request_changes']),
  decisionRefs: AutopilotWorkReviewRefs,
  rejectionRefs: AutopilotWorkReviewRefs,
  revisionRequestRefs: AutopilotWorkReviewRefs,
})
type AutopilotWorkReviewDecisionRequest =
  typeof AutopilotWorkReviewDecisionRequest.Type

const AutopilotWorkFallbackCloseoutRequest = S.Struct({
  assignmentRefs: S.Array(S.String),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  authorityReceiptRefs: S.optionalKey(S.Array(S.String)),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  buildRefs: S.optionalKey(S.Array(S.String)),
  changeCaptureRefs: S.optionalKey(S.Array(S.String)),
  changeCaptureStatus: S.optionalKey(
    S.Literals(['blocked', 'review_ready', 'stale']),
  ),
  closeoutRefs: S.Array(S.String),
  deliveryReadinessFreshness: S.optionalKey(S.Literals(['fresh', 'stale'])),
  deliveryReadinessRefs: S.optionalKey(S.Array(S.String)),
  deliveryReadinessStatus: S.optionalKey(
    S.Literals(['blocked', 'ready', 'scoped_exception']),
  ),
  fileCount: S.optionalKey(S.Number),
  addedLineCount: S.optionalKey(S.Number),
  patchDigestRef: S.optionalKey(S.NullOr(S.String)),
  previewRefs: S.optionalKey(S.Array(S.String)),
  proofRefs: S.Array(S.String),
  removedLineCount: S.optionalKey(S.Number),
  resultRefs: S.Array(S.String),
  reviewCaveatRefs: S.optionalKey(S.Array(S.String)),
  runnerKind: OpenAgentsAutopilotRunnerKind,
  summaryRefs: S.optionalKey(S.Array(S.String)),
  testRefs: S.optionalKey(S.Array(S.String)),
  verificationRefs: S.optionalKey(S.Array(S.String)),
  worktreeIdentityStatus: S.optionalKey(S.Literals(['blocked', 'ready', 'stale'])),
  writebackRequired: S.optionalKey(S.Boolean),
})
type AutopilotWorkFallbackCloseoutRequest =
  typeof AutopilotWorkFallbackCloseoutRequest.Type

const safeBuyerPaymentProofRef = (value: string | null): string | undefined =>
  value !== null &&
  safePaymentProofRefPattern.test(value) &&
  !/(invoice|lnbc|lntb|lnbcrt|preimage|secret|token|wallet)/iu.test(value)
    ? value
    : undefined

export const publicSafeExecutionCloseoutRef = (value: string): boolean =>
  safeExecutionCloseoutRefPattern.test(value) &&
  !unsafeExecutionCloseoutRefPattern.test(value)

const allPublicSafeExecutionCloseoutRefs = (
  refs: ReadonlyArray<string>,
): boolean => refs.length > 0 && refs.every(publicSafeExecutionCloseoutRef)

const optionalPublicSafeExecutionCloseoutRefs = (
  refs: ReadonlyArray<string> | undefined,
): boolean => refs === undefined || refs.every(publicSafeExecutionCloseoutRef)

const optionalPublicSafeExecutionCloseoutRef = (
  ref: string | null | undefined,
): boolean => ref === null || ref === undefined || publicSafeExecutionCloseoutRef(ref)

const optionalNonNegativeInteger = (value: number | undefined): boolean =>
  value === undefined || (Number.isSafeInteger(value) && value >= 0)

const publicSafeReviewRefs = (refs: ReadonlyArray<string>): boolean =>
  refs.every(publicSafeExecutionCloseoutRef)

const publicSafeRefsFromBody = (
  body: Record<string, unknown>,
  key: string,
): ReadonlyArray<string> =>
  Array.isArray(body[key])
    ? body[key].filter((ref): ref is string => typeof ref === 'string')
    : []

const optionalPublicSafeRefsFromBody = (
  body: Record<string, unknown>,
  key: string,
): ReadonlyArray<string> | undefined =>
  key in body ? publicSafeRefsFromBody(body, key) : undefined

const optionalPublicSafeRefFromBody = (
  body: Record<string, unknown>,
  key: string,
): string | null | undefined =>
  key in body
    ? body[key] === null
      ? null
      : typeof body[key] === 'string'
        ? body[key]
        : undefined
    : undefined

const optionalNonNegativeIntegerFromBody = (
  body: Record<string, unknown>,
  key: string,
): number | undefined =>
  typeof body[key] === 'number' && Number.isSafeInteger(body[key]) && body[key] >= 0
    ? body[key]
    : undefined

const optionalChangeCaptureStatusFromBody = (
  body: Record<string, unknown>,
): AutopilotWorkExecutionCloseoutRecord['changeCaptureStatus'] =>
  body.changeCaptureStatus === 'blocked' ||
  body.changeCaptureStatus === 'review_ready' ||
  body.changeCaptureStatus === 'stale'
    ? body.changeCaptureStatus
    : undefined

const optionalDeliveryReadinessFreshnessFromBody = (
  body: Record<string, unknown>,
): AutopilotWorkExecutionCloseoutRecord['deliveryReadinessFreshness'] =>
  body.deliveryReadinessFreshness === 'fresh' ||
  body.deliveryReadinessFreshness === 'stale'
    ? body.deliveryReadinessFreshness
    : undefined

const optionalDeliveryReadinessStatusFromBody = (
  body: Record<string, unknown>,
): AutopilotWorkExecutionCloseoutRecord['deliveryReadinessStatus'] =>
  body.deliveryReadinessStatus === 'blocked' ||
  body.deliveryReadinessStatus === 'ready' ||
  body.deliveryReadinessStatus === 'scoped_exception'
    ? body.deliveryReadinessStatus
    : undefined

const optionalWorktreeIdentityStatusFromBody = (
  body: Record<string, unknown>,
): AutopilotWorkExecutionCloseoutRecord['worktreeIdentityStatus'] =>
  body.worktreeIdentityStatus === 'blocked' ||
  body.worktreeIdentityStatus === 'ready' ||
  body.worktreeIdentityStatus === 'stale'
    ? body.worktreeIdentityStatus
    : undefined

const reviewStateForAction = (
  action: AutopilotWorkReviewAction,
): Extract<
  OpenAgentsAutopilotWorkStateType,
  'accepted' | 'rejected' | 'revision_required'
> =>
  action === 'accept'
    ? 'accepted'
    : action === 'reject'
      ? 'rejected'
      : 'revision_required'

const reviewDecisionProjectionForRecord = (
  record: AutopilotWorkReviewDecisionRecord | null,
): AutopilotWorkReviewDecisionProjection | null =>
  record === null
    ? null
    : {
        ...record,
        acceptedWorkAuthority: false,
        deployAuthority: false,
        forumAutoPublishAllowed: false,
        publicSafe: true,
        settlementAuthority: false,
        workerPayoutAuthority: false,
      }

const verifyBuyerPaymentProofFromRequest = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    nowIso: string
    request: Request
    record: AutopilotWorkOrderRecord
  }>,
): Effect.Effect<AutopilotWorkBuyerPaymentProof | undefined, AutopilotWorkStoreError> =>
  Effect.gen(function* () {
    const quote = makeAutopilotWorkQuote(input.record.request)

    if (!quote.paymentRequired || input.record.buyerPaymentProofRef !== null) {
      return undefined
    }

    if (input.record.request.paymentPolicy.buyerPaymentMode === 'l402') {
      const parsed = (() => {
      try {
        return parseOpenAgentsPaymentHeaders(input.request.headers)
      } catch {
        return undefined
      }
    })()
      const paymentProofRef = safeBuyerPaymentProofRef(parsed?.proofRef ?? null)

      if (
        parsed?.credential === undefined ||
        parsed.credential === null ||
        paymentProofRef === undefined
      ) {
        return undefined
      }

      const signer = yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'storage_error',
            reason: errorReason(error),
          }),
        try: () =>
          dependencies.l402SigningBoundary?.(env) ?? Promise.resolve(null),
      })

      if (signer === null) {
        return yield* new AutopilotWorkStoreError({
          kind: 'validation_error',
          reason: 'Autopilot L402 payment verifier is not configured.',
        })
      }

      const challenge = yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'storage_error',
            reason: errorReason(error),
          }),
        try: () => autopilotBuyerPaymentChallengeRecord(input.record),
      })
      const verification = yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'validation_error',
            reason: errorReason(error),
          }),
        try: () =>
          verifyOpenAgentsL402Credential(parsed.credential ?? '', signer, {
            amount: challenge.price,
            challengeRef: challenge.challengeRef,
            endpointRef: AutopilotWorkL402EndpointRef,
            entitlementScopeRefs: autopilotL402EntitlementScopeRefs(
              input.record,
            ),
            method: challenge.method,
            nowIso: input.nowIso,
            path: challenge.path,
            paymentProofRef,
            productId: challenge.productId,
            requestBodyDigest: challenge.requestBodyDigest,
            requirePaymentProof: true,
          }),
      })

      if (verification.status !== 'valid' || verification.payload === null) {
        return yield* new AutopilotWorkStoreError({
          kind: 'validation_error',
          reason: verification.reasonRef,
        })
      }

      const credentialPayload = verification.payload

      if (
        credentialPayload.credentialRef !==
          autopilotL402CredentialRef(input.record) ||
        credentialPayload.replayNonceRef !==
          autopilotL402ReplayNonceRef(input.record)
      ) {
        return yield* new AutopilotWorkStoreError({
          kind: 'validation_error',
          reason:
            'Autopilot L402 credential does not match the stored work-order challenge.',
        })
      }

      const proofVerification = yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'validation_error',
            reason: errorReason(error),
          }),
        try: () =>
          dependencies.verifyL402PaymentProof?.(env, {
            credentialPayload,
            paymentProofRef,
            quote,
            workOrderRef: input.record.workOrderRef,
          }) ?? Promise.resolve(null),
      })

      if (proofVerification === null) {
        return yield* new AutopilotWorkStoreError({
          kind: 'validation_error',
          reason: 'Autopilot L402 payment proof was not verified.',
        })
      }

      const verifiedProofRef = safeBuyerPaymentProofRef(
        proofVerification.paymentProofRef,
      )

      if (verifiedProofRef === undefined) {
        return yield* new AutopilotWorkStoreError({
          kind: 'validation_error',
          reason:
            'Autopilot L402 payment verifier returned an unsafe proof ref.',
        })
      }

      return { proofRef: verifiedProofRef, source: 'l402' }
    }

    if (input.record.request.paymentPolicy.buyerPaymentMode === 'mdk_checkout') {
      return undefined
    }

    return undefined
  })

const paymentChallengeForRecord = (
  record: AutopilotWorkOrderRecord,
): AutopilotWorkPaymentChallengeProjection | null => {
  const quote = makeAutopilotWorkQuote(record.request)

  if (record.paymentChallengeRef === null || !quote.paymentRequired) {
    return null
  }

  if (
    record.request.paymentPolicy.buyerPaymentMode !== 'l402' &&
    record.request.paymentPolicy.buyerPaymentMode !== 'mdk_checkout'
  ) {
    return null
  }

  const kind = record.request.paymentPolicy.buyerPaymentMode
  const checkoutIntentRef = kind === 'mdk_checkout'
    ? `checkout_intent.${quote.quoteRef}`
    : null
  const checkoutUrlRef = kind === 'mdk_checkout'
    ? `checkout_url.${quote.quoteRef}`
    : null
  const l402HeaderRef = kind === 'l402'
    ? 'WWW-Authenticate: L402'
    : null

  return {
    amountCents: quote.amountCents,
    challengeRef: record.paymentChallengeRef,
    checkoutIntentRef,
    checkoutUrlRef,
    expiresAt: addSecondsIso(
      record.createdAt,
      AutopilotWorkL402ChallengeTtlSeconds,
    ),
    kind,
    l402CredentialRef: kind === 'l402'
      ? autopilotL402CredentialRef(record)
      : null,
    l402HeaderRef,
    quoteRef: quote.quoteRef,
    status: record.buyerPaymentProofRef === null
      ? 'payment_required'
      : 'paid_ready',
  }
}

const mintAutopilotL402CredentialForRecord = async (
  record: AutopilotWorkOrderRecord,
  signer: OpenAgentsL402SigningBoundary,
): Promise<string> => {
  const challenge = await autopilotBuyerPaymentChallengeRecord(record)
  const payload = l402PayloadFromBuyerPaymentChallenge({
    challenge,
    credentialRef: autopilotL402CredentialRef(record),
    endpointRef: AutopilotWorkL402EndpointRef,
    entitlementScopeRefs: autopilotL402EntitlementScopeRefs(record),
    issuedAt: record.createdAt,
    paymentHashRef: autopilotL402PaymentHashRef(record),
    replayNonceRef: autopilotL402ReplayNonceRef(record),
  })
  const envelope = await mintOpenAgentsL402Credential(payload, signer)

  return envelope.credential
}

const paymentRequiredResponse = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    projection: AutopilotWorkOrderProjection
    record: AutopilotWorkOrderRecord
  }>,
): Effect.Effect<HttpResponse, AutopilotWorkStoreError> => Effect.gen(function* () {
  const headers = new Headers()
  const challenge = paymentChallengeForRecord(input.record)

  if (
    challenge !== null &&
    input.record.request.paymentPolicy.buyerPaymentMode === 'l402'
  ) {
    if (dependencies.makeBuyerPaymentLedgerStore !== undefined) {
      const buyerPaymentChallenge = yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'storage_error',
            reason: errorReason(error),
          }),
        try: () => autopilotBuyerPaymentChallengeRecord(input.record),
      })

      yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'storage_error',
            reason: errorReason(error),
          }),
        try: () =>
          dependencies
            .makeBuyerPaymentLedgerStore?.(env)
            .createChallenge(buyerPaymentChallenge) ?? Promise.resolve(),
      })
    }

    headers.set(
      'www-authenticate',
      formatOpenAgentsL402WwwAuthenticate({
        amount: {
          amountMinorUnits: challenge.amountCents,
          asset: 'usd',
          denomination: 'usd_cent',
        },
        challengeRef: challenge.challengeRef,
        docsRef: 'docs.autopilot.work.l402',
        endpointRef: 'endpoint.autopilot.work',
        expiresAt: challenge.expiresAt,
        productId: 'product.autopilot.work',
      }),
    )
    const signer = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: errorReason(error),
        }),
      try: () =>
        dependencies.l402SigningBoundary?.(env) ?? Promise.resolve(null),
    })

    if (signer !== null) {
      const credential = yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'storage_error',
            reason: errorReason(error),
          }),
        try: () => mintAutopilotL402CredentialForRecord(input.record, signer),
      })

      headers.set('x-openagents-l402-credential', credential)
      headers.set('x-openagents-l402-proof-format', 'public-safe-ref')
    }
  }

  return noStoreJsonResponse(
    {
      error: 'payment_required',
      generatedAt: input.projection.generatedAt,
      work: input.projection,
    },
    { headers, status: 402 },
  )
})

const fundingForRecord = (
  record: AutopilotWorkOrderRecord,
): AutopilotWorkFundingProjection => {
  const quote = makeAutopilotWorkQuote(record.request)
  const buyerFundingState = !quote.paymentRequired
    ? 'not_required'
    : record.buyerPaymentProofRef === null
      ? 'payment_required'
      : 'funded'
  const settlementBlockedReasonRef =
    record.request.paymentPolicy.settlementMode === 'no_worker_payout'
      ? 'settlement.no_worker_payout_mode'
      : buyerFundingState === 'payment_required'
        ? 'settlement.buyer_payment_required'
        : 'settlement.accepted_work_required'

  return {
    buyerFundingState,
    buyerPaymentProofRef: record.buyerPaymentProofRef,
    fundedAmountCents: buyerFundingState === 'funded'
      ? quote.amountCents
      : 0,
    quoteRef: quote.quoteRef,
    settlementBlockedReasonRef,
    settlementEligible: false,
    workerPayoutEligible: false,
  }
}

const placementPolicyForRecord = (
  record: AutopilotWorkOrderRecord,
): AutopilotWorkPlacementPolicyRecordProjection => {
  const policy = record.request.placementPolicy

  return {
    allowedRunnerKinds: policy.allowedRunnerKinds,
    auditable: true,
    disallowedRunnerKinds: policy.disallowedRunnerKinds,
    localOnlyAllowed: policy.localOnlyAllowed,
    placementPolicyRef: `placement_policy.${record.workOrderRef}`,
    preferredRunnerKinds: policy.preferredRunnerKinds,
    privacyTier: policy.privacyTier,
    promptKeywordRouting: false,
    publicTraceAllowed: policy.publicTraceAllowed,
    reasonRefs: [
      `placement.privacy.${policy.privacyTier}`,
      policy.localOnlyAllowed
        ? 'placement.local_only.allowed'
        : 'placement.local_only.not_allowed',
      policy.publicTraceAllowed
        ? 'placement.public_trace.allowed'
        : 'placement.public_trace.blocked',
      policy.requiresSecretBroker
        ? 'placement.secret_broker.required'
        : 'placement.secret_broker.not_required',
    ],
    requiresSecretBroker: policy.requiresSecretBroker,
  }
}

const pricingPolicyForPlacement = (
  placementDecision: AutopilotPlacementDecisionProjection,
): AutopilotWorkOrderProjection['pricingPolicy'] => ({
  ...autopilotWorkPricingPolicy,
  activeLane: pricingLaneForRunnerKind(placementDecision.selectedRunnerKind),
})

const executionCloseoutForRecord = (
  record: AutopilotWorkOrderRecord,
): AutopilotWorkExecutionCloseoutProjection | null =>
  record.executionCloseout === null
    ? null
    : {
        ...record.executionCloseout,
        acceptedWorkAuthority: false,
        forumAutoPublishAllowed: false,
        publicSafe: true,
        workerPayoutAuthority: false,
      }

const lifecycleStateForTask = (
  record: AutopilotWorkOrderRecord,
  taskAccessRequirements: ReadonlyArray<AutopilotWorkAccessRequirementProjection>,
  funding: AutopilotWorkFundingProjection,
): AutopilotWorkTaskLifecycleState => {
  if (taskAccessRequirements.length > 0) {
    return 'access_required'
  }

  if (funding.buyerFundingState === 'payment_required') {
    return 'payment_required'
  }

  if (scheduledLaunchHoldsDispatch(record.scheduledLaunch)) {
    return 'scheduled'
  }

  switch (record.state) {
    case 'accepted':
      return 'accepted'
    case 'blocked':
    case 'invalid':
      return 'blocked'
    case 'delivered':
      return 'delivered'
    case 'queued_or_running':
      return 'queued_or_running'
    case 'accepted_free_slice':
    case 'paid_ready':
      return 'ready_for_assignment'
    case 'rejected':
      return 'rejected'
    case 'revision_required':
      return 'revision_required'
    case 'access_required':
    case 'payment_required':
      return 'ready_for_assignment'
    case 'scheduled':
      return 'scheduled'
  }
}

const placementStateForLifecycle = (
  lifecycleState: AutopilotWorkTaskLifecycleState,
): AutopilotWorkTaskPlacementState => {
  switch (lifecycleState) {
    case 'access_required':
      return 'blocked_on_access'
    case 'accepted':
      return 'accepted'
    case 'payment_required':
      return 'blocked_on_payment'
    case 'blocked':
      return 'blocked'
    case 'delivered':
      return 'delivered'
    case 'queued_or_running':
      return 'queued_or_running'
    case 'rejected':
      return 'rejected'
    case 'ready_for_assignment':
      return 'ready_for_assignment'
    case 'revision_required':
      return 'revision_required'
    case 'scheduled':
      return 'scheduled'
  }
}

const taskRecordsForRecord = (
  record: AutopilotWorkOrderRecord,
): ReadonlyArray<AutopilotWorkTaskRecordProjection> => {
  const funding = fundingForRecord(record)

  return record.request.tasks.map(task => {
    const accessRequirements = accessRequirementsForTask(task)
    const lifecycleState = lifecycleStateForTask(
      record,
      accessRequirements,
      funding,
    )

    return {
      acceptanceCriteriaRefs: task.acceptanceCriteriaRefs,
      accessRequirements,
      accessState: accessRequirements.length === 0
        ? 'satisfied'
        : 'missing_required_access',
      checkout: task.checkout ?? null,
      kind: task.kind,
      lifecycleState,
      objective: task.objective,
      paymentState: funding.buyerFundingState,
      placementState: placementStateForLifecycle(lifecycleState),
      requestedAdapter: task.requestedAdapter ?? null,
      requestedAdapterProfileRef: task.requestedAdapterProfileRef ?? null,
      repository: task.repository ?? null,
      taskRef: task.taskRef,
    }
  })
}

const nextActionForRecord = (
  record: AutopilotWorkOrderRecord,
  funding: AutopilotWorkFundingProjection,
  placementDecision: AutopilotPlacementDecisionProjection,
  nowIso: string,
): AutopilotWorkNextActionProjection => {
  if (record.state === 'accepted') {
    return {
      callerActionRefs: ['caller.wait_for_autopilot_settlement_policy'],
      reasonRefs: ['next_action.customer_accepted_work'],
      retryAfterSeconds: null,
      state: 'accepted',
    }
  }

  if (record.state === 'rejected') {
    return {
      callerActionRefs: ['caller.create_follow_up_autopilot_work'],
      reasonRefs: ['next_action.customer_rejected_work'],
      retryAfterSeconds: null,
      state: 'rejected',
    }
  }

  if (record.state === 'revision_required') {
    return {
      callerActionRefs: ['caller.wait_for_or_create_revision_work'],
      reasonRefs: ['next_action.customer_requested_changes'],
      retryAfterSeconds: null,
      state: 'revision_required',
    }
  }

  if (record.state === 'delivered') {
    return {
      callerActionRefs: ['caller.review_autopilot_closeout'],
      reasonRefs: ['next_action.review_delivered_work'],
      retryAfterSeconds: null,
      state: 'delivered',
    }
  }

  if (funding.buyerFundingState === 'payment_required') {
    return {
      callerActionRefs: ['caller.pay_autopilot_quote'],
      reasonRefs: ['next_action.payment_required'],
      retryAfterSeconds: null,
      state: 'payment_required',
    }
  }

  if (
    record.scheduledLaunch !== null &&
    scheduledLaunchHoldsDispatch(record.scheduledLaunch)
  ) {
    return {
      callerActionRefs: ['caller.wait_for_scheduled_launch'],
      reasonRefs: [
        'next_action.scheduled_launch_pending',
        'scheduled_launch.placement_at_launch_time',
      ],
      retryAfterSeconds: scheduledLaunchRetryAfterSeconds(
        record.scheduledLaunch,
        nowIso,
      ),
      state: 'retry_later',
    }
  }

  if (placementDecision.source === 'none_available') {
    return {
      callerActionRefs: placementDecision.callerActionRefs,
      reasonRefs: placementDecision.refusalReasonRefs,
      retryAfterSeconds: placementDecision.retryAfterSeconds,
      state: placementDecision.availabilityState === 'retry_later'
        ? 'retry_later'
        : 'needs_input',
    }
  }

  return {
    callerActionRefs: [],
    reasonRefs: ['next_action.ready_for_assignment'],
    retryAfterSeconds: null,
    state: 'ready',
  }
}

const stateForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
  scheduledLaunch: AutopilotWorkScheduledLaunchRecord | null,
): OpenAgentsAutopilotWorkStateType => {
  if (accessRequirementsForRequest(request).length > 0) {
    return 'access_required'
  }

  if (paymentChallengeRefForRequest(request) !== null) {
    return 'payment_required'
  }

  if (scheduledLaunchHoldsDispatch(scheduledLaunch)) {
    return 'scheduled'
  }

  return 'accepted_free_slice'
}

const projectionForRecord = (
  record: AutopilotWorkOrderRecord,
  idempotent: boolean,
  nowIso: string,
  pylonRegistrations: ReadonlyArray<PylonApiRegistrationRecord>,
): AutopilotWorkOrderProjection => {
  const funding = fundingForRecord(record)
  const placementDecision = selectAutopilotPlacement({
    nowIso,
    ownerAgentUserId: record.agentUserId,
    placementPolicy: record.request.placementPolicy,
    pylonRegistrations,
  })
  const work = {
    accessRequirements: accessRequirementsForRequest(record.request),
    accessRequestRefs: record.accessRequestRefs,
    assignmentIntents: [],
    buyerPaymentProofRef: record.buyerPaymentProofRef,
    clientRequestRef: record.clientRequestRef,
    createdAt: record.createdAt,
    eventStreamRef: record.eventStreamRef,
    executionCloseout: executionCloseoutForRecord(record),
    fallbackLeaseIntents: [],
    funding,
    generatedAt: nowIso,
    idempotent,
    nextAction: nextActionForRecord(record, funding, placementDecision, nowIso),
    paymentChallenge: paymentChallengeForRecord(record),
    paymentChallengeRef: record.paymentChallengeRef,
    placementDecision,
    placementPolicy: placementPolicyForRecord(record),
    pricingPolicy: pricingPolicyForPlacement(placementDecision),
    promiseRef:
      record.request.promiseRef === undefined
        ? null
        : {
            blockerRefs: record.request.promiseRef.blockerRefs ?? [],
            promiseId: record.request.promiseRef.promiseId,
            registryVersion: record.request.promiseRef.registryVersion,
          },
    pylonAssignmentIntents: [],
    quote: makeAutopilotWorkQuote(record.request),
    repositoryAuthorities: repositoryAuthoritiesForRequest(record.request),
    reviewDecision: reviewDecisionProjectionForRecord(record.reviewDecision),
    scheduledLaunch: scheduledLaunchProjection(record.scheduledLaunch),
    state: record.state,
    statusUrlRef: record.statusUrlRef,
    taskRefs: record.taskRefs,
    tasks: taskRecordsForRecord(record),
    updatedAt: record.updatedAt,
    workOrderRef: record.workOrderRef,
  } satisfies AutopilotWorkOrderProjection

  const assignmentIntents = assignmentIntentsForWorkOrder(work)

  return {
    ...work,
    assignmentIntents,
    fallbackLeaseIntents: fallbackLeaseIntentsForAutopilotWork({
      assignmentIntents,
      placementDecision: work.placementDecision,
      tasks: work.tasks,
      workOrderRef: work.workOrderRef,
    }),
    pylonAssignmentIntents: pylonAssignmentIntentsForAutopilotWork({
      assignmentIntents,
      placementDecision: work.placementDecision,
      tasks: work.tasks,
      workOrderRef: work.workOrderRef,
    }),
  }
}

const hostedGeminiRunnerKind: OpenAgentsAutopilotRunnerKindType =
  'hosted_gemini'

const validateExecutionCloseoutForWork = (
  executionCloseout: AutopilotWorkExecutionCloseoutRecord,
  work: AutopilotWorkOrderProjection,
): Effect.Effect<AutopilotWorkExecutionCloseoutRecord, AutopilotWorkStoreError> => {
  const assignmentRefs = new Set(
    work.fallbackLeaseIntents.map(intent => intent.assignmentRef),
  )
  const assignmentRefsMatch =
    executionCloseout.assignmentRefs.length > 0 &&
    executionCloseout.assignmentRefs.every(ref => assignmentRefs.has(ref))
  const refsArePublicSafe =
    executionCloseoutRefsArePublicSafe(executionCloseout)
  const runnerMatches =
    work.placementDecision.selectedRunnerKind === hostedGeminiRunnerKind &&
    executionCloseout.runnerKind === hostedGeminiRunnerKind

  if (!assignmentRefsMatch || !refsArePublicSafe || !runnerMatches) {
    return Effect.fail(
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason:
          'Autopilot execution closeout must match the selected hosted runner and contain only public-safe assignment, closeout, proof, and result refs.',
      }),
    )
  }

  return Effect.succeed(executionCloseout)
}

const executionCloseoutRefsArePublicSafe = (
  executionCloseout: AutopilotWorkExecutionCloseoutRecord,
): boolean =>
  allPublicSafeExecutionCloseoutRefs(executionCloseout.assignmentRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.artifactRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.authorityReceiptRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.blockerRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.buildRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.changeCaptureRefs) &&
  allPublicSafeExecutionCloseoutRefs(executionCloseout.closeoutRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.deliveryReadinessRefs) &&
  optionalNonNegativeInteger(executionCloseout.fileCount) &&
  optionalNonNegativeInteger(executionCloseout.addedLineCount) &&
  optionalPublicSafeExecutionCloseoutRef(executionCloseout.patchDigestRef) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.previewRefs) &&
  allPublicSafeExecutionCloseoutRefs(executionCloseout.proofRefs) &&
  optionalNonNegativeInteger(executionCloseout.removedLineCount) &&
  allPublicSafeExecutionCloseoutRefs(executionCloseout.resultRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.reviewCaveatRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.summaryRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.testRefs) &&
  optionalPublicSafeExecutionCloseoutRefs(executionCloseout.verificationRefs)

const codingAssignmentString = (
  assignment: PylonApiAssignmentRecord,
  key: string,
): string | undefined => {
  const value = assignment.codingAssignment?.[key]

  return typeof value === 'string' ? value : undefined
}

const autopilotWorkOrderRefForAssignment = (
  assignment: PylonApiAssignmentRecord,
): string | undefined =>
  assignment.taskRefs.find(ref => ref.startsWith('autopilot_work_order.')) ??
  codingAssignmentString(assignment, 'workOrderRef')

const autopilotTaskRefForAssignment = (
  assignment: PylonApiAssignmentRecord,
): string | undefined =>
  codingAssignmentString(assignment, 'taskRef') ??
  assignment.taskRefs.find(ref => ref.startsWith('task.'))

const executionCloseoutFromPylonWorkerCloseout = (
  input: Readonly<{
    assignment: PylonApiAssignmentRecord
    body: Record<string, unknown>
    record: AutopilotWorkOrderRecord
  }>,
): Effect.Effect<AutopilotWorkExecutionCloseoutRecord, AutopilotWorkStoreError> => {
  const workOrderRef = autopilotWorkOrderRefForAssignment(input.assignment)
  const taskRef = autopilotTaskRefForAssignment(input.assignment)
  const artifactRefs = publicSafeRefsFromBody(input.body, 'artifactRefs')
  const authorityReceiptRefs = optionalPublicSafeRefsFromBody(
    input.body,
    'authorityReceiptRefs',
  )
  const blockerRefs = publicSafeRefsFromBody(input.body, 'blockerRefs')
  const buildRefs = publicSafeRefsFromBody(input.body, 'buildRefs')
  const changeCaptureRefs = optionalPublicSafeRefsFromBody(
    input.body,
    'changeCaptureRefs',
  )
  const closeoutRefs = publicSafeRefsFromBody(input.body, 'closeoutRefs')
  const deliveryReadinessRefs = optionalPublicSafeRefsFromBody(
    input.body,
    'deliveryReadinessRefs',
  )
  const previewRefs = publicSafeRefsFromBody(input.body, 'previewRefs')
  const proofRefs = publicSafeRefsFromBody(input.body, 'proofRefs')
  const resultRefs = publicSafeRefsFromBody(input.body, 'resultRefs')
  const reviewCaveatRefs = optionalPublicSafeRefsFromBody(
    input.body,
    'reviewCaveatRefs',
  )
  const summaryRefs = publicSafeRefsFromBody(input.body, 'summaryRefs')
  const testRefs = publicSafeRefsFromBody(input.body, 'testRefs')
  const verificationRefs = optionalPublicSafeRefsFromBody(
    input.body,
    'verificationRefs',
  )
  const changeCaptureStatus = optionalChangeCaptureStatusFromBody(input.body)
  const deliveryReadinessFreshness =
    optionalDeliveryReadinessFreshnessFromBody(input.body)
  const deliveryReadinessStatus =
    optionalDeliveryReadinessStatusFromBody(input.body)
  const fileCount = optionalNonNegativeIntegerFromBody(input.body, 'fileCount')
  const addedLineCount = optionalNonNegativeIntegerFromBody(
    input.body,
    'addedLineCount',
  )
  const patchDigestRef = optionalPublicSafeRefFromBody(
    input.body,
    'patchDigestRef',
  )
  const removedLineCount = optionalNonNegativeIntegerFromBody(
    input.body,
    'removedLineCount',
  )
  const worktreeIdentityStatus =
    optionalWorktreeIdentityStatusFromBody(input.body)
  const executionCloseout: AutopilotWorkExecutionCloseoutRecord = {
    assignmentRefs: [input.assignment.assignmentRef],
    artifactRefs,
    ...(authorityReceiptRefs === undefined ? {} : { authorityReceiptRefs }),
    blockerRefs,
    buildRefs,
    ...(changeCaptureRefs === undefined ? {} : { changeCaptureRefs }),
    ...(changeCaptureStatus === undefined ? {} : { changeCaptureStatus }),
    closeoutRefs,
    ...(deliveryReadinessFreshness === undefined
      ? {}
      : { deliveryReadinessFreshness }),
    ...(deliveryReadinessRefs === undefined ? {} : { deliveryReadinessRefs }),
    ...(deliveryReadinessStatus === undefined ? {} : { deliveryReadinessStatus }),
    ...(fileCount === undefined ? {} : { fileCount }),
    ...(addedLineCount === undefined ? {} : { addedLineCount }),
    ...(patchDigestRef === undefined ? {} : { patchDigestRef }),
    previewRefs,
    proofRefs,
    ...(removedLineCount === undefined ? {} : { removedLineCount }),
    resultRefs,
    ...(reviewCaveatRefs === undefined ? {} : { reviewCaveatRefs }),
    runnerKind: 'requester_pylon',
    summaryRefs,
    testRefs,
    ...(verificationRefs === undefined ? {} : { verificationRefs }),
    ...(worktreeIdentityStatus === undefined ? {} : { worktreeIdentityStatus }),
    ...(typeof input.body.writebackRequired === 'boolean'
      ? { writebackRequired: input.body.writebackRequired }
      : {}),
  }
  const refsArePublicSafe =
    executionCloseoutRefsArePublicSafe(executionCloseout)
  const refsMatch =
    workOrderRef === input.record.workOrderRef &&
    taskRef !== undefined &&
    input.record.taskRefs.includes(taskRef) &&
    input.assignment.ownerAgentUserId === input.record.agentUserId

  if (!refsMatch || !refsArePublicSafe) {
    return Effect.fail(
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason:
          'Autopilot worker closeout must match the work order, task, assignment owner, and contain only public-safe closeout refs.',
      }),
    )
  }

  return Effect.succeed(executionCloseout)
}

const executionCloseoutRecordFromFallbackCloseoutBody = (
  body: AutopilotWorkFallbackCloseoutRequest,
): AutopilotWorkExecutionCloseoutRecord => ({
  assignmentRefs: body.assignmentRefs,
  ...(body.artifactRefs === undefined ? {} : { artifactRefs: body.artifactRefs }),
  ...(body.authorityReceiptRefs === undefined
    ? {}
    : { authorityReceiptRefs: body.authorityReceiptRefs }),
  ...(body.blockerRefs === undefined ? {} : { blockerRefs: body.blockerRefs }),
  ...(body.buildRefs === undefined ? {} : { buildRefs: body.buildRefs }),
  ...(body.changeCaptureRefs === undefined
    ? {}
    : { changeCaptureRefs: body.changeCaptureRefs }),
  ...(body.changeCaptureStatus === undefined
    ? {}
    : { changeCaptureStatus: body.changeCaptureStatus }),
  closeoutRefs: body.closeoutRefs,
  ...(body.deliveryReadinessFreshness === undefined
    ? {}
    : { deliveryReadinessFreshness: body.deliveryReadinessFreshness }),
  ...(body.deliveryReadinessRefs === undefined
    ? {}
    : { deliveryReadinessRefs: body.deliveryReadinessRefs }),
  ...(body.deliveryReadinessStatus === undefined
    ? {}
    : { deliveryReadinessStatus: body.deliveryReadinessStatus }),
  ...(body.fileCount === undefined ? {} : { fileCount: body.fileCount }),
  ...(body.addedLineCount === undefined
    ? {}
    : { addedLineCount: body.addedLineCount }),
  ...(body.patchDigestRef === undefined
    ? {}
    : { patchDigestRef: body.patchDigestRef }),
  ...(body.previewRefs === undefined ? {} : { previewRefs: body.previewRefs }),
  proofRefs: body.proofRefs,
  ...(body.removedLineCount === undefined
    ? {}
    : { removedLineCount: body.removedLineCount }),
  resultRefs: body.resultRefs,
  ...(body.reviewCaveatRefs === undefined
    ? {}
    : { reviewCaveatRefs: body.reviewCaveatRefs }),
  runnerKind: body.runnerKind,
  ...(body.summaryRefs === undefined ? {} : { summaryRefs: body.summaryRefs }),
  ...(body.testRefs === undefined ? {} : { testRefs: body.testRefs }),
  ...(body.verificationRefs === undefined
    ? {}
    : { verificationRefs: body.verificationRefs }),
  ...(body.worktreeIdentityStatus === undefined
    ? {}
    : { worktreeIdentityStatus: body.worktreeIdentityStatus }),
  ...(body.writebackRequired === undefined
    ? {}
    : { writebackRequired: body.writebackRequired }),
})

const executionCloseoutFromFallbackCloseout = (
  input: Readonly<{
    body: AutopilotWorkFallbackCloseoutRequest
    nowIso: string
    pylonRegistrations: ReadonlyArray<PylonApiRegistrationRecord>
    record: AutopilotWorkOrderRecord
  }>,
): Effect.Effect<AutopilotWorkExecutionCloseoutRecord, AutopilotWorkStoreError> => {
  const work = projectionForRecord(
    input.record,
    false,
    input.nowIso,
    input.pylonRegistrations,
  )
  const fallbackLeaseIntents = new Map(
    work.fallbackLeaseIntents.map(intent => [intent.assignmentRef, intent]),
  )
  const assignmentRefsMatch =
    input.body.assignmentRefs.length > 0 &&
    input.body.assignmentRefs.every(ref => fallbackLeaseIntents.has(ref))
  const runnerMatches =
    work.placementDecision.source === 'fallback' &&
    work.placementDecision.selectedRunnerKind === input.body.runnerKind &&
    input.body.assignmentRefs.every(
      ref => fallbackLeaseIntents.get(ref)?.runnerKind === input.body.runnerKind,
    )
  const executionCloseout = executionCloseoutRecordFromFallbackCloseoutBody(
    input.body,
  )

  if (
    !assignmentRefsMatch ||
    !runnerMatches ||
    !executionCloseoutRefsArePublicSafe(executionCloseout)
  ) {
    return Effect.fail(
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason:
          'Autopilot fallback closeout must match the selected fallback assignment and runner, and contain only public-safe closeout refs.',
      }),
    )
  }

  return Effect.succeed(executionCloseout)
}

export const recordAutopilotWorkerCloseoutFromPylon = async (
  store: AutopilotWorkStore,
  input: AutopilotWorkerCloseoutIngestionInput,
): Promise<AutopilotWorkOrderRecord | undefined> => {
  const workOrderRef = autopilotWorkOrderRefForAssignment(input.assignment)

  if (workOrderRef === undefined) {
    return undefined
  }

  const record = await store.readWorkOrder(workOrderRef)

  if (record === undefined) {
    return undefined
  }

  const executionCloseoutExit = await Effect.runPromiseExit(
    executionCloseoutFromPylonWorkerCloseout({
      assignment: input.assignment,
      body: input.body,
      record,
    }),
  )

  if (Exit.isFailure(executionCloseoutExit)) {
    return undefined
  }

  return store.recordExecutionCloseout({
    executionCloseout: executionCloseoutExit.value,
    ownerUserId: record.ownerUserId,
    updatedAt: input.nowIso,
    workOrderRef: record.workOrderRef,
  })
}

const maybeExecuteReadyWork = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    idempotent: boolean
    nowIso: string
    pylonRegistrations: ReadonlyArray<PylonApiRegistrationRecord>
    record: AutopilotWorkOrderRecord
  }>,
): Effect.Effect<AutopilotWorkOrderRecord, AutopilotWorkStoreError> =>
  Effect.gen(function* () {
    if (
      dependencies.executeReadyWork === undefined ||
      input.record.executionCloseout !== null ||
      input.record.state === 'payment_required' ||
      input.record.state === 'access_required' ||
      scheduledLaunchHoldsDispatch(input.record.scheduledLaunch)
    ) {
      return input.record
    }

    const work = projectionForRecord(
      input.record,
      input.idempotent,
      input.nowIso,
      input.pylonRegistrations,
    )

    if (
      work.placementDecision.selectedRunnerKind !== hostedGeminiRunnerKind ||
      work.fallbackLeaseIntents.length === 0
    ) {
      return input.record
    }

    const executionCloseout = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.executeReadyWork?.(env, {
        nowIso: input.nowIso,
        work,
      }) ?? Promise.resolve(undefined),
    })

    if (executionCloseout === undefined) {
      return input.record
    }

    const validExecutionCloseout = yield* validateExecutionCloseoutForWork(
      executionCloseout,
      work,
    )
    const delivered = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        dependencies.makeStore(env).recordExecutionCloseout({
          executionCloseout: validExecutionCloseout,
          ownerUserId: input.record.ownerUserId,
          updatedAt: input.nowIso,
          workOrderRef: input.record.workOrderRef,
        }),
    })

    return delivered ?? input.record
  })

const autopilotPylonAssignmentIdempotencyKey = (
  workOrderRef: string,
  assignmentRef: string,
): string => `autopilot:pylon_assignment:${workOrderRef}:${assignmentRef}`

const pylonAssignmentRequestForIntent = (
  work: AutopilotWorkOrderProjection,
  intent: AutopilotPylonAssignmentIntentProjection,
) => {
  const codingAssignment =
    autopilotCodingAssignmentsForWork({
      fallbackLeaseIntents: [],
      funding: work.funding,
      paymentChallengeRef: work.paymentChallengeRef,
      pylonAssignmentIntents: [intent],
      quote: work.quote,
      tasks: work.tasks,
      workOrderRef: work.workOrderRef,
    })[0] ?? null
  const codingAssignmentJson =
    codingAssignment === null
      ? null
      : (parseJsonUnknown(
          JSON.stringify(codingAssignment),
        ) as Record<string, unknown>)

  return {
    acceptanceCriteriaRefs: intent.acceptanceCriteriaRefs,
    assignmentRef: intent.assignmentRef,
    campaignPaused: false,
    campaignPolicyRefs: [
      'policy.public.autopilot_coder.no_spend_pylon_assignment',
    ],
    campaignRef: `campaign.public.autopilot_coder.no_spend.${work.workOrderRef}`,
    closeoutPathRefs: intent.closeoutPathRefs,
    ...(codingAssignmentJson === null
      ? {}
      : { codingAssignment: codingAssignmentJson }),
    forumAutoPublishAllowed: false,
    idempotencyRefs: [
      `idempotency.public.${intent.assignmentRef}.autopilot_work_order`,
    ],
    jobKind: intent.jobKind,
    leaseSeconds: 15 * 60,
    noDuplicateAssignmentRefs: [
      `dedupe.public.${intent.assignmentRef}.single_active_lease`,
    ],
    noForumAutoPublishRefs: intent.noForumAutoPublishRefs,
    operatorPauseRefs: ['pause.public.autopilot_coder.no_operator_pause'],
    paymentMode: intent.paymentMode,
    pylonRef: intent.pylonRef,
    requiredCapabilityRefs: intent.requiredCapabilityRefs,
    resultExpectationRefs: intent.resultExpectationRefs,
    rollbackRefs: intent.rollbackRefs,
    selectionPolicyRefs: [
      `placement_policy.${work.workOrderRef}`,
      ...intent.selectionPolicyRefs,
    ],
    spendCapRefs: intent.spendCapRefs,
    taskRefs: [work.workOrderRef, intent.taskRef],
  }
}

const createPylonAssignmentForIntent = async (
  input: Readonly<{
    intent: AutopilotPylonAssignmentIntentProjection
    makeId: () => string
    nowIso: string
    ownerAgentUserId: string
    store: AutopilotPylonAssignmentLeaseStore
    work: AutopilotWorkOrderProjection
  }>,
): Promise<PylonApiAssignmentRecord | undefined> => {
  const idempotencyKeyHash = await sha256Hex(
    autopilotPylonAssignmentIdempotencyKey(
      input.work.workOrderRef,
      input.intent.assignmentRef,
    ),
  )
  const existingByIdempotency =
    await input.store.readAssignmentByIdempotencyKeyHash(idempotencyKeyHash)

  if (existingByIdempotency !== undefined) {
    return existingByIdempotency
  }

  const existingByRef = await input.store.readAssignment(
    input.intent.assignmentRef,
  )

  if (existingByRef !== undefined) {
    return existingByRef
  }

  const registration = await input.store.readRegistration(input.intent.pylonRef)

  if (
    registration === undefined ||
    registration.ownerAgentUserId !== input.ownerAgentUserId
  ) {
    return undefined
  }

  const assignment = buildPylonApiAssignmentRecord({
    idempotencyKeyHash,
    makeId: input.makeId,
    nowIso: input.nowIso,
    ownerAgentUserId: registration.ownerAgentUserId,
    request: pylonAssignmentRequestForIntent(input.work, input.intent),
  })
  const result = await input.store.createAssignment(assignment)

  return result.record
}

const maybeDispatchPylonAssignments = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    idempotent: boolean
    nowIso: string
    pylonRegistrations: ReadonlyArray<PylonApiRegistrationRecord>
    record: AutopilotWorkOrderRecord
  }>,
): Effect.Effect<AutopilotWorkOrderRecord, AutopilotWorkStoreError> =>
  Effect.gen(function* () {
    if (
      input.record.executionCloseout !== null ||
      input.record.state === 'access_required' ||
      input.record.state === 'payment_required' ||
      input.record.state === 'queued_or_running' ||
      scheduledLaunchHoldsDispatch(input.record.scheduledLaunch)
    ) {
      return input.record
    }

    const pylonStore = pylonAssignmentLeaseStore(
      dependencies.makePylonApiStore?.(env),
    )

    if (pylonStore === undefined) {
      return input.record
    }

    const work = projectionForRecord(
      input.record,
      input.idempotent,
      input.nowIso,
      input.pylonRegistrations,
    )

    if (work.pylonAssignmentIntents.length === 0) {
      return input.record
    }

    const assignments = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: errorReason(error),
        }),
      try: () =>
        Promise.all(
          work.pylonAssignmentIntents.map(intent =>
            createPylonAssignmentForIntent({
              intent,
              makeId: () => routeMakeId(dependencies),
              nowIso: input.nowIso,
              ownerAgentUserId: input.record.agentUserId,
              store: pylonStore,
              work,
            })
          ),
        ),
    })

    if (assignments.filter(Boolean).length === 0) {
      return input.record
    }

    const dispatched = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: errorReason(error),
        }),
      try: () =>
        dependencies.makeStore(env).recordPylonAssignmentDispatch({
          ownerUserId: input.record.ownerUserId,
          updatedAt: input.nowIso,
          workOrderRef: input.record.workOrderRef,
        }),
    })

    return dispatched ?? input.record
  })

export type AutopilotScheduledLaunchDispatchReport = Readonly<{
  dispatchedWorkOrderRefs: ReadonlyArray<string>
  expiredWorkOrderRefs: ReadonlyArray<string>
  generatedAt: string
  heldWorkOrders: ReadonlyArray<Readonly<{
    reasonRef: string
    workOrderRef: string
  }>>
}>

const releasedStateForScheduledRecord = (
  record: AutopilotWorkOrderRecord,
): OpenAgentsAutopilotWorkStateType =>
  record.state === 'scheduled' ? 'accepted_free_slice' : record.state

const dispatchableScheduledStates: ReadonlySet<OpenAgentsAutopilotWorkStateType> =
  new Set(['accepted_free_slice', 'paid_ready', 'scheduled'])

export const dispatchDueScheduledAutopilotWork = <
  Bindings extends AutopilotWorkRouteEnv,
>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{ limit?: number; nowIso: string }>,
): Effect.Effect<AutopilotScheduledLaunchDispatchReport> =>
  Effect.gen(function* () {
    const nowIso = input.nowIso
    const store = dependencies.makeStore(env)
    const pending = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: errorReason(error),
        }),
      try: () =>
        store.listPendingScheduledWorkOrders({ limit: input.limit ?? 50 }),
    })
    const dispatchedWorkOrderRefs: Array<string> = []
    const expiredWorkOrderRefs: Array<string> = []
    const heldWorkOrders: Array<{ reasonRef: string; workOrderRef: string }> =
      []
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )

    for (const record of pending) {
      const scheduledLaunch = record.scheduledLaunch

      if (
        scheduledLaunch === null ||
        !scheduledLaunchHoldsDispatch(scheduledLaunch)
      ) {
        continue
      }

      if (scheduledLaunchWindowExpired(scheduledLaunch, nowIso)) {
        yield* Effect.tryPromise({
          catch: error =>
            new AutopilotWorkStoreError({
              kind: 'storage_error',
              reason: errorReason(error),
            }),
          try: () =>
            store.recordScheduledLaunchTransition({
              ownerUserId: record.ownerUserId,
              scheduledLaunch: expiredScheduledLaunch(scheduledLaunch, nowIso),
              state: 'blocked',
              updatedAt: nowIso,
              workOrderRef: record.workOrderRef,
            }),
        })
        expiredWorkOrderRefs.push(record.workOrderRef)
        continue
      }

      if (!scheduledLaunchDue(scheduledLaunch, nowIso)) {
        continue
      }

      if (!dispatchableScheduledStates.has(record.state)) {
        heldWorkOrders.push({
          reasonRef: `scheduled_launch.held.${record.state}`,
          workOrderRef: record.workOrderRef,
        })
        continue
      }

      const released = yield* Effect.tryPromise({
        catch: error =>
          new AutopilotWorkStoreError({
            kind: 'storage_error',
            reason: errorReason(error),
          }),
        try: () =>
          store.recordScheduledLaunchTransition({
            ownerUserId: record.ownerUserId,
            scheduledLaunch: dispatchedScheduledLaunch(
              scheduledLaunch,
              nowIso,
            ),
            state: releasedStateForScheduledRecord(record),
            updatedAt: nowIso,
            workOrderRef: record.workOrderRef,
          }),
      })

      if (released === undefined) {
        heldWorkOrders.push({
          reasonRef: 'scheduled_launch.held.transition_lost',
          workOrderRef: record.workOrderRef,
        })
        continue
      }

      const executedRecord = yield* maybeExecuteReadyWork(dependencies, env, {
        idempotent: false,
        nowIso,
        pylonRegistrations,
        record: released,
      })

      yield* maybeDispatchPylonAssignments(dependencies, env, {
        idempotent: false,
        nowIso,
        pylonRegistrations,
        record: executedRecord,
      })
      dispatchedWorkOrderRefs.push(record.workOrderRef)
    }

    return {
      dispatchedWorkOrderRefs,
      expiredWorkOrderRefs,
      generatedAt: nowIso,
      heldWorkOrders,
    }
  }).pipe(
    Effect.catch(error =>
      Effect.succeed({
        dispatchedWorkOrderRefs: [],
        expiredWorkOrderRefs: [],
        generatedAt: input.nowIso,
        heldWorkOrders: [
          {
            reasonRef: `scheduled_launch.dispatch_failed.${error.kind}`,
            workOrderRef: 'scheduled_launch.dispatch_batch',
          },
        ],
      }),
    ),
  )

const terminalEventKindForState = (
  state: OpenAgentsAutopilotWorkStateType,
): AutopilotWorkEventKind | undefined => {
  switch (state) {
    case 'accepted':
      return 'accepted'
    case 'access_required':
      return 'needs_access'
    case 'blocked':
    case 'invalid':
      return 'blocked'
    case 'delivered':
      return 'delivered'
    case 'payment_required':
      return 'payment_required'
    case 'paid_ready':
      return 'running'
    case 'queued_or_running':
      return 'running'
    case 'rejected':
      return 'rejected'
    case 'revision_required':
      return 'revision_required'
    case 'scheduled':
      return 'scheduled'
    case 'accepted_free_slice':
      return undefined
  }
}

const eventForRecord = (
  record: AutopilotWorkOrderRecord,
  eventKind: AutopilotWorkEventKind,
  sequence: number,
  occurredAt: string,
): AutopilotWorkEventProjection => ({
  eventKind,
  eventRef: `event.${record.workOrderRef}.${sequence}`,
  occurredAt,
  publicSafe: true,
  sequence,
  state: record.state,
  taskRefs: record.taskRefs,
  workOrderRef: record.workOrderRef,
})

export const eventsForRecord = (
  record: AutopilotWorkOrderRecord,
): ReadonlyArray<AutopilotWorkEventProjection> => {
  const events = [
    eventForRecord(record, 'queued', 1, record.createdAt),
  ]
  const terminalKind = terminalEventKindForState(record.state)

  if (terminalKind !== undefined) {
    events.push(
      eventForRecord(record, terminalKind, events.length + 1, record.updatedAt),
    )
  }

  return events
}

const buildWorkOrderRecord = (
  input: Readonly<{
    agentCredentialId: string
    agentUserId: string
    id: string
    idempotencyKeyHash: string
    nowIso: string
    ownerUserId: string
    request: OpenAgentsAutopilotWorkRequest
  }>,
): AutopilotWorkOrderRecord => {
  const workOrderRef = workOrderRefForId(input.id)
  const paymentChallengeRef = paymentChallengeRefForRequest(input.request)
  const scheduledLaunch = scheduledLaunchRecordForRequest(
    input.request,
    input.nowIso,
  )

  return {
    accessRequestRefs: accessRequestRefsForRequest(input.request),
    agentCredentialId: input.agentCredentialId,
    agentUserId: input.agentUserId,
    archivedAt: null,
    buyerPaymentProofRef: null,
    clientRequestRef: input.request.clientRequestRef,
    createdAt: input.nowIso,
    eventStreamRef: eventStreamRefForWorkOrder(workOrderRef),
    executionCloseout: null,
    id: input.id,
    idempotencyKeyHash: input.idempotencyKeyHash,
    ownerUserId: input.ownerUserId,
    paymentChallengeRef,
    request: input.request,
    reviewDecision: null,
    scheduledLaunch,
    state: stateForRequest(input.request, scheduledLaunch),
    statusUrlRef: statusUrlRefForWorkOrder(workOrderRef),
    taskRefs: input.request.tasks.map(task => task.taskRef),
    updatedAt: input.nowIso,
    workOrderRef,
  }
}

const createWorkOrder = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.write',
      },
    )
    const idempotencyKeyHash = hasBearerAuthorization(request)
      ? yield* requireIdempotencyHash(request)
      : yield* idempotencyHashForBrowserRequest(
          dependencies,
          request,
          auth.ownerUserId,
        )
    const existing = yield* Effect.promise(() =>
      dependencies
        .makeStore(env)
        .readWorkOrderByIdempotency(auth.ownerUserId, idempotencyKeyHash)
    )

    if (existing !== undefined) {
      const proof = yield* verifyBuyerPaymentProofFromRequest(
        dependencies,
        env,
        {
          nowIso,
          record: existing,
          request,
        },
      )
      const paid = proof === undefined
        ? existing
        : yield* Effect.tryPromise({
            catch: error =>
              error instanceof AutopilotWorkStoreError
                ? error
                : new AutopilotWorkStoreError({
                    kind: 'storage_error',
                    reason: errorReason(error),
                  }),
            try: () =>
              dependencies.makeStore(env).recordBuyerPaymentProof({
                buyerPaymentProofRef: proof.proofRef,
                ownerUserId: auth.ownerUserId,
                updatedAt: nowIso,
                workOrderRef: existing.workOrderRef,
              }),
          })
      const record = paid ?? existing
      const executedRecord = yield* maybeExecuteReadyWork(
        dependencies,
        env,
        {
          idempotent: true,
          nowIso,
          pylonRegistrations,
          record,
        },
      )
      const dispatchedRecord = yield* maybeDispatchPylonAssignments(
        dependencies,
        env,
        {
          idempotent: true,
          nowIso,
          pylonRegistrations,
          record: executedRecord,
        },
      )
      const projection = projectionForRecord(
        dispatchedRecord,
        true,
        nowIso,
        pylonRegistrations,
      )

      return dispatchedRecord.state === 'payment_required'
        ? yield* paymentRequiredResponse(dependencies, env, {
            projection,
            record: dispatchedRecord,
          })
        : noStoreJsonResponse(
            { generatedAt: nowIso, work: projection },
            { status: 200 },
          )
    }

    const workRequest = yield* decodeWorkRequest(request)
    const record = buildWorkOrderRecord({
      agentCredentialId: auth.actorAgentCredentialId,
      agentUserId: auth.actorAgentUserId,
      id: routeMakeId(dependencies),
      idempotencyKeyHash,
      nowIso,
      ownerUserId: auth.ownerUserId,
      request: workRequest,
    })
    const horizonReason = scheduledLaunchHorizonReason(
      record.scheduledLaunch,
      nowIso,
    )

    if (horizonReason !== undefined) {
      return yield* new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason: horizonReason,
      })
    }
    const proof = yield* verifyBuyerPaymentProofFromRequest(
      dependencies,
      env,
      {
        nowIso,
        record,
        request,
      },
    )
    const recordWithProof = proof === undefined
      ? record
      : {
          ...record,
          buyerPaymentProofRef: proof.proofRef,
          state: 'paid_ready' as const,
        }
    const created = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).createWorkOrder(recordWithProof),
    })
    const executedRecord = yield* maybeExecuteReadyWork(
      dependencies,
      env,
      {
        idempotent: created.idempotent,
        nowIso,
        pylonRegistrations,
        record: created.record,
      },
    )
    const dispatchedRecord = yield* maybeDispatchPylonAssignments(
      dependencies,
      env,
      {
        idempotent: created.idempotent,
        nowIso,
        pylonRegistrations,
        record: executedRecord,
      },
    )
    const projection = projectionForRecord(
      dispatchedRecord,
      created.idempotent,
      nowIso,
      pylonRegistrations,
    )

    return dispatchedRecord.state === 'payment_required'
      ? yield* paymentRequiredResponse(dependencies, env, {
          projection,
          record: dispatchedRecord,
        })
      : noStoreJsonResponse(
          { generatedAt: nowIso, work: projection },
          { status: created.idempotent ? 200 : 202 },
        )
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const readWorkOrder = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.read',
      },
    )
    const record = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).readWorkOrder(workOrderRef),
    })

    if (record === undefined || record.ownerUserId !== auth.ownerUserId) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse({
      generatedAt: nowIso,
      work: projectionForRecord(record, false, nowIso, pylonRegistrations),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

// Lane C fanout (#4783): server-side gate that bursts a product work order to
// the public labor market when owned capacity is dark, the customer opted in,
// and the public trust-tier floor is met — then creates the linked market work
// request. The tier floor + opt-in + budget cap are enforced HERE (server-side)
// before any market listing is created; a private order can never leave the
// first-party lanes through this route.
const laneCFanoutWorkOrder = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(dependencies, env)
    const auth = yield* authenticateAutopilotWorkRequest(dependencies, request, env, {
      ctx,
      nowIso: () => nowIso,
      requiredScope: 'customer_orders.write',
    })
    const record = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).readWorkOrder(workOrderRef),
    })
    if (record === undefined || record.ownerUserId !== auth.ownerUserId) {
      return noStoreJsonResponse(
        { error: 'autopilot_work_not_found', reason: 'Autopilot work order was not found.' },
        { status: 404 },
      )
    }

    const body = (yield* Effect.tryPromise({
      catch: () =>
        new AutopilotWorkStoreError({ kind: 'validation_error', reason: 'Invalid Lane C fanout body.' }),
      try: () => request.json() as Promise<Record<string, unknown>>,
    })) ?? {}
    const customerOptIn = body.customerOptIn === true
    const budgetCapSats = typeof body.budgetCapSats === 'number' ? body.budgetCapSats : 0
    const requestedWorkClass =
      typeof body.workClass === 'string' ? body.workClass : undefined
    if (
      requestedWorkClass !== undefined &&
      !isMarketplaceWorkClassId(requestedWorkClass)
    ) {
      return noStoreJsonResponse(
        {
          error: 'lane_c_fanout_invalid_work_class',
          reason: `Unknown marketplace work class: ${requestedWorkClass}`,
        },
        { status: 400 },
      )
    }

    const work = projectionForRecord(record, false, nowIso, pylonRegistrations)
    // Server-supplied placement/policy + readiness facts for the lane-C gate.
    // The customer never asserts these — they come from the work order
    // projection, so the public-trust floor stays enforced server-side.
    const workOrderFacts = {
      placementSource: work.placementDecision.source,
      placementAvailabilityState: work.placementDecision.availabilityState,
      privacyTier: work.placementPolicy.privacyTier,
      settlementBridgeReady: true, // P4 (#4780) USD->sats settlement bridge is built/closed.
      marketInventoryReady: true,
      artifactAuthorityReady: true,
      validatorPolicyReady: true,
      missionWorkOrderUnified: true,
      providerTrustTier: 'public_rung1' as const,
    }
    const fanout = evaluateLaneCFanoutForWorkOrder({
      ...workOrderFacts,
      customerOptIn,
      budgetCapSats,
      // The fanout authorizes market quotes up to the budget cap; the per-quote
      // budget check is enforced again at escrow-reserve time on acceptance.
      quotedSats: budgetCapSats,
    })

    // Self-serve fanout plan (autopilot.control_center_fanout_marketplace.v1):
    // this route is customer-authenticated (customer_orders.write), so the
    // customer (not an operator) initiates the fanout in a SINGLE self-serve
    // action. Build the typed plan so the response is the self-serve capability,
    // not an operator-staged two-step. The plan reuses the SAME lane-C gate, so
    // a blocked gate yields a plan with marketWorkRequest=null. It is INERT and
    // clears the self-serve blocker without bypassing class-specific
    // capability or verification contracts.
    const selfServePlanResult = buildSelfServeFanoutPlan(
      {
        workOrderRef,
        customerRef: `agent:${auth.ownerUserId}`,
        customerOptIn,
        budgetCapSats,
        title: `Lane C fanout: ${workOrderRef}`,
        ...(requestedWorkClass === undefined
          ? {}
          : { workClass: requestedWorkClass }),
      },
      workOrderFacts,
      nowIso,
    )
    if (!selfServePlanResult.ok) {
      return noStoreJsonResponse(
        {
          error: 'lane_c_fanout_invalid_work_class',
          reason: selfServePlanResult.error.reason,
        },
        { status: 400 },
      )
    }
    const selfServePlan = selfServePlanResult.plan

    if (!fanout.readyForMarket) {
      return noStoreJsonResponse(
        {
          error: 'lane_c_fanout_blocked',
          fanout: {
            lane: fanout.decision.lane,
            ownedCapacityState: fanout.ownedCapacityState,
            reasonRefs: fanout.decision.reasonRefs,
            state: fanout.decision.state,
          },
          reason: 'Lane C fanout gate not satisfied.',
        },
        { status: 409 },
      )
    }

    // Server gate passed. The public-tier floor + opt-in + budget cap are now
    // enforced server-side; the route authorizes the fanout and returns the
    // public-safe objective ref + the market work-request input the requester
    // uses to list the linked job on the open market
    // (POST /api/forum/work-requests). A private/non-public order never reaches
    // this branch (it gets a 409 above), so the floor cannot be bypassed.
    const objectiveRef = laneCFanoutObjectiveRef(workOrderRef)
    const marketWorkRequestInput = selfServePlan.marketWorkRequest ?? {
      budgetSats: budgetCapSats,
      deadlineRef: 'deadline.public.lane_c_fanout.20261231',
      objectiveRef,
      requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
      title: `Lane C fanout: ${workOrderRef}`.slice(0, 160),
      verificationCommandRef: 'command.public.pylon.labor.bun_test',
      workClass: 'code_task',
    }
    return noStoreJsonResponse(
      {
        fanout: {
          authorized: true,
          lane: fanout.decision.lane,
          objectiveRef,
          ownedCapacityState: fanout.ownedCapacityState,
          reasonRefs: fanout.decision.reasonRefs,
          state: fanout.decision.state,
        },
        generatedAt: nowIso,
        marketWorkRequestInput,
        // The customer-initiated self-serve fanout plan (single-action,
        // INERT until dispatch is armed). This makes the route the
        // self-serve capability rather than an operator-staged two-step.
        selfServeFanout: selfServePlan,
        workOrderRef,
      },
      { status: 201 },
    )
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () => Effect.succeed(unauthorized())),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const validateReviewDecisionRequest = (
  body: AutopilotWorkReviewDecisionRequest,
): Effect.Effect<AutopilotWorkReviewDecisionRequest, AutopilotWorkStoreError> => {
  const decisionRefs = body.decisionRefs ?? []
  const rejectionRefs = body.rejectionRefs ?? []
  const revisionRequestRefs = body.revisionRequestRefs ?? []
  const refsArePublicSafe =
    publicSafeReviewRefs(decisionRefs) &&
    publicSafeReviewRefs(rejectionRefs) &&
    publicSafeReviewRefs(revisionRequestRefs)
  const actionHasRequiredRefs =
    body.action === 'accept'
      ? decisionRefs.length > 0
      : body.action === 'reject'
        ? rejectionRefs.length > 0
        : revisionRequestRefs.length > 0

  if (!refsArePublicSafe || !actionHasRequiredRefs) {
    return Effect.fail(
      new AutopilotWorkStoreError({
        kind: 'validation_error',
        reason:
          'Autopilot review decisions require public-safe action refs for the selected review action.',
      }),
    )
  }

  return Effect.succeed(body)
}

const reviewDecisionRecordFromRequest = (
  input: Readonly<{
    actorAgentCredentialId: string
    actorAgentUserId: string
    body: AutopilotWorkReviewDecisionRequest
    idempotencyKeyHash: string
    nowIso: string
  }>,
): AutopilotWorkReviewDecisionRecord => ({
  action: input.body.action,
  actorAgentCredentialId: input.actorAgentCredentialId,
  actorAgentUserId: input.actorAgentUserId,
  decisionRefs: input.body.decisionRefs ?? [],
  idempotencyKeyHash: input.idempotencyKeyHash,
  recordedAt: input.nowIso,
  rejectionRefs: input.body.rejectionRefs ?? [],
  revisionRequestRefs: input.body.revisionRequestRefs ?? [],
})

const closeoutRecordsEqual = (
  left: AutopilotWorkExecutionCloseoutRecord,
  right: AutopilotWorkExecutionCloseoutRecord,
): boolean => JSON.stringify(left) === JSON.stringify(right)

const closeoutWorkOrder = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.write',
      },
    )
    yield* requireIdempotencyHash(request)
    const body = yield* decodeFallbackCloseoutRequest(request)
    const existing = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).readWorkOrder(workOrderRef),
    })

    if (existing === undefined || existing.ownerUserId !== auth.ownerUserId) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    const requestedExecutionCloseout =
      executionCloseoutRecordFromFallbackCloseoutBody(body)

    if (existing.executionCloseout !== null) {
      if (
        executionCloseoutRefsArePublicSafe(requestedExecutionCloseout) &&
        closeoutRecordsEqual(
          existing.executionCloseout,
          requestedExecutionCloseout,
        )
      ) {
        return noStoreJsonResponse(
          {
            generatedAt: nowIso,
            idempotent: true,
            work: projectionForRecord(
              existing,
              true,
              nowIso,
              pylonRegistrations,
            ),
          },
          { status: 200 },
        )
      }

      return yield* new AutopilotWorkStoreError({
        kind: 'conflict',
        reason:
          'Autopilot work already has execution closeout evidence.',
      })
    }

    const executionCloseout = yield* executionCloseoutFromFallbackCloseout({
      body,
      nowIso,
      pylonRegistrations,
      record: existing,
    })

    const delivered = yield* Effect.tryPromise({
      catch: error =>
        error instanceof AutopilotWorkStoreError
          ? error
          : new AutopilotWorkStoreError({
              kind: 'storage_error',
              reason: error instanceof Error ? error.message : String(error),
            }),
      try: () =>
        dependencies.makeStore(env).recordExecutionCloseout({
          executionCloseout,
          ownerUserId: auth.ownerUserId,
          updatedAt: nowIso,
          workOrderRef,
        }),
    })

    if (delivered === undefined) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse(
      {
        generatedAt: nowIso,
        idempotent: false,
        work: projectionForRecord(delivered, false, nowIso, pylonRegistrations),
      },
      { status: 201 },
    )
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const reviewWorkOrder = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.write',
      },
    )
    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const body = yield* Effect.flatMap(
      decodeReviewDecisionRequest(request),
      validateReviewDecisionRequest,
    )
    const reviewDecision = reviewDecisionRecordFromRequest({
      actorAgentCredentialId: auth.actorAgentCredentialId,
      actorAgentUserId: auth.actorAgentUserId,
      body,
      idempotencyKeyHash,
      nowIso,
    })
    const state = reviewStateForAction(body.action)
    const result = yield* Effect.tryPromise({
      catch: error =>
        error instanceof AutopilotWorkStoreError
          ? error
          : new AutopilotWorkStoreError({
              kind: 'storage_error',
              reason: error instanceof Error ? error.message : String(error),
            }),
      try: () =>
        dependencies.makeStore(env).recordReviewDecision({
          ownerUserId: auth.ownerUserId,
          reviewDecision,
          state,
          updatedAt: nowIso,
          workOrderRef,
        }),
    })

    if (result === undefined) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse(
      {
        generatedAt: nowIso,
        idempotent: result.idempotent,
        work: projectionForRecord(
          result.record,
          result.idempotent,
          nowIso,
          pylonRegistrations,
        ),
      },
      { status: result.idempotent ? 200 : 201 },
    )
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const parseAfterCursor = (request: Request): number => {
  const url = new URL(request.url)
  const headerCursor = request.headers.get('Last-Event-ID')
  const queryCursor = url.searchParams.get('after')
  const rawCursor = headerCursor === null || headerCursor === ''
    ? queryCursor
    : headerCursor

  if (rawCursor === null || rawCursor === undefined || rawCursor === '') {
    return 0
  }

  const cursor = Number(rawCursor)

  return Number.isSafeInteger(cursor) && cursor >= 0 ? cursor : 0
}

const eventStreamPayload = (
  events: ReadonlyArray<AutopilotWorkEventProjection>,
): string => {
  const body = events
    .map(event =>
      [
        `id: ${event.sequence}`,
        `event: ${event.eventKind}`,
        `data: ${JSON.stringify({ event })}`,
        '',
      ].join('\n'),
    )
    .join('\n')

  return body === '' ? ': no events\n\n' : `${body}\n`
}

const eventStreamResponse = (
  events: ReadonlyArray<AutopilotWorkEventProjection>,
) =>
  new globalThis.Response(eventStreamPayload(events), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/event-stream; charset=utf-8',
      'x-accel-buffering': 'no',
    },
  })

const wantsEventStream = (request: Request): boolean => {
  const url = new URL(request.url)

  return (
    request.headers.get('accept')?.includes('text/event-stream') === true ||
    url.searchParams.get('stream') === 'sse'
  )
}

const readWorkOrderEvents = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.read',
      },
    )
    const record = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).readWorkOrder(workOrderRef),
    })

    if (record === undefined || record.ownerUserId !== auth.ownerUserId) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    const after = parseAfterCursor(request)
    const events = eventsForRecord(record).filter(
      event => event.sequence > after,
    )

    if (wantsEventStream(request)) {
      return eventStreamResponse(events)
    }

    return noStoreJsonResponse({
      events,
      generatedAt: nowIso,
      nextAfter: events.length === 0
        ? after
        : events[events.length - 1]?.sequence ?? after,
      workOrderRef: record.workOrderRef,
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const workOrderRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)$/.exec(pathname)

  return match?.[1]
}

const workOrderBriefingRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/briefing$/.exec(pathname)

  return match?.[1]
}

const readWorkOrderBriefing = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.read',
      },
    )
    const record = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.makeStore(env).readWorkOrder(workOrderRef),
    })

    if (record === undefined || record.ownerUserId !== auth.ownerUserId) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_work_not_found',
          reason: 'Autopilot work order was not found.',
        },
        { status: 404 },
      )
    }

    return noStoreJsonResponse({
      briefing: missionBriefingForWorkOrder({
        events: eventsForRecord(record),
        nowIso,
        work: projectionForRecord(record, false, nowIso, pylonRegistrations),
      }),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

const workOrderEventsRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/events$/.exec(pathname)

  return match?.[1]
}

const workOrderCloseoutRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/closeout$/.exec(pathname)

  return match?.[1]
}

const workOrderLaneCFanoutRefFromPath = (
  pathname: string,
): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/lane-c-fanout$/.exec(pathname)

  return match?.[1]
}

const workOrderReviewRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/review$/.exec(pathname)

  return match?.[1]
}

const promiseIdQueryPattern = /^[a-z0-9_]+(\.[a-z0-9_]+)*\.v\d+$/

const routingSummaryForProjection = (
  projection: AutopilotWorkOrderProjection,
) => {
  const activeLane = projection.pricingPolicy.activeLane

  return {
    availabilityState: projection.placementDecision.availabilityState,
    buyerDebitRequired: activeLane?.buyerDebitRequired ?? false,
    fallbackLeaseIntentCount: projection.fallbackLeaseIntents.length,
    fallbackRunnerKind: projection.placementDecision.fallbackRunnerKind,
    laneRef: activeLane?.laneRef ?? null,
    meterKind: activeLane?.meterKind ?? null,
    pylonAssignmentIntentCount: projection.pylonAssignmentIntents.length,
    selectedRunnerKind: projection.placementDecision.selectedRunnerKind,
    source: projection.placementDecision.source,
  }
}

const listWorkOrders = <Bindings extends AutopilotWorkRouteEnv>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
  url: URL,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const promiseId = url.searchParams.get('promiseId') ?? ''

    if (!promiseIdQueryPattern.test(promiseId)) {
      return noStoreJsonResponse(
        {
          error: 'autopilot_work_list_requires_promise_id',
          reason:
            'List recovery requires a promiseId query parameter shaped like autopilot.mission_briefing.v1.',
        },
        { status: 400 },
      )
    }

    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateAutopilotWorkRequest(
      dependencies,
      request,
      env,
      {
        ctx,
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.read',
      },
    )
    const records = yield* Effect.tryPromise({
      catch: error =>
        new AutopilotWorkStoreError({
          kind: 'storage_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        dependencies.makeStore(env).listWorkOrdersForOwner({
          limit: 200,
          ownerUserId: auth.ownerUserId,
        }),
    })
    const matching = records.filter(
      record => record.request.promiseRef?.promiseId === promiseId,
    )
    const pylonRegistrations = yield* routePylonRegistrations(dependencies, env)

    return noStoreJsonResponse({
      generatedAt: nowIso,
      promiseId,
      workOrders: matching.map(record => {
        const projection = projectionForRecord(
          record,
          false,
          nowIso,
          pylonRegistrations,
        )

        return {
          createdAt: record.createdAt,
          generatedAt: nowIso,
          issueRefs: record.taskRefs
            .flatMap(ref => /^task\.github_issue\.issue_(\d+)\./.exec(ref)?.[1] ?? [])
            .map(issueNumber => `github.issue.${issueNumber}`),
          promiseRef: {
            blockerRefs: record.request.promiseRef?.blockerRefs ?? [],
            promiseId: record.request.promiseRef?.promiseId ?? promiseId,
            registryVersion: record.request.promiseRef?.registryVersion ?? null,
          },
          routing: routingSummaryForProjection(projection),
          state: record.state,
          taskRefs: record.taskRefs,
          updatedAt: record.updatedAt,
          workOrderRef: record.workOrderRef,
        }
      }),
    })
  }).pipe(
    Effect.catchTag('CustomerOrderAgentAuthFailure', () =>
      Effect.succeed(unauthorized())
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

export const makeAutopilotWorkRoutes = <
  Bindings extends AutopilotWorkRouteEnv,
>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
) => ({
  routeAutopilotWorkRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/autopilot/work') {
      return M.value(request.method).pipe(
        M.when('POST', () => createWorkOrder(dependencies, request, env, ctx)),
        M.when('GET', () =>
          listWorkOrders(dependencies, request, env, ctx, url)
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET', 'POST']))),
      )
    }

    const workOrderEventsRef = workOrderEventsRefFromPath(url.pathname)

    if (workOrderEventsRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkOrderEvents(
            dependencies,
            request,
            env,
            ctx,
            workOrderEventsRef,
          )
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const workOrderBriefingRef = workOrderBriefingRefFromPath(url.pathname)

    if (workOrderBriefingRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkOrderBriefing(
            dependencies,
            request,
            env,
            ctx,
            workOrderBriefingRef,
          )
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const workOrderCloseoutRef = workOrderCloseoutRefFromPath(url.pathname)

    if (workOrderCloseoutRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          closeoutWorkOrder(
            dependencies,
            request,
            env,
            ctx,
            workOrderCloseoutRef,
          )
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const workOrderReviewRef = workOrderReviewRefFromPath(url.pathname)

    if (workOrderReviewRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          reviewWorkOrder(dependencies, request, env, ctx, workOrderReviewRef)
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const workOrderLaneCFanoutRef = workOrderLaneCFanoutRefFromPath(url.pathname)

    if (workOrderLaneCFanoutRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('POST', () =>
          laneCFanoutWorkOrder(
            dependencies,
            request,
            env,
            ctx,
            workOrderLaneCFanoutRef,
          )
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
      )
    }

    const workOrderRef = workOrderRefFromPath(url.pathname)

    if (workOrderRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkOrder(dependencies, request, env, ctx, workOrderRef)
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    return undefined
  },
})

const executionCloseoutFromRowValue = (
  value: unknown,
): AutopilotWorkExecutionCloseoutRecord | null =>
  typeof value === 'string' && value.trim() !== ''
    ? S.decodeUnknownSync(AutopilotWorkExecutionCloseoutRecord)(
        parseJsonUnknown(value),
      )
    : null

const reviewDecisionFromRowValue = (
  value: unknown,
): AutopilotWorkReviewDecisionRecord | null =>
  typeof value === 'string' && value.trim() !== ''
    ? S.decodeUnknownSync(AutopilotWorkReviewDecisionRecord)(
        parseJsonUnknown(value),
      )
    : null

const scheduledLaunchFromRowValue = (
  value: unknown,
): AutopilotWorkScheduledLaunchRecord | null =>
  typeof value === 'string' && value.trim() !== ''
    ? S.decodeUnknownSync(AutopilotWorkScheduledLaunchRecord)(
        parseJsonUnknown(value),
      )
    : null

const recordFromRow = (
  row: Readonly<Record<string, unknown>>,
): AutopilotWorkOrderRecord => ({
  accessRequestRefs: parseJsonStringArray(String(row.access_request_refs_json)),
  agentCredentialId: String(row.agent_credential_id),
  agentUserId: String(row.agent_user_id),
  archivedAt:
    typeof row.archived_at === 'string' ? row.archived_at : null,
  buyerPaymentProofRef:
    typeof row.buyer_payment_proof_ref === 'string'
      ? row.buyer_payment_proof_ref
      : null,
  clientRequestRef: String(row.client_request_ref),
  createdAt: String(row.created_at),
  eventStreamRef: String(row.event_stream_ref),
  executionCloseout: executionCloseoutFromRowValue(
    row.execution_closeout_json,
  ),
  id: String(row.id),
  idempotencyKeyHash: String(row.idempotency_key_hash),
  ownerUserId: String(row.owner_user_id),
  paymentChallengeRef:
    typeof row.payment_challenge_ref === 'string'
      ? row.payment_challenge_ref
      : null,
  request: decodeOpenAgentsAutopilotWorkRequest(
    parseJsonUnknown(String(row.request_json)),
  ),
  reviewDecision: reviewDecisionFromRowValue(row.review_decision_json),
  scheduledLaunch: scheduledLaunchFromRowValue(row.scheduled_launch_json),
  state: S.decodeUnknownSync(OpenAgentsAutopilotWorkState)(row.state),
  statusUrlRef: String(row.status_url_ref),
  taskRefs: parseJsonStringArray(String(row.task_refs_json)),
  updatedAt: String(row.updated_at),
  workOrderRef: String(row.work_order_ref),
})

export const makeD1AutopilotWorkStore = (
  db: D1Database,
): AutopilotWorkStore => ({
  createWorkOrder: async record => {
    const existing = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE owner_user_id = ?
           AND idempotency_key_hash = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(record.ownerUserId, record.idempotencyKeyHash)
      .first<Record<string, unknown>>()

    if (existing !== null) {
      return { idempotent: true, record: recordFromRow(existing) }
    }

    await db
      .prepare(
        `INSERT INTO autopilot_work_orders (
          id,
          work_order_ref,
          owner_user_id,
          agent_user_id,
          agent_credential_id,
          idempotency_key_hash,
          client_request_ref,
          request_json,
          state,
          task_refs_json,
          access_request_refs_json,
          buyer_payment_proof_ref,
          payment_challenge_ref,
          placement_policy_json,
          scheduled_launch_json,
          status_url_ref,
          event_stream_ref,
          created_at,
          updated_at,
          archived_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        record.id,
        record.workOrderRef,
        record.ownerUserId,
        record.agentUserId,
        record.agentCredentialId,
        record.idempotencyKeyHash,
        record.clientRequestRef,
        JSON.stringify(record.request),
        record.state,
        JSON.stringify(record.taskRefs),
        JSON.stringify(record.accessRequestRefs),
        record.buyerPaymentProofRef,
        record.paymentChallengeRef,
        JSON.stringify(record.request.placementPolicy),
        record.scheduledLaunch === null
          ? null
          : JSON.stringify(record.scheduledLaunch),
        record.statusUrlRef,
        record.eventStreamRef,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    return { idempotent: false, record }
  },
  listWorkOrdersForOwner: async input => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE owner_user_id = ?
           AND archived_at IS NULL
         ORDER BY created_at DESC
         LIMIT ?`,
      )
      .bind(input.ownerUserId, input.limit)
      .all<Record<string, unknown>>()

    return (rows.results ?? []).map(recordFromRow)
  },
  recordPylonAssignmentDispatch: async input => {
    await db
      .prepare(
        `UPDATE autopilot_work_orders
         SET state = 'queued_or_running',
             updated_at = ?
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
           AND state NOT IN ('access_required', 'payment_required', 'delivered', 'scheduled')`,
      )
      .bind(input.updatedAt, input.workOrderRef, input.ownerUserId)
      .run()

    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(input.workOrderRef, input.ownerUserId)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
  recordExecutionCloseout: async input => {
    await db
      .prepare(
        `UPDATE autopilot_work_orders
         SET execution_closeout_json = ?,
             state = 'delivered',
             updated_at = ?
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
           AND state NOT IN ('access_required', 'payment_required')`,
      )
      .bind(
        JSON.stringify(input.executionCloseout),
        input.updatedAt,
        input.workOrderRef,
        input.ownerUserId,
      )
      .run()

    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(input.workOrderRef, input.ownerUserId)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
  recordReviewDecision: async input => {
    const existing = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(input.workOrderRef, input.ownerUserId)
      .first<Record<string, unknown>>()

    if (existing === null) {
      return undefined
    }

    const existingRecord = recordFromRow(existing)

    if (existingRecord.reviewDecision !== null) {
      if (
        existingRecord.reviewDecision.idempotencyKeyHash ===
        input.reviewDecision.idempotencyKeyHash
      ) {
        return { idempotent: true, record: existingRecord }
      }

      throw new AutopilotWorkStoreError({
        kind: 'conflict',
        reason:
          'Autopilot work already has a review decision with a different idempotency key.',
      })
    }

    if (existingRecord.state !== 'delivered') {
      throw new AutopilotWorkStoreError({
        kind: 'conflict',
        reason: 'Autopilot work must be delivered before review.',
      })
    }

    await db
      .prepare(
        `UPDATE autopilot_work_orders
         SET review_decision_json = ?,
             state = ?,
             updated_at = ?
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
           AND review_decision_json IS NULL
           AND state = 'delivered'`,
      )
      .bind(
        JSON.stringify(input.reviewDecision),
        input.state,
        input.updatedAt,
        input.workOrderRef,
        input.ownerUserId,
      )
      .run()

    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(input.workOrderRef, input.ownerUserId)
      .first<Record<string, unknown>>()

    return row === null
      ? undefined
      : { idempotent: false, record: recordFromRow(row) }
  },
  recordDecisionCloseoutReceipt: async receipt => {
    await db
      .prepare(
        `INSERT INTO autopilot_decision_closeout_receipts (
          closeout_ref,
          decision_ref,
          work_order_ref,
          action,
          resolved_state,
          outcome,
          actor_agent_user_id,
          decided_at,
          receipt_refs_json,
          has_answer,
          line,
          receipt_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(closeout_ref) DO NOTHING`,
      )
      .bind(
        receipt.closeoutRef,
        receipt.decisionRef,
        receipt.workOrderRef,
        receipt.action,
        receipt.resolvedState,
        receipt.outcome,
        receipt.actorAgentUserId,
        receipt.decidedAt,
        JSON.stringify(receipt.receiptRefs),
        receipt.hasAnswer ? 1 : 0,
        receipt.line,
        JSON.stringify(receipt),
      )
      .run()
  },
  listDecisionCloseoutReceiptsForWorkOrder: async input => {
    const rows = await db
      .prepare(
        `SELECT r.receipt_json
         FROM autopilot_decision_closeout_receipts r
         INNER JOIN autopilot_work_orders w
           ON w.work_order_ref = r.work_order_ref
         WHERE r.work_order_ref = ?
           AND w.owner_user_id = ?
           AND w.archived_at IS NULL
         ORDER BY r.decided_at DESC`,
      )
      .bind(input.workOrderRef, input.ownerUserId)
      .all<Record<string, unknown>>()

    return (rows.results ?? []).flatMap(row => {
      if (typeof row.receipt_json !== 'string') {
        return []
      }

      try {
        const parsed = parseJsonUnknown(row.receipt_json)

        return validateAutopilotDecisionCloseoutReceipt(parsed)
          ? [parsed]
          : []
      } catch {
        return []
      }
    })
  },
  readDecisionCloseoutReceipt: async input => {
    const row = await db
      .prepare(
        `SELECT r.receipt_json
         FROM autopilot_decision_closeout_receipts r
         INNER JOIN autopilot_work_orders w
           ON w.work_order_ref = r.work_order_ref
         WHERE r.closeout_ref = ?
           AND w.owner_user_id = ?
           AND w.archived_at IS NULL
         LIMIT 1`,
      )
      .bind(input.closeoutRef, input.ownerUserId)
      .first<Record<string, unknown>>()

    if (typeof row?.receipt_json !== 'string') {
      return undefined
    }

    try {
      const parsed = parseJsonUnknown(row.receipt_json)

      return validateAutopilotDecisionCloseoutReceipt(parsed)
        ? parsed
        : undefined
    } catch {
      return undefined
    }
  },
  recordBuyerPaymentProof: async input => {
    await db
      .prepare(
        `UPDATE autopilot_work_orders
         SET buyer_payment_proof_ref = ?,
             state = 'paid_ready',
             updated_at = ?
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
           AND state = 'payment_required'
           AND buyer_payment_proof_ref IS NULL`,
      )
      .bind(
        input.buyerPaymentProofRef,
        input.updatedAt,
        input.workOrderRef,
        input.ownerUserId,
      )
      .run()

    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(input.workOrderRef, input.ownerUserId)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
  readWorkOrder: async workOrderRef => {
    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(workOrderRef)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
  readWorkOrderByIdempotency: async (ownerUserId, idempotencyKeyHash) => {
    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE owner_user_id = ?
           AND idempotency_key_hash = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(ownerUserId, idempotencyKeyHash)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
  listPendingScheduledWorkOrders: async input => {
    const rows = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE scheduled_launch_json IS NOT NULL
           AND archived_at IS NULL
           AND state IN (
             'scheduled',
             'accepted_free_slice',
             'paid_ready',
             'access_required',
             'payment_required'
           )
         ORDER BY created_at ASC
         LIMIT ?`,
      )
      .bind(input.limit)
      .all<Record<string, unknown>>()

    return (rows.results ?? [])
      .map(recordFromRow)
      .filter(record => scheduledLaunchHoldsDispatch(record.scheduledLaunch))
  },
  recordScheduledLaunchTransition: async input => {
    await db
      .prepare(
        `UPDATE autopilot_work_orders
         SET scheduled_launch_json = ?,
             state = ?,
             updated_at = ?
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
           AND scheduled_launch_json IS NOT NULL`,
      )
      .bind(
        JSON.stringify(input.scheduledLaunch),
        input.state,
        input.updatedAt,
        input.workOrderRef,
        input.ownerUserId,
      )
      .run()

    const row = await db
      .prepare(
        `SELECT *
         FROM autopilot_work_orders
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL
         LIMIT 1`,
      )
      .bind(input.workOrderRef, input.ownerUserId)
      .first<Record<string, unknown>>()

    return row === null ? undefined : recordFromRow(row)
  },
})
