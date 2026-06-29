import { Context, Effect, Layer, Schema as S } from 'effect'

import {
  NexusPaymentAuthorityReceiptRecord,
  NexusTreasuryPayoutAmount,
  NexusTreasuryPayoutAdapterKind,
  NexusTreasuryPayoutAttemptRecord,
  NexusTreasuryPayoutIntentRecord,
  NexusTreasuryPayoutLedgerProjection,
  NexusTreasuryPayoutLedgerRecordKind,
  NexusTreasuryPayoutLedgerUnsafe,
  NexusTreasuryPayoutReconciliationEventRecord,
  assertNexusTreasuryPayoutIntentSafe,
  decodeNexusTreasuryPayoutAmount,
  projectNexusTreasuryPayoutLedgerRecord,
} from './nexus-treasury-payout-ledger'
import type {
  NexusTreasuryPayoutLedgerRecord,
  NexusTreasuryPayoutLedgerStore,
} from './nexus-treasury-payout-ledger'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'

export const TreasuryPaymentAuthorityRejectionReason = S.Literals([
  'adapter_unavailable',
  'malformed_payout_amount',
  'malformed_payout_target',
  'missing_accepted_work_ref',
  'missing_payout_target_approval',
  'large_payout_requires_approval',
  'paused_agent',
  'paused_adapter',
  'paused_authority',
  'paused_payout_target',
  'paused_pylon',
  'payout_intent_not_found',
  'replayed_idempotency_key',
  'spend_cap_exceeded',
  'stale_or_absent_wallet_readiness',
])
export type TreasuryPaymentAuthorityRejectionReason =
  typeof TreasuryPaymentAuthorityRejectionReason.Type

export class TreasuryPaymentAuthorityError extends S.TaggedErrorClass<TreasuryPaymentAuthorityError>()(
  'TreasuryPaymentAuthorityError',
  {
    message: S.String,
    reason: TreasuryPaymentAuthorityRejectionReason,
  },
) {}

export const TreasuryPaymentAuthorityWalletReadiness = S.Literals([
  'absent',
  'ready',
  'stale',
])
export type TreasuryPaymentAuthorityWalletReadiness =
  typeof TreasuryPaymentAuthorityWalletReadiness.Type

export const TreasuryPaymentAuthorityPauseState = S.Struct({
  authorityPaused: S.Boolean,
  pausedActorRefs: S.optionalKey(S.Array(S.String)),
  pausedAdapters: S.Array(NexusTreasuryPayoutAdapterKind),
  pausedPayoutTargetRefs: S.optionalKey(S.Array(S.String)),
  pausedPylonRefs: S.optionalKey(S.Array(S.String)),
})
export type TreasuryPaymentAuthorityPauseState =
  typeof TreasuryPaymentAuthorityPauseState.Type

export const TreasuryPaymentAuthoritySpendPolicy = S.Struct({
  largePayoutApprovalRefs: S.Array(S.String),
  largePayoutThreshold: S.NullOr(NexusTreasuryPayoutAmount),
  policyRef: S.String,
})
export type TreasuryPaymentAuthoritySpendPolicy =
  typeof TreasuryPaymentAuthoritySpendPolicy.Type

export const TreasuryPaymentAuthorityPolicyDecision = S.Struct({
  allowed: S.Boolean,
  reason: S.NullOr(TreasuryPaymentAuthorityRejectionReason),
  receipt: S.NullOr(NexusPaymentAuthorityReceiptRecord),
})
export type TreasuryPaymentAuthorityPolicyDecision =
  typeof TreasuryPaymentAuthorityPolicyDecision.Type

export const TreasuryPaymentAuthorityPayoutPreviewRequest = S.Struct({
  intent: NexusTreasuryPayoutIntentRecord,
  walletReadiness: TreasuryPaymentAuthorityWalletReadiness,
})
export type TreasuryPaymentAuthorityPayoutPreviewRequest =
  typeof TreasuryPaymentAuthorityPayoutPreviewRequest.Type

export const TreasuryPaymentAuthorityPayoutPreview = S.Struct({
  adapterKind: NexusTreasuryPayoutAdapterKind,
  amount: NexusTreasuryPayoutAmount,
  dispatchAllowed: S.Boolean,
  payoutIntentRef: S.String,
  payoutTargetApprovalRef: S.String,
  policySnapshotRef: S.String,
  spendCap: NexusTreasuryPayoutAmount,
})
export type TreasuryPaymentAuthorityPayoutPreview =
  typeof TreasuryPaymentAuthorityPayoutPreview.Type

