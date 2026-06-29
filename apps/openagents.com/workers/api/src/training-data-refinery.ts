import { Schema as S } from 'effect'

import {
  isRecord,
  optionalString,
  parseJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import {
  Cs336A4DataRefineryJobKind,
  Cs336A4HomeworkStages,
  Cs336A4PsionicLaneRef,
  type Cs336A4HomeworkStage,
} from './cs336-a4-data-refinery'
import {
  Cs336A4EvalDeltaBonusPolicyRef,
  Cs336A4EvalDeltaBoundaryRef,
  Cs336A4EvalDeltaPaymentSchemaVersion,
} from './cs336-a4-eval-delta-payment'
import {
  Cs336A4AcquisitionModes,
  Cs336A4ProvenanceSchemaVersion,
  type Cs336A4ProvenanceReceipt,
  type Cs336A4TransformStep,
} from './cs336-a4-provenance'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

export type DataRefineryShard = Readonly<{
  corpusProvenanceReceipt: Cs336A4ProvenanceReceipt | null
  corpusProvenanceReceiptRef: string | null
  corpusProvenanceVerified: boolean
  inputDocumentCount: number | null
  outputDigestRef: string
  pylonRef: string | null
  receiptRefs: ReadonlyArray<string>
  settledPayoutSats: number
  shardRef: string
  sourceRefs: ReadonlyArray<string>
  stage: Cs336A4HomeworkStage
  trainingRunRef: string
  verificationRefs: ReadonlyArray<string>
  verified: boolean
}>

export type DataRefineryEvalDeltaPaymentGate = Readonly<{
  blockerRefs: ReadonlyArray<string>
  bonusPolicyRef: typeof Cs336A4EvalDeltaBonusPolicyRef
  boundaryRef: typeof Cs336A4EvalDeltaBoundaryRef
  clearsBlockerRefs: ReadonlyArray<string>
  fixedTrainerEvalMeasurementAvailable: boolean
  greenGateSatisfied: false
  leaderboardLane: 'a4_eval_delta'
  operatorFundingParametersAvailable: boolean
  payableSettlementCount: number
  paymentComputationAvailable: boolean
  paymentSchemaVersion: typeof Cs336A4EvalDeltaPaymentSchemaVersion
  remainingProductBlockerRefs: ReadonlyArray<
    'blocker.product_promises.eval_delta_payment_missing'
  >
  settlementReceiptAvailable: boolean
  settledBonusSats: number
  sourceRefs: ReadonlyArray<string>
  verifiedMeasurementRowCount: number
}>

export type DataRefineryProjection = Readonly<{
  blockerRefs: ReadonlyArray<string>
  corpusProvenanceReceiptBlockerRefs: ReadonlyArray<string>
  corpusProvenanceReceiptRefs: ReadonlyArray<string>
  corpusProvenanceReceiptStatus: 'missing' | 'partial' | 'available'
  evalDeltaBonusBlockerRefs: ReadonlyArray<string>
  evalDeltaPaymentGate: DataRefineryEvalDeltaPaymentGate
  observedVerifiedShardCount: number
  observedVerifiedStages: ReadonlyArray<Cs336A4HomeworkStage>
  psionicLaneRef: string
  requiredVerifiedStageCount: number
  schemaVersion: 'openagents.training.data_refinery.v1'
  scopeBoundaryRefs: ReadonlyArray<string>
  shards: ReadonlyArray<DataRefineryShard>
  sourceRefs: ReadonlyArray<string>
  status:
    | 'blocked_no_shards'
    | 'collecting_shards'
    | 'stages_verified'
}>

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))

const Cs336A4SourceProvenanceEvidence = S.Struct({
  acquisitionMode: S.Literals(Cs336A4AcquisitionModes),
  licenseRef: PublicSafeRef,
  snapshotRef: PublicSafeRef,
  sourceRef: PublicSafeRef,
})

const Cs336A4TransformStepEvidence = S.Struct({
  codeVersionRef: PublicSafeRef,
  inputDigestRef: PublicSafeRef,
  outputDigestRef: PublicSafeRef,
  recomputedDigestRef: PublicSafeRef,
  stage: S.Literals([...Cs336A4HomeworkStages]),
})

