import { Effect, Match as M, Schema as S } from 'effect'

import {
  type AgentRegistrationStore,
  sha256Hex,
} from './agent-registration'
import {
  autopilotCodingAssignmentsForWork,
} from './autopilot-coding-assignment'
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
} from './customer-order-agent-auth'
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
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
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
  type OpenAgentsAutopilotAccessRequestKind,
  OpenAgentsAutopilotRunnerKind,
  type OpenAgentsAutopilotRunnerKind as OpenAgentsAutopilotRunnerKindType,
  OpenAgentsAutopilotWorkState,
  type OpenAgentsAutopilotWorkRequest,
  type OpenAgentsAutopilotWorkState as OpenAgentsAutopilotWorkStateType,
  decodeOpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'

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
  kind: 'l402' | 'mdk_checkout'
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
    | 'blocked'
    | 'delivered'
    | 'needs_input'
    | 'payment_required'
    | 'ready'
    | 'retry_later'
}>

export type AutopilotWorkTaskAccessState =
  | 'missing_required_access'
  | 'satisfied'

export type AutopilotWorkTaskLifecycleState =
  | 'access_required'
  | 'blocked'
  | 'delivered'
  | 'payment_required'
  | 'queued_or_running'
  | 'ready_for_assignment'

export type AutopilotWorkTaskPlacementState =
  | 'blocked'
  | 'blocked_on_access'
  | 'blocked_on_payment'
  | 'delivered'
  | 'queued_or_running'
  | 'ready_for_assignment'

export type AutopilotWorkTaskRecordProjection = Readonly<{
  acceptanceCriteriaRefs: ReadonlyArray<string>
  accessRequirements: ReadonlyArray<AutopilotWorkAccessRequirementProjection>
  accessState: AutopilotWorkTaskAccessState
  kind: OpenAgentsAutopilotWorkRequest['tasks'][number]['kind']
  lifecycleState: AutopilotWorkTaskLifecycleState
  paymentState: AutopilotWorkFundingProjection['buyerFundingState']
  placementState: AutopilotWorkTaskPlacementState
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
  state: OpenAgentsAutopilotWorkStateType
  statusUrlRef: string
  taskRefs: ReadonlyArray<string>
  updatedAt: string
  workOrderRef: string
}>

const AutopilotWorkExecutionCloseoutRecord = S.Struct({
  assignmentRefs: S.Array(S.String),
  closeoutRefs: S.Array(S.String),
  proofRefs: S.Array(S.String),
  resultRefs: S.Array(S.String),
  runnerKind: OpenAgentsAutopilotRunnerKind,
})
export type AutopilotWorkExecutionCloseoutRecord =
  typeof AutopilotWorkExecutionCloseoutRecord.Type

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
  idempotent: boolean
  nextAction: AutopilotWorkNextActionProjection
  paymentChallenge: AutopilotWorkPaymentChallengeProjection | null
  paymentChallengeRef: string | null
  placementDecision: AutopilotPlacementDecisionProjection
  placementPolicy: AutopilotWorkPlacementPolicyRecordProjection
  pylonAssignmentIntents: ReadonlyArray<AutopilotPylonAssignmentIntentProjection>
  quote: AutopilotWorkQuote
  repositoryAuthorities: ReadonlyArray<AutopilotWorkRepositoryAuthorityProjection>
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
  | 'running'
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
  makeId?: () => string
  makePylonApiStore?: (env: Bindings) => AutopilotPylonApiStore
  makeStore: (env: Bindings) => AutopilotWorkStore
  nowIso?: () => string
  pylonRegistrations?: (
    env: Bindings,
  ) => Promise<ReadonlyArray<PylonApiRegistrationRecord>>
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

const routeNowIso = <Bindings>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

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
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|checkout|cookie|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?(key|repo)|provider[_-]?(account|grant|payload|token)|raw[_-]?(auth|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|tool[_-]?log|webhook)|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet[_-]?(home|material|mnemonic|path|private|secret|state)|webhook[_-]?secret)/iu

const safeBuyerPaymentProofRef = (value: string | null): string | undefined =>
  value !== null &&
  safePaymentProofRefPattern.test(value) &&
  !/(invoice|lnbc|lntb|lnbcrt|preimage|secret|token|wallet)/iu.test(value)
    ? value
    : undefined