export const TreasuryPaymentAuthorityIntentCreationRequest = S.Struct({
  intent: NexusTreasuryPayoutIntentRecord,
  walletReadiness: TreasuryPaymentAuthorityWalletReadiness,
})
export type TreasuryPaymentAuthorityIntentCreationRequest =
  typeof TreasuryPaymentAuthorityIntentCreationRequest.Type

export const TreasuryPaymentAuthorityIntentCreationResult = S.Struct({
  intent: NexusTreasuryPayoutIntentRecord,
  projection: NexusTreasuryPayoutLedgerProjection,
  replayed: S.Boolean,
})
export type TreasuryPaymentAuthorityIntentCreationResult =
  typeof TreasuryPaymentAuthorityIntentCreationResult.Type

export const TreasuryPaymentAuthorityDispatchRequest = S.Struct({
  attempt: NexusTreasuryPayoutAttemptRecord,
  payoutIntentRef: S.String,
})
export type TreasuryPaymentAuthorityDispatchRequest =
  typeof TreasuryPaymentAuthorityDispatchRequest.Type

export const TreasuryPaymentAuthorityDispatchResult = S.Struct({
  attempt: NexusTreasuryPayoutAttemptRecord,
  intent: NexusTreasuryPayoutIntentRecord,
  projection: NexusTreasuryPayoutLedgerProjection,
})
export type TreasuryPaymentAuthorityDispatchResult =
  typeof TreasuryPaymentAuthorityDispatchResult.Type

export const TreasuryPaymentAuthorityReconciliationRequest = S.Struct({
  event: NexusTreasuryPayoutReconciliationEventRecord,
})
export type TreasuryPaymentAuthorityReconciliationRequest =
  typeof TreasuryPaymentAuthorityReconciliationRequest.Type

export const TreasuryPaymentAuthorityReconciliationResult = S.Struct({
  event: NexusTreasuryPayoutReconciliationEventRecord,
  projection: NexusTreasuryPayoutLedgerProjection,
})
export type TreasuryPaymentAuthorityReconciliationResult =
  typeof TreasuryPaymentAuthorityReconciliationResult.Type

export const TreasuryPaymentAuthorityReceiptProjectionRequest = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  record: S.Any,
  recordKind: NexusTreasuryPayoutLedgerRecordKind,
})
export type TreasuryPaymentAuthorityReceiptProjectionRequest = Readonly<{
  audience: typeof OpenAgentsPaymentPolicyAudience.Type
  record: NexusTreasuryPayoutLedgerRecord
  recordKind: NexusTreasuryPayoutLedgerRecordKind
}>

export type TreasuryPaymentAuthorityAdapterPreviewInput = Readonly<{
  intent: NexusTreasuryPayoutIntentRecord
}>

export type TreasuryPaymentAuthorityAdapterDispatchInput = Readonly<{
  attempt: NexusTreasuryPayoutAttemptRecord
  intent: NexusTreasuryPayoutIntentRecord
}>

export type TreasuryPaymentAuthorityAdapterReconcileInput = Readonly<{
  event: NexusTreasuryPayoutReconciliationEventRecord
}>

export type TreasuryPaymentAuthorityAdapter = Readonly<{
  adapterKind: NexusTreasuryPayoutAdapterKind
  dispatch: (
    input: TreasuryPaymentAuthorityAdapterDispatchInput,
  ) => Effect.Effect<NexusTreasuryPayoutAttemptRecord, TreasuryPaymentAuthorityError>
  preview: (
    input: TreasuryPaymentAuthorityAdapterPreviewInput,
  ) => Effect.Effect<TreasuryPaymentAuthorityPayoutPreview, TreasuryPaymentAuthorityError>
  reconcile: (
    input: TreasuryPaymentAuthorityAdapterReconcileInput,
  ) => Effect.Effect<
    NexusTreasuryPayoutReconciliationEventRecord,
    TreasuryPaymentAuthorityError
  >
}>