const Cs336A4ProvenanceReceiptEvidence = S.Struct({
  assignmentRef: PublicSafeRef,
  contentDigestRef: S.Trim.check(
    S.isNonEmpty(),
    S.isPattern(/^[0-9a-f]{64}$/),
  ),
  finalOutputDigestRef: PublicSafeRef,
  inputShardRef: PublicSafeRef,
  jobKind: S.Literal(Cs336A4DataRefineryJobKind),
  provenance: Cs336A4SourceProvenanceEvidence,
  receiptRef: PublicSafeRef,
  recomputeVerified: S.Boolean,
  schemaVersion: S.Literal(Cs336A4ProvenanceSchemaVersion),
  sourceInputDigestRef: PublicSafeRef,
  transformChain: S.Array(Cs336A4TransformStepEvidence),
})

export const Cs336A4RefineryStageEvidence = S.Struct({
  corpusProvenanceReceipt: Cs336A4ProvenanceReceiptEvidence,
  inputDocumentCount: S.optionalKey(S.Number),
  outputDigestRef: PublicSafeRef,
  pylonRef: S.optionalKey(PublicSafeRef),
  receiptRefs: S.Array(PublicSafeRef),
  shardRef: S.optionalKey(PublicSafeRef),
  sourceRefs: PublicSafeRefs,
  stage: S.Literals([...Cs336A4HomeworkStages]),
  verificationRefs: PublicSafeRefs,
})
export type Cs336A4RefineryStageEvidence =
  typeof Cs336A4RefineryStageEvidence.Type

export const Cs336A4DataRefineryEvidenceRequest = S.Struct({
  psionicLaneRef: S.optionalKey(PublicSafeRef),
  receiptRefs: PublicSafeRefs,
  shards: S.Array(Cs336A4RefineryStageEvidence),
  sourceRefs: PublicSafeRefs,
})
export type Cs336A4DataRefineryEvidenceRequest =
  typeof Cs336A4DataRefineryEvidenceRequest.Type

export class DataRefineryUnsafeProjectionError extends Error {
  readonly _tag = 'DataRefineryUnsafeProjectionError'
}

export class DataRefineryEvidenceValidationError extends Error {
  readonly _tag = 'DataRefineryEvidenceValidationError'
}

/**
 * Public-safety guard for admitted A4 refinery evidence. Pylon refs and
 * output-digest refs are legitimate public provenance, but the guard
 * still rejects wallet, payment, invoice, mnemonic, key, raw-shard, and
 * private path material before it can reach D1.
 */
const unsafeRefineryMaterialPattern =
  /(\"?(mnemonic|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*|wallet[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner|shard|warc)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafeRefineryMaterialPattern.test(json)) {
    throw new DataRefineryUnsafeProjectionError(
      'CS336 A4 data-refinery projection contains wallet, payment, raw-shard, or private material.',
    )
  }

  return json
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const optionalNumber = (value: unknown): number | undefined => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value
  }

  if (typeof value !== 'string' || value.trim() === '') {
    return undefined
  }

  const parsed = Number(value)

  return Number.isFinite(parsed) ? parsed : undefined
}

const isStage = (value: unknown): value is Cs336A4HomeworkStage =>
  typeof value === 'string' &&
  (Cs336A4HomeworkStages as ReadonlyArray<string>).includes(value)

const isAcquisitionMode = (
  value: unknown,
): value is Cs336A4ProvenanceReceipt['provenance']['acquisitionMode'] =>
  typeof value === 'string' &&
  (Cs336A4AcquisitionModes as ReadonlyArray<string>).includes(value)

