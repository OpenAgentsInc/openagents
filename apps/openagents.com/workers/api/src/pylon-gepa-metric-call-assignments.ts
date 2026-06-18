import { Schema as S } from 'effect'

import { publicRefSegment, uniqueRefs } from './public-ref-format'

export const PylonGepaMetricCallAssignmentSchemaVersion =
  'omega.pylon_gepa_metric_call_assignment.v1'
export const PylonGepaMetricCallCoordinatorImportSchemaVersion =
  'omega.pylon_gepa_metric_call_coordinator_import.v1'

export const PylonGepaMetricCallPaymentMode = S.Literals([
  'operator_credit',
  'payable_pending_settlement',
  'rejected_no_pay',
  'settled_bitcoin',
  'unpaid_smoke',
])
export type PylonGepaMetricCallPaymentMode =
  typeof PylonGepaMetricCallPaymentMode.Type

export const PylonGepaMetricCallLifecycleState = S.Literals([
  'accepted',
  'accepted_work',
  'created',
  'progress_reported',
  'rejected_work',
  'result_submitted',
  'running',
])
export type PylonGepaMetricCallLifecycleState =
  typeof PylonGepaMetricCallLifecycleState.Type

export const PylonGepaMetricCallCloseoutDecision = S.Literals([
  'accepted',
  'open',
  'rejected',
])
export type PylonGepaMetricCallCloseoutDecision =
  typeof PylonGepaMetricCallCloseoutDecision.Type

export class PylonGepaMetricCallAssignmentRecord extends S.Class<PylonGepaMetricCallAssignmentRecord>(
  'PylonGepaMetricCallAssignmentRecord',
)({
  acceptedAtIso: S.NullOr(S.String),
  artifactRefs: S.Array(S.String),
  assignmentRef: S.String,
  backendProfileRef: S.String,
  benchmarkSuiteRef: S.String,
  campaignId: S.String,
  candidateHash: S.String,
  caveatRefs: S.Array(S.String),
  closeoutDecision: PylonGepaMetricCallCloseoutDecision,
  closeoutRequirementRefs: S.Array(S.String),
  closeoutResultRefs: S.Array(S.String),
  closedAtIso: S.NullOr(S.String),
  createdAtIso: S.String,
  expectedArtifactRefs: S.Array(S.String),
  expectedProofBundleRefs: S.Array(S.String),
  leaseRef: S.NullOr(S.String),
  noSpendEvidenceRefs: S.Array(S.String),
  paymentMode: PylonGepaMetricCallPaymentMode,
  paymentReceiptRefs: S.Array(S.String),
  probeCommit: S.String,
  progressRefs: S.Array(S.String),
  proofBundleRefs: S.Array(S.String),
  resourceUsageRefs: S.Array(S.String),
  runtimeRef: S.String,
  schemaVersion: S.Literal(PylonGepaMetricCallAssignmentSchemaVersion),
  scorerRef: S.String,
  settlementReceiptRefs: S.Array(S.String),
  splitRef: S.String,
  state: PylonGepaMetricCallLifecycleState,
  taskRef: S.String,
  timeoutBudgetRef: S.String,
  updatedAtIso: S.String,
  verifierRef: S.String,
  verifierResultRefs: S.Array(S.String),
  workerRef: S.NullOr(S.String),
}) {}

export class PylonGepaMetricCallAssignmentInput extends S.Class<PylonGepaMetricCallAssignmentInput>(
  'PylonGepaMetricCallAssignmentInput',
)({
  assignmentRef: S.optionalKey(S.String),
  backendProfileRef: S.String,
  benchmarkSuiteRef: S.String,
  campaignId: S.String,
  candidateHash: S.String,
  caveatRefs: S.optionalKey(S.Array(S.String)),
  closeoutRequirementRefs: S.Array(S.String),
  expectedArtifactRefs: S.Array(S.String),
  expectedProofBundleRefs: S.Array(S.String),
  paymentMode: PylonGepaMetricCallPaymentMode,
  probeCommit: S.String,
  runtimeRef: S.String,
  scorerRef: S.String,
  splitRef: S.String,
  taskRef: S.String,
  timeoutBudgetRef: S.String,
  verifierRef: S.String,
}) {}

