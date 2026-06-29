import { Schema as S } from 'effect'

import {
  isRecord,
  optionalString,
  parseJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

export type ScalingSweepCell = Readonly<{
  cellRef: string
  computeBudgetFlops: number
  parameterCount: number
  pylonRef: string | null
  receiptRefs: ReadonlyArray<string>
  settledPayoutSats: number
  sourceRefs: ReadonlyArray<string>
  tokenCount: number
  trainingRunRef: string
  validationLoss: number | null
  verificationRefs: ReadonlyArray<string>
  verified: boolean
}>

export type ScalingSweepFitArtifact = Readonly<{
  artifactRef: string
  copyBoundaryRefs: ReadonlyArray<string>
  exponentRefs: ReadonlyArray<string>
  predictedBestConfig: Readonly<{
    parameterCount: number
    tokenCount: number
  }>
  provenanceLabel: string
  sourceRefs: ReadonlyArray<string>
}>

export type ScalingSweepProjection = Readonly<{
  blockerRefs: ReadonlyArray<string>
  cells: ReadonlyArray<ScalingSweepCell>
  fitArtifact: ScalingSweepFitArtifact | null
  observedVerifiedCellCount: number
  psionicLaneRef: string
  requiredVerifiedCellCount: number
  schemaVersion: 'openagents.training.scaling_sweep.v1'
  scopeBoundaryRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  status:
    | 'blocked_no_cells'
    | 'collecting_cells'
    | 'fit_ready'
    | 'fit_published'
}>

export const Cs336A3ScalingSweepJobKind = 'cs336_a3_scaling_sweep'
export const Cs336A3ScalingSweepPsionicLaneRef =
  'psion_cs336_a3_scaling_reference_v1'
export const Cs336A3SweepRequestSchemaRef =
  'openagents.cs336_a3_scaling_sweep_request.v1'
export const Cs336A3SweepOutputSchemaRef =
  'openagents.cs336_a3_scaling_sweep_output.v1'

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))

export const Cs336A3SweepCellEvidence = S.Struct({
  cellRef: S.optionalKey(PublicSafeRef),
  computeBudgetFlops: S.Number,
  parameterCount: S.Number,
  pylonRef: S.optionalKey(PublicSafeRef),
  receiptRefs: S.Array(PublicSafeRef),
  sourceRefs: PublicSafeRefs,
  tokenCount: S.Number,
  validationLoss: S.Number,
  verificationRefs: PublicSafeRefs,
})
export type Cs336A3SweepCellEvidence = typeof Cs336A3SweepCellEvidence.Type

export const Cs336A3SweepFitArtifactEvidence = S.Struct({
  artifactRef: PublicSafeRef,
  exponentRefs: PublicSafeRefs,
  predictedBestConfig: S.Struct({
    parameterCount: S.Number,
    tokenCount: S.Number,
  }),
  provenanceLabel: S.optionalKey(NonEmptyTrimmedString),
  sourceRefs: PublicSafeRefs,
})
export type Cs336A3SweepFitArtifactEvidence =
  typeof Cs336A3SweepFitArtifactEvidence.Type

export const Cs336A3ScalingSweepEvidenceRequest = S.Struct({
  cells: S.Array(Cs336A3SweepCellEvidence),
  fitArtifact: S.optionalKey(Cs336A3SweepFitArtifactEvidence),
  psionicLaneRef: S.optionalKey(PublicSafeRef),
  receiptRefs: PublicSafeRefs,
  sourceRefs: PublicSafeRefs,
})
export type Cs336A3ScalingSweepEvidenceRequest =
  typeof Cs336A3ScalingSweepEvidenceRequest.Type

export class ScalingSweepUnsafeProjectionError extends Error {
  readonly _tag = 'ScalingSweepUnsafeProjectionError'
}

export class ScalingSweepEvidenceValidationError extends Error {
  readonly _tag = 'ScalingSweepEvidenceValidationError'
}

/**
 * Public-safety guard for admitted A3 sweep evidence. Pylon refs are
 * legitimate public provenance on sweep cells, so unlike the A2
 * device-capability guard this one allows `pylonRef` keys, but it
 * still rejects wallet, payment, invoice, mnemonic, key, and private
 * path material before it can reach D1.
 */
const unsafeSweepMaterialPattern =
  /(\"?(mnemonic|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*|wallet[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafeSweepMaterialPattern.test(json)) {
    throw new ScalingSweepUnsafeProjectionError(
      'CS336 A3 scaling-sweep projection contains wallet, payment, or private material.',
    )
  }

  return json
}