const corpusProvenanceReceiptFromUnknown = (
  value: unknown,
): Cs336A4ProvenanceReceipt | null => {
  if (!isRecord(value)) {
    return null
  }

  const provenanceRecord = value.provenance
  const transformChainValue = value.transformChain

  if (!isRecord(provenanceRecord) || !Array.isArray(transformChainValue)) {
    return null
  }

  const assignmentRef = optionalString(value.assignmentRef)
  const contentDigestRef = optionalString(value.contentDigestRef)
  const finalOutputDigestRef = optionalString(value.finalOutputDigestRef)
  const inputShardRef = optionalString(value.inputShardRef)
  const jobKind = optionalString(value.jobKind)
  const receiptRef = optionalString(value.receiptRef)
  const schemaVersion = optionalString(value.schemaVersion)
  const sourceInputDigestRef = optionalString(value.sourceInputDigestRef)
  const acquisitionMode = optionalString(provenanceRecord.acquisitionMode)
  const licenseRef = optionalString(provenanceRecord.licenseRef)
  const snapshotRef = optionalString(provenanceRecord.snapshotRef)
  const sourceRef = optionalString(provenanceRecord.sourceRef)

  if (
    assignmentRef === undefined ||
    contentDigestRef === undefined ||
    !/^[0-9a-f]{64}$/.test(contentDigestRef) ||
    finalOutputDigestRef === undefined ||
    inputShardRef === undefined ||
    jobKind !== Cs336A4DataRefineryJobKind ||
    receiptRef === undefined ||
    schemaVersion !== Cs336A4ProvenanceSchemaVersion ||
    sourceInputDigestRef === undefined ||
    acquisitionMode === undefined ||
    !isAcquisitionMode(acquisitionMode) ||
    licenseRef === undefined ||
    snapshotRef === undefined ||
    sourceRef === undefined ||
    typeof value.recomputeVerified !== 'boolean'
  ) {
    return null
  }

  const transformChain: Array<Cs336A4TransformStep> = []

  for (const step of transformChainValue) {
    if (!isRecord(step) || !isStage(step.stage)) {
      return null
    }

    const codeVersionRef = optionalString(step.codeVersionRef)
    const inputDigestRef = optionalString(step.inputDigestRef)
    const outputDigestRef = optionalString(step.outputDigestRef)
    const recomputedDigestRef = optionalString(step.recomputedDigestRef)

    if (
      codeVersionRef === undefined ||
      inputDigestRef === undefined ||
      outputDigestRef === undefined ||
      recomputedDigestRef === undefined
    ) {
      return null
    }

    transformChain.push({
      codeVersionRef,
      inputDigestRef,
      outputDigestRef,
      recomputedDigestRef,
      stage: step.stage,
    })
  }

  const receipt: Cs336A4ProvenanceReceipt = {
    assignmentRef,
    contentDigestRef,
    finalOutputDigestRef,
    inputShardRef,
    jobKind: Cs336A4DataRefineryJobKind,
    provenance: {
      acquisitionMode,
      licenseRef,
      snapshotRef,
      sourceRef,
    },
    receiptRef,
    recomputeVerified: value.recomputeVerified,
    schemaVersion: Cs336A4ProvenanceSchemaVersion,
    sourceInputDigestRef,
    transformChain,
  }

  publicSafeJson(receipt)

  return receipt
}

const assertLinkedCorpusProvenanceReceipt = (
  shard: Cs336A4RefineryStageEvidence,
): Cs336A4ProvenanceReceipt => {
  const receipt = shard.corpusProvenanceReceipt as
    | Cs336A4ProvenanceReceipt
    | undefined

  if (receipt === undefined) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 refinery shard evidence requires a corpusProvenanceReceipt.',
    )
  }

  if (receipt.jobKind !== Cs336A4DataRefineryJobKind) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receipt must use the CS336 A4 data-refinery job kind.',
    )
  }

  if (receipt.schemaVersion !== Cs336A4ProvenanceSchemaVersion) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receipt carries an unsupported schema version.',
    )
  }

  if (receipt.finalOutputDigestRef !== shard.outputDigestRef) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receipt final output digest must match the shard outputDigestRef.',
    )
  }

  if (!receipt.recomputeVerified) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receipt must be recompute verified.',
    )
  }

  if (!receipt.receiptRef.endsWith(receipt.contentDigestRef.slice(0, 16))) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receiptRef must include the content digest prefix.',
    )
  }

  if (receipt.transformChain.length === 0) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receipt requires a non-empty transform chain.',
    )
  }

  let previousOutputDigestRef = receipt.sourceInputDigestRef

  for (const [index, step] of receipt.transformChain.entries()) {
    if (step.inputDigestRef !== previousOutputDigestRef) {
      throw new DataRefineryEvidenceValidationError(
        `CS336 A4 corpus provenance transform chain is not linked at step ${index}.`,
      )
    }

    if (step.recomputedDigestRef !== step.outputDigestRef) {
      throw new DataRefineryEvidenceValidationError(
        `CS336 A4 corpus provenance recompute digest mismatch at step ${index}.`,
      )
    }

    previousOutputDigestRef = step.outputDigestRef
  }

  const lastStep = receipt.transformChain[receipt.transformChain.length - 1]!

  if (lastStep.stage !== shard.stage) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receipt last transform stage must match the shard stage.',
    )
  }

  if (lastStep.outputDigestRef !== receipt.finalOutputDigestRef) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 corpus provenance receipt final output digest must equal the last transform output digest.',
    )
  }

  publicSafeJson(receipt)

  return receipt
}