export class PylonGepaMetricCallCoordinatorImport extends S.Class<PylonGepaMetricCallCoordinatorImport>(
  'PylonGepaMetricCallCoordinatorImport',
)({
  acceptedWorkClaimAllowed: S.Boolean,
  artifactRefs: S.Array(S.String),
  assignmentRef: S.String,
  benchmarkSuiteRef: S.String,
  campaignId: S.String,
  candidateHash: S.String,
  closeoutDecision: PylonGepaMetricCallCloseoutDecision,
  closeoutResultRefs: S.Array(S.String),
  noSpendEvidenceRefs: S.Array(S.String),
  paymentMode: PylonGepaMetricCallPaymentMode,
  payableWorkClaimAllowed: S.Boolean,
  paymentReceiptRefs: S.Array(S.String),
  proofBundleRefs: S.Array(S.String),
  resourceUsageRefs: S.Array(S.String),
  schemaVersion: S.Literal(PylonGepaMetricCallCoordinatorImportSchemaVersion),
  settledBitcoinPayoutClaimAllowed: S.Boolean,
  settlementReceiptRefs: S.Array(S.String),
  splitRef: S.String,
  state: PylonGepaMetricCallLifecycleState,
  taskRef: S.String,
  verifierRef: S.String,
  verifierResultRefs: S.Array(S.String),
  workerRef: S.NullOr(S.String),
}) {}

export class PylonGepaMetricCallAssignmentUnsafe extends S.TaggedErrorClass<PylonGepaMetricCallAssignmentUnsafe>()(
  'PylonGepaMetricCallAssignmentUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(channel|key|repo)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|provider|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const defaultAssignmentRef = (
  input: PylonGepaMetricCallAssignmentInput,
): string =>
  `assignment.public.pylon_gepa_metric_call.${publicRefSegment(
    `${input.campaignId}.${input.taskRef}.${input.candidateHash}`,
    'record',
  )}`

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason: `${label} contains private data, raw logs, provider secrets, wallet/payment material, payout targets, private repo refs, or raw timestamps.`,
    })
  }
}

export const assertPylonGepaMetricCallPublicRefs = assertSafeRefs

const assertRequiredRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)

  if (normalized.length === 0) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason: `${label} is required.`,
    })
  }

  assertSafeRefs(label, normalized)

  return normalized
}

const assertRecordSafe = (
  record: PylonGepaMetricCallAssignmentRecord,
): void => {
  assertSafeRefs('GEPA metric-call identity refs', [
    record.assignmentRef,
    record.backendProfileRef,
    record.benchmarkSuiteRef,
    record.campaignId,
    record.candidateHash,
    record.probeCommit,
    record.runtimeRef,
    record.scorerRef,
    record.splitRef,
    record.taskRef,
    record.timeoutBudgetRef,
    record.verifierRef,
    ...(record.leaseRef === null ? [] : [record.leaseRef]),
    ...(record.workerRef === null ? [] : [record.workerRef]),
  ])
  assertSafeRefs('GEPA metric-call artifact refs', record.artifactRefs)
  assertSafeRefs('GEPA metric-call caveat refs', record.caveatRefs)
  assertSafeRefs(
    'GEPA metric-call closeout requirement refs',
    record.closeoutRequirementRefs,
  )
  assertSafeRefs(
    'GEPA metric-call closeout result refs',
    record.closeoutResultRefs,
  )
  assertSafeRefs(
    'GEPA metric-call expected artifact refs',
    record.expectedArtifactRefs,
  )
  assertSafeRefs(
    'GEPA metric-call expected proof bundle refs',
    record.expectedProofBundleRefs,
  )
  assertSafeRefs('GEPA metric-call progress refs', record.progressRefs)
  assertSafeRefs('GEPA metric-call proof bundle refs', record.proofBundleRefs)
  assertSafeRefs(
    'GEPA metric-call resource usage refs',
    record.resourceUsageRefs,
  )
  assertSafeRefs(
    'GEPA metric-call no-spend evidence refs',
    record.noSpendEvidenceRefs,
  )
  assertSafeRefs(
    'GEPA metric-call payment receipt refs',
    record.paymentReceiptRefs,
  )
  assertSafeRefs(
    'GEPA metric-call settlement receipt refs',
    record.settlementReceiptRefs,
  )
  assertSafeRefs(
    'GEPA metric-call verifier result refs',
    record.verifierResultRefs,
  )

  if (
    record.settlementReceiptRefs.length > 0 &&
    record.paymentMode !== 'settled_bitcoin'
  ) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason:
        'Settlement receipt refs can only be attached to settled_bitcoin assignments.',
    })
  }

  if (
    record.paymentMode === 'settled_bitcoin' &&
    record.state === 'accepted_work' &&
    (record.paymentReceiptRefs.length === 0 ||
      record.settlementReceiptRefs.length === 0)
  ) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason:
        'Settled bitcoin assignments require payment and settlement receipt refs.',
    })
  }

  if (
    record.paymentMode === 'payable_pending_settlement' &&
    record.state === 'accepted_work' &&
    record.paymentReceiptRefs.length === 0
  ) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason:
        'Payable pending settlement assignments require payment receipt refs.',
    })
  }

  if (
    record.paymentMode === 'operator_credit' &&
    record.state === 'accepted_work' &&
    record.paymentReceiptRefs.length === 0
  ) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason: 'Operator credit assignments require credit receipt refs.',
    })
  }

  if (
    record.paymentMode === 'unpaid_smoke' &&
    record.state === 'accepted_work' &&
    record.noSpendEvidenceRefs.length === 0
  ) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason: 'Accepted unpaid smoke work requires no-spend evidence refs.',
    })
  }

  if (
    record.paymentMode === 'rejected_no_pay' &&
    record.state !== 'rejected_work'
  ) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason: 'Rejected no-pay mode requires rejected work state.',
    })
  }
}

