import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  artanisAdminCloseoutReceiptRef,
  type ArtanisAdminCloseoutReceiptRecord,
} from './artanis-admin-closeout-receipts'
import { exampleNexusPylonVisibilityFixture } from './nexus-pylon-visibility'
import { makeNexusPylonVisibilityRoutes } from './nexus-pylon-visibility-routes'
import type {
  NexusPaymentAuthorityReceiptRecord,
  NexusPayoutTargetApprovalRecord,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import type {
  PylonApiAssignmentRecord,
  PylonApiEventRecord,
  PylonApiRegistrationRecord,
} from './pylon-api'
import {
  type TreasuryPaymentAuthorityAdapter,
  TreasuryPaymentAuthorityError,
  makeTreasuryPaymentAuthority,
} from './treasury-payment-authority'

const nowIso = '2026-06-07T07:30:00.000Z'
const persistedNowIso = '2026-06-07T07:20:00.000Z'
const bridgeAssignmentRef = 'assignment.public.artanis_pylon_launch_20260607'
const acceptedAssignmentRef =
  'assignment.public.artanis_pylon_accepted_work_20260607'
const acceptedPylonRef = 'pylon.public.artanis_accepted_work'
const artanisAdminAssignmentRef = 'assignment.artanis_admin.20260611011429'
const artanisAdminTraceDigest =
  'f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b'

const executionContext = {
  passThroughOnException: () => undefined,
  props: {},
  waitUntil: () => undefined,
} satisfies ExecutionContext

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

const persistedIntent: NexusTreasuryPayoutIntentRecord = {
  acceptedWorkRefs: ['accepted_work.public.issue_431'],
  actorRef: 'agent.artanis',
  adapterKind: 'mdk_agent_wallet',
  amount: {
    amountMinorUnits: 1_000_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  archivedAt: null,
  artanisDispatchRef: 'artanis.dispatch.issue_431',
  assignmentRef: 'assignment.public.issue_431',
  buyerPaymentRef: 'buyer_payment.public.issue_431',
  createdAt: persistedNowIso,
  id: 'nexus_treasury_payout_intent_issue_431',
  idempotencyKeyHash: 'hash.issue_431.intent',
  metadataRefs: ['metadata.public.issue_431.intent'],
  ownerUserId: 'user_openagents_operator',
  payoutIntentRef: 'payout_intent.issue_431',
  payoutTargetApprovalRef: 'approval.public.issue_431',
  payoutTargetRef: 'payout_target.public.issue_431',
  policySnapshotRef: 'policy_snapshot.public.issue_431',
  publicProjectionJson: JSON.stringify({
    moneyMovement: 'real_bitcoin',
    state: 'intent_created',
  }),
  pylonJobRef: 'pylon_job.public.issue_431',
  sourceKind: 'operator_test',
  spendCap: {
    amountMinorUnits: 2_000_000,
    asset: 'bitcoin',
    denomination: 'bitcoin_millisatoshi',
  },
  status: 'approved',
  updatedAt: persistedNowIso,
}

const persistedAttempt: NexusTreasuryPayoutAttemptRecord = {
  adapterAttemptRef: 'adapter_attempt.mdk_agent_wallet.issue_431',
  adapterKind: 'mdk_agent_wallet',
  amount: persistedIntent.amount,
  archivedAt: null,
  createdAt: persistedNowIso,
  id: 'nexus_treasury_payout_attempt_issue_431',
  idempotencyKeyHash: 'hash.issue_431.attempt',
  metadataRefs: ['metadata.public.issue_431.attempt'],
  payoutAttemptRef: 'payout_attempt.issue_431',
  payoutIntentRef: persistedIntent.payoutIntentRef,
  publicProjectionJson: JSON.stringify({
    moneyMovement: 'real_bitcoin',
    state: 'dispatch_recorded',
  }),
  redactedDestinationRef: 'destination.redacted.issue_431',
  redactedPaymentRef: 'payment.redacted.mdk_agent_wallet.issue_431',
  status: 'dispatched',
  updatedAt: persistedNowIso,
}

const persistedEvent: NexusTreasuryPayoutReconciliationEventRecord = {
  adapterKind: 'mdk_agent_wallet',
  archivedAt: null,
  createdAt: persistedNowIso,
  eventRef: 'reconciliation.issue_431',
  externalEventRef: 'payment.redacted.mdk_agent_wallet.issue_431',
  id: 'nexus_treasury_reconciliation_issue_431',
  idempotencyKeyHash: 'hash.issue_431.reconciliation',
  metadataRefs: ['metadata.public.issue_431.reconciliation'],
  payoutAttemptRef: persistedAttempt.payoutAttemptRef,
  payoutIntentRef: persistedIntent.payoutIntentRef,
  providerRef: 'provider.public.mdk_agent_wallet',
  publicProjectionJson: JSON.stringify({
    moneyMovement: 'real_bitcoin',
    state: 'matched',
  }),
  resultRef: 'payment.redacted.mdk_agent_wallet.issue_431',
  status: 'matched',
}

const persistedReceipt: NexusPaymentAuthorityReceiptRecord = {
  archivedAt: null,
  audience: 'public',
  createdAt: persistedNowIso,
  eventRef: persistedEvent.eventRef,
  id: 'nexus_payment_authority_receipt_issue_431',
  metadataRefs: ['metadata.public.issue_431.settlement'],
  payoutAttemptRef: persistedAttempt.payoutAttemptRef,
  payoutIntentRef: persistedIntent.payoutIntentRef,
  publicProjectionJson: JSON.stringify({
    amountSats: 1000,
    moneyMovement: 'real_bitcoin',
    state: 'settled',
  }),
  receiptKind: 'settlement_recorded',
  receiptRef: 'receipt.nexus.issue_431.real_mdk_settlement',
}

const bridgeEvent = (
  input: Readonly<{
    body: Record<string, unknown>
    eventKind: PylonApiEventRecord['eventKind']
    eventRef: string
    status: string
  }>,
): PylonApiEventRecord => ({
  assignmentRef: bridgeAssignmentRef,
  createdAt: persistedNowIso,
  eventBody: input.body,
  eventKind: input.eventKind,
  eventRef: input.eventRef,
  id: input.eventRef.replaceAll('.', '_'),
  idempotencyKeyHash: `hash.${input.eventRef}`,
  ownerAgentUserId: 'user_agent_bridge_test',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.public.artanis_bridge_test',
  status: input.status,
})

const bridgeEvents: ReadonlyArray<PylonApiEventRecord> = [
  bridgeEvent({
    body: {
      acceptanceRefs: ['accepted_work.public.artanis_bridge_test'],
      accepted: true,
    },
    eventKind: 'assignment_acceptance',
    eventRef: 'pylon_event.public.artanis_bridge_test.acceptance',
    status: 'accepted',
  }),
  bridgeEvent({
    body: {
      artifactRefs: ['artifact.public.artanis_bridge_test'],
      proofRefs: ['proof.public.artanis_bridge_test'],
    },
    eventKind: 'artifact_proof_metadata',
    eventRef: 'pylon_event.public.artanis_bridge_test.artifacts',
    status: 'submitted',
  }),
  bridgeEvent({
    body: {
      paymentProofRefs: ['payment_proof.public.artanis_bridge_test'],
      receiptRefs: ['receipt.public.artanis_bridge_test.payout'],
      settlementRefs: ['settlement.public.artanis_bridge_test.payout'],
    },
    eventKind: 'payment_receipt',
    eventRef: 'pylon_event.public.artanis_bridge_test.payment',
    status: 'reported',
  }),
  bridgeEvent({
    body: {
      settlementRefs: ['settlement.public.artanis_bridge_test.final'],
      treasuryReceiptRefs: ['treasury_receipt.public.artanis_bridge_test'],
    },
    eventKind: 'settlement_status',
    eventRef: 'pylon_event.public.artanis_bridge_test.settlement',
    status: 'settled',
  }),
]

const acceptedAssignment: PylonApiAssignmentRecord = {
  acceptanceCriteriaRefs: ['criteria.public.accepted_work_smoke'],
  acceptedWorkRefs: ['accepted_work.public.artanis_accepted_work_smoke'],
  artifactRefs: ['artifact.public.artanis_accepted_work_smoke'],
  assignmentRef: acceptedAssignmentRef,
  closeoutRefs: ['closeout.public.artanis_accepted_work_smoke'],
  codingAssignment: null,
  createdAt: persistedNowIso,
  id: 'pylon_assignment_public_artanis_accepted_work_smoke',
  idempotencyKeyHash: 'hash.accepted_work.assignment',
  jobKind: 'healthcheck_echo',
  leaseExpiresAt: '2026-06-07T08:20:00.000Z',
  ownerAgentUserId: 'user_agent_accepted_work_test',
  proofRefs: ['proof.public.artanis_accepted_work_smoke'],
  publicProjectionJson: '{}',
  pylonRef: acceptedPylonRef,
  rejectionRefs: [],
  resultExpectationRefs: ['result.public.accepted_work_smoke'],
  state: 'accepted_work',
  taskRefs: ['pylon_job.public.accepted_work_smoke'],
  updatedAt: persistedNowIso,
}

const acceptedRegistration: PylonApiRegistrationRecord = {
  capabilityRefs: ['capability.public.pylon.echo'],
  clientProtocolVersion: '0.2.5',
  clientVersion: 'openagents.pylon@0.2.5',
  createdAt: persistedNowIso,
  displayName: 'Accepted Work Pylon',
  id: 'pylon_registration_public_accepted_work',
  latestHeartbeatAt: persistedNowIso,
  latestHeartbeatStatus: 'online',
  latestCapacityRefs: ['capacity.public.echo_available'],
  latestHealthRefs: ['health.public.ok'],
  latestLoadRefs: ['load.public.low'],
  latestResourceMode: 'overnight_full',
  ownerAgentCredentialId: 'credential_agent_accepted_work',
  ownerAgentTokenPrefix: 'oa_agent_test',
  ownerAgentUserId: 'user_agent_accepted_work_test',
  providerMarketRelayRefs: [],
  providerNip90LaneRefs: [],
  providerNostrNpub: null,
  providerNostrPubkey: null,
  publicProjectionJson: '{}',
  pylonRef: acceptedPylonRef,
  resourceMode: 'overnight_full',
  status: 'active',
  updatedAt: persistedNowIso,
  walletReady: true,
  walletRef: 'wallet.public.artanis_accepted_work',
}

const acceptedEvent = (
  input: Readonly<{
    body: Record<string, unknown>
    eventKind: PylonApiEventRecord['eventKind']
    eventRef: string
    status: string
  }>,
): PylonApiEventRecord => ({
  assignmentRef: acceptedAssignmentRef,
  createdAt: persistedNowIso,
  eventBody: input.body,
  eventKind: input.eventKind,
  eventRef: input.eventRef,
  id: input.eventRef.replaceAll('.', '_'),
  idempotencyKeyHash: `hash.${input.eventRef}`,
  ownerAgentUserId: 'user_agent_accepted_work_test',
  publicProjectionJson: '{}',
  pylonRef: acceptedPylonRef,
  status: input.status,
})

const acceptedEvents: ReadonlyArray<PylonApiEventRecord> = [
  acceptedEvent({
    body: {
      acceptanceRefs: acceptedAssignment.acceptedWorkRefs,
      accepted: true,
    },
    eventKind: 'assignment_acceptance',
    eventRef: 'pylon_event.public.accepted_work.acceptance',
    status: 'accepted',
  }),
  acceptedEvent({
    body: {
      artifactRefs: acceptedAssignment.artifactRefs,
      proofRefs: acceptedAssignment.proofRefs,
      storageRefs: ['storage.public.accepted_work.artifact_manifest'],
    },
    eventKind: 'artifact_proof_metadata',
    eventRef: 'pylon_event.public.accepted_work.artifacts',
    status: 'submitted',
  }),
  acceptedEvent({
    body: {
      liquidityRefs: ['liquidity.public.accepted_work.minimum_satisfied'],
      readinessRefs: ['readiness.public.accepted_work.wallet_ready'],
      walletReady: true,
      walletRef: acceptedRegistration.walletRef,
    },
    eventKind: 'wallet_readiness',
    eventRef: 'pylon_event.public.accepted_work.wallet_ready',
    status: 'ready',
  }),
]

const artanisAdminCloseoutReceipt: ArtanisAdminCloseoutReceiptRecord = {
  acceptedWorkRefs: [
    'accepted_work.tassadar_poc.trace_digest.f2995c4e3c959b42',
  ],
  artifactRefs: [
    `artifact.tassadar_poc.trace_digest.${artanisAdminTraceDigest}`,
  ],
  assignmentCreatedAt: '2026-06-11T01:14:29.000Z',
  assignmentRef: artanisAdminAssignmentRef,
  assignmentState: 'accepted_work',
  assignmentUpdatedAt: '2026-06-11T01:22:12.000Z',
  claimedTraceDigest: artanisAdminTraceDigest,
  claimedTraceDigestPrefix: artanisAdminTraceDigest.slice(0, 16),
  closeoutRefs: [
    'closeout.tassadar_poc.trace_digest.f2995c4e3c959b42',
  ],
  decisionCreatedAt: '2026-06-11T01:14:29.000Z',
  decisionId: 'artanis_admin_tick_decision_test',
  decisionState: 'dispatched',
  jobKind: 'tassadar_poc_trace',
  proofRefs: ['proof.tassadar_poc.trace_digest.f2995c4e3c959b42'],
  pylonRef: 'pylon.public.tassadar_executor',
  verdictAcceptState: 'accepted',
  verdictCreatedAt: '2026-06-11T01:26:12.000Z',
  verdictOutcome: 'verified',
  verdictRef: 'verdict.artanis_closeout.verified',
}

const makeMemoryLedgerStore = () => {
  const attempts = new Map<string, NexusTreasuryPayoutAttemptRecord>([
    [persistedAttempt.payoutAttemptRef, persistedAttempt],
  ])
  const attemptsByIdempotency = new Map<
    string,
    NexusTreasuryPayoutAttemptRecord
  >([[persistedAttempt.idempotencyKeyHash, persistedAttempt]])
  const events = new Map<string, NexusTreasuryPayoutReconciliationEventRecord>([
    [persistedEvent.eventRef, persistedEvent],
  ])
  const intents = new Map<string, NexusTreasuryPayoutIntentRecord>([
    [persistedIntent.payoutIntentRef, persistedIntent],
  ])
  const intentsByIdempotency = new Map<string, NexusTreasuryPayoutIntentRecord>(
    [[persistedIntent.idempotencyKeyHash, persistedIntent]],
  )
  const receipts = new Map<string, NexusPaymentAuthorityReceiptRecord>([
    [persistedReceipt.receiptRef, persistedReceipt],
  ])
  const approvals = new Map<string, NexusPayoutTargetApprovalRecord>()

  return {
    createPayoutAttempt: async (record: NexusTreasuryPayoutAttemptRecord) => {
      if (!attemptsByIdempotency.has(record.idempotencyKeyHash)) {
        attempts.set(record.payoutAttemptRef, record)
        attemptsByIdempotency.set(record.idempotencyKeyHash, record)
      }
    },
    createPayoutIntent: async (record: NexusTreasuryPayoutIntentRecord) => {
      if (!intentsByIdempotency.has(record.idempotencyKeyHash)) {
        intents.set(record.payoutIntentRef, record)
        intentsByIdempotency.set(record.idempotencyKeyHash, record)
      }
    },
    createPayoutTargetApproval: async (
      record: NexusPayoutTargetApprovalRecord,
    ) => {
      approvals.set(record.approvalRef, record)
    },
    createPaymentAuthorityReceipt: async (
      record: NexusPaymentAuthorityReceiptRecord,
    ) => {
      receipts.set(record.receiptRef, record)
    },
    createReconciliationEvent: async (
      record: NexusTreasuryPayoutReconciliationEventRecord,
    ) => {
      events.set(record.eventRef, record)
    },
    readPaymentAuthorityReceiptByRef: async (receiptRef: string) =>
      receipts.get(receiptRef),
    readPayoutAttemptByIdempotencyKeyHash: async (idempotencyKeyHash: string) =>
      attemptsByIdempotency.get(idempotencyKeyHash),
    readPayoutAttemptByRef: async (payoutAttemptRef: string) =>
      attempts.get(payoutAttemptRef),
    readPayoutIntentByIdempotencyKeyHash: async (idempotencyKeyHash: string) =>
      intentsByIdempotency.get(idempotencyKeyHash),
    readPayoutIntentByBuyerPaymentRef: async (buyerPaymentRef: string) =>
      [...intents.values()].find(
        intent => intent.buyerPaymentRef === buyerPaymentRef,
      ),
    readPayoutIntentByRef: async (payoutIntentRef: string) =>
      intents.get(payoutIntentRef),
    readReconciliationEventByRef: async (eventRef: string) =>
      events.get(eventRef),
  }
}

type AcceptedPayoutMode =
  | 'insufficient_liquidity'
  | 'pending'
  | 'paused'
  | 'rejected'
  | 'success'
  | 'unavailable'

const makeTestHostedMdkAdapter = (
  mode: AcceptedPayoutMode,
  dispatchCalls?: { count: number },
): TreasuryPaymentAuthorityAdapter => ({
  adapterKind: 'hosted_mdk',
  dispatch: input =>
    Effect.gen(function* () {
      if (mode === 'unavailable') {
        return yield* new TreasuryPaymentAuthorityError({
          message: 'hosted_mdk_rpc_error',
          reason: 'adapter_unavailable',
        })
      }

      if (mode === 'insufficient_liquidity') {
        return yield* new TreasuryPaymentAuthorityError({
          message: 'hosted_mdk_insufficient_liquidity',
          reason: 'adapter_unavailable',
        })
      }

      if (dispatchCalls !== undefined) {
        dispatchCalls.count += 1
      }

      return {
        ...input.attempt,
        adapterAttemptRef: `adapter_attempt.hosted_mdk.${input.attempt.idempotencyKeyHash}`,
        metadataRefs: [
          ...input.attempt.metadataRefs,
          'metadata.nexus.hosted_mdk.test_dispatch',
        ],
        publicProjectionJson: JSON.stringify({
          adapter: 'hosted_mdk',
          moneyMovement: 'real_bitcoin',
          rawMaterialStored: false,
          state: 'dispatch_reported',
        }),
        redactedPaymentRef: `payment.redacted.hosted_mdk.${input.attempt.idempotencyKeyHash}`,
        status: 'dispatched',
      } satisfies NexusTreasuryPayoutAttemptRecord
    }),
  preview: input =>
    Effect.succeed({
      adapterKind: 'hosted_mdk',
      amount: input.intent.amount,
      dispatchAllowed: mode !== 'unavailable',
      payoutIntentRef: input.intent.payoutIntentRef,
      payoutTargetApprovalRef: input.intent.payoutTargetApprovalRef ?? '',
      policySnapshotRef: input.intent.policySnapshotRef,
      spendCap: input.intent.spendCap,
    }),
  reconcile: input =>
    Effect.succeed({
      ...input.event,
      metadataRefs: [
        ...input.event.metadataRefs,
        `metadata.nexus.hosted_mdk.test_${mode}`,
      ],
      publicProjectionJson: JSON.stringify({
        adapter: 'hosted_mdk',
        moneyMovement: 'real_bitcoin',
        rawMaterialStored: false,
        state:
          mode === 'pending'
            ? 'reconciliation_requested'
            : mode === 'rejected'
              ? 'reconciliation_failed'
              : 'reconciliation_success',
      }),
      resultRef:
        mode === 'pending'
          ? `result.hosted_mdk.requested.${input.event.idempotencyKeyHash}`
          : `payment.redacted.hosted_mdk.${input.event.idempotencyKeyHash}`,
      status:
        mode === 'pending'
          ? 'observed'
          : mode === 'rejected'
            ? 'rejected'
            : 'matched',
    } satisfies NexusTreasuryPayoutReconciliationEventRecord),
})

const route = (
  options: Readonly<{
    acceptedAssignment?: PylonApiAssignmentRecord | undefined
    acceptedEvents?: ReadonlyArray<PylonApiEventRecord> | undefined
    acceptedRegistration?: PylonApiRegistrationRecord | undefined
    adminToken?: boolean
    artanisCloseoutReceipt?: ArtanisAdminCloseoutReceiptRecord | undefined
    browserEmail?: string | undefined
    bridgeEvents?: ReadonlyArray<PylonApiEventRecord>
    dispatchCalls?: { count: number } | undefined
    persisted?: boolean
    paymentMode?: AcceptedPayoutMode | undefined
    resolvedPrivateDestination?: { value?: string | undefined } | undefined
    tipRecipientLightningAddress?: string | undefined
  }> = {},
) => {
  const ledgerStore =
    options.persisted === true ||
    options.bridgeEvents !== undefined ||
    options.acceptedAssignment !== undefined
      ? makeMemoryLedgerStore()
      : undefined

  return makeNexusPylonVisibilityRoutes<TestSession, Record<string, never>>({
    appendRefreshedSessionCookies: response => response,
    currentIsoTimestamp: () => nowIso,
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    ...(options.artanisCloseoutReceipt === undefined
      ? {}
      : {
          makeArtanisAdminCloseoutReceiptStore: () => ({
            readCloseoutReceiptByRef: async receiptRef => {
              const canonicalReceiptRef = artanisAdminCloseoutReceiptRef(
                options.artanisCloseoutReceipt!.assignmentRef,
              )

              return receiptRef ===
                options.artanisCloseoutReceipt!.assignmentRef ||
                receiptRef === canonicalReceiptRef
                ? options.artanisCloseoutReceipt
                : undefined
            },
          }),
        }),
    ...(ledgerStore !== undefined
      ? {
          makeLedgerStore: () => ledgerStore,
        }
      : {}),
    ...(options.acceptedAssignment === undefined
      ? {}
      : {
          makePaymentAuthority: (_env, context) => {
            if (options.resolvedPrivateDestination !== undefined) {
              options.resolvedPrivateDestination.value =
                context.privatePayoutDestination
            }

            return makeTreasuryPaymentAuthority({
              adapters: [
                makeTestHostedMdkAdapter(
                  options.paymentMode ?? 'success',
                  options.dispatchCalls,
                ),
              ],
              ledgerStore: context.ledgerStore as any,
              pauseState: {
                authorityPaused: options.paymentMode === 'paused',
                pausedAdapters: [],
              },
            })
          },
        }),
    ...(options.bridgeEvents === undefined &&
    options.acceptedEvents === undefined
      ? {}
      : {
          makePylonApiStore: () => ({
            listEventsForAssignment: async (assignmentRef, limit) =>
              [
                ...(options.bridgeEvents ?? []),
                ...(options.acceptedEvents ?? []),
              ]
                .filter(event => event.assignmentRef === assignmentRef)
                .slice(0, limit),
            listEventsForPylon: async (pylonRef, limit) =>
              [
                ...(options.bridgeEvents ?? []),
                ...(options.acceptedEvents ?? []),
              ]
                .filter(event => event.pylonRef === pylonRef)
                .slice(0, limit),
            readAssignment: async assignmentRef =>
              options.acceptedAssignment?.assignmentRef === assignmentRef
                ? options.acceptedAssignment
                : undefined,
            readRegistration: async pylonRef =>
              options.acceptedRegistration?.pylonRef === pylonRef
                ? options.acceptedRegistration
                : undefined,
          }),
        }),
    ...(options.tipRecipientLightningAddress === undefined
      ? {}
      : {
          makeTipRecipientReadinessReader: () => ({
            readForActor: actorRef =>
              Effect.succeed({
                actorRef,
                directPayment: {
                  bolt12Offer: 'lno1public_test_offer',
                  kind: 'bolt12_offer',
                  lightningAddress: options.tipRecipientLightningAddress,
                  settlementAuthority: 'recipient_wallet_direct',
                },
                state: 'ready',
              }),
          }),
        }),
    requireAdminApiToken: request =>
      Promise.resolve(
        options.adminToken === true &&
          request.headers.get('authorization') === 'Bearer admin',
      ),
    requireBrowserSession: () =>
      Promise.resolve(
        options.browserEmail === undefined
          ? undefined
          : {
              user: {
                email: options.browserEmail,
                userId: 'github:operator',
              },
            },
      ),
  }).routeNexusPylonVisibilityRequest
}

const runRoute = async (
  request: Request,
  options: Readonly<{
    acceptedAssignment?: PylonApiAssignmentRecord | undefined
    acceptedEvents?: ReadonlyArray<PylonApiEventRecord> | undefined
    acceptedRegistration?: PylonApiRegistrationRecord | undefined
    adminToken?: boolean
    artanisCloseoutReceipt?: ArtanisAdminCloseoutReceiptRecord | undefined
    browserEmail?: string | undefined
    bridgeEvents?: ReadonlyArray<PylonApiEventRecord>
    dispatchCalls?: { count: number } | undefined
    persisted?: boolean
    paymentMode?: AcceptedPayoutMode | undefined
    resolvedPrivateDestination?: { value?: string | undefined } | undefined
    tipRecipientLightningAddress?: string | undefined
  }> = {},
): Promise<Response> => {
  const matched = route(options)(request, {}, executionContext)

  if (matched === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(matched)
}

// `generatedAt` is a deliberate, public-safe exact-ISO field declared by the
// projection-staleness contract (see `PublicProjectionStalenessContract` /
// `feat(public): declare legacy projection staleness`): clients need an exact
// machine-readable timestamp to compute staleness against
// `staleness.maxStalenessSeconds`. It is not leaked private material, so the
// "no raw timestamp" privacy scan below strips it before matching and still
// catches any OTHER unexpected raw timestamp (which would indicate a real
// private-material leak).
const withoutPublicGeneratedAt = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map(withoutPublicGeneratedAt)
  }
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .filter(([key]) => key !== 'generatedAt')
        .map(([key, entry]) => [key, withoutPublicGeneratedAt(entry)]),
    )
  }
  return value
}

