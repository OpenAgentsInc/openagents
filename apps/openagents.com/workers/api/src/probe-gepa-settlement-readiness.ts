import { Schema as S } from 'effect'

import {
  PylonGepaMetricCallAssignmentRecord,
  PylonGepaMetricCallPaymentMode,
  assertPylonGepaMetricCallPublicRefs,
  pylonGepaMetricCallAcceptedWorkClaimAllowed,
} from './pylon-gepa-metric-call-assignments'
import { publicRefSegment, uniqueRefs } from './public-ref-format'

export const ProbeGepaSettlementReadinessSchemaVersion =
  'omega.probe_gepa_settlement_readiness.v1'
export const ProbeGepaSettlementAccountingSchemaVersion =
  'omega.probe_gepa_settlement_accounting.v1'

export const ProbeGepaSettlementRequestedPaymentMode = S.Literals([
  'operator_credit',
  'payable_pending_settlement',
  'settled_bitcoin',
  'unpaid_smoke',
])
export type ProbeGepaSettlementRequestedPaymentMode =
  typeof ProbeGepaSettlementRequestedPaymentMode.Type

export const ProbeGepaSettlementPublicClaimState = S.Literals([
  'no_spend',
  'operator_credit',
  'payable_pending_settlement',
  'settled_bitcoin',
])
export type ProbeGepaSettlementPublicClaimState =
  typeof ProbeGepaSettlementPublicClaimState.Type

export const ProbeGepaSettlementReadinessDecision = S.Literals([
  'ready',
  'rejected',
])
export type ProbeGepaSettlementReadinessDecision =
  typeof ProbeGepaSettlementReadinessDecision.Type

export const ProbeGepaSettlementReadinessState = S.Literals([
  'operator_credit_ready',
  'payable_pending_settlement_ready',
  'rejected',
  'settled_bitcoin_ready',
  'unpaid_smoke_complete',
])
export type ProbeGepaSettlementReadinessState =
  typeof ProbeGepaSettlementReadinessState.Type

export class ProbeGepaSettlementAccountingRecord extends S.Class<ProbeGepaSettlementAccountingRecord>(
  'ProbeGepaSettlementAccountingRecord',
)({
  accountingRef: S.String,
  assignmentRef: S.String,
  closeoutResultRefs: S.Array(S.String),
  operatorRef: S.String,
  paymentReceiptRefs: S.Array(S.String),
  proofBundleRefs: S.Array(S.String),
  resourceUsageRefs: S.Array(S.String),
  schemaVersion: S.Literal(ProbeGepaSettlementAccountingSchemaVersion),
  settlementReceiptRefs: S.Array(S.String),
  verifierResultRefs: S.Array(S.String),
}) {}

export class ProbeGepaSettlementReadinessInput extends S.Class<ProbeGepaSettlementReadinessInput>(
  'ProbeGepaSettlementReadinessInput',
)({
  accountingRecords: S.Array(ProbeGepaSettlementAccountingRecord),
  assignmentRecords: S.Array(PylonGepaMetricCallAssignmentRecord),
  batchRef: S.String,
  operatorAccountingRefs: S.Array(S.String),
  publicClaimState: ProbeGepaSettlementPublicClaimState,
  requestedPaymentMode: ProbeGepaSettlementRequestedPaymentMode,
}) {}

export class ProbeGepaSettlementReadinessResult extends S.Class<ProbeGepaSettlementReadinessResult>(
  'ProbeGepaSettlementReadinessResult',
)({
  acceptedAssignmentRefs: S.Array(S.String),
  accountingRefs: S.Array(S.String),
  batchRef: S.String,
  blockerRefs: S.Array(S.String),
  decision: ProbeGepaSettlementReadinessDecision,
  noSpendBatchClaimAllowed: S.Boolean,
  operatorAccountingRefs: S.Array(S.String),
  payableWorkClaimAllowed: S.Boolean,
  publicClaimState: ProbeGepaSettlementPublicClaimState,
  publicSummaryLabel: S.String,
  readinessDecisionRef: S.String,
  readinessState: ProbeGepaSettlementReadinessState,
  requestedPaymentMode: ProbeGepaSettlementRequestedPaymentMode,
  schemaVersion: S.Literal(ProbeGepaSettlementReadinessSchemaVersion),
  settledBitcoinClaimAllowed: S.Boolean,
}) {}