const assertCanTransition = (
  record: PylonGepaMetricCallAssignmentRecord,
  allowedStates: ReadonlyArray<PylonGepaMetricCallLifecycleState>,
  action: string,
): void => {
  if (!allowedStates.includes(record.state)) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason: `${action} cannot transition assignment in ${record.state} state.`,
    })
  }
}

const decodeRecord = (
  value: PylonGepaMetricCallAssignmentRecord,
): PylonGepaMetricCallAssignmentRecord => {
  const record = S.decodeUnknownSync(PylonGepaMetricCallAssignmentRecord)(value)
  assertRecordSafe(record)

  return record
}

export const createPylonGepaMetricCallAssignment = (
  input: PylonGepaMetricCallAssignmentInput,
  nowIso: string,
): PylonGepaMetricCallAssignmentRecord => {
  const record = new PylonGepaMetricCallAssignmentRecord({
    acceptedAtIso: null,
    artifactRefs: [],
    assignmentRef: input.assignmentRef ?? defaultAssignmentRef(input),
    backendProfileRef: input.backendProfileRef,
    benchmarkSuiteRef: input.benchmarkSuiteRef,
    campaignId: input.campaignId,
    candidateHash: input.candidateHash,
    caveatRefs: uniqueRefs(
      input.caveatRefs ?? [
        'caveat.public.pylon_gepa_metric_call.assignment_not_payment',
        'caveat.public.pylon_gepa_metric_call.no_settlement_without_evidence',
      ],
    ),
    closeoutDecision: 'open',
    closeoutRequirementRefs: assertRequiredRefs(
      'closeoutRequirementRefs',
      input.closeoutRequirementRefs,
    ),
    closeoutResultRefs: [],
    closedAtIso: null,
    createdAtIso: nowIso,
    expectedArtifactRefs: assertRequiredRefs(
      'expectedArtifactRefs',
      input.expectedArtifactRefs,
    ),
    expectedProofBundleRefs: assertRequiredRefs(
      'expectedProofBundleRefs',
      input.expectedProofBundleRefs,
    ),
    leaseRef: null,
    noSpendEvidenceRefs:
      input.paymentMode === 'unpaid_smoke'
        ? ['evidence.public.pylon_gepa_metric_call.no_spend_requested']
        : [],
    paymentMode: input.paymentMode,
    paymentReceiptRefs: [],
    probeCommit: input.probeCommit,
    progressRefs: [],
    proofBundleRefs: [],
    resourceUsageRefs: [],
    runtimeRef: input.runtimeRef,
    schemaVersion: PylonGepaMetricCallAssignmentSchemaVersion,
    scorerRef: input.scorerRef,
    settlementReceiptRefs: [],
    splitRef: input.splitRef,
    state: 'created',
    taskRef: input.taskRef,
    timeoutBudgetRef: input.timeoutBudgetRef,
    updatedAtIso: nowIso,
    verifierRef: input.verifierRef,
    verifierResultRefs: [],
    workerRef: null,
  })

  return decodeRecord(record)
}