const serializedWithoutPublicTimestamps = (body: unknown): string =>
  JSON.stringify(withoutPublicGeneratedAt(body))

const fixtureReceiptRef = (): string => {
  const receiptRef =
    exampleNexusPylonVisibilityFixture(nowIso).receipts.at(-1)?.receiptRef

  if (receiptRef === undefined) {
    throw new Error('fixture receipt missing')
  }

  return receiptRef
}

describe('Nexus/Pylon visibility routes', () => {
  test('serves public-safe receipt JSON without private payment material', async () => {
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/public/nexus-pylon/receipts/${encodeURIComponent(
          fixtureReceiptRef(),
        )}`,
      ),
    )
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      audience: 'public',
      movementMode: 'simulation',
      payoutMovement: {
        terminalSettlementClaimAllowed: false,
      },
      realBitcoinMoved: false,
      schemaVersion: 'openagents.nexus_pylon.public_receipt.v1',
    })
    expect(body.receiptPageUrl).toContain('/nexus-pylon/receipts/')
    expect(body.settlement.stateLabel).toContain('simulation only')
    expect(serialized).not.toMatch(
      /operatorRefs|redactedDestinationRef|redactedPaymentRef/,
    )
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
    expect(serializedWithoutPublicTimestamps(body)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('serves a public receipt page without private fields', async () => {
    const response = await runRoute(
      new Request(
        `https://openagents.com/nexus-pylon/receipts/${encodeURIComponent(
          fixtureReceiptRef(),
        )}`,
      ),
    )
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/html')
    expect(html).toContain('OpenAgents Nexus / Pylon receipt')
    expect(html).toContain(
      'Dispatch acceptance is separate from terminal bitcoin settlement',
    )
    expect(html).toContain('real bitcoin moved: no')
    expect(html).toContain('Settled (simulation only)')
    expect(html).not.toMatch(
      /operatorRefs|redactedDestinationRef|redactedPaymentRef/,
    )
    expect(html).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
  })

  test('serves persisted real-bitcoin receipt JSON without private payment material', async () => {
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/public/nexus-pylon/receipts/${encodeURIComponent(
          persistedReceipt.receiptRef,
        )}`,
      ),
      { persisted: true },
    )
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      assignmentRef: persistedIntent.assignmentRef,
      audience: 'public',
      movementMode: 'real_bitcoin',
      payoutMovement: {
        terminalSettlementClaimAllowed: true,
      },
      realBitcoinMoved: true,
      receiptKind: 'settlement_recorded',
      schemaVersion: 'openagents.nexus_pylon.public_receipt.v1',
    })
    expect(body.settlement.providerRef).toBe('provider.public.mdk_agent_wallet')
    expect(body.settlement.stateLabel).toBe('Settled')
    expect(serialized).not.toMatch(
      /operatorRefs|redactedDestinationRef|redactedPaymentRef/,
    )
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
    expect(serializedWithoutPublicTimestamps(body)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('serves Artanis admin closeout receipts by assignment ref', async () => {
    const canonicalReceiptRef = artanisAdminCloseoutReceiptRef(
      artanisAdminAssignmentRef,
    )
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/public/nexus-pylon/receipts/${encodeURIComponent(
          artanisAdminAssignmentRef,
        )}`,
      ),
      { artanisCloseoutReceipt: artanisAdminCloseoutReceipt },
    )
    const canonicalResponse = await runRoute(
      new Request(
        `https://openagents.com/api/public/nexus-pylon/receipts/${encodeURIComponent(
          canonicalReceiptRef,
        )}`,
      ),
      { artanisCloseoutReceipt: artanisAdminCloseoutReceipt },
    )
    const body = (await response.json()) as Record<string, any>
    const canonicalBody = (await canonicalResponse.json()) as Record<
      string,
      any
    >
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(200)
    expect(canonicalResponse.status).toBe(200)
    expect(body).toMatchObject({
      assignmentRef: artanisAdminAssignmentRef,
      audience: 'public',
      movementMode: 'simulation',
      payoutAttemptRef: null,
      payoutIntentRef: null,
      realBitcoinMoved: false,
      receiptKind: 'artanis_admin_assignment_closeout',
      receiptRef: canonicalReceiptRef,
      schemaVersion: 'openagents.nexus_pylon.public_receipt.v1',
      settlement: {
        state: 'accepted_work_verified',
        stateLabel: 'Accepted work verified',
      },
      status: 'accepted_work_verified',
    })
    expect(canonicalBody.receiptRef).toBe(canonicalReceiptRef)
    expect(body.payoutMovement).toMatchObject({
      dispatchAccepted: true,
      terminalResultObserved: true,
      terminalSettlementClaimAllowed: false,
    })
    expect(body.publicProjection).toMatchObject({
      acceptedWorkObserved: true,
      assignmentRef: artanisAdminAssignmentRef,
      assignmentState: 'accepted_work',
      claimedTraceDigest: artanisAdminTraceDigest,
      claimedTraceDigestPrefix: artanisAdminTraceDigest.slice(0, 16),
      closeoutSubmittedObserved: true,
      expectationRef: `expectation.tassadar_poc.trace_digest.${artanisAdminTraceDigest.slice(
        0,
        16,
      )}`,
      verdictAcceptState: 'accepted',
      verdictOutcome: 'verified',
      verdictRef: 'verdict.artanis_closeout.verified',
    })
    expect(body.publicProjection.evidenceRefs).toEqual(
      expect.arrayContaining([
        artanisAdminAssignmentRef,
        `route:/api/public/nexus-pylon/receipts/${artanisAdminAssignmentRef}`,
        'verdict.artanis_closeout.verified',
      ]),
    )
    expect(body.apiUrl).toContain(encodeURIComponent(canonicalReceiptRef))
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
    expect(serializedWithoutPublicTimestamps(body)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('requires operator authority for dashboard and exposes redacted operational state', async () => {
    const request = new Request(
      'https://openagents.com/api/operator/nexus-pylon/dashboard',
      {
        headers: { authorization: 'Bearer admin' },
      },
    )
    const anonymous = await runRoute(
      new Request('https://openagents.com/api/operator/nexus-pylon/dashboard'),
    )
    const nonAdmin = await runRoute(
      new Request('https://openagents.com/api/operator/nexus-pylon/dashboard'),
      {
        browserEmail: 'user@example.com',
      },
    )
    const response = await runRoute(request, { adminToken: true })
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(anonymous.status).toBe(401)
    expect(nonAdmin.status).toBe(403)
    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      movementMode: 'simulation',
      realBitcoinMoved: false,
      schemaVersion: 'openagents.nexus_pylon.operator_dashboard.v1',
    })
    expect(body.blockedGates.length).toBeGreaterThan(0)
    expect(body.payoutAttempts.length).toBeGreaterThan(0)
    expect(body.payoutIntents.length).toBeGreaterThan(0)
    expect(body.receiptProjections.length).toBeGreaterThan(0)
    expect(body.releaseGateEvidence.length).toBeGreaterThan(0)
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
  })

  test('serves operator receipt detail and handles missing receipts', async () => {
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/receipts/${encodeURIComponent(
          fixtureReceiptRef(),
        )}`,
        {
          headers: { authorization: 'Bearer admin' },
        },
      ),
      { adminToken: true },
    )
    const missing = await runRoute(
      new Request(
        'https://openagents.com/api/public/nexus-pylon/receipts/missing',
      ),
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(body.receiptRecord.receiptRef).toBe(fixtureReceiptRef())
    expect(body.movementMode).toBe('simulation')
    expect(missing.status).toBe(404)
  })

  test('bridges accepted Pylon assignment reports into a public real-bitcoin receipt', async () => {
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          bridgeAssignmentRef,
        )}/settlement-bridges`,
        {
          body: JSON.stringify({
            amountSats: 100,
            artanisDispatchRef: 'artanis.dispatch.public.bridge_test',
            payoutTargetApprovalRef: 'approval.public.artanis_bridge_test',
            payoutTargetRef: 'payout_target.public.artanis_bridge_test',
            policySnapshotRef: 'policy_snapshot.public.artanis_bridge_test',
            providerRef: 'provider.public.mdk_agent_wallet',
            pylonJobRef: 'pylon_job.public.artanis_bridge_test',
            spendCapSats: 100,
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'bridge-test',
          },
          method: 'POST',
        },
      ),
      { adminToken: true, bridgeEvents },
    )
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      bridge: {
        assignmentRef: bridgeAssignmentRef,
        idempotent: false,
        receipt: {
          assignmentRef: bridgeAssignmentRef,
          movementMode: 'real_bitcoin',
          realBitcoinMoved: true,
          receiptKind: 'settlement_recorded',
        },
      },
      schemaVersion: 'openagents.nexus_pylon.assignment_settlement_bridge.v1',
    })
    expect(body.bridge.receipt.receiptRef).toBe(
      'receipt.nexus_pylon.settlement.assignment_public_artanis_pylon_launch_20260607',
    )
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
    expect(serializedWithoutPublicTimestamps(body)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('settles accepted Pylon work through payment authority with a public receipt', async () => {
    const privateDestination = 'lnbc_private_destination_that_must_not_echo'
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          acceptedAssignmentRef,
        )}/accepted-work-payouts`,
        {
          body: JSON.stringify({
            amountSats: 100,
            payoutTargetApprovalRef: 'approval.public.accepted_work_payout',
            payoutTargetRef: 'payout_target.public.accepted_work_payout',
            policySnapshotRef: 'policy_snapshot.public.accepted_work_payout',
            privatePayoutDestination: privateDestination,
            providerRef: 'provider.public.hosted_mdk',
            redactedDestinationRef: 'destination.redacted.accepted_work_payout',
            spendCapSats: 100,
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'accepted-work-payout',
          },
          method: 'POST',
        },
      ),
      {
        acceptedAssignment,
        acceptedEvents,
        acceptedRegistration,
        adminToken: true,
      },
    )
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      payout: {
        assignmentRef: acceptedAssignmentRef,
        idempotent: false,
        receipt: {
          assignmentRef: acceptedAssignmentRef,
          movementMode: 'real_bitcoin',
          realBitcoinMoved: true,
          receiptKind: 'settlement_recorded',
        },
        walletReadiness: 'ready',
      },
      schemaVersion: 'openagents.nexus_pylon.accepted_work_payout.v1',
    })
    expect(body.payout.receipt.receiptRef).toBe(
      'receipt.nexus_pylon.accepted_work_settlement.assignment_public_artanis_pylon_accepted_work_20260607',
    )
    expect(serialized).not.toContain(privateDestination)
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
    expect(serializedWithoutPublicTimestamps(body)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('resolves accepted-work payout destination from the agent Spark Lightning Address', async () => {
    const sparkLightningAddress =
      'oab38ad12345abcd9_that_must_not_echo@spark.money'
    const resolvedPrivateDestination: { value?: string | undefined } = {}
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          acceptedAssignmentRef,
        )}/accepted-work-payouts`,
        {
          body: JSON.stringify({
            amountSats: 100,
            payoutTargetApprovalRef:
              'approval.public.accepted_work_spark_address_payout',
            payoutTargetRef:
              'payout_target.public.accepted_work_spark_address_payout',
            policySnapshotRef:
              'policy_snapshot.public.accepted_work_spark_address_payout',
            providerRef: 'provider.public.hosted_mdk',
            redactedDestinationRef:
              'destination.redacted.accepted_work_spark_address_payout',
            spendCapSats: 100,
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'accepted-work-spark-address-payout',
          },
          method: 'POST',
        },
      ),
      {
        acceptedAssignment,
        acceptedEvents,
        acceptedRegistration,
        adminToken: true,
        resolvedPrivateDestination,
        tipRecipientLightningAddress: sparkLightningAddress,
      },
    )
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(resolvedPrivateDestination.value).toBe(sparkLightningAddress)
    expect(body.payout.receipt).toMatchObject({
      movementMode: 'real_bitcoin',
      realBitcoinMoved: true,
      receiptKind: 'settlement_recorded',
    })
    expect(serialized).not.toContain(sparkLightningAddress)
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
  })

  test('deduplicates accepted-work payouts for the same assignment', async () => {
    const dispatchCalls = { count: 0 }
    const handler = route({
      acceptedAssignment,
      acceptedEvents,
      acceptedRegistration,
      adminToken: true,
      dispatchCalls,
    })
    const request = () =>
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          acceptedAssignmentRef,
        )}/accepted-work-payouts`,
        {
          body: JSON.stringify({
            amountSats: 100,
            payoutTargetApprovalRef: 'approval.public.accepted_work_payout',
            payoutTargetRef: 'payout_target.public.accepted_work_payout',
            policySnapshotRef: 'policy_snapshot.public.accepted_work_payout',
            privatePayoutDestination:
              'lnbc_private_destination_that_must_not_echo',
            providerRef: 'provider.public.hosted_mdk',
            redactedDestinationRef: 'destination.redacted.accepted_work_payout',
            spendCapSats: 100,
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'accepted-work-payout',
          },
          method: 'POST',
        },
      )
    const run = async (nextRequest: Request): Promise<Response> => {
      const matched = handler(nextRequest, {}, executionContext)

      if (matched === undefined) {
        throw new Error('route did not match')
      }

      return Effect.runPromise(matched)
    }
    const first = await run(request())
    const second = await run(request())
    const firstBody = (await first.json()) as Record<string, any>
    const secondBody = (await second.json()) as Record<string, any>

    expect(first.status).toBe(201)
    expect(second.status).toBe(200)
    expect(dispatchCalls.count).toBe(1)
    expect(firstBody.payout.idempotent).toBe(false)
    expect(secondBody.payout.idempotent).toBe(true)
    expect(secondBody.payout.receipt.receiptRef).toBe(
      firstBody.payout.receipt.receiptRef,
    )
  })

  test('blocks accepted-work payout when wallet readiness is stale or policy is paused', async () => {
    const staleWalletEvents = acceptedEvents.map(event =>
      event.eventKind === 'wallet_readiness'
        ? { ...event, createdAt: '2026-06-05T07:20:00.000Z' }
        : event,
    )
    const stale = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          acceptedAssignmentRef,
        )}/accepted-work-payouts`,
        {
          body: JSON.stringify({
            amountSats: 100,
            payoutTargetApprovalRef: 'approval.public.accepted_work_payout',
            payoutTargetRef: 'payout_target.public.accepted_work_payout',
            policySnapshotRef: 'policy_snapshot.public.accepted_work_payout',
            privatePayoutDestination:
              'lnbc_private_destination_that_must_not_echo',
            redactedDestinationRef: 'destination.redacted.accepted_work_payout',
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'accepted-work-stale',
          },
          method: 'POST',
        },
      ),
      {
        acceptedAssignment,
        acceptedEvents: staleWalletEvents,
        acceptedRegistration,
        adminToken: true,
      },
    )
    const paused = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          acceptedAssignmentRef,
        )}/accepted-work-payouts`,
        {
          body: JSON.stringify({
            amountSats: 100,
            payoutTargetApprovalRef: 'approval.public.accepted_work_payout',
            payoutTargetRef: 'payout_target.public.accepted_work_payout',
            policySnapshotRef: 'policy_snapshot.public.accepted_work_payout',
            privatePayoutDestination:
              'lnbc_private_destination_that_must_not_echo',
            redactedDestinationRef: 'destination.redacted.accepted_work_payout',
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'accepted-work-paused',
          },
          method: 'POST',
        },
      ),
      {
        acceptedAssignment,
        acceptedEvents,
        acceptedRegistration,
        adminToken: true,
        paymentMode: 'paused',
      },
    )
    const staleBody = (await stale.json()) as Record<string, any>
    const pausedBody = (await paused.json()) as Record<string, any>

    expect(stale.status).toBe(409)
    expect(paused.status).toBe(409)
    expect(staleBody.reason).toContain('stale')
    expect(pausedBody.reason).toContain('paused_authority')
  })

  test('keeps accepted-work payout unpaid when adapter reports insufficient liquidity', async () => {
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          acceptedAssignmentRef,
        )}/accepted-work-payouts`,
        {
          body: JSON.stringify({
            amountSats: 100,
            payoutTargetApprovalRef: 'approval.public.accepted_work_payout',
            payoutTargetRef: 'payout_target.public.accepted_work_payout',
            policySnapshotRef: 'policy_snapshot.public.accepted_work_payout',
            privatePayoutDestination:
              'lnbc_private_destination_that_must_not_echo',
            providerRef: 'provider.public.hosted_mdk',
            redactedDestinationRef: 'destination.redacted.accepted_work_payout',
            spendCapSats: 100,
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'accepted-work-insufficient',
          },
          method: 'POST',
        },
      ),
      {
        acceptedAssignment,
        acceptedEvents,
        acceptedRegistration,
        adminToken: true,
        paymentMode: 'insufficient_liquidity',
      },
    )
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(409)
    expect(body.reason).toContain('hosted_mdk_insufficient_liquidity')
    expect(serialized).not.toContain('settlement_recorded')
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
  })

  test('blocks assignment settlement bridge without required evidence', async () => {
    const response = await runRoute(
      new Request(
        `https://openagents.com/api/operator/nexus-pylon/assignments/${encodeURIComponent(
          bridgeAssignmentRef,
        )}/settlement-bridges`,
        {
          body: JSON.stringify({
            amountSats: 100,
            payoutTargetApprovalRef: 'approval.public.artanis_bridge_test',
            payoutTargetRef: 'payout_target.public.artanis_bridge_test',
            policySnapshotRef: 'policy_snapshot.public.artanis_bridge_test',
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'bridge-test-missing-evidence',
          },
          method: 'POST',
        },
      ),
      { adminToken: true, bridgeEvents: bridgeEvents.slice(0, 2) },
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(409)
    expect(body).toMatchObject({
      error: 'nexus_pylon_bridge_blocked',
    })
  })

  test('runs an operator proof-run around the settlement bridge', async () => {
    const response = await runRoute(
      new Request(
        'https://openagents.com/api/operator/nexus-pylon/proof-runs',
        {
          body: JSON.stringify({
            amountSats: 100,
            artanisRunRef: 'run.public.artanis.bridge_test',
            assignmentRef: bridgeAssignmentRef,
            payoutTargetApprovalRef: 'approval.public.artanis_bridge_test',
            payoutTargetRef: 'payout_target.public.artanis_bridge_test',
            policySnapshotRef: 'policy_snapshot.public.artanis_bridge_test',
            providerRef: 'provider.public.mdk_agent_wallet',
            pylonJobRef: 'pylon_job.public.artanis_bridge_test',
            settlementIntentRef: 'settlement_intent.public.artanis_bridge_test',
            spendCapSats: 100,
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'proof-run-test',
          },
          method: 'POST',
        },
      ),
      { adminToken: true, bridgeEvents },
    )
    const body = (await response.json()) as Record<string, any>
    const serialized = JSON.stringify(body)

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      proofRun: {
        bridgeStatus: 201,
        idempotent: false,
        postTrace: {
          acceptedWorkObserved: true,
          artifactProofObserved: true,
          paymentEvidenceObserved: true,
          publicReceiptObserved: true,
          realBitcoinMoved: true,
          sameAssignmentIdObserved: true,
          settlementEvidenceObserved: true,
          state: 'complete',
          terminalSettlementObserved: true,
        },
        preTrace: {
          publicReceiptObserved: false,
          state: 'partial',
        },
      },
      schemaVersion: 'openagents.nexus_pylon.assignment_proof_run.v1',
    })
    expect(body.proofRun.publicReceiptUrl).toContain('/nexus-pylon/receipts/')
    expect(body.proofRun.proofRunRef).toContain(
      'proof_run.public.artanis_pylon.assignment_public_artanis_pylon_launch_20260607',
    )
    expect(serialized).not.toMatch(
      /lnbc|lntb|mnemonic|preimage|secret|wallet_(config|key|material|mnemonic|secret|seed|state)/i,
    )
    expect(serializedWithoutPublicTimestamps(body)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('requires admin auth and idempotency for proof-runs', async () => {
    const noAuth = await runRoute(
      new Request(
        'https://openagents.com/api/operator/nexus-pylon/proof-runs',
        {
          body: JSON.stringify({
            amountSats: 100,
            artanisRunRef: 'run.public.artanis.bridge_test',
            assignmentRef: bridgeAssignmentRef,
            payoutTargetApprovalRef: 'approval.public.artanis_bridge_test',
            payoutTargetRef: 'payout_target.public.artanis_bridge_test',
            policySnapshotRef: 'policy_snapshot.public.artanis_bridge_test',
          }),
          headers: { 'idempotency-key': 'proof-run-no-auth' },
          method: 'POST',
        },
      ),
      { bridgeEvents },
    )
    const noIdempotency = await runRoute(
      new Request(
        'https://openagents.com/api/operator/nexus-pylon/proof-runs',
        {
          body: JSON.stringify({
            amountSats: 100,
            artanisRunRef: 'run.public.artanis.bridge_test',
            assignmentRef: bridgeAssignmentRef,
            payoutTargetApprovalRef: 'approval.public.artanis_bridge_test',
            payoutTargetRef: 'payout_target.public.artanis_bridge_test',
            policySnapshotRef: 'policy_snapshot.public.artanis_bridge_test',
          }),
          headers: { authorization: 'Bearer admin' },
          method: 'POST',
        },
      ),
      { adminToken: true, bridgeEvents },
    )

    expect(noAuth.status).toBe(401)
    expect(noIdempotency.status).toBe(409)
  })

  test('returns proof-run blockers when bridge evidence is missing', async () => {
    const response = await runRoute(
      new Request(
        'https://openagents.com/api/operator/nexus-pylon/proof-runs',
        {
          body: JSON.stringify({
            amountSats: 100,
            artanisRunRef: 'run.public.artanis.bridge_test',
            assignmentRef: bridgeAssignmentRef,
            payoutTargetApprovalRef: 'approval.public.artanis_bridge_test',
            payoutTargetRef: 'payout_target.public.artanis_bridge_test',
            policySnapshotRef: 'policy_snapshot.public.artanis_bridge_test',
          }),
          headers: {
            authorization: 'Bearer admin',
            'idempotency-key': 'proof-run-missing-evidence',
          },
          method: 'POST',
        },
      ),
      { adminToken: true, bridgeEvents: bridgeEvents.slice(0, 2) },
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(409)
    expect(body).toMatchObject({
      proofRun: {
        bridge: { error: 'nexus_pylon_bridge_blocked' },
        bridgeStatus: 409,
        postTrace: {
          paymentEvidenceObserved: false,
          publicReceiptObserved: false,
          state: 'partial',
        },
      },
    })
    expect(body.proofRun.postTrace.missingEvidenceRefs).toContain(
      'missing.public.artanis_pylon_proof.payment_evidence',
    )
  })
})
