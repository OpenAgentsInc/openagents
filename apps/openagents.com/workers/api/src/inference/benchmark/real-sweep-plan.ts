import type { BenchmarkLane, BenchmarkMatrixConfig } from './matrix'
import { expandMatrix } from './matrix'

export type RealSweepBlockerCode =
  | 'owner_confirmation_missing'
  | 'owner_approval_ref_missing'
  | 'budget_cap_missing'
  | 'billable_sample_cap_missing'
  | 'billable_sample_cap_exceeded'
  | 'real_traffic_evidence_missing'
  | 'real_traffic_evidence_invalid'
  | 'no_available_cells'

export type RealSweepWarningCode =
  | 'synthetic_traffic_not_decision_grade'
  | 'future_lanes_skipped'

export type RealSweepBlocker = Readonly<{
  code: RealSweepBlockerCode
  message: string
}>

export type RealSweepWarning = Readonly<{
  code: RealSweepWarningCode
  message: string
}>

export type RealSweepPreflightOptions = Readonly<{
  ownerConfirmed: boolean
  ownerApprovalRef?: string | undefined
  budgetCapMsat?: number | undefined
  maxBillableSamples: number
  trafficEvidence?: ReadonlyArray<RealTrafficShapeEvidence> | undefined
  executableFixtureOnlyLanes?: ReadonlyArray<BenchmarkLane> | undefined
  billableLanes?: ReadonlyArray<BenchmarkLane> | undefined
}>

export type RealTrafficShapeEvidence = Readonly<{
  shapeId: string
  evidenceRef: string
  observedRequestCount: number
  source: 'gateway_telemetry' | 'receipt_projection' | 'operator_export'
  publicSafe: boolean
}>

export type RealSweepPreflight = Readonly<{
  configId: string
  canArmRealSeam: boolean
  decisionGradeEligible: boolean
  ownerApprovalRef: string | null
  budgetCapMsat: number | null
  executableCells: number
  skippedFutureCells: number
  executableSampleUpperBound: number
  billableSampleUpperBound: number
  maxBillableSamples: number
  billableLanes: ReadonlyArray<BenchmarkLane>
  realisticShapes: number
  syntheticShapes: number
  realTrafficEvidenceRefs: ReadonlyArray<string>
  executableFixtureOnlyLanes: ReadonlyArray<BenchmarkLane>
  blockers: ReadonlyArray<RealSweepBlocker>
  warnings: ReadonlyArray<RealSweepWarning>
}>