const publicSafeExecutionCloseoutRef = (value: string): boolean =>
  safeExecutionCloseoutRefPattern.test(value) &&
  !unsafeExecutionCloseoutRefPattern.test(value)

const allPublicSafeExecutionCloseoutRefs = (
  refs: ReadonlyArray<string>,
): boolean => refs.length > 0 && refs.every(publicSafeExecutionCloseoutRef)

const buyerPaymentProofFromRequest = (
  request: Request,
  workRequest: OpenAgentsAutopilotWorkRequest,
): AutopilotWorkBuyerPaymentProof | undefined => {
  const quote = makeAutopilotWorkQuote(workRequest)

  if (!quote.paymentRequired) {
    return undefined
  }

  if (workRequest.paymentPolicy.buyerPaymentMode === 'l402') {
    const parsed = (() => {
      try {
        return parseOpenAgentsPaymentHeaders(request.headers)
      } catch {
        return undefined
      }
    })()
    const proofRef = safeBuyerPaymentProofRef(parsed?.proofRef ?? null)

    return proofRef === undefined
      ? undefined
      : { proofRef, source: 'l402' }
  }

  if (workRequest.paymentPolicy.buyerPaymentMode === 'mdk_checkout') {
    const proofRef = safeBuyerPaymentProofRef(
      request.headers.get('x-openagents-mdk-checkout-proof'),
    )

    return proofRef === undefined
      ? undefined
      : { proofRef, source: 'mdk_checkout' }
  }

  return undefined
}

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
    kind,
    l402HeaderRef,
    quoteRef: quote.quoteRef,
    status: record.buyerPaymentProofRef === null
      ? 'payment_required'
      : 'paid_ready',
  }
}

const paymentRequiredResponse = (
  record: AutopilotWorkOrderRecord,
  projection: AutopilotWorkOrderProjection,
): HttpResponse => {
  const headers = new Headers()
  const challenge = paymentChallengeForRecord(record)

  if (
    challenge !== null &&
    record.request.paymentPolicy.buyerPaymentMode === 'l402'
  ) {
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
        expiresAt: record.updatedAt,
        productId: 'product.autopilot.work',
      }),
    )
  }

  return noStoreJsonResponse(
    {
      error: 'payment_required',
      work: projection,
    },
    { headers, status: 402 },
  )
}

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

  switch (record.state) {
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
    case 'access_required':
    case 'payment_required':
      return 'ready_for_assignment'
  }
}

const placementStateForLifecycle = (
  lifecycleState: AutopilotWorkTaskLifecycleState,
): AutopilotWorkTaskPlacementState => {
  switch (lifecycleState) {
    case 'access_required':
      return 'blocked_on_access'
    case 'payment_required':
      return 'blocked_on_payment'
    case 'blocked':
      return 'blocked'
    case 'delivered':
      return 'delivered'
    case 'queued_or_running':
      return 'queued_or_running'
    case 'ready_for_assignment':
      return 'ready_for_assignment'
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
      kind: task.kind,
      lifecycleState,
      paymentState: funding.buyerFundingState,
      placementState: placementStateForLifecycle(lifecycleState),
      repository: task.repository ?? null,
      taskRef: task.taskRef,
    }
  })
}

