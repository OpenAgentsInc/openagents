import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type AssetBoundaryAsset,
  validateAssetBoundary,
} from './asset-bitcoin-boundary'
import {
  type InferenceMonetizationKind,
  type ProviderAccountAuthMode,
  authorizeInferenceMonetization,
} from './inference-resale-authorization'
import {
  type NexusPaymentAuthorityReceiptRecord,
  type NexusPayoutTargetApprovalRecord,
  NexusTreasuryPayoutAdapterKind,
  type NexusTreasuryPayoutAmount,
  type NexusTreasuryPayoutAttemptRecord,
  type NexusTreasuryPayoutIntentRecord,
  type NexusTreasuryPayoutReconciliationEventRecord,
} from './nexus-treasury-payout-ledger'
import { realSettlementMovementMode } from './tassadar-run-settlement'
import {
  type TassadarRealSettlementGate,
  type TassadarSettlementAdapterDecision,
  resolveTassadarSettlementAdapter,
} from './tassadar-run-settlement-gate'

/**
 * Firm-up escrow -> real Bitcoin settlement against an EXECUTED verification
 * (openagents #5459, EPIC #5457 / §2H "Firm-up / escrow primitive").
 *
 * Today the NIP-90 firm-up escrow (`labor-escrow.ts`) releases INTERNAL msat
 * credits and gates release on a caller-supplied attestation string
 * (`verificationVerdictRef`). This module is the missing wire: it turns a
 * firmed-up, EXECUTED-verified labor job into a real Bitcoin payout to the
 * worker (provider) through the SAME owner-gated, receipt-first, idempotent
 * Spark treasury rail the Tassadar + hygiene lanes already use.
 *
 * It is a PURE decision + builder surface. It does not move money, dispatch,
 * read wallets, or write receipts. The caller drives the proven
 * `dispatchRealRunSettlementCore` Spark dispatch with the built records, exactly
 * as the hygiene lane does.
 *
 * MONEY-SAFETY (the hard invariants this module enforces, fail-closed):
 *   1. EXECUTED VERIFICATION, NOT MANUAL ATTESTATION. The release verdict must
 *      carry an executed-trace digest (`executedTraceDigestPrefix`) proving it
 *      came from an actually-run verification command — the same exact-replay /
 *      executed-check substrate the Pylon `labor-market.ts` runs for result
 *      delivery (`runVerificationCommand` -> exit-code -> verdict ref). A bare
 *      attestation string with no executed-trace digest is REFUSED. Payment
 *      clears only against a checkable, executed outcome referenced in the
 *      receipt.
 *   2. WORKER != VALIDATOR. The worker (provider) and the validator that
 *      produced the verdict must be distinct actors. A self-verified firm-up is
 *      never settleable.
 *   3. ONE PAYOUT PER ESCROW RELEASE. The settlement receipt ref is derived
 *      deterministically from the escrow release; a retry pays AT MOST once
 *      (enforced by the receipt-first dispatch core).
 *   4. THE OWNER GATE IS UNCHANGED. The real branch routes through the SAME
 *      `resolveTassadarSettlementAdapter` gate (`OPENAGENTS_REAL_SETTLEMENT_GATE`)
 *      with NO broadening. Arming a firm-up run = adding a firm-up run-ref of the
 *      shape `run.firmup.lane.YYYYMMDD` to the gate's `allowedRunRefs` with its
 *      own `maxPayoutSats` cap. No firm-up run-ref is added to the gate by this
 *      change; the default everywhere stays the honest internal-credit /
 *      simulation path until the owner deliberately arms a firm-up run.
 */

// The single allowed real adapter for a firm-up settlement: the proven Spark
// treasury rail, identical to the Tassadar/hygiene allowed adapter.
export const FirmupSettlementAllowedAdapterKind = 'spark_treasury' as const

// Hard per-payout sat cap for the firm-up lane. The gate's own `maxPayoutSats`
// is the binding cap at settle time; this is a defense-in-depth ceiling so a
// misread budget can never authorize an unbounded firm-up payout.
export const FirmupSettlementHardPerPayoutCapSats = 100_000

// The firm-up lane run-ref shape. Arming the lane adds a concrete ref of this
// shape (e.g. `run.firmup.lane.20260618`) to the gate's allowedRunRefs. The date
// suffix lets the operator rotate the lane scope per UTC day window without
// widening any other run's authority. This module never adds it; the operator
// does, deliberately, when authorizing the first real firm-up payout.
const FirmupLaneRunRefPattern = /^run\.firmup\.lane\.\d{8}$/