export class ProbeGepaSettlementReadinessUnsafe extends S.TaggedErrorClass<ProbeGepaSettlementReadinessUnsafe>()(
  'ProbeGepaSettlementReadinessUnsafe',
  {
    reason: S.String,
  },
) {}

const publicClaimRank = (
  publicClaimState: ProbeGepaSettlementPublicClaimState,
): number => {
  if (publicClaimState === 'settled_bitcoin') {
    return 3
  }

  if (publicClaimState === 'payable_pending_settlement') {
    return 2
  }

  if (publicClaimState === 'operator_credit') {
    return 1
  }

  return 0
}

const paymentModeClaimRank = (
  paymentMode: ProbeGepaSettlementRequestedPaymentMode,
): number => {
  if (paymentMode === 'settled_bitcoin') {
    return 3
  }

  if (paymentMode === 'payable_pending_settlement') {
    return 2
  }

  if (paymentMode === 'operator_credit') {
    return 1
  }

  return 0
}

const readinessStateFor = (
  paymentMode: ProbeGepaSettlementRequestedPaymentMode,
): ProbeGepaSettlementReadinessState => {
  if (paymentMode === 'settled_bitcoin') {
    return 'settled_bitcoin_ready'
  }

  if (paymentMode === 'payable_pending_settlement') {
    return 'payable_pending_settlement_ready'
  }

  if (paymentMode === 'operator_credit') {
    return 'operator_credit_ready'
  }

  return 'unpaid_smoke_complete'
}

const publicSummaryLabelFor = (
  readinessState: ProbeGepaSettlementReadinessState,
): string => {
  if (readinessState === 'settled_bitcoin_ready') {
    return 'settled bitcoin receipts present'
  }

  if (readinessState === 'payable_pending_settlement_ready') {
    return 'payable pending settlement; accounting receipts present'
  }

  if (readinessState === 'operator_credit_ready') {
    return 'operator credit recorded; no bitcoin settlement claim'
  }

  if (readinessState === 'unpaid_smoke_complete') {
    return 'unpaid smoke complete; no payout claim'
  }

  return 'settlement readiness rejected'
}

const refsCover = (
  accountingRefs: ReadonlyArray<string>,
  assignmentRefs: ReadonlyArray<string>,
): boolean =>
  assignmentRefs.every(assignmentRef => accountingRefs.includes(assignmentRef))

const accountingRecordsByAssignment = (
  records: ReadonlyArray<ProbeGepaSettlementAccountingRecord>,
): Map<string, ProbeGepaSettlementAccountingRecord> =>
  new Map(records.map(record => [record.assignmentRef, record]))

const safeAccountingRefs = (
  records: ReadonlyArray<ProbeGepaSettlementAccountingRecord>,
): ReadonlyArray<string> =>
  uniqueRefs(records.map(record => record.accountingRef))

const normalizeAccountingRecord = (
  record: ProbeGepaSettlementAccountingRecord,
): ProbeGepaSettlementAccountingRecord =>
  new ProbeGepaSettlementAccountingRecord({
    ...record,
    closeoutResultRefs: uniqueRefs(record.closeoutResultRefs),
    paymentReceiptRefs: uniqueRefs(record.paymentReceiptRefs),
    proofBundleRefs: uniqueRefs(record.proofBundleRefs),
    resourceUsageRefs: uniqueRefs(record.resourceUsageRefs),
    settlementReceiptRefs: uniqueRefs(record.settlementReceiptRefs),
    verifierResultRefs: uniqueRefs(record.verifierResultRefs),
  })

const normalizeInput = (
  input: ProbeGepaSettlementReadinessInput,
): ProbeGepaSettlementReadinessInput =>
  new ProbeGepaSettlementReadinessInput({
    ...input,
    accountingRecords: input.accountingRecords.map(normalizeAccountingRecord),
    operatorAccountingRefs: uniqueRefs(input.operatorAccountingRefs),
  })

const assertPublicRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  try {
    assertPylonGepaMetricCallPublicRefs(label, refs)
  } catch (error) {
    throw new ProbeGepaSettlementReadinessUnsafe({
      reason:
        error instanceof Error
          ? error.message
          : `${label} contains unsafe refs.`,
    })
  }
}