export type Cs336A3SweepAssignmentPayload = Readonly<{
  assignmentRef: string
  jobKind: typeof Cs336A3ScalingSweepJobKind
  outputSchemaRef: typeof Cs336A3SweepOutputSchemaRef
  psionicLaneRef: typeof Cs336A3ScalingSweepPsionicLaneRef
  requestSchemaRef: typeof Cs336A3SweepRequestSchemaRef
  verificationClass: 'deterministic_recompute'
}>

export const buildCs336A3SweepAssignmentPayload = (
  input: Readonly<{ assignmentRef: string }>,
): Cs336A3SweepAssignmentPayload => {
  const payload: Cs336A3SweepAssignmentPayload = {
    assignmentRef: input.assignmentRef,
    jobKind: Cs336A3ScalingSweepJobKind,
    outputSchemaRef: Cs336A3SweepOutputSchemaRef,
    psionicLaneRef: Cs336A3ScalingSweepPsionicLaneRef,
    requestSchemaRef: Cs336A3SweepRequestSchemaRef,
    verificationClass: 'deterministic_recompute',
  }

  publicSafeJson(payload)

  return payload
}

const assertAdmissibleCell = (cell: Cs336A3SweepCellEvidence): void => {
  const quantities = [
    cell.computeBudgetFlops,
    cell.parameterCount,
    cell.tokenCount,
  ]

  if (!quantities.every(value => Number.isFinite(value) && value > 0)) {
    throw new ScalingSweepEvidenceValidationError(
      'CS336 A3 sweep cell evidence requires positive finite parameter, data, and compute quantities.',
    )
  }

  if (!Number.isFinite(cell.validationLoss)) {
    throw new ScalingSweepEvidenceValidationError(
      'CS336 A3 sweep cell evidence requires a finite validation loss.',
    )
  }

  if (cell.receiptRefs.length === 0) {
    throw new ScalingSweepEvidenceValidationError(
      'CS336 A3 sweep cell evidence requires at least one receipt ref; unreceipted cells are not admissible.',
    )
  }
}

/**
 * Admits receipted CS336 A3 scaling-sweep cells (and optionally the
 * Psionic-fitted IsoFLOP artifact) into a training run's public
 * projection. The public-safety guard runs at admission time on the
 * exact evidence that will be projected; a fit artifact is admissible
 * only over a sweep of at least 20 cells, mirroring the dashboard's
 * verified-cell threshold.
 */
export const admitCs336A3ScalingSweepEvidence = (
  input: Readonly<{
    nowIso: string
    request: Cs336A3ScalingSweepEvidenceRequest
    run: TrainingRunRecord
  }>,
): TrainingRunRecord => {
  if (input.request.cells.length === 0) {
    throw new ScalingSweepEvidenceValidationError(
      'CS336 A3 scaling-sweep evidence requires at least one cell.',
    )
  }

  for (const cell of input.request.cells) {
    assertAdmissibleCell(cell)
  }

  if (
    input.request.fitArtifact !== undefined &&
    input.request.cells.length < 20
  ) {
    throw new ScalingSweepEvidenceValidationError(
      'CS336 A3 fit artifacts are admissible only over a sweep of at least 20 receipted cells.',
    )
  }

  const evidence = {
    cells: input.request.cells,
    ...(input.request.fitArtifact === undefined
      ? {}
      : { fitArtifact: input.request.fitArtifact }),
    jobKind: Cs336A3ScalingSweepJobKind,
    psionicLaneRef:
      input.request.psionicLaneRef ?? Cs336A3ScalingSweepPsionicLaneRef,
    receiptRefs: uniqueRefs([...(input.request.receiptRefs ?? [])]),
    sourceRefs: uniqueRefs([...(input.request.sourceRefs ?? [])]),
  }

  publicSafeJson(evidence)

  const projection = parseJsonRecord(input.run.publicProjectionJson) ?? {}

  return {
    ...input.run,
    publicProjectionJson: JSON.stringify({
      ...projection,
      a3ScalingSweep: evidence,
    }),
    updatedAt: input.nowIso,
  }
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

const sweepEvidenceRecord = (
  run: TrainingRunRecord,
): Record<string, unknown> | undefined => {
  const projection = parseJsonRecord(run.publicProjectionJson)
  const nested = projection?.a3ScalingSweep

  return isRecord(nested) ? nested : undefined
}

const cellsFromEvidence = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    evidence: Record<string, unknown> | undefined
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
  }>,
): ReadonlyArray<ScalingSweepCell> => {
  const cells = input.evidence?.cells

  if (!Array.isArray(cells)) {
    return []
  }

  return cells.flatMap((cell, index): ReadonlyArray<ScalingSweepCell> => {
    if (!isRecord(cell)) {
      return []
    }

    const parameterCount = optionalNumber(cell.parameterCount)
    const tokenCount = optionalNumber(cell.tokenCount)
    const computeBudgetFlops = optionalNumber(cell.computeBudgetFlops)

    if (
      parameterCount === undefined ||
      tokenCount === undefined ||
      computeBudgetFlops === undefined
    ) {
      return []
    }

    const pylonRef = optionalString(cell.pylonRef) ?? null
    const verificationRefs = uniqueRefs([
      ...stringArrayFromUnknown(cell.verificationRefs),
      ...input.challenges
        .filter(challenge => challenge.state === 'Verified')
        .map(challenge => challenge.challengeRef),
    ])

    return [
      {
        cellRef:
          optionalString(cell.cellRef) ??
          `training.scaling_sweep.${input.run.trainingRunRef}.cell.${index + 1}`,
        computeBudgetFlops,
        parameterCount,
        pylonRef,
        receiptRefs: uniqueRefs(stringArrayFromUnknown(cell.receiptRefs)),
        settledPayoutSats: 0,
        sourceRefs: uniqueRefs(stringArrayFromUnknown(cell.sourceRefs)),
        tokenCount,
        trainingRunRef: input.run.trainingRunRef,
        validationLoss: optionalNumber(cell.validationLoss) ?? null,
        verificationRefs,
        verified: verificationRefs.length > 0,
      },
    ]
  })
}