export const isFirmupLaneRunRef = (value: string): boolean =>
  FirmupLaneRunRefPattern.test(value)

/**
 * The honest verification basis for a firm-up settlement. Firm-up jobs are
 * verified by an EXECUTED verification command (the Pylon-side replay/verify
 * substrate), so the receipt states this `firmup_executed_verification` basis —
 * never a manual attestation and never an unrelated trace-replay verdict. The
 * receipt names the executed verdict + command refs as the reason money moved.
 */
export const FirmupSettlementVerificationClass =
  'firmup_executed_verification' as const

export const FirmupSettlementPolicySnapshotRef =
  'policy.public.firmup_lane.executed_verification.v1' as const

export const FirmupSettlementPayoutTargetApprovalPolicyRef =
  'policy.public.firmup_lane.settlement_payout' as const

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,260}$/
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const refIsPublicSafe = (ref: string): boolean =>
  safeRefPattern.test(ref) &&
  !containsProviderSecretMaterial(ref) &&
  !rawTimestampPattern.test(ref)

export class FirmupSettlementUnsafe extends S.TaggedErrorClass<FirmupSettlementUnsafe>()(
  'FirmupSettlementUnsafe',
  {
    reason: S.String,
  },
) {}

/**
 * An EXECUTED verification verdict for a firm-up labor job.
 *
 * This is the trust anchor that distinguishes a real firm-up settlement from a
 * manual attestation. It is produced by the validator actually re-running the
 * job's verification command (Pylon `runVerificationCommand`) and recording the
 * outcome:
 *   - `outcome` is the executed result: `verified` (exit 0) or `rejected`.
 *   - `executedTraceDigestPrefix` is a bounded, public-safe digest prefix of the
 *     executed verification trace. Its PRESENCE is what proves the verdict came
 *     from a real execution and not a hand-written attestation. A manual
 *     attestation cannot supply it.
 *   - `verificationCommandRef` is the command that was executed.
 *   - `validatorActorRef` is the actor that ran the verification (must differ
 *     from the worker).
 */
export const FirmupExecutedVerificationVerdict = S.Struct({
  executedTraceDigestPrefix: S.Trim.check(
    S.isNonEmpty(),
    S.isMaxLength(120),
    S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:#-]{0,118}$/),
  ),
  outcome: S.Literals(['verified', 'rejected']),
  validatorActorRef: S.Trim.check(S.isNonEmpty(), S.isMaxLength(261)),
  verdictRef: S.Trim.check(S.isNonEmpty(), S.isMaxLength(261)),
  verificationCommandRef: S.Trim.check(S.isNonEmpty(), S.isMaxLength(261)),
})
export type FirmupExecutedVerificationVerdict =
  typeof FirmupExecutedVerificationVerdict.Type

const decodeVerdict = S.decodeUnknownOption(FirmupExecutedVerificationVerdict)

/**
 * Classify a release verdict input as an EXECUTED verification verdict or a
 * manual attestation. The release path uses this to decide whether the real
 * Bitcoin branch is even eligible: only an executed, verified verdict can clear
 * a real payout. Anything that does not decode (e.g. a bare attestation string,
 * or a verdict missing the executed-trace digest) is `manual_attestation` and
 * the caller stays on the internal-credit / simulation path, honestly.
 */
export type FirmupVerdictClassification =
  | Readonly<{
      kind: 'executed_verified'
      verdict: FirmupExecutedVerificationVerdict
    }>
  | Readonly<{
      kind: 'executed_rejected'
      verdict: FirmupExecutedVerificationVerdict
    }>
  | Readonly<{ kind: 'manual_attestation' }>

export const classifyFirmupReleaseVerdict = (
  value: unknown,
): FirmupVerdictClassification => {
  const decoded = decodeVerdict(value)

  if (decoded._tag === 'None') {
    return { kind: 'manual_attestation' }
  }

  const verdict = decoded.value

  return verdict.outcome === 'verified'
    ? { kind: 'executed_verified', verdict }
    : { kind: 'executed_rejected', verdict }
}

export type FirmupSettlementBlockedReason =
  | 'amount_not_positive'
  | 'amount_over_hard_cap'
  | 'asset_boundary_violation'
  | 'gate_decision_blocked'
  | 'monetization_not_authorized'
  | 'not_executed_verification'
  | 'not_firmup_lane_run_ref'
  | 'verification_rejected'
  | 'worker_validator_not_distinct'