export type TreasuryPaymentAuthorityShape = Readonly<{
  createPayoutIntent: (
    request: TreasuryPaymentAuthorityIntentCreationRequest,
  ) => Effect.Effect<
    TreasuryPaymentAuthorityIntentCreationResult,
    TreasuryPaymentAuthorityError
  >
  dispatchPayout: (
    request: TreasuryPaymentAuthorityDispatchRequest,
  ) => Effect.Effect<TreasuryPaymentAuthorityDispatchResult, TreasuryPaymentAuthorityError>
  previewPayout: (
    request: TreasuryPaymentAuthorityPayoutPreviewRequest,
  ) => Effect.Effect<TreasuryPaymentAuthorityPayoutPreview, TreasuryPaymentAuthorityError>
  projectReceipt: (
    request: TreasuryPaymentAuthorityReceiptProjectionRequest,
  ) => Effect.Effect<NexusTreasuryPayoutLedgerProjection, TreasuryPaymentAuthorityError>
  readPauseState: Effect.Effect<TreasuryPaymentAuthorityPauseState>
  reconcilePayout: (
    request: TreasuryPaymentAuthorityReconciliationRequest,
  ) => Effect.Effect<
    TreasuryPaymentAuthorityReconciliationResult,
    TreasuryPaymentAuthorityError
  >
}>

export class TreasuryPaymentAuthority extends Context.Service<
  TreasuryPaymentAuthority,
  TreasuryPaymentAuthorityShape
>()('@openagentsinc/TreasuryPaymentAuthority') {}

export type TreasuryPaymentAuthorityDependencies = Readonly<{
  adapters: ReadonlyArray<TreasuryPaymentAuthorityAdapter>
  ledgerStore: NexusTreasuryPayoutLedgerStore
  pauseState?: TreasuryPaymentAuthorityPauseState | undefined
  spendPolicy?: TreasuryPaymentAuthoritySpendPolicy | undefined
}>

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeDestinationPattern =
  /(lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?preimage|preimage|raw[_-]?invoice|secret|wallet[_-]?(config|mnemonic|secret|state)|\S+@\S+)/i
const policyReceiptReasonPattern = /[^A-Za-z0-9_.:/-]+/g

const rejection = (
  reason: TreasuryPaymentAuthorityRejectionReason,
  message: string,
): TreasuryPaymentAuthorityError =>
  new TreasuryPaymentAuthorityError({ message, reason })

const validateAmount = (
  intent: NexusTreasuryPayoutIntentRecord,
): TreasuryPaymentAuthorityError | undefined => {
  try {
    const amount = decodeNexusTreasuryPayoutAmount(intent.amount)
    const spendCap = decodeNexusTreasuryPayoutAmount(intent.spendCap)

    if (
      amount.asset !== spendCap.asset ||
      amount.denomination !== spendCap.denomination
    ) {
      return rejection(
        'malformed_payout_amount',
        'Payout amount and spend cap must use the same asset and denomination.',
      )
    }

    if (amount.amountMinorUnits > spendCap.amountMinorUnits) {
      return rejection(
        'spend_cap_exceeded',
        'Payout amount exceeds the configured spend cap.',
      )
    }

    return undefined
  } catch {
    return rejection(
      'malformed_payout_amount',
      'Payout amount must be a non-negative integer with matching denomination.',
    )
  }
}

const validatePayoutTarget = (
  intent: NexusTreasuryPayoutIntentRecord,
): TreasuryPaymentAuthorityError | undefined =>
  !stableRefPattern.test(intent.payoutTargetRef) ||
  unsafeDestinationPattern.test(intent.payoutTargetRef)
    ? rejection(
        'malformed_payout_target',
        'Payout target must be a stable redacted payout target reference.',
      )
    : undefined

const amountGreaterThan = (
  left: NexusTreasuryPayoutAmount,
  right: NexusTreasuryPayoutAmount,
): boolean =>
  left.asset === right.asset &&
  left.denomination === right.denomination &&
  left.amountMinorUnits > right.amountMinorUnits

const intentPylonRefs = (
  intent: NexusTreasuryPayoutIntentRecord,
): ReadonlyArray<string> => [
  ...(
    intent.assignmentRef === null ? [] : [intent.assignmentRef]
  ),
  ...(intent.pylonJobRef === null ? [] : [intent.pylonJobRef]),
  ...intent.metadataRefs.filter(ref => ref.startsWith('pylon.')),
]

const pauseListIncludes = (
  refs: ReadonlyArray<string> | undefined,
  value: string,
): boolean => (refs ?? []).includes(value)

