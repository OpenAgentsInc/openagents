import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
} from './agent-registration'
import { createHostedGeminiExecutorBinding } from './autopilot-hosted-gemini-binding'
import { makeHostedGeminiExecuteReadyWork } from './autopilot-hosted-gemini-executor-env'
import {
  createHostedGeminiWorkExecutor,
  type HostedGeminiInferenceCaller,
} from './autopilot-hosted-gemini-executor'
import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
} from './autopilot-work-request'
import {
  type OpenAgentsL402CredentialPayload,
  makeOpenAgentsL402HmacSigningBoundary,
} from './l402-credential-service'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
} from './inference/provider-adapter'
import { formatOpenAgentsPaymentCredentialPair } from './l402-payment-headers'
import {
  type AutopilotWorkExecutor,
  type AutopilotWorkExecutionCloseoutRecord,
  type AutopilotWorkL402PaymentVerificationInput,
  type AutopilotWorkL402PaymentVerificationResult,
  type AutopilotWorkOrderProjection,
  type AutopilotWorkOrderRecord,
  type AutopilotWorkReviewDecisionRecord,
  type AutopilotWorkStore,
  AutopilotWorkStoreError,
  dispatchDueScheduledAutopilotWork,
  makeAutopilotWorkRoutes,
  recordAutopilotWorkerCloseoutFromPylon,
  verifyAutopilotL402PaymentProofFromBuyerLedger,
} from './autopilot-work-routes'
import type {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerStore,
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
  BuyerPaymentRedemptionRecord,
  BuyerPaymentSpendLimitRecord,
  BuyerPaymentCreditDebitRecord,
} from './buyer-payment-ledger'
import type {
  PylonApiAssignmentRecord,
  PylonApiEventRecord,
  PylonApiProviderJobLifecycleRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'
import {
  PylonApiStoreError,
  providerJobLifecycleRecordFromAssignment,
} from './pylon-api'
import { makePylonApiRoutes } from './pylon-api-routes'

class MemoryAutopilotWorkStore implements AutopilotWorkStore {
  readonly records = new Map<string, AutopilotWorkOrderRecord>()
  readonly recordsByOwnerIdempotency = new Map<string, AutopilotWorkOrderRecord>()

  createWorkOrder = async (record: AutopilotWorkOrderRecord) => {
    const key = `${record.ownerUserId}:${record.idempotencyKeyHash}`
    const existing = this.recordsByOwnerIdempotency.get(key)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.records.set(record.workOrderRef, record)
    this.recordsByOwnerIdempotency.set(key, record)

    return { idempotent: false, record }
  }

  readWorkOrder = async (workOrderRef: string) =>
    this.records.get(workOrderRef)

  listWorkOrdersForOwner = async (
    input: Readonly<{ limit: number; ownerUserId: string }>,
  ) =>
    [...this.records.values()]
      .filter(record => record.ownerUserId === input.ownerUserId)
      .slice(0, input.limit)

  recordPylonAssignmentDispatch = async (input: Readonly<{
    ownerUserId: string
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    const updated = {
      ...existing,
      state: 'queued_or_running' as const,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return updated
  }

  recordExecutionCloseout = async (input: Readonly<{
    executionCloseout: AutopilotWorkExecutionCloseoutRecord
    ownerUserId: string
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    const updated = {
      ...existing,
      executionCloseout: input.executionCloseout,
      state: 'delivered' as const,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return updated
  }

  recordReviewDecision = async (input: Readonly<{
    ownerUserId: string
    reviewDecision: AutopilotWorkReviewDecisionRecord
    state: 'accepted' | 'rejected' | 'revision_required'
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    if (existing.reviewDecision !== null) {
      if (
        existing.reviewDecision.idempotencyKeyHash ===
        input.reviewDecision.idempotencyKeyHash
      ) {
        return { idempotent: true, record: existing }
      }

      throw new AutopilotWorkStoreError({
        kind: 'conflict',
        reason:
          'Autopilot work already has a review decision with a different idempotency key.',
      })
    }

    if (existing.state !== 'delivered') {
      throw new AutopilotWorkStoreError({
        kind: 'conflict',
        reason: 'Autopilot work must be delivered before review.',
      })
    }

    const updated = {
      ...existing,
      reviewDecision: input.reviewDecision,
      state: input.state,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return { idempotent: false, record: updated }
  }

  recordBuyerPaymentProof = async (input: Readonly<{
    buyerPaymentProofRef: string
    ownerUserId: string
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    if (
      existing.state !== 'payment_required' ||
      existing.buyerPaymentProofRef !== null
    ) {
      return existing
    }

    const updated = {
      ...existing,
      buyerPaymentProofRef: input.buyerPaymentProofRef,
      state: 'paid_ready' as const,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return updated
  }

  readWorkOrderByIdempotency = async (
    ownerUserId: string,
    idempotencyKeyHash: string,
  ) => this.recordsByOwnerIdempotency.get(`${ownerUserId}:${idempotencyKeyHash}`)

  listPendingScheduledWorkOrders = async (
    input: Readonly<{ limit: number }>,
  ) =>
    [...this.records.values()]
      .filter(record =>
        record.scheduledLaunch !== null &&
        record.scheduledLaunch.dispatchedAt === null &&
        record.scheduledLaunch.expiredAt === null
      )
      .slice(0, input.limit)

  recordScheduledLaunchTransition = async (input: Readonly<{
    ownerUserId: string
    scheduledLaunch: NonNullable<AutopilotWorkOrderRecord['scheduledLaunch']>
    state: AutopilotWorkOrderRecord['state']
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (
      existing === undefined ||
      existing.ownerUserId !== input.ownerUserId ||
      existing.scheduledLaunch === null
    ) {
      return undefined
    }

    const updated = {
      ...existing,
      scheduledLaunch: input.scheduledLaunch,
      state: input.state,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return updated
  }
}

class MemoryPylonApiStore implements PylonApiStore {
  readonly assignments = new Map<string, PylonApiAssignmentRecord>()
  readonly assignmentsByIdempotency = new Map<string, PylonApiAssignmentRecord>()
  readonly events = new Map<string, PylonApiEventRecord>()
  readonly eventsByIdempotency = new Map<string, PylonApiEventRecord>()
  readonly providerJobLifecycle = new Map<
    string,
    PylonApiProviderJobLifecycleRecord
  >()
  readonly registrations = new Map<string, PylonApiRegistrationRecord>()

  constructor(registrations: ReadonlyArray<PylonApiRegistrationRecord>) {
    registrations.forEach(registration => {
      this.registrations.set(registration.pylonRef, registration)
    })
  }

  createAssignment = async (record: PylonApiAssignmentRecord) => {
    const existing = this.assignmentsByIdempotency.get(
      record.idempotencyKeyHash,
    )

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.assignments.set(record.assignmentRef, record)
    this.assignmentsByIdempotency.set(record.idempotencyKeyHash, record)
    this.providerJobLifecycle.set(
      record.assignmentRef,
      providerJobLifecycleRecordFromAssignment(record),
    )

    return { idempotent: false, record }
  }

  createEvent = async (record: PylonApiEventRecord) => {
    const existing = this.eventsByIdempotency.get(record.idempotencyKeyHash)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.events.set(record.eventRef, record)
    this.eventsByIdempotency.set(record.idempotencyKeyHash, record)

    return { idempotent: false, record }
  }

  listAssignmentsForPylon = async (pylonRef: string, limit: number) =>
    Array.from(this.assignments.values())
      .filter(assignment => assignment.pylonRef === pylonRef)
      .slice(0, limit)

  listEventsForPylon = async (pylonRef: string, limit: number) =>
    Array.from(this.events.values())
      .filter(event => event.pylonRef === pylonRef)
      .slice(0, limit)

  listEventsForAssignment = async (assignmentRef: string, limit: number) =>
    Array.from(this.events.values())
      .filter(event => event.assignmentRef === assignmentRef)
      .slice(0, limit)

  listRegistrations = async (limit: number) =>
    Array.from(this.registrations.values()).slice(0, limit)

  listProviderJobLifecycleForPylons = async (
    pylonRefs: ReadonlyArray<string>,
    limit: number,
  ) =>
    Array.from(this.providerJobLifecycle.values())
      .filter(record => pylonRefs.includes(record.pylonRef))
      .slice(0, limit)

  readAssignment = async (assignmentRef: string) =>
    this.assignments.get(assignmentRef)

  readAssignmentByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.assignmentsByIdempotency.get(idempotencyKeyHash)

  readEventByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.eventsByIdempotency.get(idempotencyKeyHash)

  readRegistration = async (pylonRef: string) =>
    this.registrations.get(pylonRef)

  updateAssignment = async (record: PylonApiAssignmentRecord) => {
    this.assignments.set(record.assignmentRef, record)
    this.assignmentsByIdempotency.set(record.idempotencyKeyHash, record)
    this.providerJobLifecycle.set(
      record.assignmentRef,
      providerJobLifecycleRecordFromAssignment(record),
    )

    return record
  }

  updateAssignmentIfState = async (
    record: PylonApiAssignmentRecord,
    expectedState: PylonApiAssignmentRecord['state'],
  ) => {
    const current = this.assignments.get(record.assignmentRef)

    if (current === undefined || current.state !== expectedState) {
      return undefined
    }

    return this.updateAssignment(record)
  }

  upsertProviderJobLifecycle = async (
    record: PylonApiProviderJobLifecycleRecord,
  ) => {
    this.providerJobLifecycle.set(record.assignmentRef, record)

    return record
  }

  upsertRegistration = async (record: PylonApiRegistrationRecord) => {
    const existing = this.registrations.get(record.pylonRef)

    if (
      existing !== undefined &&
      existing.ownerAgentUserId !== record.ownerAgentUserId
    ) {
      throw new PylonApiStoreError({
        kind: 'conflict',
        reason: 'Pylon ref is already owned by another registered agent.',
      })
    }

    const next =
      existing === undefined
        ? record
        : {
            ...record,
            createdAt: existing.createdAt,
            id: existing.id,
          }

    this.registrations.set(record.pylonRef, next)

    return next
  }
}

class MemoryBuyerPaymentLedgerStore implements BuyerPaymentLedgerStore {
  readonly challenges = new Map<string, BuyerPaymentChallengeRecord>()
  readonly challengesByIdempotency = new Map<string, BuyerPaymentChallengeRecord>()
  readonly creditDebits = new Map<string, BuyerPaymentCreditDebitRecord>()
  readonly entitlements = new Map<string, BuyerPaymentEntitlementRecord>()
  readonly receipts = new Map<string, BuyerPaymentReceiptRecord>()
  readonly reconciliations = new Map<string, BuyerPaymentReconciliationEventRecord>()
  readonly redemptions = new Map<string, BuyerPaymentRedemptionRecord>()
  readonly spendLimits = new Map<string, BuyerPaymentSpendLimitRecord>()

  createChallenge = async (record: BuyerPaymentChallengeRecord) => {
    this.challenges.set(record.challengeRef, record)
    this.challengesByIdempotency.set(record.idempotencyKeyHash, record)
  }

  createCreditDebit = async (record: BuyerPaymentCreditDebitRecord) => {
    this.creditDebits.set(record.debitRef, record)
  }

  createReceiptEntitlementBundle = async (input: {
    entitlement: BuyerPaymentEntitlementRecord
    receipt: BuyerPaymentReceiptRecord
  }) => {
    this.entitlements.set(input.entitlement.entitlementRef, input.entitlement)
    this.receipts.set(input.receipt.receiptRef, input.receipt)
  }

  createReconciliationEvent = async (
    record: BuyerPaymentReconciliationEventRecord,
  ) => {
    this.reconciliations.set(record.receiptRef ?? record.eventRef, record)
  }

  createRedemptionBundle = async (input: {
    entitlement: BuyerPaymentEntitlementRecord
    receipt: BuyerPaymentReceiptRecord
    redemption: BuyerPaymentRedemptionRecord
  }) => {
    await this.createReceiptEntitlementBundle(input)
    this.redemptions.set(input.redemption.challengeRef, input.redemption)
  }

  createSpendLimit = async (record: BuyerPaymentSpendLimitRecord) => {
    this.spendLimits.set(record.spendLimitRef, record)
  }

  readChallengeByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.challengesByIdempotency.get(idempotencyKeyHash)

  readEntitlementByRef = async (entitlementRef: string) =>
    this.entitlements.get(entitlementRef)

  readReceiptByRef = async (receiptRef: string) => this.receipts.get(receiptRef)

  readReconciliationEventByReceiptRef = async (receiptRef: string) =>
    this.reconciliations.get(receiptRef)

  readReconciliationEventByProviderEvent = async (
    providerRef: string,
    externalEventRef: string,
  ) =>
    [...this.reconciliations.values()].find(
      event =>
        event.providerRef === providerRef &&
        event.externalEventRef === externalEventRef,
    )

  readRedemptionByChallengeRef = async (challengeRef: string) =>
    this.redemptions.get(challengeRef)
}

const agentToken = `${AGENT_TOKEN_PREFIX}autopilot-work-test`
const autopilotL402SigningSecret = 'autopilot-work-route-test-l402-secret'

const autopilotL402SigningBoundary = () =>
  makeOpenAgentsL402HmacSigningBoundary({
    secretKeyMaterial: autopilotL402SigningSecret,
    signerRef: 'binding.autopilot.route.mdk.sandbox',
  })

const verifiedAutopilotProofRefs = new Set<string>()

const verifyAutopilotL402PaymentProof = async (
  input: AutopilotWorkL402PaymentVerificationInput,
): Promise<AutopilotWorkL402PaymentVerificationResult | null> =>
  verifiedAutopilotProofRefs.has(input.paymentProofRef)
    ? {
        paymentProofRef: input.paymentProofRef,
        verifierRef: 'verifier.autopilot_l402.route_test',
      }
    : null

const authorizeAutopilotL402 = (
  credential: string | null,
  proofRef: string,
): string => {
  if (credential === null) {
    throw new Error('Autopilot L402 credential header was not minted.')
  }

  return formatOpenAgentsPaymentCredentialPair({ credential, proofRef })
}

const decodeAutopilotL402Payload = (
  credential: string,
): OpenAgentsL402CredentialPayload => {
  const [, payloadBase64Url] = credential.split('.')

  if (payloadBase64Url === undefined) {
    throw new Error('Autopilot L402 credential payload is missing.')
  }

  const normalized = payloadBase64Url.replaceAll('-', '+').replaceAll('_', '/')
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=')

  return JSON.parse(atob(padded)) as OpenAgentsL402CredentialPayload
}

const recordMatchedAutopilotLedgerPayment = async (
  ledger: MemoryBuyerPaymentLedgerStore,
  input: Readonly<{
    credential: string | null
    proofRef: string
  }>,
) => {
  const credentialPayload = decodeAutopilotL402Payload(input.credential ?? '')
  const challenge = ledger.challenges.get(credentialPayload.challengeRef)

  if (challenge === undefined) {
    throw new Error('Autopilot L402 challenge was not persisted.')
  }

  const refSuffix = input.proofRef.replace(/[^A-Za-z0-9_]+/g, '_')
  const receiptRef = `receipt.autopilot_work.${refSuffix}`
  const entitlementRef = `entitlement.autopilot_work.${refSuffix}`

  await ledger.createRedemptionBundle({
    entitlement: {
      actorRef: challenge.actorRef,
      archivedAt: null,
      challengeRef: challenge.challengeRef,
      consumedAt: null,
      createdAt: '2026-06-09T17:31:00.000Z',
      entitlementRef,
      expiresAt: '2026-06-10T17:31:00.000Z',
      id: `buyer_payment_entitlement_autopilot_work_${refSuffix}`,
      ownerUserId: challenge.ownerUserId,
      productId: challenge.productId,
      receiptRef,
      scopeRefs: credentialPayload.entitlementScopeRefs,
      status: 'active',
      surface: challenge.surface,
    },
    receipt: {
      actorRef: challenge.actorRef,
      amount: challenge.price,
      archivedAt: null,
      challengeRef: challenge.challengeRef,
      createdAt: '2026-06-09T17:31:00.000Z',
      entitlementRef,
      id: `buyer_payment_receipt_autopilot_work_${refSuffix}`,
      metadataRefs: [`metadata.autopilot_work.${refSuffix}`],
      ownerUserId: challenge.ownerUserId,
      productId: challenge.productId,
      publicProjectionJson: '{}',
      receiptRef,
      redactedPaymentRef: input.proofRef,
      status: 'issued',
      surface: challenge.surface,
    },
    redemption: {
      actorRef: challenge.actorRef,
      archivedAt: null,
      challengeRef: challenge.challengeRef,
      createdAt: '2026-06-09T17:31:00.000Z',
      entitlementRef,
      id: `buyer_payment_redemption_autopilot_work_${refSuffix}`,
      idempotencyKeyHash: `hash.autopilot_work.${refSuffix}`,
      metadataRefs: [`metadata.autopilot_work.${refSuffix}`],
      proofRef: input.proofRef,
      receiptRef,
      redemptionRef: `redemption.autopilot_work.${refSuffix}`,
      replayed: 0,
      status: 'redeemed',
    },
  })
  await ledger.createReconciliationEvent({
    archivedAt: null,
    challengeRef: challenge.challengeRef,
    createdAt: '2026-06-09T17:31:01.000Z',
    eventRef: `reconciliation.autopilot_work.${refSuffix}`,
    externalEventRef: `external_event.mdk.autopilot_work.${refSuffix}`,
    id: `buyer_payment_reconciliation_autopilot_work_${refSuffix}`,
    idempotencyKeyHash: `hash.reconciliation.autopilot_work.${refSuffix}`,
    metadataRefs: [`metadata.autopilot_work.${refSuffix}`],
    productId: challenge.productId,
    providerRef: 'provider.mdk.hosted',
    publicProjectionJson: '{}',
    receiptRef,
    resultRef: 'result.reconciliation.matched',
    status: 'matched',
  })

  return { challenge, credentialPayload, entitlementRef, receiptRef }
}

const agentStoreForScopes = (
  scopes: ReadonlyArray<string> = [
    'customer_orders.read',
    'customer_orders.write',
  ],
  ownerUserId = 'github:autopilot-owner',
): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve({
      credentialId: 'agent_credential_autopilot_work_test',
      profileMetadataJson: JSON.stringify({
        customerOrderGrants: [
          {
            expiresAt: null,
            ownerUserId,
            scopes,
            status: 'active',
          },
        ],
      }),
      tokenPrefix: `${AGENT_TOKEN_PREFIX}autopilot`,
      user: {
        avatarUrl: null,
        createdAt: '2026-06-09T17:30:00.000Z',
        displayName: 'Autopilot Work Agent',
        id: 'agent_user_autopilot_work',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: '2026-06-09T17:30:00.000Z',
      },
    }),
  touchAgentCredential: () => Promise.resolve(),
  updateAgentDisplayName: () => Promise.resolve(0),
})

const pylonRegistration = (
  override: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: [
    'capability.pylon.assignment_ready',
    'capability.pylon.local_claude_agent',
  ],
  clientProtocolVersion: '0.2.5',
  clientVersion: '0.2.5',
  createdAt: '2026-06-09T17:25:00.000Z',
  displayName: 'Requester Pylon',
  id: 'pylon_registration_1',
  latestCapacityRefs: ['capacity.pylon.assignment_ready'],
  latestHeartbeatAt: '2026-06-09T17:29:30.000Z',
  latestHeartbeatStatus: 'ready',
  latestHealthRefs: ['health.pylon.ready'],
  latestLoadRefs: ['load.pylon.available'],
  latestResourceMode: 'balanced',
  ownerAgentCredentialId: 'agent_credential_autopilot_work_test',
  ownerAgentTokenPrefix: 'oa_agent',
  ownerAgentUserId: 'agent_user_autopilot_work',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef: 'pylon.local.docs_agent',
  resourceMode: 'balanced',
  status: 'active',
  updatedAt: '2026-06-09T17:29:30.000Z',
  walletReady: true,
  walletRef: 'wallet_ref.pylon.local.docs_agent',
  ...override,
})

const route = async (
  store: MemoryAutopilotWorkStore,
  path: string,
  options: Readonly<{
    body?: unknown
    buyerPaymentLedgerStore?: BuyerPaymentLedgerStore
    headers?: HeadersInit
    idempotencyKey?: string
    executeReadyWork?: AutopilotWorkExecutor
    method?: string
    nowIso?: string
    pylonApiStore?: PylonApiStore
    pylonRegistrations?: ReadonlyArray<PylonApiRegistrationRecord>
    pylonStoreRegistrations?: ReadonlyArray<PylonApiRegistrationRecord>
    ownerUserId?: string
    scopes?: ReadonlyArray<string>
    sessionUserId?: string
    token?: string
    verifyL402PaymentProof?: (
      input: AutopilotWorkL402PaymentVerificationInput,
    ) => Promise<AutopilotWorkL402PaymentVerificationResult | null>
  }> = {},
) => {
  let counter = 0
  const maybePylonApiStore = options.pylonApiStore
  const dependencies = {
    agentStore: () => agentStoreForScopes(options.scopes, options.ownerUserId),
    makeId: () => `autopilot_work_order.test_${++counter}`,
    makeStore: () => store,
    nowIso: () => options.nowIso ?? '2026-06-09T17:30:00.000Z',
    l402SigningBoundary: () => autopilotL402SigningBoundary(),
    ...(options.buyerPaymentLedgerStore === undefined
      ? {}
      : {
          makeBuyerPaymentLedgerStore: () =>
            options.buyerPaymentLedgerStore as BuyerPaymentLedgerStore,
        }),
    verifyL402PaymentProof: (
      _env: Record<string, unknown>,
      input: AutopilotWorkL402PaymentVerificationInput,
    ) =>
      options.verifyL402PaymentProof?.(input) ??
      verifyAutopilotL402PaymentProof(input),
    ...(options.sessionUserId === undefined
      ? {}
      : {
          requireBrowserSession: () =>
            Promise.resolve({
              user: { userId: options.sessionUserId ?? 'github:browser-user' },
            }),
        }),
    ...(options.executeReadyWork === undefined
      ? {}
      : {
          executeReadyWork: (
            _env: Record<string, unknown>,
            input: Parameters<AutopilotWorkExecutor>[0],
          ) => options.executeReadyWork?.(input) ?? Promise.resolve(undefined),
        }),
    ...(options.pylonRegistrations === undefined
      ? {}
      : {
          pylonRegistrations: () =>
            Promise.resolve(options.pylonRegistrations ?? []),
        }),
    ...(options.pylonStoreRegistrations === undefined
      ? {}
      : {
          makePylonApiStore: () => ({
            listRegistrations: () =>
              Promise.resolve(options.pylonStoreRegistrations ?? []),
          }),
        }),
    ...(maybePylonApiStore === undefined
      ? {}
      : { makePylonApiStore: () => maybePylonApiStore }),
  }
  const routes = makeAutopilotWorkRoutes<Record<string, unknown>>(
    dependencies,
  )
  const body = options.body === undefined
    ? {}
    : { body: JSON.stringify(options.body) }
  const request = new Request(`https://openagents.com${path}`, {
    ...body,
    headers: {
      ...options.headers,
      ...(options.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(options.idempotencyKey === undefined
        ? {}
        : { 'Idempotency-Key': options.idempotencyKey }),
      ...(options.token === undefined
        ? { authorization: `Bearer ${agentToken}` }
        : options.token === ''
          ? {}
          : { authorization: `Bearer ${options.token}` }),
    },
    method: options.method ?? (options.body === undefined ? 'GET' : 'POST'),
  })
  const response = routes.routeAutopilotWorkRequest(
    request,
    {},
    {} as ExecutionContext,
  )

  if (response === undefined) {
    throw new Error(`No Autopilot work route matched ${path}`)
  }

  return Effect.runPromise(response)
}

const pylonRoute = async (
  store: PylonApiStore,
  path: string,
  options: Readonly<{
    body?: unknown
    headers?: HeadersInit
    idempotencyKey?: string
    method?: string
    recordAutopilotWorkerCloseout?: Parameters<
      typeof makePylonApiRoutes<Record<string, unknown>>
    >[0]['recordAutopilotWorkerCloseout']
    token?: string
  }> = {},
) => {
  const routes = makePylonApiRoutes<Record<string, unknown>>({
    agentStore: () => agentStoreForScopes(),
    makeStore: () => store,
    nowIso: () => '2026-06-09T17:30:30.000Z',
    ...(options.recordAutopilotWorkerCloseout === undefined
      ? {}
      : {
          recordAutopilotWorkerCloseout:
            options.recordAutopilotWorkerCloseout,
        }),
  })
  const body = options.body === undefined
    ? {}
    : { body: JSON.stringify(options.body) }
  const request = new Request(`https://openagents.com${path}`, {
    ...body,
    headers: {
      ...options.headers,
      ...(options.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(options.idempotencyKey === undefined
        ? {}
        : { 'Idempotency-Key': options.idempotencyKey }),
      ...(options.token === undefined
        ? { authorization: `Bearer ${agentToken}` }
        : options.token === ''
          ? {}
          : { authorization: `Bearer ${options.token}` }),
    },
    method: options.method ?? (options.body === undefined ? 'GET' : 'POST'),
  })
  const response = routes.routePylonApiRequest(request, {}, {} as ExecutionContext)

  if (response === undefined) {
    throw new Error(`No Pylon API route matched ${path}`)
  }

  return Effect.runPromise(response)
}

const createDeliveredPylonBackedWork = async () => {
  const store = new MemoryAutopilotWorkStore()
  const pylonApiStore = new MemoryPylonApiStore([
    pylonRegistration({
      pylonRef: 'pylon.production.docs_agent',
    }),
  ])
  const assignmentRef =
    'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract'

  await route(store, '/api/autopilot/work', {
    body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
    idempotencyKey: `idem-${assignmentRef}`,
    pylonApiStore,
  })
  await pylonRoute(
    pylonApiStore,
    `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/accept`,
    {
      body: {
        acceptanceRefs: ['acceptance.public.autopilot_pylon.accepted'],
        accepted: true,
      },
      idempotencyKey: `accept-${assignmentRef}`,
      method: 'POST',
    },
  )
  await pylonRoute(
    pylonApiStore,
    `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/closeout`,
    {
      body: {
        artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
        buildRefs: ['build.public.autopilot_docs.not_required'],
        closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
        previewRefs: ['preview.public.autopilot_docs.not_required'],
        proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
        resultRefs: ['result.public.autopilot_docs.delivered'],
        status: 'closeout_submitted',
        summaryRefs: ['summary.public.autopilot_docs.customer_safe'],
        testRefs: ['test.public.autopilot_docs.not_required'],
      },
      idempotencyKey: `closeout-${assignmentRef}`,
      method: 'POST',
      recordAutopilotWorkerCloseout: (_env, input) =>
        recordAutopilotWorkerCloseoutFromPylon(store, input),
    },
  )

  return { assignmentRef, pylonApiStore, store }
}

const responseJson = async (response: Response) =>
  response.json() as Promise<Readonly<{
    briefing?: Readonly<{
      briefingRef: string
      drilldown: ReadonlyArray<Readonly<{ kind: string; refs: ReadonlyArray<string> }>>
      receipts: Readonly<{
        authorityReceiptRefs: ReadonlyArray<string>
        buyerPaymentProofRef: string | null
        proofRefs: ReadonlyArray<string>
        settlementEligible: boolean
        verificationRefs: ReadonlyArray<string>
      }>
      risk: Readonly<{
        blockerCount: number
        changeCaptureStatus: string | null
        deliveryReadinessStatus: string | null
        level: string
        reviewCaveatRefs: ReadonlyArray<string>
        settlementBlockedReasonRef: string
        worktreeIdentityStatus: string | null
      }>
      whatHappened: ReadonlyArray<Readonly<{ eventKind: string; sequence: number }>>
    }>
    error?: string
    events?: ReadonlyArray<Readonly<{
      eventKind: string
      publicSafe: boolean
      sequence: number
      taskRefs: ReadonlyArray<string>
      workOrderRef: string
    }>>
    idempotent?: boolean
    nextAfter?: number
    work?: Readonly<{
      accessRequirements?: ReadonlyArray<Readonly<{
        accessRequestRef: string
        grantAction: string
        kind: string
        ownerActionRef: string
        reasonRef: string
        requiredBeforeLaunch: boolean
        status: string
        taskRef: string
      }>>
      assignmentIntents?: ReadonlyArray<Readonly<{
        accessState: string
        assignmentIntentRef: string
        assignmentKind: string
        deployAuthority: boolean
        paymentState: string
        placementState: string
        plannerReasonRefs: ReadonlyArray<string>
        plannerState: string
        readyForAssignment: boolean
        spendAuthority: boolean
        taskRef: string
        workerPayoutEligible: boolean
        workOrderRef: string
      }>>
      buyerPaymentProofRef?: string | null
      accessRequestRefs?: ReadonlyArray<string>
      executionCloseout?: Readonly<{
        acceptedWorkAuthority: boolean
        artifactRefs?: ReadonlyArray<string>
        assignmentRefs: ReadonlyArray<string>
        blockerRefs?: ReadonlyArray<string>
        buildRefs?: ReadonlyArray<string>
        closeoutRefs: ReadonlyArray<string>
        forumAutoPublishAllowed: boolean
        previewRefs?: ReadonlyArray<string>
        proofRefs: ReadonlyArray<string>
        publicSafe: boolean
        resultRefs: ReadonlyArray<string>
        runnerKind: string
        summaryRefs?: ReadonlyArray<string>
        testRefs?: ReadonlyArray<string>
        workerPayoutAuthority: boolean
      }> | null
      fallbackLeaseIntents?: ReadonlyArray<Readonly<{
        assignmentRef: string
        fallbackLaneRef: string
        forumAutoPublishAllowed: boolean
        paymentMode: string
        requiredCapabilityRefs: ReadonlyArray<string>
        runnerKind: string
        spendCapRefs: ReadonlyArray<string>
        taskRef: string
        workerPayoutAuthority: boolean
      }>>
      funding?: Readonly<{
        buyerFundingState: string
        buyerPaymentProofRef: string | null
        fundedAmountCents: number
        quoteRef: string
        settlementBlockedReasonRef: string
        settlementEligible: boolean
        workerPayoutEligible: boolean
      }>
      idempotent: boolean
      nextAction?: Readonly<{
        callerActionRefs: ReadonlyArray<string>
        reasonRefs: ReadonlyArray<string>
        retryAfterSeconds: number | null
        state: string
      }>
      paymentChallenge?: Readonly<{
        amountCents: number
        challengeRef: string
        kind: string
        quoteRef: string
        status: string
      }> | null
      paymentChallengeRef: string | null
      placementDecision?: Readonly<{
        availabilityState: string
        callerActionRefs: ReadonlyArray<string>
        fallbackRunnerKind: string | null
        pylonCandidates: ReadonlyArray<Readonly<{
          assignmentReady: boolean
          heartbeatFresh: boolean
          localExecutionReady: boolean
          ownerLinked: boolean
          pylonRef: string
          selected: boolean
          versionCompatible: boolean
          walletReady: boolean
        }>>
        reasonRefs: ReadonlyArray<string>
        refusalReasonRefs: ReadonlyArray<string>
        retryAfterSeconds: number | null
        selectedPylonRef: string | null
        selectedRunnerKind: string | null
        source: string
      }>
      placementPolicy?: Readonly<{
        allowedRunnerKinds: ReadonlyArray<string>
        auditable: boolean
        disallowedRunnerKinds: ReadonlyArray<string>
        localOnlyAllowed: boolean
        placementPolicyRef: string
        preferredRunnerKinds: ReadonlyArray<string>
        privacyTier: string
        promptKeywordRouting: boolean
        publicTraceAllowed: boolean
        reasonRefs: ReadonlyArray<string>
        requiresSecretBroker: boolean
      }>
      pylonAssignmentIntents?: ReadonlyArray<Readonly<{
        assignmentRef: string
        forumAutoPublishAllowed: boolean
        paymentMode: string
        pylonRef: string
        requiredCapabilityRefs: ReadonlyArray<string>
        spendCapRefs: ReadonlyArray<string>
        taskRef: string
      }>>
      quote?: Readonly<{
        amountCents: number
        paymentRequired: boolean
        quoteRef: string
      }>
      repositoryAuthorities?: ReadonlyArray<Readonly<{
        deployAuthority: boolean
        fullName: string
        pullRequestAuthority: string
        readAuthority: string
        spendAuthority: boolean
        taskRef: string
        writeAuthority: string
      }>>
      reviewDecision?: Readonly<{
        acceptedWorkAuthority: boolean
        action: string
        actorAgentCredentialId: string
        actorAgentUserId: string
        decisionRefs: ReadonlyArray<string>
        deployAuthority: boolean
        forumAutoPublishAllowed: boolean
        publicSafe: boolean
        recordedAt: string
        rejectionRefs: ReadonlyArray<string>
        revisionRequestRefs: ReadonlyArray<string>
        settlementAuthority: boolean
        workerPayoutAuthority: boolean
      }> | null
      state: string
      taskRefs: ReadonlyArray<string>
      tasks?: ReadonlyArray<Readonly<{
        acceptanceCriteriaRefs: ReadonlyArray<string>
        accessRequirements: ReadonlyArray<Readonly<{
          accessRequestRef: string
          grantAction: string
          kind: string
          ownerActionRef: string
          reasonRef: string
          requiredBeforeLaunch: boolean
          status: string
          taskRef: string
        }>>
        accessState: string
        kind: string
        lifecycleState: string
        paymentState: string
        placementState: string
        repository: Readonly<{
          branch: string
          fullName: string
          provider: string
          visibility: string
        }> | null
        taskRef: string
      }>>
      workOrderRef: string
    }>
    assignment?: Readonly<{
      assignmentRef: string
      leaseState: string
      state: string
    }>
    assignments?: ReadonlyArray<Readonly<{
      assignmentRef: string
      leaseState: string
      state: string
      taskRefs: ReadonlyArray<string>
    }>>
  }>>

describe('Autopilot work routes', () => {
  const retainedProjectionPrivateMaterialPattern =
    /\/Users\/|\/home\/|bearer|invoice|lnbc|lntb|lnbcrt|mnemonic|payment[_-]?preimage(?=[:=._/-]|$)|payout[_-]?(address|destination|target)(?=[:=._/-]|$)|provider[_-]?(payload|token)(?=[:=._/-]|$)|raw[_-]?(prompt|runner|run[_-]?log|source[_-]?archive)(?=[:=._/-]|$)|secret:\/\/|secret[_-]?(key|token|value|material)(?=[:=._/-]|$)|webhook[_-]?secret(?=[:=._/-]|$)|sk-[a-z0-9]|wallet[_-]?(material|mnemonic|secret)(?=[:=._/-]|$)|Google Cloud|\bcredits?\b/i

  test('creates and recovers the same work projection with an idempotency key', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-create',
    })
    const replay = await route(store, '/api/autopilot/work', {
      body: {
        prompt: 'This malformed replay body should not replace the record.',
      },
      idempotencyKey: 'idem-autopilot-work-create',
    })
    const firstJson = await responseJson(first)
    const replayJson = await responseJson(replay)

    expect(first.status).toBe(202)
    expect(replay.status).toBe(200)
    expect(firstJson.work).toMatchObject({
      idempotent: false,
      placementPolicy: {
        allowedRunnerKinds: ['requester_pylon', 'openagents_shc'],
        auditable: true,
        disallowedRunnerKinds: [],
        localOnlyAllowed: false,
        placementPolicyRef:
          'placement_policy.autopilot_work_order.test_1',
        preferredRunnerKinds: ['requester_pylon'],
        privacyTier: 'public_beta',
        promptKeywordRouting: false,
        publicTraceAllowed: true,
        reasonRefs: [
          'placement.privacy.public_beta',
          'placement.local_only.not_allowed',
          'placement.public_trace.allowed',
          'placement.secret_broker.not_required',
        ],
        requiresSecretBroker: false,
      },
      state: 'accepted_free_slice',
      taskRefs: ['task.autopilot_coder.docs_contract'],
      workOrderRef: 'autopilot_work_order.test_1',
    })
    expect(replayJson.work).toEqual({
      ...firstJson.work,
      idempotent: true,
    })

    const detail = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}`,
      { method: 'GET' },
    )
    const detailJson = await responseJson(detail)

    expect(detail.status).toBe(200)
    expect(detailJson.work).toEqual(firstJson.work)
  })

  test('creates browser-session work orders for own Pylon and metered SHC fallback', async () => {
    const store = new MemoryAutopilotWorkStore()
    const browserUserId = 'github:browser-autopilot-owner'
    const promiseRef = {
      blockerRefs: [],
      promiseId: 'autopilot.mission_briefing.v1',
      registryVersion: '2026-06-11.1',
    }
    const ownPylon = new MemoryPylonApiStore([
      pylonRegistration({
        ownerAgentCredentialId: 'browser_session.github_browser-autopilot-owner',
        ownerAgentUserId: browserUserId,
      }),
    ])
    const ownPylonRequest = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      caller: { kind: 'browser_session', ownerRef: 'owner_ref.browser' },
      clientRequestRef: 'client.browser.20260611.own_pylon',
      promiseRef,
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
          taskRef: 'task.autopilot_coder.browser_own_pylon',
        },
      ],
    }
    const fallbackRequest = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      caller: { kind: 'browser_session', ownerRef: 'owner_ref.browser' },
      clientRequestRef: 'client.browser.20260611.shc_metered',
      promiseRef,
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].tasks[0],
          taskRef: 'task.autopilot_coder.browser_shc_metered',
        },
      ],
    }

    const ownPylonResponse = await route(store, '/api/autopilot/work', {
      body: ownPylonRequest,
      idempotencyKey: 'browser-own-pylon',
      pylonApiStore: ownPylon,
      sessionUserId: browserUserId,
      token: '',
    })
    const ownPylonListResponse = await route(
      store,
      '/api/autopilot/work?promiseId=autopilot.mission_briefing.v1',
      {
        pylonApiStore: ownPylon,
        sessionUserId: browserUserId,
        token: '',
      },
    )
    const fallbackResponse = await route(store, '/api/autopilot/work', {
      body: fallbackRequest,
      idempotencyKey: 'browser-shc-metered',
      sessionUserId: browserUserId,
      token: '',
    })
    const fallbackListResponse = await route(
      store,
      '/api/autopilot/work?promiseId=autopilot.mission_briefing.v1',
      {
        pylonApiStore: ownPylon,
        sessionUserId: browserUserId,
        token: '',
      },
    )
    const ownPylonJson = await responseJson(ownPylonResponse)
    const fallbackJson = await responseJson(fallbackResponse)
    const ownPylonWork =
      ownPylonJson.work as AutopilotWorkOrderProjection | undefined
    const fallbackWork =
      fallbackJson.work as AutopilotWorkOrderProjection | undefined
    type ListedAutopilotWorkOrder = Readonly<{
      routing: Readonly<{
        availabilityState: string
        buyerDebitRequired: boolean
        fallbackLeaseIntentCount: number
        fallbackRunnerKind: string | null
        laneRef: string | null
        meterKind: string | null
        pylonAssignmentIntentCount: number
        selectedRunnerKind: string | null
        source: string
      }>
      workOrderRef: string
    }>
    const ownPylonListJson = (await ownPylonListResponse.json()) as Readonly<{
      workOrders: ReadonlyArray<ListedAutopilotWorkOrder>
    }>
    const fallbackListJson = (await fallbackListResponse.json()) as Readonly<{
      workOrders: ReadonlyArray<ListedAutopilotWorkOrder>
    }>
    const listedOwnPylon = ownPylonListJson.workOrders.find(
      order => order.workOrderRef === ownPylonWork?.workOrderRef,
    )
    const listedFallback = fallbackListJson.workOrders.find(
      order => order.workOrderRef === fallbackWork?.workOrderRef,
    )

    expect(ownPylonResponse.status).toBe(202)
    expect(ownPylonWork?.state).toBe('queued_or_running')
    expect(ownPylonWork?.placementDecision?.selectedRunnerKind).toBe(
      'requester_pylon',
    )
    expect(ownPylonWork?.placementDecision?.reasonRefs).toEqual([
      'placement.selected.requester_pylon',
      'placement.pylon.preferred_before_fallback',
      'pricing.autopilot_work.own_pylon_free',
      'placement.reason.placed_on_your_pylon_free',
    ])
    expect(ownPylonWork?.pricingPolicy.activeLane).toMatchObject({
      buyerDebitRequired: false,
      laneRef: 'lane.autopilot_work.requester_pylon_own_job',
      meterKind: 'none',
      runnerKind: 'requester_pylon',
      unitAmountCents: 0,
    })
    expect([...ownPylon.assignments.values()]).toHaveLength(1)
    expect(fallbackResponse.status).toBe(402)
    expect(fallbackWork?.state).toBe('payment_required')
    expect(
      fallbackWork?.placementDecision?.fallbackRunnerKind ??
        fallbackWork?.placementDecision?.selectedRunnerKind,
    ).toBe('openagents_shc')
    expect(fallbackWork?.placementDecision?.reasonRefs).toEqual([
      'placement.selected.fallback',
      'placement.fallback.openagents_shc',
      'pricing.autopilot_work.hosted_runner_metered',
      'placement.reason.your_pylon_unavailable_hosted_metered',
    ])
    expect(fallbackWork?.pricingPolicy.activeLane).toMatchObject({
      buyerDebitRequired: true,
      laneRef: 'lane.autopilot_work.openagents_shc_fallback',
      meterKind: 'usd_credits',
      runnerKind: 'openagents_shc',
    })
    expect(
      ownPylonListJson.workOrders.map(order => order.workOrderRef),
    ).toContain(ownPylonJson.work?.workOrderRef)
    expect(
      fallbackListJson.workOrders.map(order => order.workOrderRef),
    ).toContain(fallbackJson.work?.workOrderRef)
    expect(listedOwnPylon?.routing).toMatchObject({
      availabilityState: 'selected',
      buyerDebitRequired: false,
      fallbackLeaseIntentCount: 0,
      fallbackRunnerKind: 'openagents_shc',
      laneRef: 'lane.autopilot_work.requester_pylon_own_job',
      meterKind: 'none',
      pylonAssignmentIntentCount: 0,
      selectedRunnerKind: 'requester_pylon',
      source: 'requester_pylon',
    })
    expect(listedFallback?.routing).toMatchObject({
      availabilityState: 'selected',
      buyerDebitRequired: true,
      fallbackRunnerKind: 'openagents_shc',
      laneRef: 'lane.autopilot_work.openagents_shc_fallback',
      meterKind: 'usd_credits',
      selectedRunnerKind: 'openagents_shc',
      source: 'fallback',
    })
  })

  test('carries promiseRef through projections, briefing, and the list filter', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      promiseRef: {
        blockerRefs: ['blocker.product_promises.drilldown_artifact_refs_incomplete'],
        promiseId: 'autopilot.mission_briefing.v1',
        registryVersion: '2026-06-09.17',
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const created = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-promise-ref',
    })
    const createdJson = await responseJson(created)
    const workOrderRef = createdJson.work?.workOrderRef
    const detail = await route(store, `/api/autopilot/work/${workOrderRef}`, {
      method: 'GET',
    })
    const detailJson = await responseJson(detail)
    const briefing = await route(
      store,
      `/api/autopilot/work/${workOrderRef}/briefing`,
      { method: 'GET' },
    )
    const briefingJson = (await briefing.json()) as Readonly<{
      briefing: Readonly<{ promiseRef: Readonly<{ promiseId: string }> | null }>
    }>
    const listed = await route(
      store,
      '/api/autopilot/work?promiseId=autopilot.mission_briefing.v1',
      { method: 'GET' },
    )
    const listedJson = (await listed.json()) as Readonly<{
      promiseId: string
      workOrders: ReadonlyArray<Readonly<{ workOrderRef: string }>>
    }>
    const listedOther = await route(
      store,
      '/api/autopilot/work?promiseId=forum.content_tipping.v1',
      { method: 'GET' },
    )
    const listedOtherJson = (await listedOther.json()) as Readonly<{
      workOrders: ReadonlyArray<unknown>
    }>
    const malformed = await route(
      store,
      '/api/autopilot/work?promiseId=DROP%20TABLE',
      { method: 'GET' },
    )
    const rejected = await route(store, '/api/autopilot/work', {
      body: {
        ...request,
        promiseRef: { promiseId: 'not a promise id', registryVersion: 'nope' },
      },
      idempotencyKey: 'idem-autopilot-work-promise-ref-bad',
    })

    expect(created.status).toBe(202)
    expect(detailJson.work).toMatchObject({
      promiseRef: {
        blockerRefs: [
          'blocker.product_promises.drilldown_artifact_refs_incomplete',
        ],
        promiseId: 'autopilot.mission_briefing.v1',
        registryVersion: '2026-06-09.17',
      },
    })
    expect(briefingJson.briefing.promiseRef).toMatchObject({
      promiseId: 'autopilot.mission_briefing.v1',
    })
    expect(listed.status).toBe(200)
    expect(listedJson.workOrders).toEqual([
      expect.objectContaining({ workOrderRef }),
    ])
    expect(listedOtherJson.workOrders).toEqual([])
    expect(malformed.status).toBe(400)
    expect(rejected.status).toBe(400)
  })

  test('requires idempotency on create', async () => {
    const response = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work',
      {
        body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      },
    )
    const body = await responseJson(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('autopilot_work_validation_error')
  })

  test('projects independent typed task records for batch requests', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
          taskRef: 'task.autopilot_coder.docs_contract',
        },
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [
            {
              kind: 'github_repo_write' as const,
              reasonRef: 'reason.repo_write_required',
            },
          ],
          acceptanceCriteriaRefs: ['acceptance.patch_tests_pass'],
          kind: 'test_repair' as const,
          taskRef: 'task.autopilot_coder.test_repair',
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-task-records',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work).toMatchObject({
      state: 'access_required',
      assignmentIntents: [
        {
          assignmentKind: 'repo_change',
          plannerReasonRefs: [
            'assignment.free_slice',
            'assignment.ready_for_assignment',
          ],
          plannerState: 'free_slice',
          readyForAssignment: true,
          taskRef: 'task.autopilot_coder.docs_contract',
        },
        {
          assignmentKind: 'test_repair',
          plannerReasonRefs: [
            'assignment.blocked.access_required',
            'access_request.task.autopilot_coder.test_repair.github_repo_write',
          ],
          plannerState: 'access_required',
          readyForAssignment: false,
          taskRef: 'task.autopilot_coder.test_repair',
        },
      ],
      taskRefs: [
        'task.autopilot_coder.docs_contract',
        'task.autopilot_coder.test_repair',
      ],
      tasks: [
        {
          acceptanceCriteriaRefs: [
            'acceptance.docs.updated',
            'acceptance.tests.contract',
          ],
          accessRequirements: [],
          accessState: 'satisfied',
          kind: 'code_change',
          lifecycleState: 'ready_for_assignment',
          paymentState: 'not_required',
          placementState: 'ready_for_assignment',
          taskRef: 'task.autopilot_coder.docs_contract',
        },
        {
          acceptanceCriteriaRefs: ['acceptance.patch_tests_pass'],
          accessRequirements: [
            {
              accessRequestRef:
                'access_request.task.autopilot_coder.test_repair.github_repo_write',
              grantAction: 'connect_github_repository',
              kind: 'github_repo_write',
              ownerActionRef:
                'owner_action.task.autopilot_coder.test_repair.github_repo_write',
              reasonRef: 'reason.repo_write_required',
              requiredBeforeLaunch: true,
              status: 'missing',
              taskRef: 'task.autopilot_coder.test_repair',
            },
          ],
          accessState: 'missing_required_access',
          kind: 'test_repair',
          lifecycleState: 'access_required',
          paymentState: 'not_required',
          placementState: 'blocked_on_access',
          taskRef: 'task.autopilot_coder.test_repair',
        },
      ],
    })
  })

  test('selects an online compatible requester Pylon before fallback', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-pylon-placement',
      pylonRegistrations: [
        pylonRegistration({
          ownerAgentUserId: 'other_agent',
          pylonRef: 'pylon.other',
        }),
        pylonRegistration(),
      ],
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.placementDecision).toMatchObject({
      fallbackRunnerKind: 'openagents_shc',
      reasonRefs: [
        'placement.selected.requester_pylon',
        'placement.pylon.preferred_before_fallback',
        'pricing.autopilot_work.own_pylon_free',
        'placement.reason.placed_on_your_pylon_free',
      ],
      selectedPylonRef: 'pylon.local.docs_agent',
      selectedRunnerKind: 'requester_pylon',
      source: 'requester_pylon',
    })
    expect(body.work?.placementDecision?.pylonCandidates).toEqual([
      expect.objectContaining({
        ownerLinked: false,
        selected: false,
      }),
      expect.objectContaining({
        assignmentReady: true,
        heartbeatFresh: true,
        localExecutionReady: true,
        ownerLinked: true,
        selected: true,
        versionCompatible: true,
        walletReady: true,
      }),
    ])
    expect(body.work?.pylonAssignmentIntents).toEqual([
      expect.objectContaining({
        assignmentRef:
          'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract',
        forumAutoPublishAllowed: false,
        paymentMode: 'unpaid_smoke',
        pylonRef: 'pylon.local.docs_agent',
        requiredCapabilityRefs: [
          'capability.pylon.assignment_ready',
          'capability.pylon.local_claude_agent',
        ],
        spendCapRefs: ['spend_cap.no_spend.autopilot_pylon_assignment'],
        taskRef: 'task.autopilot_coder.docs_contract',
      }),
    ])
  })

  test('selects requester Pylon from the production Pylon store dependency', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-production-pylon-placement',
      pylonStoreRegistrations: [
        pylonRegistration({
          capabilityRefs: ['capability.pylon.assignment_ready'],
          pylonRef: 'pylon.missing_local_agent',
        }),
        pylonRegistration({
          pylonRef: 'pylon.production.docs_agent',
        }),
      ],
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.placementDecision).toMatchObject({
      fallbackRunnerKind: 'openagents_shc',
      selectedPylonRef: 'pylon.production.docs_agent',
      selectedRunnerKind: 'requester_pylon',
      source: 'requester_pylon',
    })
    expect(body.work?.placementDecision?.pylonCandidates).toEqual([
      expect.objectContaining({
        localExecutionReady: false,
        pylonRef: 'pylon.missing_local_agent',
        selected: false,
      }),
      expect.objectContaining({
        assignmentReady: true,
        heartbeatFresh: true,
        localExecutionReady: true,
        ownerLinked: true,
        pylonRef: 'pylon.production.docs_agent',
        selected: true,
        versionCompatible: true,
        walletReady: true,
      }),
    ])
    expect(body.work?.pylonAssignmentIntents).toEqual([
      expect.objectContaining({
        pylonRef: 'pylon.production.docs_agent',
        taskRef: 'task.autopilot_coder.docs_contract',
      }),
    ])
  })

  test('creates one durable no-spend Pylon claude_agent_task git_checkout lease for requester Pylon work', async () => {
    const store = new MemoryAutopilotWorkStore()
    const buyerPaymentLedgerStore = new MemoryBuyerPaymentLedgerStore()
    const pylonApiStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.docs_agent',
      }),
    ])
    const create = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      buyerPaymentLedgerStore,
      idempotencyKey: 'idem-autopilot-work-pylon-lease',
      pylonApiStore,
    })
    const createJson = await responseJson(create)
    const assignmentRef =
      'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract'
    const replay = await route(store, '/api/autopilot/work', {
      body: { ignored: 'idempotent replay does not create another lease' },
      idempotencyKey: 'idem-autopilot-work-pylon-lease',
      pylonApiStore,
    })
    const replayJson = await responseJson(replay)
    const poll = await pylonRoute(
      pylonApiStore,
      '/api/pylons/pylon.production.docs_agent/assignments',
    )
    const pollJson = await responseJson(poll)
    const accept = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.autopilot_pylon.accepted'],
          accepted: true,
        },
        idempotencyKey: 'accept-autopilot-pylon-lease',
        method: 'POST',
      },
    )
    const acceptJson = await responseJson(accept)

    expect(create.status).toBe(202)
    expect(createJson.work).toMatchObject({
      assignmentIntents: [
        expect.objectContaining({
          plannerReasonRefs: ['assignment.queued_or_running'],
          plannerState: 'queued_or_running',
          readyForAssignment: false,
        }),
      ],
      placementDecision: {
        reasonRefs: [
          'placement.selected.requester_pylon',
          'placement.pylon.preferred_before_fallback',
          'pricing.autopilot_work.own_pylon_free',
          'placement.reason.placed_on_your_pylon_free',
        ],
        selectedPylonRef: 'pylon.production.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      pricingPolicy: {
        activeLane: {
          buyerDebitRequired: false,
          laneRef: 'lane.autopilot_work.requester_pylon_own_job',
          meterKind: 'none',
          runnerKind: 'requester_pylon',
          unitAmountCents: 0,
        },
        policyRef: 'pricing_policy.autopilot_work.v0_3.lane_meter_mapping',
      },
      pylonAssignmentIntents: [],
      state: 'queued_or_running',
    })
    expect(createJson.work?.paymentChallengeRef).toBeNull()
    expect(createJson.work?.funding).toMatchObject({
      buyerFundingState: 'not_required',
      fundedAmountCents: 0,
    })
    expect(buyerPaymentLedgerStore.creditDebits.size).toBe(0)
    expect(pylonApiStore.assignments.size).toBe(1)
    expect(replay.status).toBe(200)
    expect(replayJson.work?.state).toBe('queued_or_running')
    expect(pylonApiStore.assignments.size).toBe(1)
    expect(poll.status).toBe(200)
    expect(pollJson.assignments).toEqual([
      expect.objectContaining({
        assignmentRef,
        codingAssignment: expect.objectContaining({
          assignmentRef,
          budget: expect.objectContaining({
            paymentMode: 'unpaid_smoke',
            workerPayoutAuthority: false,
          }),
          claudeAgent: expect.objectContaining({
            agentKind: 'claude_agent_sdk',
            schema: 'openagents.pylon.claude_agent_task.v0.3',
          }),
          closeoutSchema: expect.objectContaining({
            acceptedWorkAuthority: false,
          }),
          objective: expect.objectContaining({
            publicSummary: 'Add public-safe Autopilot coder contract docs.',
          }),
          publicSafe: true,
          requiredCapabilityRefs: [
            'capability.pylon.assignment_ready',
            'capability.pylon.local_claude_agent',
          ],
          runnerKind: 'requester_pylon',
          schema: 'openagents.autopilot_coding_assignment.v1',
          tracePolicy: expect.objectContaining({
            rawPromptAllowed: false,
            rawProviderPayloadAllowed: false,
            rawRunnerLogAllowed: false,
            rawSourceArchiveAllowed: false,
          }),
          workspace: expect.objectContaining({
            kind: 'git_checkout',
            repository: expect.objectContaining({
              commitSha: '1745cd4b54b8a12a50922f80b5d345314c91d70d',
              fullName: 'OpenAgentsInc/openagents',
              visibility: 'public',
            }),
            verificationCommand: {
              args: ['bun', 'test'],
              commandRef: 'command.public.autopilot_coder.bun_test',
            },
          }),
        }),
        jobKind: 'claude_agent_task',
        leaseState: 'active',
        state: 'offered',
        taskRefs: [
          'autopilot_work_order.test_1',
          'task.autopilot_coder.docs_contract',
        ],
      }),
    ])
    expect(accept.status).toBe(201)
    expect(acceptJson.assignment).toMatchObject({
      assignmentRef,
      leaseState: 'active',
      state: 'accepted',
    })
  })

  test('ingests Pylon worker closeout refs into delivered Autopilot work', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.docs_agent',
      }),
    ])
    const assignmentRef =
      'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract'
    const create = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-pylon-closeout',
      pylonApiStore,
    })

    expect(create.status).toBe(202)

    const accept = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.autopilot_pylon.accepted'],
          accepted: true,
        },
        idempotencyKey: 'accept-autopilot-pylon-closeout',
        method: 'POST',
      },
    )
    const closeout = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/closeout`,
      {
        body: {
          artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
          authorityReceiptRefs: ['authority.public.autopilot_docs.writeback_ready'],
          blockerRefs: [],
          buildRefs: ['build.public.autopilot_docs.not_required'],
          changeCaptureRefs: ['change-capture.public.autopilot_docs.pack_c'],
          changeCaptureStatus: 'review_ready',
          closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
          deliveryReadinessFreshness: 'fresh',
          deliveryReadinessRefs: ['delivery.public.autopilot_docs.ready'],
          deliveryReadinessStatus: 'ready',
          fileCount: 2,
          addedLineCount: 18,
          patchDigestRef: 'patch-digest.public.autopilot_docs.sha256_abc123',
          previewRefs: ['preview.public.autopilot_docs.not_required'],
          proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
          removedLineCount: 4,
          resultRefs: [
            'result.public.autopilot_docs.delivered',
            'result.public.pylon.claude_agent_task.git_checkout_verified_passed',
          ],
          reviewCaveatRefs: ['review-caveat.public.autopilot_docs.summary_only'],
          status: 'closeout_submitted',
          summaryRefs: ['summary.public.autopilot_docs.customer_safe'],
          testRefs: ['test.public.autopilot_docs.not_required'],
          verificationRefs: ['verification.public.autopilot_docs.bun_test'],
          worktreeIdentityStatus: 'ready',
          writebackRequired: true,
        },
        idempotencyKey: 'worker-closeout-autopilot-pylon',
        method: 'POST',
        recordAutopilotWorkerCloseout: (_env, input) =>
          recordAutopilotWorkerCloseoutFromPylon(store, input),
      },
    )
    const delivered = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const events = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/events',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const deliveredJson = await responseJson(delivered)
    const eventsJson = await responseJson(events)
    const closeoutJson = await responseJson(closeout)

    expect(accept.status).toBe(201)
    expect(closeout.status).toBe(201)
    expect(closeoutJson.assignment).toMatchObject({
      assignmentRef,
      acceptedWorkRefs: [],
      artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
      closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
      proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
      state: 'closeout_submitted',
    })
    expect(delivered.status).toBe(200)
    expect(deliveredJson.work).toMatchObject({
      executionCloseout: {
        acceptedWorkAuthority: false,
        artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
        authorityReceiptRefs: ['authority.public.autopilot_docs.writeback_ready'],
        blockerRefs: [],
        buildRefs: ['build.public.autopilot_docs.not_required'],
        changeCaptureRefs: ['change-capture.public.autopilot_docs.pack_c'],
        changeCaptureStatus: 'review_ready',
        closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
        deliveryReadinessFreshness: 'fresh',
        deliveryReadinessRefs: ['delivery.public.autopilot_docs.ready'],
        deliveryReadinessStatus: 'ready',
        fileCount: 2,
        forumAutoPublishAllowed: false,
        addedLineCount: 18,
        patchDigestRef: 'patch-digest.public.autopilot_docs.sha256_abc123',
        previewRefs: ['preview.public.autopilot_docs.not_required'],
        proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
        publicSafe: true,
        removedLineCount: 4,
        resultRefs: [
          'result.public.autopilot_docs.delivered',
          'result.public.pylon.claude_agent_task.git_checkout_verified_passed',
        ],
        reviewCaveatRefs: ['review-caveat.public.autopilot_docs.summary_only'],
        runnerKind: 'requester_pylon',
        summaryRefs: ['summary.public.autopilot_docs.customer_safe'],
        testRefs: ['test.public.autopilot_docs.not_required'],
        verificationRefs: ['verification.public.autopilot_docs.bun_test'],
        worktreeIdentityStatus: 'ready',
        writebackRequired: true,
        workerPayoutAuthority: false,
      },
      nextAction: {
        reasonRefs: ['next_action.review_delivered_work'],
        state: 'delivered',
      },
      state: 'delivered',
    })
    expect(events.status).toBe(200)
    expect(eventsJson.events).toEqual([
      expect.objectContaining({ eventKind: 'queued' }),
      expect.objectContaining({ eventKind: 'delivered' }),
    ])

    const briefing = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/briefing',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const briefingJson = await responseJson(briefing)

    expect(briefing.status).toBe(200)
    expect(briefingJson.briefing).toMatchObject({
      briefingRef: 'briefing.autopilot_work_order.test_1',
      costs: {
        currency: 'USD',
        fundedAmountCents: 0,
      },
      decisionsWaiting: {
        nextActionState: 'delivered',
        reasonRefs: ['next_action.review_delivered_work'],
        reviewAction: null,
      },
      kind: 'autopilot_mission_briefing',
      publicSafe: true,
      state: 'delivered',
      whatChanged: {
        artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
        resultRefs: [
          'result.public.autopilot_docs.delivered',
          'result.public.pylon.claude_agent_task.git_checkout_verified_passed',
        ],
        runnerKind: 'requester_pylon',
        summaryRefs: ['summary.public.autopilot_docs.customer_safe'],
      },
      whatIsBlocked: {
        accessRequirementRefs: [],
        blockerRefs: [],
      },
      whatIsRunning: {
        running: false,
      },
      workOrderRef: 'autopilot_work_order.test_1',
    })
    expect(briefingJson.briefing?.whatHappened).toEqual([
      expect.objectContaining({ eventKind: 'queued', sequence: 1 }),
      expect.objectContaining({ eventKind: 'delivered', sequence: 2 }),
    ])
    expect(
      briefingJson.briefing?.drilldown.map(
        (group: { kind: string }) => group.kind,
      ),
    ).toEqual([
      'artifact',
      'assignment',
      'build',
      'closeout',
      'preview',
      'proof',
      'result',
      'summary',
      'test',
    ])
    expect(briefingJson.briefing?.receipts).toEqual({
      authorityReceiptRefs: ['authority.public.autopilot_docs.writeback_ready'],
      buyerPaymentProofRef: null,
      proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
      settlementEligible: false,
      verificationRefs: ['verification.public.autopilot_docs.bun_test'],
    })
    expect(briefingJson.briefing?.risk).toEqual({
      blockerCount: 0,
      changeCaptureStatus: 'review_ready',
      deliveryReadinessStatus: 'ready',
      level: 'attention',
      reviewCaveatRefs: ['review-caveat.public.autopilot_docs.summary_only'],
      settlementBlockedReasonRef: 'settlement.no_worker_payout_mode',
      worktreeIdentityStatus: 'ready',
    })
    expect(JSON.stringify(briefingJson)).not.toMatch(
      /mnemonic|invoice|preimage|\/Users\//,
    )

    const briefingUnauthorized = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/briefing',
      {
        method: 'GET',
        pylonApiStore,
        token: '',
      },
    )

    expect(briefingUnauthorized.status).toBe(401)
  })

  test('rejects unsafe Pylon worker closeout refs before Autopilot delivery persistence', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.docs_agent',
      }),
    ])
    const assignmentRef =
      'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract'

    await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-unsafe-pylon-closeout',
      pylonApiStore,
    })
    await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.autopilot_pylon.accepted'],
          accepted: true,
        },
        idempotencyKey: 'accept-autopilot-pylon-unsafe-closeout',
        method: 'POST',
      },
    )

    const closeout = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/closeout`,
      {
        body: {
          artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
          changeCaptureRefs: ['diff --git a/private.ts b/private.ts'],
          closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
          proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
          resultRefs: ['result.public.autopilot_docs.delivered'],
          status: 'closeout_submitted',
        },
        idempotencyKey: 'worker-closeout-autopilot-pylon-unsafe',
        method: 'POST',
        recordAutopilotWorkerCloseout: (_env, input) =>
          recordAutopilotWorkerCloseoutFromPylon(store, input),
      },
    )
    const recovered = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const recoveredJson = await responseJson(recovered)

    expect(closeout.status).toBe(400)
    expect(recovered.status).toBe(200)
    expect(recoveredJson.work).toMatchObject({
      executionCloseout: null,
      state: 'queued_or_running',
    })
  })

  test('records SHC fallback closeout refs and keeps review as a separate gate', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      clientRequestRef: 'client.example.20260609.shc_closeout',
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].placementPolicy,
        allowedRunnerKinds: ['openagents_shc'] as const,
        preferredRunnerKinds: ['openagents_shc'] as const,
        privacyTier: 'openagents_shc' as const,
        publicTraceAllowed: false,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const create = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-shc-closeout',
      pylonRegistrations: [],
    })
    const createJson = await responseJson(create)
    const workOrderRef = createJson.work?.workOrderRef
    const assignmentRef =
      createJson.work?.fallbackLeaseIntents?.[0]?.assignmentRef

    if (workOrderRef === undefined || assignmentRef === undefined) {
      throw new Error('Expected SHC fallback work and assignment refs.')
    }

    expect(create.status).toBe(202)
    expect(createJson.work).toMatchObject({
      fallbackLeaseIntents: [
        expect.objectContaining({
          assignmentRef:
            'fallback_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract',
          runnerKind: 'openagents_shc',
          workerPayoutAuthority: false,
        }),
      ],
      placementDecision: {
        selectedRunnerKind: 'openagents_shc',
        source: 'fallback',
      },
      state: 'accepted_free_slice',
    })

    const closeoutBody = {
      artifactRefs: ['artifact.public.autopilot_shc.patch_summary'],
      assignmentRefs: [assignmentRef],
      buildRefs: ['build.public.autopilot_shc.not_required'],
      closeoutRefs: ['closeout.public.autopilot_shc.worker_summary'],
      previewRefs: ['preview.public.autopilot_shc.not_required'],
      proofRefs: ['proof.public.autopilot_shc.worker_closeout'],
      resultRefs: ['result.public.autopilot_shc.delivered'],
      runnerKind: 'openagents_shc',
      summaryRefs: ['summary.public.autopilot_shc.customer_safe'],
      testRefs: ['test.public.autopilot_shc.bun_passed'],
    }
    const closeout = await route(store, `/api/autopilot/work/${workOrderRef}/closeout`, {
      body: closeoutBody,
      idempotencyKey: 'closeout-autopilot-work-shc',
      method: 'POST',
      pylonRegistrations: [],
    })
    const replay = await route(store, `/api/autopilot/work/${workOrderRef}/closeout`, {
      body: closeoutBody,
      idempotencyKey: 'closeout-autopilot-work-shc',
      method: 'POST',
      pylonRegistrations: [],
    })
    const review = await route(store, `/api/autopilot/work/${workOrderRef}/review`, {
      body: {
        action: 'accept',
        decisionRefs: ['review.public.autopilot_shc.customer_accepts'],
      },
      idempotencyKey: 'review-autopilot-work-shc',
      method: 'POST',
      pylonRegistrations: [],
    })
    const closeoutJson = await responseJson(closeout)
    const replayJson = await responseJson(replay)
    const reviewJson = await responseJson(review)

    expect(closeout.status).toBe(201)
    expect(closeoutJson.work).toMatchObject({
      executionCloseout: {
        acceptedWorkAuthority: false,
        artifactRefs: ['artifact.public.autopilot_shc.patch_summary'],
        assignmentRefs: [assignmentRef],
        closeoutRefs: ['closeout.public.autopilot_shc.worker_summary'],
        forumAutoPublishAllowed: false,
        proofRefs: ['proof.public.autopilot_shc.worker_closeout'],
        publicSafe: true,
        resultRefs: ['result.public.autopilot_shc.delivered'],
        runnerKind: 'openagents_shc',
        workerPayoutAuthority: false,
      },
      nextAction: {
        reasonRefs: ['next_action.review_delivered_work'],
        state: 'delivered',
      },
      state: 'delivered',
    })
    expect(replay.status).toBe(200)
    expect(replayJson.idempotent).toBe(true)
    expect(review.status).toBe(201)
    expect(reviewJson.work).toMatchObject({
      reviewDecision: {
        action: 'accept',
        acceptedWorkAuthority: false,
        deployAuthority: false,
        forumAutoPublishAllowed: false,
        settlementAuthority: false,
        workerPayoutAuthority: false,
      },
      state: 'accepted',
    })
  })

  test('rejects unsafe or mismatched SHC fallback closeout refs before delivery persistence', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      clientRequestRef: 'client.example.20260609.shc_closeout_reject',
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].placementPolicy,
        allowedRunnerKinds: ['openagents_shc'] as const,
        preferredRunnerKinds: ['openagents_shc'] as const,
        privacyTier: 'openagents_shc' as const,
        publicTraceAllowed: false,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const create = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-shc-closeout-reject',
      pylonRegistrations: [],
    })
    const createJson = await responseJson(create)
    const workOrderRef = createJson.work?.workOrderRef
    const assignmentRef =
      createJson.work?.fallbackLeaseIntents?.[0]?.assignmentRef

    if (workOrderRef === undefined || assignmentRef === undefined) {
      throw new Error('Expected SHC fallback work and assignment refs.')
    }

    const unsafe = await route(store, `/api/autopilot/work/${workOrderRef}/closeout`, {
      body: {
        assignmentRefs: [assignmentRef],
        closeoutRefs: ['closeout.public.autopilot_shc.worker_summary'],
        proofRefs: ['proof.public./Users/christopher/raw_runner_log'],
        resultRefs: ['result.public.autopilot_shc.delivered'],
        runnerKind: 'openagents_shc',
      },
      idempotencyKey: 'closeout-autopilot-work-shc-unsafe',
      method: 'POST',
      pylonRegistrations: [],
    })
    const mismatch = await route(store, `/api/autopilot/work/${workOrderRef}/closeout`, {
      body: {
        assignmentRefs: [assignmentRef],
        closeoutRefs: ['closeout.public.autopilot_shc.worker_summary'],
        proofRefs: ['proof.public.autopilot_shc.worker_closeout'],
        resultRefs: ['result.public.autopilot_shc.delivered'],
        runnerKind: 'cloud_sandbox',
      },
      idempotencyKey: 'closeout-autopilot-work-shc-mismatch',
      method: 'POST',
      pylonRegistrations: [],
    })
    const detail = await route(store, `/api/autopilot/work/${workOrderRef}`, {
      method: 'GET',
      pylonRegistrations: [],
    })
    const detailJson = await responseJson(detail)

    expect(unsafe.status).toBe(400)
    expect(mismatch.status).toBe(400)
    expect(detailJson.work).toMatchObject({
      executionCloseout: null,
      state: 'accepted_free_slice',
    })
  })

  test('accepts delivered Autopilot work without granting payout or settlement authority', async () => {
    const { pylonApiStore, store } = await createDeliveredPylonBackedWork()
    const review = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public.customer_accepts_delivered_refs'],
        },
        idempotencyKey: 'review-accept-autopilot-work',
        method: 'POST',
        pylonApiStore,
      },
    )
    const replay = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public.customer_accepts_delivered_refs'],
        },
        idempotencyKey: 'review-accept-autopilot-work',
        method: 'POST',
        pylonApiStore,
      },
    )
    const events = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/events',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const reviewJson = await responseJson(review)
    const replayJson = await responseJson(replay)
    const eventsJson = await responseJson(events)

    expect(review.status).toBe(201)
    expect(replay.status).toBe(200)
    expect(replayJson.idempotent).toBe(true)
    expect(reviewJson.work).toMatchObject({
      nextAction: {
        reasonRefs: ['next_action.customer_accepted_work'],
        state: 'accepted',
      },
      reviewDecision: {
        acceptedWorkAuthority: false,
        action: 'accept',
        decisionRefs: ['review.public.customer_accepts_delivered_refs'],
        deployAuthority: false,
        forumAutoPublishAllowed: false,
        publicSafe: true,
        settlementAuthority: false,
        workerPayoutAuthority: false,
      },
      state: 'accepted',
    })
    expect(eventsJson.events).toEqual([
      expect.objectContaining({ eventKind: 'queued' }),
      expect.objectContaining({ eventKind: 'accepted' }),
    ])
  })

  test('no-spend Autopilot Coder end-to-end smoke retains public-safe evidence refs', async () => {
    const { pylonApiStore, store } = await createDeliveredPylonBackedWork()
    const review = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public.no_spend_smoke.customer_accepts'],
        },
        idempotencyKey: 'smoke-no-spend-autopilot-coder-review',
        method: 'POST',
        pylonApiStore,
      },
    )
    const detail = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const events = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/events',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const reviewJson = await responseJson(review)
    const detailJson = await responseJson(detail)
    const eventsJson = await responseJson(events)
    const retainedProjection = JSON.stringify({
      detail: detailJson,
      events: eventsJson,
      review: reviewJson,
    })

    expect(review.status).toBe(201)
    expect(detail.status).toBe(200)
    expect(events.status).toBe(200)
    expect(detailJson.work).toMatchObject({
      executionCloseout: {
        artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
        buildRefs: ['build.public.autopilot_docs.not_required'],
        closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
        previewRefs: ['preview.public.autopilot_docs.not_required'],
        proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
        resultRefs: ['result.public.autopilot_docs.delivered'],
        summaryRefs: ['summary.public.autopilot_docs.customer_safe'],
        testRefs: ['test.public.autopilot_docs.not_required'],
        workerPayoutAuthority: false,
      },
      funding: {
        buyerFundingState: 'not_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      reviewDecision: {
        action: 'accept',
        decisionRefs: ['review.public.no_spend_smoke.customer_accepts'],
        settlementAuthority: false,
        workerPayoutAuthority: false,
      },
      state: 'accepted',
    })
    expect(eventsJson.events).toEqual([
      expect.objectContaining({ eventKind: 'queued' }),
      expect.objectContaining({ eventKind: 'accepted' }),
    ])
    expect(retainedProjection).not.toMatch(
      retainedProjectionPrivateMaterialPattern,
    )
  })

  test('paid Autopilot Coder end-to-end smoke keeps settlement blocked after verified delivery and review', async () => {
    const store = new MemoryAutopilotWorkStore()
    const buyerPaymentLedgerStore = new MemoryBuyerPaymentLedgerStore()
    const pylonApiStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.paid_agent',
      }),
    ])
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      clientRequestRef: 'client.example.20260609.paid_smoke',
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        quoteRef: null,
        quotedAmountCents: null,
      },
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
        allowedRunnerKinds: ['requester_pylon'] as const,
        preferredRunnerKinds: ['requester_pylon'] as const,
        privacyTier: 'public_beta' as const,
        publicTraceAllowed: true,
      },
    }
    const assignmentRef =
      'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.paid_test_repair'
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      buyerPaymentLedgerStore,
      idempotencyKey: 'idem-autopilot-coder-paid-smoke',
      pylonApiStore,
      verifyL402PaymentProof: input =>
        verifyAutopilotL402PaymentProofFromBuyerLedger(
          buyerPaymentLedgerStore,
          input,
        ),
    })
    const firstJson = await responseJson(first)
    const proofRef = 'payment_proof.autopilot_work.paid_smoke'
    await recordMatchedAutopilotLedgerPayment(buyerPaymentLedgerStore, {
      credential: first.headers.get('x-openagents-l402-credential'),
      proofRef,
    })
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry uses stored request' },
      buyerPaymentLedgerStore,
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          first.headers.get('x-openagents-l402-credential'),
          proofRef,
        ),
      },
      idempotencyKey: 'idem-autopilot-coder-paid-smoke',
      pylonApiStore,
      verifyL402PaymentProof: input =>
        verifyAutopilotL402PaymentProofFromBuyerLedger(
          buyerPaymentLedgerStore,
          input,
        ),
    })
    const paidJson = await responseJson(paid)
    const assignment = await pylonRoute(
      pylonApiStore,
      '/api/pylons/pylon.production.paid_agent/assignments',
      { method: 'GET' },
    )
    const assignmentJson = await assignment.json() as {
      assignments?: Array<{
        assignmentRef: string
        codingAssignment?: { budget?: { paymentMode?: string } }
        paymentMode?: string
      }>
    }
    await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.paid_agent/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.autopilot_paid_smoke.accepted'],
          accepted: true,
        },
        idempotencyKey: `accept-${assignmentRef}`,
        method: 'POST',
      },
    )
    await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.paid_agent/assignments/${assignmentRef}/closeout`,
      {
        body: {
          artifactRefs: ['artifact.public.autopilot_paid_smoke.patch_summary'],
          buildRefs: ['build.public.autopilot_paid_smoke.not_required'],
          closeoutRefs: ['closeout.public.autopilot_paid_smoke.worker_summary'],
          previewRefs: ['preview.public.autopilot_paid_smoke.not_required'],
          proofRefs: ['proof.public.autopilot_paid_smoke.worker_closeout'],
          resultRefs: ['result.public.autopilot_paid_smoke.delivered'],
          status: 'closeout_submitted',
          summaryRefs: ['summary.public.autopilot_paid_smoke.customer_safe'],
          testRefs: ['test.public.autopilot_paid_smoke.not_required'],
        },
        idempotencyKey: `closeout-${assignmentRef}`,
        method: 'POST',
        recordAutopilotWorkerCloseout: (_env, input) =>
          recordAutopilotWorkerCloseoutFromPylon(store, input),
      },
    )
    const detail = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const review = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public.autopilot_paid_smoke.customer_accepts'],
        },
        idempotencyKey: 'review-autopilot-coder-paid-smoke',
        method: 'POST',
        pylonApiStore,
      },
    )
    const events = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/events',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const detailJson = await responseJson(detail)
    const reviewJson = await responseJson(review)
    const eventsJson = await responseJson(events)
    const retainedProjection = JSON.stringify({
      assignment: assignmentJson,
      detail: detailJson,
      events: eventsJson,
      first: firstJson,
      paid: paidJson,
      review: reviewJson,
    })

    expect(first.status).toBe(402)
    expect(firstJson.work).toMatchObject({
      funding: {
        buyerFundingState: 'payment_required',
        settlementBlockedReasonRef: 'settlement.buyer_payment_required',
      },
      state: 'payment_required',
    })
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef: proofRef,
      funding: {
        buyerFundingState: 'funded',
        buyerPaymentProofRef: proofRef,
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      state: 'queued_or_running',
    })
    expect(assignmentJson.assignments).toEqual([
      expect.objectContaining({
        assignmentRef,
        codingAssignment: expect.objectContaining({
          budget: expect.objectContaining({
            paymentMode: 'buyer_funded',
            workerPayoutAuthority: false,
          }),
        }),
      }),
    ])
    expect(detailJson.work).toMatchObject({
      executionCloseout: {
        acceptedWorkAuthority: false,
        artifactRefs: ['artifact.public.autopilot_paid_smoke.patch_summary'],
        closeoutRefs: ['closeout.public.autopilot_paid_smoke.worker_summary'],
        forumAutoPublishAllowed: false,
        proofRefs: ['proof.public.autopilot_paid_smoke.worker_closeout'],
        publicSafe: true,
        resultRefs: ['result.public.autopilot_paid_smoke.delivered'],
        workerPayoutAuthority: false,
      },
      funding: {
        buyerFundingState: 'funded',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      state: 'delivered',
    })
    expect(review.status).toBe(201)
    expect(reviewJson.work).toMatchObject({
      reviewDecision: {
        action: 'accept',
        acceptedWorkAuthority: false,
        deployAuthority: false,
        forumAutoPublishAllowed: false,
        settlementAuthority: false,
        workerPayoutAuthority: false,
      },
      state: 'accepted',
    })
    expect(eventsJson.events).toEqual([
      expect.objectContaining({ eventKind: 'queued' }),
      expect.objectContaining({ eventKind: 'accepted' }),
    ])
    expect(retainedProjection).not.toMatch(
      retainedProjectionPrivateMaterialPattern,
    )
  })

  test('supports delivered-work request-changes and reject review decisions', async () => {
    const revision = await createDeliveredPylonBackedWork()
    const revisionReview = await route(
      revision.store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'request_changes',
          decisionRefs: ['review.public.customer_requests_changes'],
          revisionRequestRefs: ['revision.public.tighten_acceptance_tests'],
        },
        idempotencyKey: 'review-request-changes-autopilot-work',
        method: 'POST',
        pylonApiStore: revision.pylonApiStore,
      },
    )
    const rejected = await createDeliveredPylonBackedWork()
    const rejectReview = await route(
      rejected.store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'reject',
          decisionRefs: ['review.public.customer_rejects_closeout'],
          rejectionRefs: ['rejection.public.missing_required_artifact'],
        },
        idempotencyKey: 'review-reject-autopilot-work',
        method: 'POST',
        pylonApiStore: rejected.pylonApiStore,
      },
    )
    const revisionJson = await responseJson(revisionReview)
    const rejectJson = await responseJson(rejectReview)

    expect(revisionReview.status).toBe(201)
    expect(revisionJson.work).toMatchObject({
      nextAction: {
        reasonRefs: ['next_action.customer_requested_changes'],
        state: 'revision_required',
      },
      reviewDecision: {
        action: 'request_changes',
        revisionRequestRefs: ['revision.public.tighten_acceptance_tests'],
      },
      state: 'revision_required',
    })
    expect(rejectReview.status).toBe(201)
    expect(rejectJson.work).toMatchObject({
      nextAction: {
        reasonRefs: ['next_action.customer_rejected_work'],
        state: 'rejected',
      },
      reviewDecision: {
        action: 'reject',
        rejectionRefs: ['rejection.public.missing_required_artifact'],
      },
      state: 'rejected',
    })
  })

  test('rejects review before delivery, unsafe review refs, and callers without write scope', async () => {
    const pendingStore = new MemoryAutopilotWorkStore()
    const pendingPylonStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.docs_agent',
      }),
    ])

    await route(pendingStore, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-review-pending',
      pylonApiStore: pendingPylonStore,
    })

    const beforeDelivery = await route(
      pendingStore,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public.customer_accepts_delivered_refs'],
        },
        idempotencyKey: 'review-before-delivery',
        method: 'POST',
        pylonApiStore: pendingPylonStore,
      },
    )
    const delivered = await createDeliveredPylonBackedWork()
    const unsafe = await route(
      delivered.store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public./Users/christopher/raw'],
        },
        idempotencyKey: 'review-unsafe-ref',
        method: 'POST',
        pylonApiStore: delivered.pylonApiStore,
      },
    )
    const readOnly = await route(
      delivered.store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public.customer_accepts_delivered_refs'],
        },
        idempotencyKey: 'review-read-only-denied',
        method: 'POST',
        pylonApiStore: delivered.pylonApiStore,
        scopes: ['customer_orders.read'],
      },
    )
    const nonOwner = await route(
      delivered.store,
      '/api/autopilot/work/autopilot_work_order.test_1/review',
      {
        body: {
          action: 'accept',
          decisionRefs: ['review.public.customer_accepts_delivered_refs'],
        },
        idempotencyKey: 'review-non-owner-denied',
        method: 'POST',
        ownerUserId: 'github:different-owner',
        pylonApiStore: delivered.pylonApiStore,
      },
    )

    expect(beforeDelivery.status).toBe(409)
    expect(unsafe.status).toBe(400)
    expect(readOnly.status).toBe(401)
    expect(nonOwner.status).toBe(404)
  })

  test('returns actionable placement needs-input when no runner is available', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].placementPolicy,
        allowedRunnerKinds: ['requester_pylon'] as const,
        localOnlyAllowed: true,
        preferredRunnerKinds: ['requester_pylon'] as const,
        privacyTier: 'local_only' as const,
        publicTraceAllowed: false,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-placement-needs-input',
      pylonRegistrations: [],
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.funding?.buyerFundingState).toBe('not_required')
    expect(body.work?.placementDecision).toMatchObject({
      availabilityState: 'needs_input',
      callerActionRefs: [
        'caller.add_or_restart_pylon',
        'caller.relax_privacy_or_runner_policy',
      ],
      fallbackRunnerKind: null,
      refusalReasonRefs: [
        'placement.blocked.no_compatible_runner',
        'placement.blocked.local_only_without_eligible_pylon',
        'placement.blocked.no_pylon_candidates',
      ],
      retryAfterSeconds: null,
      selectedRunnerKind: null,
      source: 'none_available',
    })
    expect(body.work?.nextAction).toEqual({
      callerActionRefs: [
        'caller.add_or_restart_pylon',
        'caller.relax_privacy_or_runner_policy',
      ],
      reasonRefs: [
        'placement.blocked.no_compatible_runner',
        'placement.blocked.local_only_without_eligible_pylon',
        'placement.blocked.no_pylon_candidates',
      ],
      retryAfterSeconds: null,
      state: 'needs_input',
    })
    expect(body.work?.fallbackLeaseIntents).toEqual([])
    expect(body.work?.pylonAssignmentIntents).toEqual([])
  })

  test('allows public read-only repository tasks to proceed', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-public-read',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work).toMatchObject({
      accessRequirements: [],
      accessRequestRefs: [],
      funding: {
        buyerFundingState: 'not_required',
        fundedAmountCents: 0,
        settlementBlockedReasonRef: 'settlement.no_worker_payout_mode',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallengeRef: null,
      state: 'accepted_free_slice',
    })
    expect(body.work?.repositoryAuthorities).toEqual([
      expect.objectContaining({
        deployAuthority: false,
        fullName: 'OpenAgentsInc/openagents',
        pullRequestAuthority: 'not_requested',
        readAuthority: 'public_read_available',
        spendAuthority: false,
        taskRef: 'task.autopilot_coder.docs_contract',
        writeAuthority: 'not_requested',
      }),
    ])
  })

  test('returns the same deterministic quote across payment challenge proof retry and detail', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        quoteRef: null,
        quotedAmountCents: null,
      },
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-paid-quote',
    })
    const replay = await route(store, '/api/autopilot/work', {
      body: { ignored: 'idempotent replay does not replace stored request' },
      idempotencyKey: 'idem-autopilot-work-paid-quote',
    })
    const firstJson = await responseJson(first)
    const replayJson = await responseJson(replay)
    const paidProofRef = 'payment_proof.autopilot_work.test_1'
    verifiedAutopilotProofRefs.add(paidProofRef)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          first.headers.get('x-openagents-l402-credential'),
          paidProofRef,
        ),
      },
      idempotencyKey: 'idem-autopilot-work-paid-quote',
    })
    const paidJson = await responseJson(paid)
    const detail = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}`,
      { method: 'GET' },
    )
    const detailJson = await responseJson(detail)

    expect(first.status).toBe(402)
    expect(first.headers.get('www-authenticate')).toContain('L402')
    expect(first.headers.get('x-openagents-l402-credential')).toMatch(
      /^oa-l402-v1\./,
    )
    expect(decodeAutopilotL402Payload(
      first.headers.get('x-openagents-l402-credential') ?? '',
    )).toMatchObject({
      amount: {
        amountMinorUnits: 6400,
        asset: 'usd',
        denomination: 'usd_cent',
      },
      challengeRef:
        'challenge.quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
      endpointRef: 'endpoint.autopilot.work',
      productId: 'product.autopilot.work',
    })
    expect(replay.status).toBe(402)
    expect(firstJson.work).toMatchObject({
      assignmentIntents: [
        {
          assignmentKind: 'test_repair',
          plannerReasonRefs: ['assignment.blocked.payment_required'],
          plannerState: 'payment_required',
          readyForAssignment: false,
          taskRef: 'task.autopilot_coder.paid_test_repair',
        },
      ],
      funding: {
        buyerFundingState: 'payment_required',
        buyerPaymentProofRef: null,
        fundedAmountCents: 0,
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        settlementBlockedReasonRef: 'settlement.buyer_payment_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallenge: {
        amountCents: 6400,
        challengeRef:
          'challenge.quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        expiresAt: '2026-06-09T17:45:00.000Z',
        kind: 'l402',
        l402CredentialRef:
          'credential.autopilot_work.autopilot_work_order_test_1',
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        status: 'payment_required',
      },
      paymentChallengeRef:
        'challenge.quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
      quote: {
        amountCents: 6400,
        paymentRequired: true,
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
      },
      state: 'payment_required',
    })
    expect(firstJson.work?.fallbackLeaseIntents).toEqual([])
    expect(replayJson.work?.quote).toEqual(firstJson.work?.quote)
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      assignmentIntents: [
        {
          assignmentKind: 'test_repair',
          plannerReasonRefs: [
            'assignment.paid_ready',
            'assignment.ready_for_assignment',
          ],
          plannerState: 'paid_ready',
          readyForAssignment: true,
          taskRef: 'task.autopilot_coder.paid_test_repair',
        },
      ],
      buyerPaymentProofRef: 'payment_proof.autopilot_work.test_1',
      funding: {
        buyerFundingState: 'funded',
        buyerPaymentProofRef: 'payment_proof.autopilot_work.test_1',
        fundedAmountCents: 6400,
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallenge: {
        status: 'paid_ready',
      },
      quote: firstJson.work?.quote,
      state: 'paid_ready',
    })
    expect(paidJson.work?.fallbackLeaseIntents).toEqual([
      expect.objectContaining({
        assignmentRef:
          'fallback_assignment.autopilot_work_order.test_1.task.autopilot_coder.paid_test_repair',
        fallbackLaneRef: 'fallback_lane.openagents.shc',
        forumAutoPublishAllowed: false,
        paymentMode: 'buyer_funded',
        requiredCapabilityRefs: [
          'capability.fallback.assignment_ready',
          'capability.openagents.shc',
        ],
        runnerKind: 'openagents_shc',
        spendCapRefs: ['spend_cap.buyer_funded.fallback_assignment'],
        taskRef: 'task.autopilot_coder.paid_test_repair',
        workerPayoutAuthority: false,
      }),
    ])
    expect(detailJson.work?.quote).toEqual(firstJson.work?.quote)
    expect(detailJson.work?.buyerPaymentProofRef).toBe(
      'payment_proof.autopilot_work.test_1',
    )
    expect(detailJson.work?.funding).toEqual(paidJson.work?.funding)
    expect(detailJson.work?.paymentChallengeRef).toBe(
      firstJson.work?.paymentChallengeRef,
    )
  })

  test('persists L402 challenges and verifies paid retries through the buyer payment ledger', async () => {
    const store = new MemoryAutopilotWorkStore()
    const buyerPaymentLedgerStore = new MemoryBuyerPaymentLedgerStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        quoteRef: null,
        quotedAmountCents: null,
      },
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      buyerPaymentLedgerStore,
      idempotencyKey: 'idem-autopilot-work-ledger-paid-quote',
      verifyL402PaymentProof: input =>
        verifyAutopilotL402PaymentProofFromBuyerLedger(
          buyerPaymentLedgerStore,
          input,
        ),
    })
    const credential = first.headers.get('x-openagents-l402-credential')
    const credentialPayload = decodeAutopilotL402Payload(credential ?? '')
    const challenge = buyerPaymentLedgerStore.challenges.get(
      credentialPayload.challengeRef,
    )
    const proofRef = 'payment_proof.autopilot_work.ledger_matched'
    const unredeemedRetry = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      buyerPaymentLedgerStore,
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(credential, proofRef),
      },
      idempotencyKey: 'idem-autopilot-work-ledger-paid-quote',
      verifyL402PaymentProof: input =>
        verifyAutopilotL402PaymentProofFromBuyerLedger(
          buyerPaymentLedgerStore,
          input,
        ),
    })

    expect(first.status).toBe(402)
    expect(challenge).toMatchObject({
      challengeRef: credentialPayload.challengeRef,
      path: '/api/autopilot/work',
      price: {
        amountMinorUnits: 6400,
        asset: 'usd',
        denomination: 'usd_cent',
      },
      productId: 'product.autopilot.work',
      status: 'issued',
      surface: 'agent_api',
    })
    expect(unredeemedRetry.status).toBe(400)

    await recordMatchedAutopilotLedgerPayment(buyerPaymentLedgerStore, {
      credential,
      proofRef,
    })

    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      buyerPaymentLedgerStore,
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(credential, proofRef),
      },
      idempotencyKey: 'idem-autopilot-work-ledger-paid-quote',
      verifyL402PaymentProof: input =>
        verifyAutopilotL402PaymentProofFromBuyerLedger(
          buyerPaymentLedgerStore,
          input,
        ),
    })
    const paidJson = await responseJson(paid)

    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef: proofRef,
      funding: {
        buyerFundingState: 'funded',
        buyerPaymentProofRef: proofRef,
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      state: 'paid_ready',
    })
  })

  test('rejects malformed, unverified, expired, mismatched, and replayed L402 retries', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        quoteRef: null,
        quotedAmountCents: null,
      },
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-l402-negative',
    })
    const credential = first.headers.get('x-openagents-l402-credential')
    const unpaidRetry = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      idempotencyKey: 'idem-autopilot-work-l402-negative',
    })
    const malformedRetry = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      headers: {
        'X-OpenAgents-L402': 'not-a-real-credential:payment_proof.autopilot_work.negative',
      },
      idempotencyKey: 'idem-autopilot-work-l402-negative',
    })
    const unverifiedRetry = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          credential,
          'payment_proof.autopilot_work.unverified',
        ),
      },
      idempotencyKey: 'idem-autopilot-work-l402-negative',
    })
    const expiredRetry = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          credential,
          'payment_proof.autopilot_work.expired',
        ),
      },
      idempotencyKey: 'idem-autopilot-work-l402-negative',
      nowIso: '2026-06-09T17:46:00.000Z',
    })
    const secondStore = new MemoryAutopilotWorkStore()
    const second = await route(secondStore, '/api/autopilot/work', {
      body: {
        ...request,
        clientRequestRef: 'client.example.20260609.mismatch',
      },
      idempotencyKey: 'idem-autopilot-work-l402-mismatch',
    })
    const verifiedMismatchProof = 'payment_proof.autopilot_work.mismatch'
    verifiedAutopilotProofRefs.add(verifiedMismatchProof)
    const mismatchedCredentialRetry = await route(secondStore, '/api/autopilot/work', {
      body: { ignored: true },
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          credential,
          verifiedMismatchProof,
        ),
      },
      idempotencyKey: 'idem-autopilot-work-l402-mismatch',
    })
    const verifiedProofRef = 'payment_proof.autopilot_work.negative'
    verifiedAutopilotProofRefs.add(verifiedProofRef)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(credential, verifiedProofRef),
      },
      idempotencyKey: 'idem-autopilot-work-l402-negative',
    })
    const replayedPaid = await route(store, '/api/autopilot/work', {
      body: { ignored: true },
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(credential, verifiedProofRef),
      },
      idempotencyKey: 'idem-autopilot-work-l402-negative',
    })
    const paidJson = await responseJson(paid)
    const replayedPaidJson = await responseJson(replayedPaid)

    expect(first.status).toBe(402)
    expect(unpaidRetry.status).toBe(402)
    expect(malformedRetry.status).toBe(402)
    expect(unverifiedRetry.status).toBe(400)
    expect(await responseJson(unverifiedRetry)).toMatchObject({
      error: 'autopilot_work_validation_error',
      reason: 'Autopilot L402 payment proof was not verified.',
    })
    expect(expiredRetry.status).toBe(400)
    expect(await responseJson(expiredRetry)).toMatchObject({
      error: 'autopilot_work_validation_error',
      reason: 'reason.l402_credential.expired',
    })
    expect(second.status).toBe(402)
    expect(mismatchedCredentialRetry.status).toBe(400)
    expect(await responseJson(mismatchedCredentialRetry)).toMatchObject({
      error: 'autopilot_work_validation_error',
      reason: 'reason.l402_credential.resource_mismatch',
    })
    expect(paid.status).toBe(200)
    expect(replayedPaid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef: verifiedProofRef,
      funding: {
        buyerFundingState: 'funded',
        buyerPaymentProofRef: verifiedProofRef,
      },
      state: 'paid_ready',
    })
    expect(replayedPaidJson.work?.buyerPaymentProofRef).toBe(verifiedProofRef)
  })

  test('projects a funded hosted Gemini fallback lease without execution authority', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      clientRequestRef: 'client.example.20260609.hosted_gemini_smoke',
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        maxSpendCents: 5000,
        quoteRef: null,
        quotedAmountCents: null,
      },
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
        allowedRunnerKinds: ['hosted_gemini'] as const,
        preferredRunnerKinds: ['hosted_gemini'] as const,
        privacyTier: 'cloud_allowed' as const,
        publicTraceAllowed: true,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].tasks[0],
          acceptanceCriteriaRefs: [
            'acceptance.audit.updated_with_hosted_gemini_smoke_result',
          ],
          kind: 'research_and_patch' as const,
          objective:
            'Audit the red hosted Gemini product promise and return a public-safe documentation patch.',
          taskRef: 'task.product_promise_docs_hosted_gemini_smoke',
        },
      ],
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-smoke',
      pylonRegistrations: [],
    })
    const firstJson = await responseJson(first)
    const paidProofRef =
      'payment_proof.autopilot_work.hosted_gemini_smoke'
    verifiedAutopilotProofRefs.add(paidProofRef)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          first.headers.get('x-openagents-l402-credential'),
          paidProofRef,
        ),
      },
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-smoke',
      pylonRegistrations: [],
    })
    const paidJson = await responseJson(paid)

    expect(first.status).toBe(402)
    expect(firstJson.work).toMatchObject({
      fallbackLeaseIntents: [],
      funding: {
        buyerFundingState: 'payment_required',
        settlementBlockedReasonRef: 'settlement.buyer_payment_required',
        workerPayoutEligible: false,
      },
      placementDecision: {
        selectedRunnerKind: 'hosted_gemini',
        source: 'fallback',
      },
      quote: {
        amountCents: 3700,
        paymentRequired: true,
      },
      state: 'payment_required',
    })
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef:
        'payment_proof.autopilot_work.hosted_gemini_smoke',
      funding: {
        buyerFundingState: 'funded',
        fundedAmountCents: 3700,
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      nextAction: {
        callerActionRefs: [],
        state: 'ready',
      },
      placementDecision: {
        selectedRunnerKind: 'hosted_gemini',
        source: 'fallback',
      },
      state: 'paid_ready',
    })
    expect(paidJson.work?.fallbackLeaseIntents).toEqual([
      expect.objectContaining({
        assignmentRef:
          'fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_smoke',
        fallbackLaneRef: 'fallback_lane.openagents.hosted_gemini',
        forumAutoPublishAllowed: false,
        paymentMode: 'buyer_funded',
        requiredCapabilityRefs: [
          'capability.fallback.assignment_ready',
          'capability.openagents.hosted_gemini',
        ],
        runnerKind: 'hosted_gemini',
        spendCapRefs: ['spend_cap.buyer_funded.fallback_assignment'],
        taskRef: 'task.product_promise_docs_hosted_gemini_smoke',
        workerPayoutAuthority: false,
      }),
    ])
    expect(paidJson.work?.pylonAssignmentIntents).toEqual([])
    expect(paidJson.work?.executionCloseout).toBeNull()
  })

  test('delivers a paid hosted Gemini work order through the execution closeout bridge', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      clientRequestRef: 'client.example.20260609.hosted_gemini_closeout',
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        maxSpendCents: 5000,
        quoteRef: null,
        quotedAmountCents: null,
      },
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
        allowedRunnerKinds: ['hosted_gemini'] as const,
        preferredRunnerKinds: ['hosted_gemini'] as const,
        privacyTier: 'cloud_allowed' as const,
        publicTraceAllowed: true,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].tasks[0],
          acceptanceCriteriaRefs: [
            'acceptance.audit.updated_with_hosted_gemini_closeout',
          ],
          kind: 'research_and_patch' as const,
          objective:
            'Audit the hosted Gemini product promise and return a public-safe closeout.',
          taskRef: 'task.product_promise_docs_hosted_gemini_closeout',
        },
      ],
    }
    const executeReadyWork: AutopilotWorkExecutor = async ({ work }) => ({
      assignmentRefs: work.fallbackLeaseIntents.map(
        intent => intent.assignmentRef,
      ),
      closeoutRefs: work.fallbackLeaseIntents.flatMap(intent => [
        `closeout.${intent.assignmentRef}.public_safe_summary_delivered`,
        `closeout.${intent.assignmentRef}.tests_or_blocker_retained`,
      ]),
      proofRefs: work.fallbackLeaseIntents.map(
        intent => `proof.${intent.assignmentRef}.route_harness`,
      ),
      resultRefs: work.fallbackLeaseIntents.flatMap(
        intent => intent.resultExpectationRefs,
      ),
      runnerKind: 'hosted_gemini',
    })
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      executeReadyWork,
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-closeout',
      pylonRegistrations: [],
    })
    const firstJson = await responseJson(first)
    const paidProofRef =
      'payment_proof.autopilot_work.hosted_gemini_closeout'
    verifiedAutopilotProofRefs.add(paidProofRef)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      executeReadyWork,
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          first.headers.get('x-openagents-l402-credential'),
          paidProofRef,
        ),
      },
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-closeout',
      pylonRegistrations: [],
    })
    const paidJson = await responseJson(paid)
    const detail = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}`,
      { method: 'GET' },
    )
    const detailJson = await responseJson(detail)
    const events = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}/events`,
      { method: 'GET' },
    )
    const eventsJson = await responseJson(events)

    expect(first.status).toBe(402)
    expect(firstJson.work?.state).toBe('payment_required')
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef:
        'payment_proof.autopilot_work.hosted_gemini_closeout',
      executionCloseout: {
        acceptedWorkAuthority: false,
        assignmentRefs: [
          'fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout',
        ],
        closeoutRefs: [
          'closeout.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.public_safe_summary_delivered',
          'closeout.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.tests_or_blocker_retained',
        ],
        forumAutoPublishAllowed: false,
        proofRefs: [
          'proof.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.route_harness',
        ],
        publicSafe: true,
        resultRefs: [
          'result.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.public_safe_closeout',
        ],
        runnerKind: 'hosted_gemini',
        workerPayoutAuthority: false,
      },
      funding: {
        buyerFundingState: 'funded',
        fundedAmountCents: 3700,
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      nextAction: {
        callerActionRefs: ['caller.review_autopilot_closeout'],
        reasonRefs: ['next_action.review_delivered_work'],
        retryAfterSeconds: null,
        state: 'delivered',
      },
      paymentChallenge: {
        status: 'paid_ready',
      },
      placementDecision: {
        selectedRunnerKind: 'hosted_gemini',
        source: 'fallback',
      },
      state: 'delivered',
      tasks: [
        {
          lifecycleState: 'delivered',
          placementState: 'delivered',
          taskRef: 'task.product_promise_docs_hosted_gemini_closeout',
        },
      ],
    })
    expect(paidJson.work?.assignmentIntents).toEqual([
      expect.objectContaining({
        plannerReasonRefs: ['assignment.delivered'],
        plannerState: 'delivered',
        readyForAssignment: false,
      }),
    ])
    expect(paidJson.work?.fallbackLeaseIntents).toEqual([])
    expect(detail.status).toBe(200)
    expect(detailJson.work?.executionCloseout).toEqual(
      paidJson.work?.executionCloseout,
    )
    expect(events.status).toBe(200)
    expect(eventsJson.events).toEqual([
      expect.objectContaining({
        eventKind: 'queued',
        publicSafe: true,
        sequence: 1,
      }),
      expect.objectContaining({
        eventKind: 'delivered',
        publicSafe: true,
        sequence: 2,
      }),
    ])
  })

  const hostedGeminiClosateRequest = () => ({
    ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
    clientRequestRef: 'client.example.20260620.hosted_gemini_binding',
    paymentPolicy: {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
      maxSpendCents: 5000,
      quoteRef: null,
      quotedAmountCents: null,
    },
    placementPolicy: {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
      allowedRunnerKinds: ['hosted_gemini'] as const,
      preferredRunnerKinds: ['hosted_gemini'] as const,
      privacyTier: 'cloud_allowed' as const,
      publicTraceAllowed: true,
    },
    tasks: [
      {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].tasks[0],
        acceptanceCriteriaRefs: [
          'acceptance.audit.updated_with_hosted_gemini_binding',
        ],
        kind: 'research_and_patch' as const,
        objective:
          'Audit the hosted Gemini product promise and return a public-safe closeout.',
        taskRef: 'task.product_promise_docs_hosted_gemini_binding',
      },
    ],
  })

  const driveHostedGeminiBinding = async (
    store: MemoryAutopilotWorkStore,
    executeReadyWork: AutopilotWorkExecutor,
    idempotencyKey: string,
    paidProofRef: string,
  ) => {
    const first = await route(store, '/api/autopilot/work', {
      body: hostedGeminiClosateRequest(),
      executeReadyWork,
      idempotencyKey,
      pylonRegistrations: [],
    })
    verifiedAutopilotProofRefs.add(paidProofRef)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      executeReadyWork,
      headers: {
        'X-OpenAgents-L402': authorizeAutopilotL402(
          first.headers.get('x-openagents-l402-credential'),
          paidProofRef,
        ),
      },
      idempotencyKey,
      pylonRegistrations: [],
    })
    return { paid, paidJson: await responseJson(paid) }
  }

  test('production hosted Gemini executor binding delivers a paid work order through the route harness when armed', async () => {
    const store = new MemoryAutopilotWorkStore()
    const inferenceCaller: HostedGeminiInferenceCaller = async ({
      assignmentRef,
    }) => ({
      modelRef: 'model.hosted_gemini.gemini-2.5-flash',
      responseDigestRef: `digest.${assignmentRef}.sha256.deadbeef`,
      usageRef: `usage.${assignmentRef}.io_units`,
    })
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      createHostedGeminiWorkExecutor({ enabled: true, inferenceCaller }),
      'idem-autopilot-work-hosted-gemini-binding',
      'payment_proof.autopilot_work.hosted_gemini_binding',
    )

    const assignmentRef =
      'fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_binding'
    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('delivered')
    expect(paidJson.work?.executionCloseout).toMatchObject({
      assignmentRefs: [assignmentRef],
      proofRefs: [
        `proof.${assignmentRef}.hosted_gemini_executor`,
        'model.hosted_gemini.gemini-2.5-flash',
        `digest.${assignmentRef}.sha256.deadbeef`,
      ],
      publicSafe: true,
      runnerKind: 'hosted_gemini',
      summaryRefs: [`summary.${assignmentRef}.hosted_gemini_closeout`],
      verificationRefs: [`usage.${assignmentRef}.io_units`],
    })
  })

  test('production hosted Gemini executor binding stays INERT (no delivery) when not armed', async () => {
    const store = new MemoryAutopilotWorkStore()
    const inferenceCaller: HostedGeminiInferenceCaller = async () => {
      throw new Error('inert binding must never call the provider')
    }
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      createHostedGeminiWorkExecutor({ enabled: false, inferenceCaller }),
      'idem-autopilot-work-hosted-gemini-inert',
      'payment_proof.autopilot_work.hosted_gemini_inert',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('paid_ready')
    expect(paidJson.work?.executionCloseout).toBeNull()
  })

  test('production hosted Gemini executor binding refuses to deliver when an inference ref is not public-safe', async () => {
    const store = new MemoryAutopilotWorkStore()
    const inferenceCaller: HostedGeminiInferenceCaller = async ({
      assignmentRef,
    }) => ({
      modelRef: 'model.hosted_gemini.gemini-2.5-flash',
      responseDigestRef: `digest.${assignmentRef}.access_token.leak`,
    })
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      createHostedGeminiWorkExecutor({ enabled: true, inferenceCaller }),
      'idem-autopilot-work-hosted-gemini-unsafe',
      'payment_proof.autopilot_work.hosted_gemini_unsafe',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('paid_ready')
    expect(paidJson.work?.executionCloseout).toBeNull()
  })

  // A spy provider adapter for the composed binding: `complete` succeeds with a
  // fixed receipt-first result and records the request it was handed.
  const hostedGeminiSpyAdapter = (
    result: InferenceResult,
  ): {
    adapter: InferenceProviderAdapter
    requests: Array<InferenceRequest>
  } => {
    const requests: Array<InferenceRequest> = []
    return {
      adapter: {
        complete: (request: InferenceRequest) => {
          requests.push(request)
          return Effect.succeed(result)
        },
        id: 'vertex-gemini',
        stream: () => Effect.succeed([]),
      },
      requests,
    }
  }

  test('composed hosted Gemini binding delivers a paid work order end-to-end from a single injected adapter when armed', async () => {
    const store = new MemoryAutopilotWorkStore()
    const { adapter, requests } = hostedGeminiSpyAdapter({
      content: 'public-safe hosted Gemini closeout summary',
      finishReason: 'stop',
      servedModel: 'gemini-3.5-flash',
      usage: { completionTokens: 7, promptTokens: 11, totalTokens: 18 },
    })
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      createHostedGeminiExecutorBinding({ adapter, enabled: true }),
      'idem-autopilot-work-hosted-gemini-composed',
      'payment_proof.autopilot_work.hosted_gemini_composed',
    )

    const assignmentRef =
      'fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_binding'
    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('delivered')
    expect(paidJson.work?.executionCloseout).toMatchObject({
      assignmentRefs: [assignmentRef],
      publicSafe: true,
      runnerKind: 'hosted_gemini',
      verificationRefs: ['usage.hosted_gemini.prompt_11.completion_7.total_18'],
    })
    // The closeout proof refs are PROJECTED from the real adapter result: the
    // served model and a SHA-256 digest of the completion (never the raw text).
    const proofRefs = paidJson.work?.executionCloseout?.proofRefs ?? []
    expect(proofRefs).toContain('model.hosted_gemini.gemini-3.5-flash')
    expect(
      proofRefs.some(ref =>
        /^proof\.hosted_gemini\.response_digest\.sha256\.[0-9a-f]{64}$/u.test(ref),
      ),
    ).toBe(true)
    // The raw completion text never appears in any persisted ref.
    expect(JSON.stringify(paidJson.work?.executionCloseout)).not.toContain(
      'public-safe hosted Gemini closeout summary',
    )
    // The request the adapter saw is non-streaming and refs-only.
    expect(requests).toHaveLength(1)
    expect(requests[0]?.stream).toBe(false)
    expect(requests[0]?.messages[1]?.content).toContain(
      'task=task.product_promise_docs_hosted_gemini_binding',
    )
  })

  test('composed hosted Gemini binding threads an injected ref-resolver into the adapter prompt', async () => {
    const store = new MemoryAutopilotWorkStore()
    const { adapter, requests } = hostedGeminiSpyAdapter({
      content: 'public-safe hosted Gemini closeout summary',
      finishReason: 'stop',
      servedModel: 'gemini-3.5-flash',
      usage: { completionTokens: 7, promptTokens: 11, totalTokens: 18 },
    })
    const resolvedRefs: Array<string> = []
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      createHostedGeminiExecutorBinding({
        adapter,
        enabled: true,
        // A datastore-backed resolver shape: dereference the task ref to real,
        // public-safe content the live adapter should act on.
        resolveRefContent: async (ref: string) => {
          resolvedRefs.push(ref)
          return ref === 'task.product_promise_docs_hosted_gemini_binding'
            ? 'Document the hosted Gemini binding seam in the launch worklog.'
            : undefined
        },
      }),
      'idem-autopilot-work-hosted-gemini-composed-resolver',
      'payment_proof.autopilot_work.hosted_gemini_composed_resolver',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('delivered')
    // The injected resolver was consulted for the task ref...
    expect(resolvedRefs).toContain(
      'task.product_promise_docs_hosted_gemini_binding',
    )
    // ...and its resolved (public-safe) content reached the adapter prompt,
    // proving the resolver threads through the single composition root rather
    // than the request staying refs-only.
    expect(requests).toHaveLength(1)
    const userContent = requests[0]?.messages[1]?.content ?? ''
    expect(userContent).toContain(
      'task_content: Document the hosted Gemini binding seam in the launch worklog.',
    )
  })

  test('composed hosted Gemini binding stays INERT (no delivery, adapter untouched) when the single flag is off', async () => {
    const store = new MemoryAutopilotWorkStore()
    const { adapter, requests } = hostedGeminiSpyAdapter({
      content: 'must never run',
      finishReason: 'stop',
      servedModel: 'gemini-3.5-flash',
      usage: { completionTokens: 1, promptTokens: 1, totalTokens: 2 },
    })
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      createHostedGeminiExecutorBinding({ adapter, enabled: false }),
      'idem-autopilot-work-hosted-gemini-composed-inert',
      'payment_proof.autopilot_work.hosted_gemini_composed_inert',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('paid_ready')
    expect(paidJson.work?.executionCloseout).toBeNull()
    expect(requests).toHaveLength(0)
  })

  test('composed hosted Gemini binding declines to deliver when the provider adapter fails', async () => {
    const store = new MemoryAutopilotWorkStore()
    const adapter: InferenceProviderAdapter = {
      complete: () =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: 'vertex-gemini',
            reason: 'quota exhausted',
            retryable: true,
          }),
        ),
      id: 'vertex-gemini',
      stream: () =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: 'vertex-gemini',
            reason: 'quota exhausted',
            retryable: true,
          }),
        ),
    }
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      createHostedGeminiExecutorBinding({ adapter, enabled: true }),
      'idem-autopilot-work-hosted-gemini-composed-fail',
      'payment_proof.autopilot_work.hosted_gemini_composed_fail',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('paid_ready')
    expect(paidJson.work?.executionCloseout).toBeNull()
  })

  test('env-gated hosted Gemini executor delivers end-to-end through the route harness when the env is armed', async () => {
    const store = new MemoryAutopilotWorkStore()
    const { adapter, requests } = hostedGeminiSpyAdapter({
      content: 'public-safe hosted Gemini closeout summary',
      finishReason: 'stop',
      servedModel: 'gemini-3.5-flash',
      usage: { completionTokens: 7, promptTokens: 11, totalTokens: 18 },
    })
    // Drive the SAME env seam that index.ts wires, with the live env armed
    // (flag on + secret present) and a spy adapter injected for the test.
    const resolve = makeHostedGeminiExecuteReadyWork({ buildAdapter: () => adapter })
    const armedEnv = {
      HOSTED_GEMINI_EXECUTOR_ENABLED: 'true',
      VERTEX_SA_KEY: '{"client_email":"x"}',
    }
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      input => resolve(armedEnv, input),
      'idem-autopilot-work-hosted-gemini-env-armed',
      'payment_proof.autopilot_work.hosted_gemini_env_armed',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('delivered')
    expect(paidJson.work?.executionCloseout).toMatchObject({
      publicSafe: true,
      runnerKind: 'hosted_gemini',
    })
    expect(requests).toHaveLength(1)
    expect(requests[0]?.stream).toBe(false)
  })

  test('env-gated hosted Gemini executor threads an injected ref-resolver into the adapter prompt when armed', async () => {
    const store = new MemoryAutopilotWorkStore()
    const { adapter, requests } = hostedGeminiSpyAdapter({
      content: 'public-safe hosted Gemini closeout summary',
      finishReason: 'stop',
      servedModel: 'gemini-3.5-flash',
      usage: { completionTokens: 7, promptTokens: 11, totalTokens: 18 },
    })
    // A deployment provisions a datastore-backed resolver via the env seam deps.
    const resolve = makeHostedGeminiExecuteReadyWork({
      buildAdapter: () => adapter,
      resolveRefContent: async (ref: string) =>
        ref === 'task.product_promise_docs_hosted_gemini_binding'
          ? 'Resolve the env-seam task content for the live adapter.'
          : undefined,
    })
    const armedEnv = {
      HOSTED_GEMINI_EXECUTOR_ENABLED: 'true',
      VERTEX_SA_KEY: '{"client_email":"x"}',
    }
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      input => resolve(armedEnv, input),
      'idem-autopilot-work-hosted-gemini-env-resolver',
      'payment_proof.autopilot_work.hosted_gemini_env_resolver',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('delivered')
    expect(requests).toHaveLength(1)
    expect(requests[0]?.messages[1]?.content ?? '').toContain(
      'task_content: Resolve the env-seam task content for the live adapter.',
    )
  })

  test('env-gated hosted Gemini executor stays INERT (no delivery, adapter untouched) when the flag is off', async () => {
    const store = new MemoryAutopilotWorkStore()
    const { adapter, requests } = hostedGeminiSpyAdapter({
      content: 'must never run',
      finishReason: 'stop',
      servedModel: 'gemini-3.5-flash',
      usage: { completionTokens: 1, promptTokens: 1, totalTokens: 2 },
    })
    const resolve = makeHostedGeminiExecuteReadyWork({ buildAdapter: () => adapter })
    // Secret present but the flag is OFF: the seam stays INERT (the prod default).
    const inertEnv = { VERTEX_SA_KEY: '{"client_email":"x"}' }
    const { paid, paidJson } = await driveHostedGeminiBinding(
      store,
      input => resolve(inertEnv, input),
      'idem-autopilot-work-hosted-gemini-env-inert',
      'payment_proof.autopilot_work.hosted_gemini_env_inert',
    )

    expect(paid.status).toBe(200)
    expect(paidJson.work?.state).toBe('paid_ready')
    expect(paidJson.work?.executionCloseout).toBeNull()
    expect(requests).toHaveLength(0)
  })

  test('keeps MDK checkout proof retries payment-required until checkout verification is wired', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        buyerPaymentMode: 'mdk_checkout' as const,
        quoteRef: null,
        quotedAmountCents: null,
      },
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-mdk-checkout',
    })
    const firstJson = await responseJson(first)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      headers: {
        'X-OpenAgents-MDK-Checkout-Proof':
          'checkout_proof.autopilot_work.test_1',
      },
      idempotencyKey: 'idem-autopilot-work-mdk-checkout',
    })
    const paidJson = await responseJson(paid)

    expect(first.status).toBe(402)
    expect(first.headers.get('www-authenticate')).toBeNull()
    expect(firstJson.work?.paymentChallenge).toMatchObject({
      amountCents: 6400,
      checkoutIntentRef:
        'checkout_intent.quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
      kind: 'mdk_checkout',
      status: 'payment_required',
    })
    expect(firstJson.work?.funding).toMatchObject({
      buyerFundingState: 'payment_required',
      buyerPaymentProofRef: null,
      fundedAmountCents: 0,
      settlementBlockedReasonRef: 'settlement.buyer_payment_required',
      settlementEligible: false,
      workerPayoutEligible: false,
    })
    expect(paid.status).toBe(402)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef: null,
      funding: {
        buyerFundingState: 'payment_required',
        fundedAmountCents: 0,
        settlementBlockedReasonRef: 'settlement.buyer_payment_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallenge: {
        kind: 'mdk_checkout',
        status: 'payment_required',
      },
      state: 'payment_required',
    })
  })

  test('returns exact structured access requirements before launch', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [
            {
              kind: 'github_account_link',
              reasonRef: 'access.github.account_link',
            },
            {
              kind: 'repository_selection',
              reasonRef: 'access.repository.selection',
            },
            {
              kind: 'github_repo_write',
              reasonRef: 'access.github.repo_write',
            },
            {
              kind: 'pylon_enrollment',
              reasonRef: 'access.pylon.enrollment',
            },
            {
              kind: 'secret_broker',
              reasonRef: 'access.broker.required',
            },
            {
              kind: 'privacy_tier_confirmation',
              reasonRef: 'access.privacy.confirmation',
            },
            {
              kind: 'customer_review',
              reasonRef: 'access.customer.review',
            },
            {
              kind: 'operator_review',
              reasonRef: 'access.operator.review',
            },
          ],
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-access-required',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.state).toBe('access_required')
    expect(body.work?.accessRequirements).toEqual([
      expect.objectContaining({
        accessRequestRef:
          'access_request.task.autopilot_coder.docs_contract.github_account_link',
        grantAction: 'connect_github_account',
        kind: 'github_account_link',
        reasonRef: 'access.github.account_link',
        requiredBeforeLaunch: true,
        status: 'missing',
        taskRef: 'task.autopilot_coder.docs_contract',
      }),
      expect.objectContaining({
        grantAction: 'select_repository',
        kind: 'repository_selection',
        reasonRef: 'access.repository.selection',
      }),
      expect.objectContaining({
        grantAction: 'connect_github_repository',
        kind: 'github_repo_write',
      }),
      expect.objectContaining({
        grantAction: 'enroll_pylon',
        kind: 'pylon_enrollment',
      }),
      expect.objectContaining({
        grantAction: 'configure_secret_broker',
        kind: 'secret_broker',
      }),
      expect.objectContaining({
        grantAction: 'confirm_privacy_tier',
        kind: 'privacy_tier_confirmation',
      }),
      expect.objectContaining({
        grantAction: 'customer_review',
        kind: 'customer_review',
      }),
      expect.objectContaining({
        grantAction: 'operator_review',
        kind: 'operator_review',
      }),
    ])
    expect(body.work?.paymentChallengeRef).toBeNull()
  })

  test('blocks branch and pull request work until owner approval', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [
            {
              kind: 'github_branch_write',
              reasonRef: 'access.github.branch_write',
            },
            {
              kind: 'github_pull_request',
              reasonRef: 'access.github.pull_request',
            },
          ],
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-branch-pr',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.state).toBe('access_required')
    expect(body.work?.accessRequirements).toEqual([
      expect.objectContaining({
        grantAction: 'authorize_github_branch',
        kind: 'github_branch_write',
        requiredBeforeLaunch: true,
        status: 'missing',
      }),
      expect.objectContaining({
        grantAction: 'authorize_github_pull_request',
        kind: 'github_pull_request',
        requiredBeforeLaunch: true,
        status: 'missing',
      }),
    ])
    expect(body.work?.repositoryAuthorities).toEqual([
      expect.objectContaining({
        deployAuthority: false,
        pullRequestAuthority: 'owner_grant_required',
        readAuthority: 'public_read_available',
        spendAuthority: false,
        writeAuthority: 'owner_grant_required',
      }),
    ])
  })

  test('requires a registered agent grant for create and read', async () => {
    const create = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work',
      {
        body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        idempotencyKey: 'idem-autopilot-work-unauthorized',
        token: '',
      },
    )
    const read = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        token: '',
      },
    )

    expect(create.status).toBe(401)
    expect(read.status).toBe(401)
  })

  test('requires read scope for detail recovery', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            accessRequests: [],
          },
        ],
      },
      idempotencyKey: 'idem-autopilot-work-read-scope',
    })
    const createJson = await responseJson(create)
    const read = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}`,
      {
        method: 'GET',
        scopes: ['customer_orders.write'],
      },
    )

    expect(read.status).toBe(401)
  })

  test('returns pollable work events without internal operator logs', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            accessRequests: [
              {
                kind: 'github_repo_write',
                reasonRef: 'access.github.repo_write',
              },
            ],
          },
        ],
      },
      idempotencyKey: 'idem-autopilot-work-events',
    })
    const createJson = await responseJson(create)
    const events = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}/events`,
      { method: 'GET' },
    )
    const eventsJson = await responseJson(events)

    expect(events.status).toBe(200)
    expect(eventsJson.nextAfter).toBe(2)
    expect(eventsJson.events).toEqual([
      expect.objectContaining({
        eventKind: 'queued',
        publicSafe: true,
        sequence: 1,
        taskRefs: ['task.autopilot_coder.docs_contract'],
        workOrderRef: 'autopilot_work_order.test_1',
      }),
      expect.objectContaining({
        eventKind: 'needs_access',
        publicSafe: true,
        sequence: 2,
        taskRefs: ['task.autopilot_coder.docs_contract'],
        workOrderRef: 'autopilot_work_order.test_1',
      }),
    ])
  })

  test('supports event cursors and server-sent event formatting', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            accessRequests: [
              {
                kind: 'github_repo_write',
                reasonRef: 'access.github.repo_write',
              },
            ],
          },
        ],
      },
      idempotencyKey: 'idem-autopilot-work-event-stream',
    })
    const createJson = await responseJson(create)
    const events = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}/events?after=1`,
      {
        headers: { accept: 'text/event-stream' },
        method: 'GET',
      },
    )
    const body = await events.text()

    expect(events.status).toBe(200)
    expect(events.headers.get('content-type')).toContain('text/event-stream')
    expect(body).toContain('id: 2')
    expect(body).toContain('event: needs_access')
    expect(body).not.toContain('id: 1')
  })

  test('requires read scope for work events', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-events-scope',
    })
    const createJson = await responseJson(create)
    const events = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}/events`,
      {
        method: 'GET',
        scopes: ['customer_orders.write'],
      },
    )

    expect(events.status).toBe(401)
  })
})

describe('Autopilot scheduled launches (M6)', () => {
  const scheduledLaunchFixture = (launchAt: string, windowMinutes?: number) => ({
    ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
    launchPolicy: {
      kind: 'scheduled' as const,
      launchAt,
      ...(windowMinutes === undefined
        ? {}
        : { launchWindowMinutes: windowMinutes }),
    },
  })

  const scheduledJson = async (response: Response) =>
    response.json() as Promise<Readonly<{
      error?: string
      reason?: string
      work?: Readonly<{
        nextAction?: Readonly<{
          reasonRefs: ReadonlyArray<string>
          retryAfterSeconds: number | null
          state: string
        }>
        pylonAssignmentIntents?: ReadonlyArray<unknown>
        scheduledLaunch?: Readonly<{
          dispatchedAt: string | null
          expiredAt: string | null
          launchAt: string
          launchState: string
          windowMinutes: number
        }> | null
        state?: string
        workOrderRef?: string
      }>
    }>>

  let scheduledDispatchCounter = 0

  const runScheduledDispatch = (
    store: MemoryAutopilotWorkStore,
    pylonApiStore: PylonApiStore,
    nowIso: string,
  ) =>
    Effect.runPromise(
      dispatchDueScheduledAutopilotWork<Record<string, unknown>>(
        {
          agentStore: () => agentStoreForScopes(),
          makeId: () => `scheduled_dispatch_${++scheduledDispatchCounter}`,
          makePylonApiStore: () => pylonApiStore,
          makeStore: () => store,
          nowIso: () => nowIso,
        },
        {},
        { nowIso },
      ),
    )

  test('a scheduled order holds placement and dispatch until launch time', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([pylonRegistration()])
    const created = await route(store, '/api/autopilot/work', {
      body: scheduledLaunchFixture('2026-06-10T03:00:00Z'),
      idempotencyKey: 'idem-scheduled-launch-holds',
      pylonApiStore,
    })
    const createdJson = await scheduledJson(created)

    expect(created.status).toBe(202)
    expect(createdJson.work?.state).toBe('scheduled')
    expect(createdJson.work?.scheduledLaunch?.launchState).toBe('pending')
    expect(createdJson.work?.nextAction?.state).toBe('retry_later')
    expect(createdJson.work?.nextAction?.reasonRefs).toContain(
      'next_action.scheduled_launch_pending',
    )
    expect(createdJson.work?.pylonAssignmentIntents).toEqual([])
    expect(pylonApiStore.assignments.size).toBe(0)

    const earlyReport = await runScheduledDispatch(
      store,
      pylonApiStore,
      '2026-06-10T02:00:00.000Z',
    )

    expect(earlyReport.dispatchedWorkOrderRefs).toEqual([])
    expect(pylonApiStore.assignments.size).toBe(0)

    await pylonApiStore.upsertRegistration(
      pylonRegistration({
        latestHeartbeatAt: '2026-06-10T03:04:30.000Z',
        updatedAt: '2026-06-10T03:04:30.000Z',
      }),
    )

    const dueReport = await runScheduledDispatch(
      store,
      pylonApiStore,
      '2026-06-10T03:05:00.000Z',
    )
    const workOrderRef = createdJson.work?.workOrderRef ?? ''
    const released = await store.readWorkOrder(workOrderRef)

    expect(dueReport.dispatchedWorkOrderRefs).toEqual([workOrderRef])
    expect(released?.state).toBe('queued_or_running')
    expect(released?.scheduledLaunch?.dispatchedAt).toBe(
      '2026-06-10T03:05:00.000Z',
    )
    expect(pylonApiStore.assignments.size).toBe(1)
  })

  test('a missed launch window expires to blocked instead of launching late', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([pylonRegistration()])
    const created = await route(store, '/api/autopilot/work', {
      body: scheduledLaunchFixture('2026-06-10T03:00:00Z', 30),
      idempotencyKey: 'idem-scheduled-launch-expires',
      pylonApiStore,
    })
    const createdJson = await scheduledJson(created)
    const workOrderRef = createdJson.work?.workOrderRef ?? ''
    const report = await runScheduledDispatch(
      store,
      pylonApiStore,
      '2026-06-10T04:00:00.000Z',
    )
    const expired = await store.readWorkOrder(workOrderRef)

    expect(report.expiredWorkOrderRefs).toEqual([workOrderRef])
    expect(report.dispatchedWorkOrderRefs).toEqual([])
    expect(expired?.state).toBe('blocked')
    expect(expired?.scheduledLaunch?.expiredAt).toBe('2026-06-10T04:00:00.000Z')
    expect(pylonApiStore.assignments.size).toBe(0)
  })

  test('scheduled dispatch is idempotent after release', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([pylonRegistration()])
    const created = await route(store, '/api/autopilot/work', {
      body: scheduledLaunchFixture('2026-06-10T03:00:00Z'),
      idempotencyKey: 'idem-scheduled-launch-idempotent',
      pylonApiStore,
    })
    const createdJson = await scheduledJson(created)
    const workOrderRef = createdJson.work?.workOrderRef ?? ''

    await pylonApiStore.upsertRegistration(
      pylonRegistration({
        latestHeartbeatAt: '2026-06-10T03:04:30.000Z',
        updatedAt: '2026-06-10T03:04:30.000Z',
      }),
    )
    await runScheduledDispatch(store, pylonApiStore, '2026-06-10T03:05:00.000Z')

    const repeatReport = await runScheduledDispatch(
      store,
      pylonApiStore,
      '2026-06-10T03:10:00.000Z',
    )

    expect(repeatReport.dispatchedWorkOrderRefs).toEqual([])
    expect(repeatReport.expiredWorkOrderRefs).toEqual([])
    expect(pylonApiStore.assignments.size).toBe(1)
    expect((await store.readWorkOrder(workOrderRef))?.state).toBe(
      'queued_or_running',
    )
  })

  test('a launchAt past the seven-day horizon is rejected', async () => {
    const store = new MemoryAutopilotWorkStore()
    const created = await route(store, '/api/autopilot/work', {
      body: scheduledLaunchFixture('2026-06-30T03:00:00Z'),
      idempotencyKey: 'idem-scheduled-launch-horizon',
    })
    const createdJson = await scheduledJson(created)

    expect(created.status).toBe(400)
    expect(createdJson.error).toBe('autopilot_work_validation_error')
    expect(createdJson.reason).toContain('7 days')
  })

  test('a past launchAt launches immediately', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([pylonRegistration()])
    const created = await route(store, '/api/autopilot/work', {
      body: scheduledLaunchFixture('2026-06-09T17:00:00Z'),
      idempotencyKey: 'idem-scheduled-launch-immediate',
      pylonApiStore,
    })
    const createdJson = await scheduledJson(created)

    expect(createdJson.work?.state).toBe('queued_or_running')
    expect(createdJson.work?.scheduledLaunch?.launchState).toBe('dispatched')
    expect(pylonApiStore.assignments.size).toBe(1)
  })
})