export type FirmupSettlementDecisionInput = Readonly<{
  // The payout amount in sats (derived from the firmed-up escrow's budget).
  amountSats: number
  // The owner gate, read from env at the boundary.
  gate: TassadarRealSettlementGate
  // The contributor (worker / provider) whose registered Spark target is paid.
  providerActorRef: string
  // The contributor's resolved adapter request (always the Spark treasury rail
  // for a real firm-up settlement; defaults to simulation).
  requestedAdapterKind?: typeof NexusTreasuryPayoutAdapterKind.Type | undefined
  // RL-3 (#5460): the asset the firm-up job's qualifying REVENUE was sourced in.
  // A firm-up labor settlement is a Bitcoin-revenue / Bitcoin-share crossing, so
  // it defaults to `bitcoin`. A non-Bitcoin revenue basis (credit/USD/free) is
  // refused by the shared credit<->Bitcoin boundary: a Bitcoin payout may never
  // be funded by credit-class or free/promo revenue.
  revenueAsset?: AssetBoundaryAsset | undefined
  // RL-3 (#5460): the kind of inference monetization this labor stream is, for
  // the no-resale gate. Firm-up labor is AGENT LABOR (a worker uses their own
  // tools to produce + sell a verified result), so it defaults to
  // `agentic_work` (the allowed path). A `subscription_capacity_resale` is
  // refused unconditionally (non-waivable, scoped to subscription accounts).
  monetizationKind?: InferenceMonetizationKind | undefined
  // The provider's account auth mode, only consulted when the monetization kind
  // is `api_inference_gateway_resale` (API-key accounts only). Irrelevant for
  // agentic work; defaults to `api_key`.
  accountAuthMode?: ProviderAccountAuthMode | undefined
  // The firm-up lane run-ref the gate must allowlist (run.firmup.lane.YYYYMMDD).
  trainingRunRef: string
  // The executed verification verdict for the firm-up job.
  verdict: FirmupExecutedVerificationVerdict
}>

export type FirmupSettlementDecision = Readonly<{
  // The sats this settlement would pay (echoed from input; never negative).
  amountSats: number
  // Why settlement is not authorized for real, or null when authorized.
  blockedReason: FirmupSettlementBlockedReason | null
  // The gate decision (real-vs-simulation, typed blockedReason), or null when
  // we fail closed before reaching the gate.
  gateDecision: TassadarSettlementAdapterDecision | null
  // Public-safe refs only. Asserted secret-free by construction.
  publicProjectionRefs: ReadonlyArray<string>
  // Whether a REAL Bitcoin settlement is authorized (executed+verified verdict
  // AND worker!=validator AND firm-up run-ref AND the owner gate authorizes the
  // run/contributor/amount). False keeps the firm-up release on the honest
  // internal-credit / simulation path.
  realAuthorized: boolean
}>

/**
 * Decide whether a firmed-up labor job may settle to real Bitcoin, and through
 * which adapter. Pure and fail-closed:
 *   - the verdict must be EXECUTED + `verified` (not a manual attestation, not a
 *     rejected outcome),
 *   - the worker (provider) must differ from the validator that produced it,
 *   - the run-ref must be a firm-up lane run-ref,
 *   - the amount must be positive and within the hard cap, and
 *   - the owner gate must authorize the real branch for this run + contributor +
 *     amount (the gate's own per-payout and daily caps still bind).
 * Any failing condition yields `realAuthorized: false` with a typed reason; the
 * caller then records the honest internal-credit / simulation chain instead.
 */
