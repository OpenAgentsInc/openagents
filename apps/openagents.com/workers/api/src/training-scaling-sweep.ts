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