const assertAdmissibleShard = (shard: Cs336A4RefineryStageEvidence): void => {
  if (shard.receiptRefs.length === 0) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 refinery shard evidence requires at least one receipt ref; unreceipted shards are not admissible.',
    )
  }

  if (
    shard.inputDocumentCount !== undefined &&
    (!Number.isFinite(shard.inputDocumentCount) || shard.inputDocumentCount <= 0)
  ) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 refinery shard input document count must be a positive finite number when present.',
    )
  }

  assertLinkedCorpusProvenanceReceipt(shard)
}

/**
 * Admits receipted CS336 A4 refinery shards into a training run's public
 * projection. The public-safety guard runs at admission time on the
 * exact evidence that will be projected. Each shard names one
 * deterministic stage, its output-digest commitment, and the receipt
 * and verification refs that back it; eval-delta leaderboard rows are
 * intentionally not admitted here because no fixed-trainer eval loop
 * exists yet (the bonus stays design-only per the payment policy).
 */
export const admitCs336A4DataRefineryEvidence = (
  input: Readonly<{
    nowIso: string
    request: Cs336A4DataRefineryEvidenceRequest
    run: TrainingRunRecord
  }>,
): TrainingRunRecord => {
  if (input.request.shards.length === 0) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 data-refinery evidence requires at least one shard.',
    )
  }

  for (const shard of input.request.shards) {
    assertAdmissibleShard(shard)
  }

  const evidence = {
    jobKind: Cs336A4DataRefineryJobKind,
    psionicLaneRef: input.request.psionicLaneRef ?? Cs336A4PsionicLaneRef,
    receiptRefs: uniqueRefs([...(input.request.receiptRefs ?? [])]),
    shards: input.request.shards,
    sourceRefs: uniqueRefs([...(input.request.sourceRefs ?? [])]),
  }

  publicSafeJson(evidence)

  const projection = parseJsonRecord(input.run.publicProjectionJson) ?? {}
  const existing = isRecord(projection.a4DataRefinery)
    ? projection.a4DataRefinery
    : undefined
  const existingLeaderboardRows = Array.isArray(existing?.leaderboardRows)
    ? existing?.leaderboardRows
    : []

  return {
    ...input.run,
    publicProjectionJson: JSON.stringify({
      ...projection,
      a4DataRefinery: {
        ...evidence,
        // Preserve any previously admitted eval-delta leaderboard rows
        // without inventing new ones; the deterministic-stage admission
        // never fabricates eval-delta scores.
        leaderboardRows: existingLeaderboardRows,
      },
    }),
    updatedAt: input.nowIso,
  }
}

const refineryEvidenceRecord = (
  run: TrainingRunRecord,
): Record<string, unknown> | undefined => {
  const projection = parseJsonRecord(run.publicProjectionJson)
  const nested = projection?.a4DataRefinery

  return isRecord(nested) ? nested : undefined
}