export const decideFirmupBitcoinSettlement = (
  input: FirmupSettlementDecisionInput,
): FirmupSettlementDecision => {
  const amountSats = input.amountSats
  const publicProjectionRefs = [
    input.verdict.verdictRef,
    input.verdict.verificationCommandRef,
    `verification.public.firmup_lane.${FirmupSettlementVerificationClass}`,
    `executed_trace.public.firmup_lane.${input.verdict.executedTraceDigestPrefix}`,
  ]
  const unsafe = publicProjectionRefs.find(ref => !refIsPublicSafe(ref))

  if (unsafe !== undefined) {
    throw new FirmupSettlementUnsafe({
      reason:
        'Firm-up settlement projection refs must be public-safe (no provider, payment, wallet, secret, or raw-timestamp material).',
    })
  }

  const blocked = (
    blockedReason: FirmupSettlementBlockedReason,
    gateDecision: TassadarSettlementAdapterDecision | null,
  ): FirmupSettlementDecision => ({
    amountSats,
    blockedReason,
    gateDecision,
    publicProjectionRefs,
    realAuthorized: false,
  })

  // 1. EXECUTED VERIFICATION, NOT MANUAL ATTESTATION. The verdict must carry the
  //    executed-trace digest; the decode already enforced its presence/shape, so
  //    reaching here with a non-empty digest is the executed path. We re-assert
  //    the outcome is `verified`: a rejected executed verification never pays.
  if (input.verdict.executedTraceDigestPrefix.trim() === '') {
    return blocked('not_executed_verification', null)
  }

  if (input.verdict.outcome !== 'verified') {
    return blocked('verification_rejected', null)
  }

  // 2. WORKER != VALIDATOR. A self-verified firm-up is never settleable.
  if (
    input.providerActorRef.trim() === '' ||
    input.providerActorRef.trim() === input.verdict.validatorActorRef.trim()
  ) {
    return blocked('worker_validator_not_distinct', null)
  }

  // 3. FIRM-UP LANE RUN-REF. The run allowlist boundary the gate keys on.
  if (!isFirmupLaneRunRef(input.trainingRunRef)) {
    return blocked('not_firmup_lane_run_ref', null)
  }

  // 3a. CREDIT<->BITCOIN ASSET BOUNDARY (RL-3 #5460, shared guard). A firm-up
  //     settlement always pays withdrawable Bitcoin, so the contributor asset is
  //     `bitcoin`. The revenue basis must be Bitcoin: credit/USD (Stripe-credit)
  //     or free/promo revenue may NOT fund a withdrawable Bitcoin payout. Fail
  //     closed on any violation before touching the owner gate or any money.
  const boundaryViolation = validateAssetBoundary({
    contributorAsset: 'bitcoin',
    movement: 'payout',
    revenueAsset: input.revenueAsset ?? 'bitcoin',
  })

  if (boundaryViolation !== null) {
    return blocked('asset_boundary_violation', null)
  }

  // 3b. NO-RESALE GATE (RL-3 #5460, shared guard). Firm-up is AGENT LABOR (the
  //     worker uses their own tools to produce + sell a verified result), the
  //     allowed `agentic_work` path. A consumer SUBSCRIPTION-seat resale is
  //     refused unconditionally (non-waivable, scoped to subscription accounts);
  //     API-inference gateway resale stays allowed on an API-key account with
  //     the full ref chain. Fail closed if the monetization is not authorized.
  const monetizationDecision = authorizeInferenceMonetization({
    kind: input.monetizationKind ?? 'agentic_work',
    ...(input.accountAuthMode === undefined
      ? {}
      : { accountAuthMode: input.accountAuthMode }),
  })

  if (!monetizationDecision.authorized) {
    return blocked('monetization_not_authorized', null)
  }

  if (!Number.isInteger(amountSats) || amountSats <= 0) {
    return blocked('amount_not_positive', null)
  }

  if (amountSats > FirmupSettlementHardPerPayoutCapSats) {
    return blocked('amount_over_hard_cap', null)
  }

  // 4. THE OWNER GATE (unchanged). Route through the SAME gate as Tassadar +
  //    hygiene. Arming = adding this firm-up run-ref to allowedRunRefs.
  const gateDecision = resolveTassadarSettlementAdapter({
    amountSats,
    contributorRef: input.providerActorRef,
    gate: input.gate,
    requestedAdapterKind: input.requestedAdapterKind ?? 'simulation',
    trainingRunRef: input.trainingRunRef,
  })

  if (!gateDecision.realAuthorized) {
    return blocked('gate_decision_blocked', gateDecision)
  }

  return {
    amountSats,
    blockedReason: null,
    gateDecision,
    publicProjectionRefs,
    realAuthorized: true,
  }
}

export type FirmupSettlementRecords = Readonly<{
  amountSats: number
  attempt: NexusTreasuryPayoutAttemptRecord
  contributorRef: string
  intent: NexusTreasuryPayoutIntentRecord
  reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord
  settlementReceipt: NexusPaymentAuthorityReceiptRecord
  settlementReceiptRef: string
  targetApproval: NexusPayoutTargetApprovalRecord
}>

const stableSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 120)

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const bitcoinAmount = (sats: number): NexusTreasuryPayoutAmount => ({
  amountMinorUnits: sats * 1000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
})

/**
 * Build the settlement ledger chain for ONE firmed-up, EXECUTED-verified labor
 * job (openagents #5459). Structurally identical to the Tassadar / hygiene
 * builders — same intent -> attempt -> reconciliation -> settlement_recorded
 * chain, same deterministic idempotency hashes, same redacted destination ref —
 * so it drives the SAME `dispatchRealRunSettlementCore` Spark rail. The ONLY
 * differences are honest:
 *   - the verification basis is `firmup_executed_verification` (executed verify
 *     command + worker!=validator verdict), and
 *   - the receipt projection cites the EXECUTED verdict + command + escrow refs
 *     as the reason money moved.
 *
 * `amountSats` is the gate-bound amount the decision already authorized. The
 * caller (`decideFirmupBitcoinSettlement`) has confirmed the executed verdict,
 * worker!=validator, the firm-up run-ref, and the gate before this is reached on
 * the real branch.
 */
export const buildFirmupBitcoinSettlement = (
  input: Readonly<{
    adapterKind: typeof NexusTreasuryPayoutAdapterKind.Type
    amountSats: number
    escrowRef: string
    idempotencyDigestHex: string
    nowIso: string
    operatorApprovalRef: string
    payoutTargetApprovalRef: string
    payoutTargetRef: string
    providerActorRef: string
    trainingRunRef: string
    validatorActorRef: string
    verdict: FirmupExecutedVerificationVerdict
    workRequestRef: string
  }>,
): FirmupSettlementRecords => {
  const adapterKind = input.adapterKind
  const moneyMovement = realSettlementMovementMode(adapterKind)
  const suffix = stableSuffix(`sha256_${input.idempotencyDigestHex}`)
  const contributorRef = input.providerActorRef.trim()
  const amount = bitcoinAmount(input.amountSats)
  const idempotencyHash = (stage: string): string =>
    `hash.firmup_lane_settlement.${stage}.${input.idempotencyDigestHex}`
  // The accepted-work basis is the EXECUTED firm-up evidence: the work request,
  // the escrow, and the executed verification verdict + command. There is no
  // manual attestation ref and no unrelated trace-replay challenge ref here.
  const acceptedWorkRefs = uniqueRefs([
    input.workRequestRef,
    input.escrowRef,
    input.verdict.verdictRef,
  ])
  const executedTraceRef = `executed_trace.public.firmup_lane.${input.verdict.executedTraceDigestPrefix}`
  const metadataRefs = uniqueRefs([
    input.trainingRunRef,
    input.workRequestRef,
    input.escrowRef,
    input.verdict.verdictRef,
    input.verdict.verificationCommandRef,
    executedTraceRef,
    input.operatorApprovalRef,
    `verification.public.firmup_lane.${FirmupSettlementVerificationClass}`,
    'metadata.firmup_lane.settlement.executed_verification',
  ])
  const redactedDestinationRef = `destination.redacted.firmup_lane_settlement.${suffix}`

  const targetApproval: NexusPayoutTargetApprovalRecord = {
    agentRef: 'agent.artanis',
    approvalPolicyRef: FirmupSettlementPayoutTargetApprovalPolicyRef,
    approvalRef: input.payoutTargetApprovalRef,
    approvedByRef: 'operator.openagents.firmup_lane_settlement',
    archivedAt: null,
    createdAt: input.nowIso,
    expiresAt: null,
    id: `nexus_payout_target_approval_firmup_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('approval'),
    ownerUserId: 'user_openagents_operator',
    payoutTargetRef: input.payoutTargetRef,
    publicProjectionJson: JSON.stringify({
      escrowRef: input.escrowRef,
      pylonRef: contributorRef,
      state: 'active',
      trainingRunRef: input.trainingRunRef,
      workRequestRef: input.workRequestRef,
    }),
    pylonRef: contributorRef,
    redactedDestinationRef,
    scopeRefs: uniqueRefs([
      input.trainingRunRef,
      input.workRequestRef,
      input.escrowRef,
    ]),
    status: 'active',
    updatedAt: input.nowIso,
  }

  const intent: NexusTreasuryPayoutIntentRecord = {
    acceptedWorkRefs,
    actorRef: 'agent.artanis',
    adapterKind,
    amount,
    archivedAt: null,
    artanisDispatchRef: `artanis_dispatch.firmup_lane_settlement.${suffix}`,
    assignmentRef: null,
    buyerPaymentRef: null,
    createdAt: input.nowIso,
    id: `nexus_treasury_payout_intent_firmup_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('intent'),
    metadataRefs,
    ownerUserId: null,
    payoutIntentRef: `payout_intent.firmup_lane_settlement.${suffix}`,
    payoutTargetApprovalRef: input.payoutTargetApprovalRef,
    payoutTargetRef: input.payoutTargetRef,
    policySnapshotRef: FirmupSettlementPolicySnapshotRef,
    publicProjectionJson: JSON.stringify({
      acceptedWork: true,
      adapter: adapterKind,
      amountSats: input.amountSats,
      moneyMovement,
      operatorApproved: true,
      state: 'approved',
      trainingRunRef: input.trainingRunRef,
      verificationBasis: FirmupSettlementVerificationClass,
    }),
    pylonJobRef: null,
    sourceKind: 'accepted_work',
    spendCap: amount,
    status: 'approved',
    updatedAt: input.nowIso,
  }

  const attempt: NexusTreasuryPayoutAttemptRecord = {
    adapterAttemptRef: `adapter_attempt.firmup_lane_settlement.${adapterKind}.${suffix}`,
    adapterKind,
    amount,
    archivedAt: null,
    createdAt: input.nowIso,
    id: `nexus_treasury_payout_attempt_firmup_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('attempt'),
    metadataRefs,
    payoutAttemptRef: `payout_attempt.firmup_lane_settlement.${suffix}`,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: input.amountSats,
      moneyMovement,
      trainingRunRef: input.trainingRunRef,
      verificationBasis: FirmupSettlementVerificationClass,
    }),
    redactedDestinationRef,
    redactedPaymentRef: null,
    status: 'confirmed',
    updatedAt: input.nowIso,
  }

  const reconciliationEvent: NexusTreasuryPayoutReconciliationEventRecord = {
    adapterKind,
    archivedAt: null,
    createdAt: input.nowIso,
    eventRef: `reconciliation.firmup_lane_settlement.${suffix}`,
    externalEventRef: `external_event.firmup_lane_settlement.${adapterKind}.${suffix}`,
    id: `nexus_treasury_reconciliation_firmup_lane_${suffix}`,
    idempotencyKeyHash: idempotencyHash('reconciliation'),
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    providerRef: `provider.${adapterKind}`,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: input.amountSats,
      moneyMovement,
      trainingRunRef: input.trainingRunRef,
      verificationBasis: FirmupSettlementVerificationClass,
    }),
    resultRef: `result.firmup_lane_settlement.${suffix}`,
    status: 'matched',
  }

  const settlementReceiptRef = `receipt.nexus.firmup_lane_settlement.${suffix}`
  const settlementReceipt: NexusPaymentAuthorityReceiptRecord = {
    archivedAt: null,
    audience: 'public',
    createdAt: input.nowIso,
    eventRef: reconciliationEvent.eventRef,
    id: `nexus_payment_authority_receipt_firmup_lane_${suffix}`,
    metadataRefs,
    payoutAttemptRef: attempt.payoutAttemptRef,
    payoutIntentRef: intent.payoutIntentRef,
    publicProjectionJson: JSON.stringify({
      adapter: adapterKind,
      amountSats: input.amountSats,
      asset: 'bitcoin',
      contributorRef,
      escrowRef: input.escrowRef,
      // HONEST basis. The receipt names the EXECUTED verification verdict +
      // command + executed-trace digest + escrow + work request as the reason
      // money moved — never a manual attestation. The verdict came from an
      // actually-run verification command on a validator distinct from the
      // worker.
      executedTraceRef,
      moneyMovement,
      state: 'settled',
      trainingRunRef: input.trainingRunRef,
      validatorActorRef: input.validatorActorRef.trim(),
      verdictRef: input.verdict.verdictRef,
      verificationBasis: FirmupSettlementVerificationClass,
      verificationCommandRef: input.verdict.verificationCommandRef,
      workRequestRef: input.workRequestRef,
    }),
    receiptKind: 'settlement_recorded',
    receiptRef: settlementReceiptRef,
  }

  return {
    amountSats: input.amountSats,
    attempt,
    contributorRef,
    intent,
    reconciliationEvent,
    settlementReceipt,
    settlementReceiptRef,
    targetApproval,
  }
}