const pauseListOverlaps = (
  refs: ReadonlyArray<string> | undefined,
  values: ReadonlyArray<string>,
): boolean => values.some(value => (refs ?? []).includes(value))

const hasLargePayoutApproval = (
  intent: NexusTreasuryPayoutIntentRecord,
  spendPolicy: TreasuryPaymentAuthoritySpendPolicy | undefined,
): boolean =>
  spendPolicy === undefined ||
  spendPolicy.largePayoutThreshold === null ||
  !amountGreaterThan(intent.amount, spendPolicy.largePayoutThreshold) ||
  spendPolicy.largePayoutApprovalRefs.some(ref =>
    intent.metadataRefs.includes(ref) ||
    intent.acceptedWorkRefs.includes(ref),
  )

const validateIntentPolicy = (
  request: TreasuryPaymentAuthorityPayoutPreviewRequest,
  pauseState: TreasuryPaymentAuthorityPauseState,
  spendPolicy: TreasuryPaymentAuthoritySpendPolicy | undefined,
): TreasuryPaymentAuthorityError | undefined => {
  if (pauseState.authorityPaused) {
    return rejection('paused_authority', 'Treasury payment authority is paused.')
  }

  if (pauseState.pausedAdapters.includes(request.intent.adapterKind)) {
    return rejection(
      'paused_adapter',
      'Selected payout adapter is currently paused.',
    )
  }

  if (pauseListIncludes(pauseState.pausedActorRefs, request.intent.actorRef)) {
    return rejection(
      'paused_agent',
      'Payout actor identity is currently paused.',
    )
  }

  if (
    pauseListIncludes(
      pauseState.pausedPayoutTargetRefs,
      request.intent.payoutTargetRef,
    )
  ) {
    return rejection(
      'paused_payout_target',
      'Payout target is currently paused.',
    )
  }

  if (pauseListOverlaps(pauseState.pausedPylonRefs, intentPylonRefs(request.intent))) {
    return rejection('paused_pylon', 'Pylon payout identity is currently paused.')
  }

  if (request.walletReadiness !== 'ready') {
    return rejection(
      'stale_or_absent_wallet_readiness',
      'Payout dispatch requires fresh wallet readiness evidence.',
    )
  }

  if (request.intent.acceptedWorkRefs.length === 0) {
    return rejection(
      'missing_accepted_work_ref',
      'Payout intent requires accepted-work evidence.',
    )
  }

  if (request.intent.payoutTargetApprovalRef === null) {
    return rejection(
      'missing_payout_target_approval',
      'Payout intent requires payout target approval.',
    )
  }

  const targetError = validatePayoutTarget(request.intent)

  if (targetError !== undefined) {
    return targetError
  }

  const amountError = validateAmount(request.intent)

  if (amountError !== undefined) {
    return amountError
  }

  if (!hasLargePayoutApproval(request.intent, spendPolicy)) {
    return rejection(
      'large_payout_requires_approval',
      'Large payout requires explicit approval or a documented policy rule.',
    )
  }

  try {
    assertNexusTreasuryPayoutIntentSafe(request.intent)

    return undefined
  } catch (error) {
    if (error instanceof NexusTreasuryPayoutLedgerUnsafe) {
      return rejection('malformed_payout_target', error.reason)
    }

    return rejection(
      'malformed_payout_target',
      error instanceof Error ? error.message : String(error),
    )
  }
}

const policyReceiptRefSuffix = (
  intent: NexusTreasuryPayoutIntentRecord,
  reason: TreasuryPaymentAuthorityRejectionReason,
): string =>
  `${reason}.${intent.idempotencyKeyHash}`.replaceAll(
    policyReceiptReasonPattern,
    '_',
  ).slice(0, 180)

export const treasuryPaymentAuthorityPolicyDecisionReceipt = (
  intent: NexusTreasuryPayoutIntentRecord,
  reason: TreasuryPaymentAuthorityRejectionReason,
  payoutAttemptRef: string | null = null,
): NexusPaymentAuthorityReceiptRecord => ({
  archivedAt: null,
  audience: 'operator',
  createdAt: intent.updatedAt,
  eventRef: null,
  id: `nexus_policy_rejection_${policyReceiptRefSuffix(intent, reason)}`,
  metadataRefs: [
    `metadata.nexus.policy_rejected.${reason}`,
    intent.policySnapshotRef,
  ],
  payoutAttemptRef,
  payoutIntentRef: intent.payoutIntentRef,
  publicProjectionJson: JSON.stringify({
    policyDecision: 'rejected',
    rawMaterialStored: false,
    reason,
  }),
  receiptKind: 'policy_rejected',
  receiptRef: `receipt.nexus.policy_rejected.${policyReceiptRefSuffix(intent, reason)}`,
})