const accountingEvidenceBlockers = (
  assignment: PylonGepaMetricCallAssignmentRecord,
  maybeAccounting: ProbeGepaSettlementAccountingRecord | undefined,
): ReadonlyArray<string> => {
  if (maybeAccounting === undefined) {
    return [
      `blocker.probe_gepa.settlement.accounting_missing.${publicRefSegment(
        assignment.assignmentRef,
        'batch',
      )}`,
    ]
  }

  return [
    ...(!refsCover(
      maybeAccounting.closeoutResultRefs,
      assignment.closeoutResultRefs,
    )
      ? [
          `blocker.probe_gepa.settlement.closeout_accounting_missing.${publicRefSegment(
            assignment.assignmentRef,
            'batch',
          )}`,
        ]
      : []),
    ...(!refsCover(maybeAccounting.proofBundleRefs, assignment.proofBundleRefs)
      ? [
          `blocker.probe_gepa.settlement.proof_accounting_missing.${publicRefSegment(
            assignment.assignmentRef,
            'batch',
          )}`,
        ]
      : []),
    ...(!refsCover(
      maybeAccounting.resourceUsageRefs,
      assignment.resourceUsageRefs,
    )
      ? [
          `blocker.probe_gepa.settlement.resource_accounting_missing.${publicRefSegment(
            assignment.assignmentRef,
            'batch',
          )}`,
        ]
      : []),
    ...(!refsCover(
      maybeAccounting.verifierResultRefs,
      assignment.verifierResultRefs,
    )
      ? [
          `blocker.probe_gepa.settlement.verifier_accounting_missing.${publicRefSegment(
            assignment.assignmentRef,
            'batch',
          )}`,
        ]
      : []),
  ]
}

const receiptBlockers = (
  paymentMode: ProbeGepaSettlementRequestedPaymentMode,
  assignment: PylonGepaMetricCallAssignmentRecord,
  maybeAccounting: ProbeGepaSettlementAccountingRecord | undefined,
): ReadonlyArray<string> => {
  if (paymentMode === 'unpaid_smoke' || maybeAccounting === undefined) {
    return []
  }

  const missingPaymentReceipt =
    maybeAccounting.paymentReceiptRefs.length === 0 &&
    assignment.paymentReceiptRefs.length === 0
  const missingSettlementReceipt =
    maybeAccounting.settlementReceiptRefs.length === 0 &&
    assignment.settlementReceiptRefs.length === 0

  return [
    ...(missingPaymentReceipt
      ? [
          `blocker.probe_gepa.settlement.payment_receipt_missing.${publicRefSegment(
            assignment.assignmentRef,
            'batch',
          )}`,
        ]
      : []),
    ...(paymentMode === 'settled_bitcoin' && missingSettlementReceipt
      ? [
          `blocker.probe_gepa.settlement.settlement_receipt_missing.${publicRefSegment(
            assignment.assignmentRef,
            'batch',
          )}`,
        ]
      : []),
  ]
}

const unpaidSmokeBlockers = (
  assignments: ReadonlyArray<PylonGepaMetricCallAssignmentRecord>,
  accountingRecords: ReadonlyArray<ProbeGepaSettlementAccountingRecord>,
): ReadonlyArray<string> =>
  [
    ...(assignments.some(
      assignment =>
        assignment.paymentReceiptRefs.length > 0 ||
        assignment.settlementReceiptRefs.length > 0 ||
        assignment.paymentMode !== 'unpaid_smoke',
    )
      ? ['blocker.probe_gepa.settlement.unpaid_smoke_contains_paid_claims']
      : []),
    ...(accountingRecords.some(
      accounting =>
        accounting.paymentReceiptRefs.length > 0 ||
        accounting.settlementReceiptRefs.length > 0,
    )
      ? ['blocker.probe_gepa.settlement.unpaid_smoke_accounting_has_paid_refs']
      : []),
  ]