const nextActionForRecord = (
  record: AutopilotWorkOrderRecord,
  funding: AutopilotWorkFundingProjection,
  placementDecision: AutopilotPlacementDecisionProjection,
): AutopilotWorkNextActionProjection => {
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
): OpenAgentsAutopilotWorkStateType => {
  if (accessRequirementsForRequest(request).length > 0) {
    return 'access_required'
  }

  if (paymentChallengeRefForRequest(request) !== null) {
    return 'payment_required'
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
    idempotent,
    nextAction: nextActionForRecord(record, funding, placementDecision),
    paymentChallenge: paymentChallengeForRecord(record),
    paymentChallengeRef: record.paymentChallengeRef,
    placementDecision,
    placementPolicy: placementPolicyForRecord(record),
    pylonAssignmentIntents: [],
    quote: makeAutopilotWorkQuote(record.request),
    repositoryAuthorities: repositoryAuthoritiesForRequest(record.request),
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
    allPublicSafeExecutionCloseoutRefs(executionCloseout.assignmentRefs) &&
    allPublicSafeExecutionCloseoutRefs(executionCloseout.closeoutRefs) &&
    allPublicSafeExecutionCloseoutRefs(executionCloseout.proofRefs) &&
    allPublicSafeExecutionCloseoutRefs(executionCloseout.resultRefs)
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
      input.record.state === 'access_required'
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
      input.record.state === 'queued_or_running'
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

const terminalEventKindForState = (
  state: OpenAgentsAutopilotWorkStateType,
): AutopilotWorkEventKind | undefined => {
  switch (state) {
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
    state: stateForRequest(input.request),
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
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )
    const auth = yield* authenticateCustomerOrderAgentRequest(
      request,
      dependencies.agentStore(env),
      {
        nowIso: () => nowIso,
        requiredScope: 'customer_orders.write',
      },
    )
    const idempotencyKeyHash = yield* requireIdempotencyHash(request)
    const existing = yield* Effect.promise(() =>
      dependencies
        .makeStore(env)
        .readWorkOrderByIdempotency(auth.ownerUserId, idempotencyKeyHash)
    )

    if (existing !== undefined) {
      const proof = buyerPaymentProofFromRequest(request, existing.request)
      const paid = proof === undefined
        ? existing
        : yield* Effect.tryPromise({
            catch: error =>
              new AutopilotWorkStoreError({
                kind: 'storage_error',
                reason: error instanceof Error ? error.message : String(error),
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
        ? paymentRequiredResponse(dispatchedRecord, projection)
        : noStoreJsonResponse({ work: projection }, { status: 200 })
    }

    const workRequest = yield* decodeWorkRequest(request)
    const proof = buyerPaymentProofFromRequest(request, workRequest)
    const record = buildWorkOrderRecord({
      agentCredentialId: auth.agent.credential.id,
      agentUserId: auth.agent.user.id,
      id: routeMakeId(dependencies),
      idempotencyKeyHash,
      nowIso,
      ownerUserId: auth.ownerUserId,
      request: workRequest,
    })
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
      ? paymentRequiredResponse(dispatchedRecord, projection)
      : noStoreJsonResponse(
          { work: projection },
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
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const pylonRegistrations = yield* routePylonRegistrations(
      dependencies,
      env,
    )
    const auth = yield* authenticateCustomerOrderAgentRequest(
      request,
      dependencies.agentStore(env),
      {
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
      work: projectionForRecord(record, false, nowIso, pylonRegistrations),
    })
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
  workOrderRef: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const auth = yield* authenticateCustomerOrderAgentRequest(
      request,
      dependencies.agentStore(env),
      {
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

const workOrderEventsRefFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/autopilot\/work\/([^/]+)\/events$/.exec(pathname)

  return match?.[1]
}

export const makeAutopilotWorkRoutes = <
  Bindings extends AutopilotWorkRouteEnv,
>(
  dependencies: AutopilotWorkRoutesDependencies<Bindings>,
) => ({
  routeAutopilotWorkRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/autopilot/work') {
      return M.value(request.method).pipe(
        M.when('POST', () => createWorkOrder(dependencies, request, env)),
        M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
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
            workOrderEventsRef,
          )
        ),
        M.orElse(() => Effect.succeed(methodNotAllowed(['GET']))),
      )
    }

    const workOrderRef = workOrderRefFromPath(url.pathname)

    if (workOrderRef !== undefined) {
      return M.value(request.method).pipe(
        M.when('GET', () =>
          readWorkOrder(dependencies, request, env, workOrderRef)
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
          status_url_ref,
          event_stream_ref,
          created_at,
          updated_at,
          archived_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
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
        record.statusUrlRef,
        record.eventStreamRef,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    return { idempotent: false, record }
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
           AND state NOT IN ('access_required', 'payment_required', 'delivered')`,
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
  recordBuyerPaymentProof: async input => {
    await db
      .prepare(
        `UPDATE autopilot_work_orders
         SET buyer_payment_proof_ref = ?,
             state = 'paid_ready',
             updated_at = ?
         WHERE work_order_ref = ?
           AND owner_user_id = ?
           AND archived_at IS NULL`,
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
})