const shardsFromEvidence = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    evidence: Record<string, unknown> | undefined
    run: TrainingRunRecord
  }>,
): ReadonlyArray<DataRefineryShard> => {
  const shards = input.evidence?.shards

  if (!Array.isArray(shards)) {
    return []
  }

  const verifiedChallengeRefs = input.challenges
    .filter(challenge => challenge.state === 'Verified')
    .map(challenge => challenge.challengeRef)

  return shards.flatMap((shard, index): ReadonlyArray<DataRefineryShard> => {
    if (!isRecord(shard) || !isStage(shard.stage)) {
      return []
    }

    const outputDigestRef = optionalString(shard.outputDigestRef)

    if (outputDigestRef === undefined) {
      return []
    }

    const corpusProvenanceReceipt = corpusProvenanceReceiptFromUnknown(
      shard.corpusProvenanceReceipt,
    )
    const corpusProvenanceVerified =
      corpusProvenanceReceipt !== null &&
      corpusProvenanceReceipt.recomputeVerified &&
      corpusProvenanceReceipt.finalOutputDigestRef === outputDigestRef

    const verificationRefs = uniqueRefs([
      ...stringArrayFromUnknown(shard.verificationRefs),
      ...verifiedChallengeRefs,
    ])

    return [
      {
        corpusProvenanceReceipt,
        corpusProvenanceReceiptRef:
          corpusProvenanceReceipt?.receiptRef ?? null,
        corpusProvenanceVerified,
        inputDocumentCount: optionalNumber(shard.inputDocumentCount) ?? null,
        outputDigestRef,
        pylonRef: optionalString(shard.pylonRef) ?? null,
        receiptRefs: uniqueRefs([
          ...stringArrayFromUnknown(shard.receiptRefs),
          ...(corpusProvenanceReceipt === null
            ? []
            : [corpusProvenanceReceipt.receiptRef]),
        ]),
        settledPayoutSats: 0,
        shardRef:
          optionalString(shard.shardRef) ??
          `training.data_refinery.${input.run.trainingRunRef}.shard.${index + 1}`,
        sourceRefs: uniqueRefs(stringArrayFromUnknown(shard.sourceRefs)),
        stage: shard.stage,
        trainingRunRef: input.run.trainingRunRef,
        verificationRefs,
        verified: verificationRefs.length > 0,
      },
    ]
  })
}

const evalDeltaBonusBlockerRefs = (): ReadonlyArray<string> => [
  'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
  'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
  'blocker.cs336_a4.psionic_classifier_adapters_partial',
]

const evalDeltaLeaderboardMeasurementRowCount = (
  evidence: Record<string, unknown> | undefined,
): number => {
  const rows = evidence?.leaderboardRows

  if (!Array.isArray(rows)) {
    return 0
  }

  return rows.filter(row => {
    if (!isRecord(row)) {
      return false
    }

    return (
      typeof row.contributorRef === 'string' &&
      typeof row.evalDelta === 'number' &&
      stringArrayFromUnknown(row.verificationRefs).length > 0
    )
  }).length
}

const evalDeltaPaymentBlockerRefs = (
  verifiedMeasurementRowCount: number,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...(verifiedMeasurementRowCount > 0
      ? []
      : ['blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus']),
    'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
    'blocker.cs336_a4.psionic_classifier_adapters_partial',
  ])

export const dataRefineryEvalDeltaPaymentGate = (
  input: Readonly<{ verifiedMeasurementRowCount: number }>,
): DataRefineryEvalDeltaPaymentGate => ({
  blockerRefs: evalDeltaPaymentBlockerRefs(input.verifiedMeasurementRowCount),
  bonusPolicyRef: Cs336A4EvalDeltaBonusPolicyRef,
  boundaryRef: Cs336A4EvalDeltaBoundaryRef,
  clearsBlockerRefs: [],
  fixedTrainerEvalMeasurementAvailable:
    input.verifiedMeasurementRowCount > 0,
  greenGateSatisfied: false,
  leaderboardLane: 'a4_eval_delta',
  operatorFundingParametersAvailable: false,
  payableSettlementCount: 0,
  paymentComputationAvailable: true,
  paymentSchemaVersion: Cs336A4EvalDeltaPaymentSchemaVersion,
  remainingProductBlockerRefs: [
    'blocker.product_promises.eval_delta_payment_missing',
  ],
  settlementReceiptAvailable: false,
  settledBonusSats: 0,
  sourceRefs: [
    'apps/openagents.com/workers/api/src/cs336-a4-eval-delta-payment.ts',
    'apps/openagents.com/docs/2026-06-10-cs336-a4-data-refinery-payment-policy.md',
    'route:/api/training/leaderboards/a4_eval_delta',
  ],
  verifiedMeasurementRowCount: input.verifiedMeasurementRowCount,
})

export const aggregateDataRefineryEvalDeltaPaymentGate = (
  projections: ReadonlyArray<DataRefineryProjection>,
): DataRefineryEvalDeltaPaymentGate =>
  dataRefineryEvalDeltaPaymentGate({
    verifiedMeasurementRowCount: projections.reduce(
      (total, projection) =>
        total + projection.evalDeltaPaymentGate.verifiedMeasurementRowCount,
      0,
    ),
  })