const normalizedApprovalRef = (value: string | undefined): string | null => {
  if (value === undefined) {
    return null
  }
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const positiveFinite = (value: number | undefined): value is number =>
  value !== undefined && Number.isFinite(value) && value > 0

const samplesForCell = (samplesPerCell: number): number =>
  Math.max(1, Math.floor(samplesPerCell))

export const DEFAULT_BILLABLE_REAL_SWEEP_LANES: ReadonlyArray<BenchmarkLane> = [
  'claude',
  'fireworks',
  'openai-gpt',
  'partner-passthrough',
  'vertex-anthropic',
  'vertex-gemini',
]

const DEFAULT_BILLABLE_REAL_SWEEP_LANE_SET = new Set(
  DEFAULT_BILLABLE_REAL_SWEEP_LANES,
)

const defaultBillableLanesForConfig = (
  config: BenchmarkMatrixConfig,
): ReadonlyArray<BenchmarkLane> => {
  const billable = new Set<BenchmarkLane>()
  for (const target of config.targets) {
    if (target.profile?.capacityClass === 'provider_managed') {
      billable.add(target.lane)
      continue
    }
    if (
      target.profile === undefined &&
      DEFAULT_BILLABLE_REAL_SWEEP_LANE_SET.has(target.lane)
    ) {
      billable.add(target.lane)
    }
  }
  return [...billable].sort()
}

const normalizedEvidenceRef = (value: string): string | null => {
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const evidenceByShapeId = (
  evidence: ReadonlyArray<RealTrafficShapeEvidence> | undefined,
): Map<string, RealTrafficShapeEvidence> => {
  const result = new Map<string, RealTrafficShapeEvidence>()
  for (const item of evidence ?? []) {
    const shapeId = item.shapeId.trim()
    if (shapeId !== '') {
      result.set(shapeId, item)
    }
  }
  return result
}

const evidenceSourceForShape = (
  source: BenchmarkMatrixConfig['shapes'][number]['source'],
): RealTrafficShapeEvidence['source'] | null => {
  switch (source) {
    case 'gateway_telemetry':
    case 'receipt_projection':
    case 'operator_export':
      return source
    case 'synthetic_fixture':
    case undefined:
      return null
  }
}

const inlineTrafficEvidenceFromConfig = (
  config: BenchmarkMatrixConfig,
): ReadonlyArray<RealTrafficShapeEvidence> => {
  const evidence: Array<RealTrafficShapeEvidence> = []
  for (const shape of config.shapes) {
    if (shape.provenance !== 'realistic') {
      continue
    }
    const source = evidenceSourceForShape(shape.source)
    if (
      source === null ||
      shape.observedTrafficEvidenceRef === undefined ||
      shape.observedRequestCount === undefined
    ) {
      continue
    }
    evidence.push({
      shapeId: shape.id,
      evidenceRef: shape.observedTrafficEvidenceRef,
      observedRequestCount: shape.observedRequestCount,
      source,
      publicSafe: true,
    })
  }
  return evidence
}

export const preflightRealBenchmarkSweep = (
  config: BenchmarkMatrixConfig,
  options: RealSweepPreflightOptions,
): RealSweepPreflight => {
  const cells = expandMatrix(config)
  const executableFixtureOnlyLanes = new Set(
    options.executableFixtureOnlyLanes ?? [],
  )
  const billableLanes = new Set(
    options.billableLanes ?? defaultBillableLanesForConfig(config),
  )
  const executableCells = cells.filter(
    cell =>
      cell.laneAvailability === 'available' ||
      (cell.laneAvailability === 'fixture_only' &&
        executableFixtureOnlyLanes.has(cell.lane)),
  )
  const skippedFutureCells = cells.length - executableCells.length
  const executableSampleUpperBound = executableCells.reduce(
    (total, cell) => total + samplesForCell(cell.samplesPerCell),
    0,
  )
  const billableSampleUpperBound = executableCells.reduce(
    (total, cell) =>
      billableLanes.has(cell.lane)
        ? total + samplesForCell(cell.samplesPerCell)
        : total,
    0,
  )
  const realisticShapeIds = new Set(
    config.shapes
      .filter(shape => shape.provenance === 'realistic')
      .map(shape => shape.id),
  )
  const syntheticShapeIds = new Set(
    config.shapes
      .filter(shape => shape.provenance === 'synthetic')
      .map(shape => shape.id),
  )
  const approvalRef = normalizedApprovalRef(options.ownerApprovalRef)
  const trafficEvidence = evidenceByShapeId([
    ...inlineTrafficEvidenceFromConfig(config),
    ...(options.trafficEvidence ?? []),
  ])
  const realTrafficEvidenceRefs: Array<string> = []
  const blockers: Array<RealSweepBlocker> = []
  const warnings: Array<RealSweepWarning> = []

  if (options.ownerConfirmed !== true) {
    blockers.push({
      code: 'owner_confirmation_missing',
      message: 'A real benchmark sweep needs explicit owner confirmation.',
    })
  }

  if (approvalRef === null) {
    blockers.push({
      code: 'owner_approval_ref_missing',
      message:
        'A real benchmark sweep needs a public-safe owner approval reference.',
    })
  }

  if (!positiveFinite(options.budgetCapMsat)) {
    blockers.push({
      code: 'budget_cap_missing',
      message: 'A real benchmark sweep needs a positive msat budget cap.',
    })
  }

  if (!positiveFinite(options.maxBillableSamples)) {
    blockers.push({
      code: 'billable_sample_cap_missing',
      message:
        'A real benchmark sweep needs a positive finite billable sample cap.',
    })
  } else if (billableSampleUpperBound > options.maxBillableSamples) {
    blockers.push({
      code: 'billable_sample_cap_exceeded',
      message:
        'The billable portion of the expanded matrix exceeds the owner-approved billable sample cap.',
    })
  }

  if (executableCells.length === 0) {
    blockers.push({
      code: 'no_available_cells',
      message: 'The matrix has no currently available lane cells to execute.',
    })
  }

  for (const shapeId of realisticShapeIds) {
    const evidence = trafficEvidence.get(shapeId)
    const evidenceRef =
      evidence === undefined
        ? null
        : normalizedEvidenceRef(evidence.evidenceRef)
    if (evidence === undefined || evidenceRef === null) {
      blockers.push({
        code: 'real_traffic_evidence_missing',
        message:
          'Every realistic sequence shape needs a public-safe observed Khala traffic evidence ref before the report can be decision-grade.',
      })
      continue
    }
    if (
      evidence.publicSafe !== true ||
      !Number.isInteger(evidence.observedRequestCount) ||
      evidence.observedRequestCount <= 0
    ) {
      blockers.push({
        code: 'real_traffic_evidence_invalid',
        message:
          'Observed traffic evidence must be public-safe and backed by at least one observed Khala request.',
      })
      continue
    }
    realTrafficEvidenceRefs.push(evidenceRef)
  }

  if (syntheticShapeIds.size > 0) {
    warnings.push({
      code: 'synthetic_traffic_not_decision_grade',
      message:
        'Synthetic sequence shapes may be used for a smoke, but not a decision-grade benchmark report.',
    })
  }

  if (skippedFutureCells > 0) {
    warnings.push({
      code: 'future_lanes_skipped',
      message:
        'Fixture-only or not-yet-available lanes stay in the comparison shape but are skipped by the real sweep.',
    })
  }

  const canArmRealSeam = blockers.length === 0
  const decisionGradeEligible =
    canArmRealSeam &&
    billableSampleUpperBound > 0 &&
    syntheticShapeIds.size === 0 &&
    realisticShapeIds.size > 0 &&
    realTrafficEvidenceRefs.length === realisticShapeIds.size

  return {
    configId: config.id,
    canArmRealSeam,
    decisionGradeEligible,
    ownerApprovalRef: approvalRef,
    budgetCapMsat: positiveFinite(options.budgetCapMsat)
      ? options.budgetCapMsat
      : null,
    executableCells: executableCells.length,
    skippedFutureCells,
    executableSampleUpperBound,
    billableSampleUpperBound,
    maxBillableSamples: options.maxBillableSamples,
    billableLanes: [...billableLanes].sort(),
    realisticShapes: realisticShapeIds.size,
    syntheticShapes: syntheticShapeIds.size,
    realTrafficEvidenceRefs,
    executableFixtureOnlyLanes: [...executableFixtureOnlyLanes].sort(),
    blockers,
    warnings,
  }
}