export const acceptPylonGepaMetricCallAssignment = (
  record: PylonGepaMetricCallAssignmentRecord,
  input: Readonly<{
    leaseRef: string
    nowIso: string
    workerRef: string
  }>,
): PylonGepaMetricCallAssignmentRecord => {
  assertCanTransition(record, ['created'], 'accept')

  return decodeRecord(
    new PylonGepaMetricCallAssignmentRecord({
      ...record,
      acceptedAtIso: input.nowIso,
      leaseRef: input.leaseRef,
      state: 'accepted',
      updatedAtIso: input.nowIso,
      workerRef: input.workerRef,
    }),
  )
}

export const reportPylonGepaMetricCallProgress = (
  record: PylonGepaMetricCallAssignmentRecord,
  input: Readonly<{
    nowIso: string
    progressRefs: ReadonlyArray<string>
  }>,
): PylonGepaMetricCallAssignmentRecord => {
  assertCanTransition(
    record,
    ['accepted', 'progress_reported', 'running'],
    'report progress',
  )

  return decodeRecord(
    new PylonGepaMetricCallAssignmentRecord({
      ...record,
      progressRefs: uniqueRefs([...record.progressRefs, ...input.progressRefs]),
      state: 'progress_reported',
      updatedAtIso: input.nowIso,
    }),
  )
}

export const submitPylonGepaMetricCallResultRefs = (
  record: PylonGepaMetricCallAssignmentRecord,
  input: Readonly<{
    artifactRefs: ReadonlyArray<string>
    closeoutResultRefs: ReadonlyArray<string>
    nowIso: string
    proofBundleRefs: ReadonlyArray<string>
    resourceUsageRefs: ReadonlyArray<string>
    verifierResultRefs: ReadonlyArray<string>
  }>,
): PylonGepaMetricCallAssignmentRecord => {
  assertCanTransition(
    record,
    ['accepted', 'progress_reported', 'running'],
    'submit result refs',
  )

  return decodeRecord(
    new PylonGepaMetricCallAssignmentRecord({
      ...record,
      artifactRefs: assertRequiredRefs('artifactRefs', input.artifactRefs),
      closeoutResultRefs: assertRequiredRefs(
        'closeoutResultRefs',
        input.closeoutResultRefs,
      ),
      proofBundleRefs: assertRequiredRefs(
        'proofBundleRefs',
        input.proofBundleRefs,
      ),
      resourceUsageRefs: assertRequiredRefs(
        'resourceUsageRefs',
        input.resourceUsageRefs,
      ),
      state: 'result_submitted',
      updatedAtIso: input.nowIso,
      verifierResultRefs: assertRequiredRefs(
        'verifierResultRefs',
        input.verifierResultRefs,
      ),
    }),
  )
}