const fitArtifactFromEvidence = (
  evidence: Record<string, unknown> | undefined,
): ScalingSweepFitArtifact | null => {
  const fit = evidence?.fitArtifact

  if (!isRecord(fit)) {
    return null
  }

  const predicted = isRecord(fit.predictedBestConfig)
    ? fit.predictedBestConfig
    : undefined
  const parameterCount = optionalNumber(predicted?.parameterCount)
  const tokenCount = optionalNumber(predicted?.tokenCount)
  const artifactRef = optionalString(fit.artifactRef)

  if (
    artifactRef === undefined ||
    parameterCount === undefined ||
    tokenCount === undefined
  ) {
    return null
  }

  return {
    artifactRef,
    copyBoundaryRefs: [
      'copy.public.training.scaling_law_is_analysis_artifact',
      'copy.public.training.scaling_law_not_capability_claim',
    ],
    exponentRefs: uniqueRefs(stringArrayFromUnknown(fit.exponentRefs)),
    predictedBestConfig: {
      parameterCount,
      tokenCount,
    },
    provenanceLabel:
      optionalString(fit.provenanceLabel) ??
      'IsoFLOP fit artifact derived from public receipt-backed sweep cells.',
    sourceRefs: uniqueRefs(stringArrayFromUnknown(fit.sourceRefs)),
  }
}

export const publicScalingSweepProjection = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): ScalingSweepProjection => {
  const evidence = sweepEvidenceRecord(input.run)
  const cells = cellsFromEvidence({ ...input, evidence })
  const fitArtifact = fitArtifactFromEvidence(evidence)
  const observedVerifiedCellCount = cells.filter(cell => cell.verified).length
  const fitReady = observedVerifiedCellCount >= 20
  const status =
    fitArtifact !== null
      ? 'fit_published'
      : fitReady
        ? 'fit_ready'
        : cells.length === 0
          ? 'blocked_no_cells'
          : 'collecting_cells'

  return {
    blockerRefs:
      status === 'fit_published'
        ? []
        : [
            'blocker.cs336_a3.requires_twenty_verified_cells',
            'blocker.cs336_a3.operator_funding_required_for_paid_cells',
            'blocker.cs336_a3.fit_artifact_not_published',
          ],
    cells,
    fitArtifact,
    observedVerifiedCellCount,
    psionicLaneRef:
      optionalString(evidence?.psionicLaneRef) ??
      Cs336A3ScalingSweepPsionicLaneRef,
    requiredVerifiedCellCount: 20,
    schemaVersion: 'openagents.training.scaling_sweep.v1',
    scopeBoundaryRefs: [
      'scope.cs336_a3.isoflop_analysis_artifact_only',
      'scope.cs336_a3.receipt_backed_cells_only',
      'scope.cs336_a3.no_settlement_without_provider_confirmation',
    ],
    sourceRefs: uniqueRefs([
      'route:/api/training/isoflop/a3',
      `route:/api/training/runs/${input.run.trainingRunRef}`,
      ...input.run.sourceRefs,
      ...input.windows.flatMap(window => window.sourceRefs),
      ...input.leases.map(lease => lease.leaseRef),
    ]),
    status,
  }
}