export const evaluateTreasuryPaymentAuthorityPolicy = (
  request: TreasuryPaymentAuthorityPayoutPreviewRequest,
  pauseState: TreasuryPaymentAuthorityPauseState,
  spendPolicy?: TreasuryPaymentAuthoritySpendPolicy | undefined,
): TreasuryPaymentAuthorityPolicyDecision => {
  const error = validateIntentPolicy(request, pauseState, spendPolicy)

  return error === undefined
    ? {
      allowed: true,
      reason: null,
      receipt: null,
    }
    : {
      allowed: false,
      reason: error.reason,
      receipt: treasuryPaymentAuthorityPolicyDecisionReceipt(
        request.intent,
        error.reason,
      ),
    }
}

const adapterByKind = (
  adapters: ReadonlyArray<TreasuryPaymentAuthorityAdapter>,
  adapterKind: NexusTreasuryPayoutAdapterKind,
): TreasuryPaymentAuthorityAdapter | undefined =>
  adapters.find(adapter => adapter.adapterKind === adapterKind)

const storeEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, TreasuryPaymentAuthorityError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      rejection(
        'adapter_unavailable',
        `${operation} failed: ${error instanceof Error ? error.message : String(error)}`,
      ),
  })

export const makeTreasuryPaymentAuthority = (
  dependencies: TreasuryPaymentAuthorityDependencies,
): TreasuryPaymentAuthorityShape => {
  const pauseState =
    dependencies.pauseState ?? {
      authorityPaused: false,
      pausedAdapters: [],
    }
  const spendPolicy = dependencies.spendPolicy
  const adapters = dependencies.adapters
  const ledgerStore = dependencies.ledgerStore

  const writePolicyRejectedReceipt = (
    intent: NexusTreasuryPayoutIntentRecord,
    error: TreasuryPaymentAuthorityError,
    payoutAttemptRef: string | null = null,
  ): Effect.Effect<void, TreasuryPaymentAuthorityError> =>
    storeEffect('create policy rejection receipt', () =>
      ledgerStore.createPaymentAuthorityReceipt(
        treasuryPaymentAuthorityPolicyDecisionReceipt(
          intent,
          error.reason,
          payoutAttemptRef,
        ),
      ),
    )

  const previewPayout = Effect.fn('TreasuryPaymentAuthority.previewPayout')(
    function* (request: TreasuryPaymentAuthorityPayoutPreviewRequest) {
      const policyError = validateIntentPolicy(
        request,
        pauseState,
        spendPolicy,
      )

      if (policyError !== undefined) {
        return yield* policyError
      }

      const adapter = adapterByKind(adapters, request.intent.adapterKind)

      if (adapter === undefined) {
        return yield* rejection(
          'adapter_unavailable',
          'No payout adapter is registered for the requested adapter kind.',
        )
      }

      return yield* adapter.preview({ intent: request.intent })
    },
  )

  const createPayoutIntent = Effect.fn(
    'TreasuryPaymentAuthority.createPayoutIntent',
  )(function* (request: TreasuryPaymentAuthorityIntentCreationRequest) {
    const policyError = validateIntentPolicy(
      request,
      pauseState,
      spendPolicy,
    )

    if (policyError !== undefined) {
      yield* writePolicyRejectedReceipt(request.intent, policyError)

      return yield* policyError
    }

    const existing = yield* storeEffect(
      'read payout intent by idempotency key hash',
      () =>
        ledgerStore.readPayoutIntentByIdempotencyKeyHash(
          request.intent.idempotencyKeyHash,
        ),
    )

    if (existing !== undefined) {
      return yield* rejection(
        'replayed_idempotency_key',
        'A payout intent already exists for this idempotency key.',
      )
    }

    yield* storeEffect('create payout intent', () =>
      ledgerStore.createPayoutIntent(request.intent),
    )

    return {
      intent: request.intent,
      projection: projectNexusTreasuryPayoutLedgerRecord(
        'intent',
        request.intent,
        'operator',
      ),
      replayed: false,
    }
  })

  const dispatchPayout = Effect.fn('TreasuryPaymentAuthority.dispatchPayout')(
    function* (request: TreasuryPaymentAuthorityDispatchRequest) {
      if (pauseState.authorityPaused) {
        return yield* rejection(
          'paused_authority',
          'Treasury payment authority is paused.',
        )
      }

      if (pauseState.pausedAdapters.includes(request.attempt.adapterKind)) {
        return yield* rejection(
          'paused_adapter',
          'Selected payout adapter is currently paused.',
        )
      }

      const intent = yield* storeEffect('read payout intent', () =>
        ledgerStore.readPayoutIntentByRef(request.payoutIntentRef),
      )

      if (intent === undefined) {
        return yield* rejection(
          'payout_intent_not_found',
          'Payout dispatch requires an existing payout intent.',
        )
      }

      const dispatchPolicyError = validateIntentPolicy(
        {
          intent,
          walletReadiness: 'ready',
        },
        pauseState,
        spendPolicy,
      )

      if (dispatchPolicyError !== undefined) {
        yield* writePolicyRejectedReceipt(
          intent,
          dispatchPolicyError,
          request.attempt.payoutAttemptRef,
        )

        return yield* dispatchPolicyError
      }

      const existingAttempt = yield* storeEffect(
        'read payout attempt by idempotency key hash',
        () =>
          ledgerStore.readPayoutAttemptByIdempotencyKeyHash(
            request.attempt.idempotencyKeyHash,
          ),
      )

      if (existingAttempt !== undefined) {
        return {
          attempt: existingAttempt,
          intent,
          projection: projectNexusTreasuryPayoutLedgerRecord(
            'attempt',
            existingAttempt,
            'operator',
          ),
        }
      }

      const adapter = adapterByKind(adapters, request.attempt.adapterKind)

      if (adapter === undefined) {
        return yield* rejection(
          'adapter_unavailable',
          'No payout adapter is registered for the requested adapter kind.',
        )
      }

      const attempt = yield* adapter.dispatch({
        attempt: request.attempt,
        intent,
      })

      yield* storeEffect('create payout attempt', () =>
        ledgerStore.createPayoutAttempt(attempt),
      )

      return {
        attempt,
        intent,
        projection: projectNexusTreasuryPayoutLedgerRecord(
          'attempt',
          attempt,
          'operator',
        ),
      }
    },
  )

  const reconcilePayout = Effect.fn('TreasuryPaymentAuthority.reconcilePayout')(
    function* (request: TreasuryPaymentAuthorityReconciliationRequest) {
      const adapter = adapterByKind(adapters, request.event.adapterKind)

      if (adapter === undefined) {
        return yield* rejection(
          'adapter_unavailable',
          'No payout adapter is registered for the requested adapter kind.',
        )
      }

      const event = yield* adapter.reconcile({ event: request.event })

      yield* storeEffect('create payout reconciliation event', () =>
        ledgerStore.createReconciliationEvent(event),
      )

      return {
        event,
        projection: projectNexusTreasuryPayoutLedgerRecord(
          'reconciliation_event',
          event,
          'operator',
        ),
      }
    },
  )

  const projectReceipt = Effect.fn('TreasuryPaymentAuthority.projectReceipt')(
    (request: TreasuryPaymentAuthorityReceiptProjectionRequest) =>
      Effect.succeed(
        projectNexusTreasuryPayoutLedgerRecord(
          request.recordKind,
          request.record,
          request.audience,
        ),
      ),
  )

  return {
    createPayoutIntent,
    dispatchPayout,
    previewPayout,
    projectReceipt,
    readPauseState: Effect.succeed(pauseState),
    reconcilePayout,
  }
}

export const TreasuryPaymentAuthorityLive = (
  dependencies: TreasuryPaymentAuthorityDependencies,
) =>
  Layer.succeed(
    TreasuryPaymentAuthority,
    makeTreasuryPaymentAuthority(dependencies),
  )

export const treasuryPaymentAuthorityReceiptProjection = (
  receipt: NexusPaymentAuthorityReceiptRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): NexusTreasuryPayoutLedgerProjection =>
  projectNexusTreasuryPayoutLedgerRecord('receipt', receipt, audience)