export const closePylonGepaMetricCallAssignment = (
  record: PylonGepaMetricCallAssignmentRecord,
  input: Readonly<{
    closeoutDecision: Exclude<PylonGepaMetricCallCloseoutDecision, 'open'>
    closeoutResultRefs?: ReadonlyArray<string>
    nowIso: string
    noSpendEvidenceRefs?: ReadonlyArray<string>
    paymentMode?: PylonGepaMetricCallPaymentMode
    paymentReceiptRefs?: ReadonlyArray<string>
    settlementReceiptRefs?: ReadonlyArray<string>
  }>,
): PylonGepaMetricCallAssignmentRecord => {
  if (
    input.closeoutDecision === 'accepted' &&
    record.state !== 'result_submitted'
  ) {
    throw new PylonGepaMetricCallAssignmentUnsafe({
      reason:
        'Accepted GEPA metric-call work requires submitted artifact, proof, verifier, closeout, and resource refs.',
    })
  }

  assertCanTransition(
    record,
    ['accepted', 'progress_reported', 'result_submitted', 'running'],
    'close',
  )

  const nextPaymentMode =
    input.closeoutDecision === 'rejected'
      ? 'rejected_no_pay'
      : (input.paymentMode ?? record.paymentMode)

  return decodeRecord(
    new PylonGepaMetricCallAssignmentRecord({
      ...record,
      closeoutDecision: input.closeoutDecision,
      closeoutResultRefs: uniqueRefs([
        ...record.closeoutResultRefs,
        ...(input.closeoutResultRefs ?? []),
      ]),
      closedAtIso: input.nowIso,
      noSpendEvidenceRefs: uniqueRefs([
        ...record.noSpendEvidenceRefs,
        ...(input.noSpendEvidenceRefs ?? []),
      ]),
      paymentMode: nextPaymentMode,
      paymentReceiptRefs: uniqueRefs([
        ...record.paymentReceiptRefs,
        ...(input.paymentReceiptRefs ?? []),
      ]),
      settlementReceiptRefs: uniqueRefs(input.settlementReceiptRefs),
      state:
        input.closeoutDecision === 'accepted'
          ? 'accepted_work'
          : 'rejected_work',
      updatedAtIso: input.nowIso,
    }),
  )
}

export const pylonGepaMetricCallAcceptedWorkClaimAllowed = (
  record: PylonGepaMetricCallAssignmentRecord,
): boolean =>
  record.state === 'accepted_work' &&
  record.closeoutDecision === 'accepted' &&
  record.artifactRefs.length > 0 &&
  record.proofBundleRefs.length > 0 &&
  record.closeoutResultRefs.length > 0

export const pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed = (
  record: PylonGepaMetricCallAssignmentRecord,
): boolean =>
  pylonGepaMetricCallAcceptedWorkClaimAllowed(record) &&
  record.paymentMode === 'settled_bitcoin' &&
  record.paymentReceiptRefs.length > 0 &&
  record.settlementReceiptRefs.length > 0

export const pylonGepaMetricCallPayableWorkClaimAllowed = (
  record: PylonGepaMetricCallAssignmentRecord,
): boolean =>
  pylonGepaMetricCallAcceptedWorkClaimAllowed(record) &&
  (record.paymentMode === 'payable_pending_settlement' ||
    record.paymentMode === 'settled_bitcoin') &&
  record.paymentReceiptRefs.length > 0

export const pylonGepaMetricCallCoordinatorImport = (
  record: PylonGepaMetricCallAssignmentRecord,
): PylonGepaMetricCallCoordinatorImport =>
  new PylonGepaMetricCallCoordinatorImport({
    acceptedWorkClaimAllowed:
      pylonGepaMetricCallAcceptedWorkClaimAllowed(record),
    artifactRefs: uniqueRefs(record.artifactRefs),
    assignmentRef: record.assignmentRef,
    benchmarkSuiteRef: record.benchmarkSuiteRef,
    campaignId: record.campaignId,
    candidateHash: record.candidateHash,
    closeoutDecision: record.closeoutDecision,
    closeoutResultRefs: uniqueRefs(record.closeoutResultRefs),
    noSpendEvidenceRefs: uniqueRefs(record.noSpendEvidenceRefs),
    paymentMode: record.paymentMode,
    payableWorkClaimAllowed: pylonGepaMetricCallPayableWorkClaimAllowed(record),
    paymentReceiptRefs: uniqueRefs(record.paymentReceiptRefs),
    proofBundleRefs: uniqueRefs(record.proofBundleRefs),
    resourceUsageRefs: uniqueRefs(record.resourceUsageRefs),
    schemaVersion: PylonGepaMetricCallCoordinatorImportSchemaVersion,
    settledBitcoinPayoutClaimAllowed:
      pylonGepaMetricCallSettledBitcoinPayoutClaimAllowed(record),
    settlementReceiptRefs: uniqueRefs(record.settlementReceiptRefs),
    splitRef: record.splitRef,
    state: record.state,
    taskRef: record.taskRef,
    verifierRef: record.verifierRef,
    verifierResultRefs: uniqueRefs(record.verifierResultRefs),
    workerRef: record.workerRef,
  })