export const corpusProvenanceReceiptStatus = (
  shards: ReadonlyArray<DataRefineryShard>,
): DataRefineryProjection['corpusProvenanceReceiptStatus'] => {
  if (shards.length === 0) {
    return 'missing'
  }

  if (shards.every(shard => shard.corpusProvenanceVerified)) {
    return 'available'
  }

  return 'partial'
}

export const corpusProvenanceReceiptBlockerRefs = (
  shards: ReadonlyArray<DataRefineryShard>,
): ReadonlyArray<string> =>
  corpusProvenanceReceiptStatus(shards) === 'available'
    ? []
    : ['blocker.cs336_a4.requires_corpus_provenance_receipts']

export const Cs336A4RequiredVerifiedStageCount = 3

/**
 * Builds the public CS336 A4 data-refinery projection for a run: the
 * receipted refinery shards, their verification state, and an honest
 * status. `stages_verified` requires at least three distinct stages
 * with a verified deterministic_recompute challenge (acceptance
 * criterion #1). The eval-delta quality bonus is reported separately as
 * blocked: deterministic stages pay per verified shard, never per
 * volume, and the bonus stays design-only until a fixed-trainer eval
 * loop and operator funding exist.
 */
export const publicDataRefineryProjection = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): DataRefineryProjection => {
  const evidence = refineryEvidenceRecord(input.run)
  const shards = shardsFromEvidence({ ...input, evidence })
  const verifiedMeasurementRowCount =
    evalDeltaLeaderboardMeasurementRowCount(evidence)
  const verifiedShards = shards.filter(shard => shard.verified)
  const observedVerifiedStages = [
    ...new Set(verifiedShards.map(shard => shard.stage)),
  ].sort() as ReadonlyArray<Cs336A4HomeworkStage>
  const stagesVerified =
    observedVerifiedStages.length >= Cs336A4RequiredVerifiedStageCount
  const stageBlockerRefs = stagesVerified
    ? []
    : [
        'blocker.cs336_a4.requires_three_verified_stages',
        'blocker.cs336_a4.operator_funding_required_for_paid_shards',
      ]
  const provenanceBlockerRefs = corpusProvenanceReceiptBlockerRefs(shards)
  const status =
    shards.length === 0
      ? 'blocked_no_shards'
      : stagesVerified
        ? 'stages_verified'
        : 'collecting_shards'

  return {
    blockerRefs: uniqueRefs([...stageBlockerRefs, ...provenanceBlockerRefs]),
    corpusProvenanceReceiptBlockerRefs: provenanceBlockerRefs,
    corpusProvenanceReceiptRefs: uniqueRefs(
      shards.flatMap(shard =>
        shard.corpusProvenanceReceiptRef === null
          ? []
          : [shard.corpusProvenanceReceiptRef],
      ),
    ),
    corpusProvenanceReceiptStatus: corpusProvenanceReceiptStatus(shards),
    evalDeltaBonusBlockerRefs: evalDeltaBonusBlockerRefs(),
    evalDeltaPaymentGate: dataRefineryEvalDeltaPaymentGate({
      verifiedMeasurementRowCount,
    }),
    observedVerifiedShardCount: verifiedShards.length,
    observedVerifiedStages,
    psionicLaneRef:
      optionalString(evidence?.psionicLaneRef) ?? Cs336A4PsionicLaneRef,
    requiredVerifiedStageCount: Cs336A4RequiredVerifiedStageCount,
    schemaVersion: 'openagents.training.data_refinery.v1',
    scopeBoundaryRefs: [
      'scope.cs336_a4.deterministic_recompute_verified_shards_only',
      'scope.cs336_a4.synthetic_public_safe_corpus_only',
      'scope.cs336_a4.pay_quality_delta_not_raw_volume_or_private_data',
      'scope.cs336_a4.no_settlement_without_provider_confirmation',
    ],
    shards,
    sourceRefs: uniqueRefs([
      'route:/api/training/refinery/a4',
      `route:/api/training/runs/${input.run.trainingRunRef}`,
      ...input.run.sourceRefs,
      ...input.windows.flatMap(window => window.sourceRefs),
      ...input.leases.map(lease => lease.leaseRef),
    ]),
    status,
  }
}