export const evaluateProbeGepaSettlementReadiness = (
  input: ProbeGepaSettlementReadinessInput,
): ProbeGepaSettlementReadinessResult => {
  const normalized = normalizeInput(
    S.decodeUnknownSync(ProbeGepaSettlementReadinessInput)(input),
  )
  const acceptedAssignments = normalized.assignmentRecords.filter(assignment =>
    pylonGepaMetricCallAcceptedWorkClaimAllowed(assignment),
  )
  const acceptedAssignmentRefs = uniqueRefs(
    acceptedAssignments.map(assignment => assignment.assignmentRef),
  )
  const accountingByAssignment = accountingRecordsByAssignment(
    normalized.accountingRecords,
  )
  const accountingRefs = safeAccountingRefs(normalized.accountingRecords)

  assertPublicRefs('Probe GEPA settlement identity refs', [
    normalized.batchRef,
    ...acceptedAssignmentRefs,
    ...accountingRefs,
    ...normalized.operatorAccountingRefs,
  ])
  normalized.accountingRecords.forEach(accounting => {
    assertPublicRefs('Probe GEPA settlement accounting refs', [
      accounting.accountingRef,
      accounting.assignmentRef,
      accounting.operatorRef,
      ...accounting.closeoutResultRefs,
      ...accounting.paymentReceiptRefs,
      ...accounting.proofBundleRefs,
      ...accounting.resourceUsageRefs,
      ...accounting.settlementReceiptRefs,
      ...accounting.verifierResultRefs,
    ])
  })

  const paidMode = normalized.requestedPaymentMode !== 'unpaid_smoke'
  const blockerRefs = uniqueRefs([
    ...(paidMode && acceptedAssignments.length === 0
      ? ['blocker.probe_gepa.settlement.no_accepted_work']
      : []),
    ...(paidMode && normalized.operatorAccountingRefs.length === 0
      ? ['blocker.probe_gepa.settlement.operator_accounting_refs_missing']
      : []),
    ...(publicClaimRank(normalized.publicClaimState) >
    paymentModeClaimRank(normalized.requestedPaymentMode)
      ? ['blocker.probe_gepa.settlement.public_claim_overstates_mode']
      : []),
    ...(normalized.requestedPaymentMode === 'unpaid_smoke'
      ? unpaidSmokeBlockers(acceptedAssignments, normalized.accountingRecords)
      : []),
    ...acceptedAssignments.flatMap(assignment => {
      const maybeAccounting = accountingByAssignment.get(assignment.assignmentRef)

      return paidMode
        ? [
            ...accountingEvidenceBlockers(assignment, maybeAccounting),
            ...receiptBlockers(
              normalized.requestedPaymentMode,
              assignment,
              maybeAccounting,
            ),
          ]
        : []
    }),
  ])
  const ready = blockerRefs.length === 0
  const readinessState = ready
    ? readinessStateFor(normalized.requestedPaymentMode)
    : 'rejected'
  const payableWorkClaimAllowed =
    ready &&
    (normalized.requestedPaymentMode === 'payable_pending_settlement' ||
      normalized.requestedPaymentMode === 'settled_bitcoin')
  const settledBitcoinClaimAllowed =
    ready && normalized.requestedPaymentMode === 'settled_bitcoin'

  return new ProbeGepaSettlementReadinessResult({
    acceptedAssignmentRefs,
    accountingRefs,
    batchRef: normalized.batchRef,
    blockerRefs,
    decision: ready ? 'ready' : 'rejected',
    noSpendBatchClaimAllowed:
      ready && normalized.requestedPaymentMode === 'unpaid_smoke',
    operatorAccountingRefs: normalized.operatorAccountingRefs,
    payableWorkClaimAllowed,
    publicClaimState: normalized.publicClaimState,
    publicSummaryLabel: publicSummaryLabelFor(readinessState),
    readinessDecisionRef: `settlement_readiness.probe_gepa.${publicRefSegment(
      normalized.batchRef,
      'batch',
    )}.${readinessState}`,
    readinessState,
    requestedPaymentMode: normalized.requestedPaymentMode,
    schemaVersion: ProbeGepaSettlementReadinessSchemaVersion,
    settledBitcoinClaimAllowed,
  })
}

export const probeGepaSettlementAssignmentModeReady = (
  result: ProbeGepaSettlementReadinessResult,
  paymentMode: PylonGepaMetricCallPaymentMode,
): boolean =>
  result.decision === 'ready' && result.requestedPaymentMode === paymentMode
