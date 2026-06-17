// Live Tassadar run → trainingRunView snapshot adapter (#5113, epic #5112).
//
// Maps the public-safe run projection (`TrainingRunPublicSummary` shape from the
// worker's training-run-window-authority — window counts, verified-work, settled
// sats, device counts, loss-under-budget, closeout) into the
// `TrainingRunVisualizationSnapshot` that `@openagentsinc/three-effect`'s
// `trainingRunView` already consumes. This replaces the hardcoded demo snapshot
// with real run data.
//
// RECEIPT-FIRST: this is a pure projection. Missing/idle fields map to honest
// zeros/nulls — a just-launched run with no verified work renders 0 verified / 0
// settled, never a faked value. The web app intentionally does NOT import the
// worker's internal types; this is a narrow structural view of the public summary.
import {
  type TrainingRunBeamDefinition,
  type TrainingRunContributorDefinition,
  type TrainingRunBurstDefinition,
  type TrainingRunEntityDefinition,
  type TrainingRunLossPoint,
  type TrainingRunVisualizationOptions,
  type TrainingRunVisualizationSnapshot,
  trainingRunVisualizationOptionsFromSnapshot,
} from '@openagentsinc/three-effect/core'

/** One public metric value (`{ value, provenanceLabel, sourceRefs }` — we read `value`). */
export interface PublicMetric {
  readonly sourceRefs?: ReadonlyArray<string>
  readonly value?: number
}

export interface PublicTrainingRunLeaderboardRow {
  readonly bestValidationLoss?: number | null
  readonly provenanceLabel?: string
  readonly pylonRef?: string
  readonly rank?: number
  readonly settledPayoutSats?: number
  readonly sourceRefs?: ReadonlyArray<string>
  readonly trainingRunRef?: string
  readonly verifiedWindowCount?: number
}

export interface PublicTrainingRunVerifiedReplayPair {
  readonly challengeRef?: string
  readonly provenanceLabel?: string
  readonly sourceRefs?: ReadonlyArray<string>
  readonly validatorRef?: string
  readonly verdictRefs?: ReadonlyArray<string>
  readonly workerRef?: string
}

export interface PublicTrainingRunRejectedReplayPair {
  readonly challengeRef?: string
  readonly failureCodes?: ReadonlyArray<string>
  readonly provenanceLabel?: string
  readonly sourceRefs?: ReadonlyArray<string>
  readonly validatorRef?: string | null
  readonly verdictRefs?: ReadonlyArray<string>
  readonly workerRef?: string
}

export interface PublicTassadarSettlementRow {
  readonly amountSats?: number
  readonly apiUrl?: string
  readonly contributorRef?: string | null
  readonly movementMode?: 'real_bitcoin' | 'simulation' | string
  readonly realBitcoinMoved?: boolean
  readonly receiptKind?: string
  readonly receiptPageUrl?: string
  readonly receiptRef?: string
  readonly sourceRefs?: ReadonlyArray<string>
  readonly state?: string
  readonly trainingRunRef?: string | null
  readonly verificationChallengeRef?: string | null
}

/** Narrow structural view of the worker's `TrainingRunPublicSummary` (public-safe). */
export interface TassadarRunPublicSummary {
  readonly corpus?: {
    readonly acceptedTraceCount?: number
    readonly traceRefs?: ReadonlyArray<string>
    readonly verdictRefs?: ReadonlyArray<string>
  }
  readonly generatedAt?: string
  readonly runRef?: string
  readonly runLabel?: string
  readonly runState?: string
  readonly staleness?: {
    readonly composition?: string
    readonly contractVersion?: string
    readonly maxStalenessSeconds?: number
  }
  readonly emptyState?: { readonly idle?: boolean; readonly reason?: string }
  readonly metrics?: {
    readonly activeWindowCount?: PublicMetric
    readonly plannedWindowCount?: PublicMetric
    readonly sealedWindowCount?: PublicMetric
    readonly reconciledWindowCount?: PublicMetric
    readonly assignedContributorCount?: PublicMetric
    readonly verifiedWorkCount?: PublicMetric
    readonly rejectedWorkCount?: PublicMetric
    readonly pendingPayoutCount?: PublicMetric
    readonly receiptRefCount?: PublicMetric
    readonly providerConfirmedSettledPayoutSats?: PublicMetric
  }
  readonly realGradient?: {
    readonly deviceRequirement?: {
      readonly observedDistinctContributorDevices?: number
      readonly requiredDistinctContributorDevices?: number
    }
    readonly lossUnderBudget?: {
      readonly finalValidationLoss?: number | null
      readonly maxValidationLoss?: number | null
      readonly satisfied?: boolean
    }
    readonly lossCurve?: ReadonlyArray<{
      readonly step?: number
      readonly validationLoss?: number
    }>
    readonly closeoutRequirement?: {
      readonly satisfied?: boolean
      readonly freivaldsCommitmentRefs?: ReadonlyArray<string>
      readonly gradientCloseoutRefs?: ReadonlyArray<string>
    }
    readonly externalAsk?: { readonly blockerRefs?: ReadonlyArray<string> }
    readonly leaderboardRows?: ReadonlyArray<PublicTrainingRunLeaderboardRow>
    readonly rejectedReplayPairs?: ReadonlyArray<PublicTrainingRunRejectedReplayPair>
    readonly verifiedReplayPairs?: ReadonlyArray<PublicTrainingRunVerifiedReplayPair>
  }
  readonly receiptRefs?: ReadonlyArray<string>
  readonly settlementRows?: ReadonlyArray<PublicTassadarSettlementRow>
  readonly windows?: ReadonlyArray<{
    readonly receiptRefs?: ReadonlyArray<string>
    readonly windowRef?: string
  }>
}

const metricValue = (metric: PublicMetric | undefined): number =>
  metric !== undefined &&
  typeof metric.value === 'number' &&
  Number.isFinite(metric.value)
    ? metric.value
    : 0

const finiteOrZero = (value: number | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : 0

const refCount = (refs: ReadonlyArray<unknown> | undefined): number =>
  Array.isArray(refs) ? refs.length : 0

const publicRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  Array.isArray(refs)
    ? refs.map(ref => ref.trim()).filter(ref => ref.length > 0)
    : []

const lossOrNull = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null

const shortRef = (ref: string): string => {
  const pieces = ref.split('.')
  const tail = pieces[pieces.length - 1] ?? ref
  return tail.length <= 10 ? tail : `${tail.slice(0, 4)}…${tail.slice(-4)}`
}

const settlementRowReceiptRef = (
  row: PublicTassadarSettlementRow,
): string | undefined => {
  const ref = row.receiptRef?.trim()
  return ref === undefined || ref.length === 0 ? undefined : ref
}

const settlementRowsForContributor = (
  rows: ReadonlyArray<PublicTassadarSettlementRow>,
  contributorRef: string,
): ReadonlyArray<PublicTassadarSettlementRow> =>
  rows.filter(row => row.contributorRef === contributorRef)

const settlementRowStatus = (row: PublicTassadarSettlementRow): string => {
  if (row.realBitcoinMoved === true) {
    return 'real_settled'
  }
  if (row.state === 'settled' && row.movementMode === 'simulation') {
    return 'simulation_settled'
  }
  if (
    row.state === 'failed' ||
    row.state === 'expired' ||
    row.state === 'rejected'
  ) {
    return 'failed_or_expired'
  }
  return 'pending_payout'
}

const contributorSettlementStatus = (
  rows: ReadonlyArray<PublicTassadarSettlementRow>,
): string | undefined => {
  if (rows.some(row => settlementRowStatus(row) === 'real_settled')) {
    return 'real_settled'
  }
  if (rows.some(row => settlementRowStatus(row) === 'simulation_settled')) {
    return 'simulation_settled'
  }
  if (rows.some(row => settlementRowStatus(row) === 'pending_payout')) {
    return 'pending_payout'
  }
  if (rows.some(row => settlementRowStatus(row) === 'failed_or_expired')) {
    return 'failed_or_expired'
  }
  return undefined
}

const leaderboardRowStatus = (
  row: PublicTrainingRunLeaderboardRow,
  settlements: ReadonlyArray<PublicTassadarSettlementRow>,
): string => {
  const settlementStatus =
    row.pylonRef === undefined
      ? undefined
      : contributorSettlementStatus(
          settlementRowsForContributor(settlements, row.pylonRef),
        )
  if (settlementStatus !== undefined) {
    return settlementStatus
  }
  if (finiteOrZero(row.verifiedWindowCount) > 0) {
    return 'verified'
  }
  return publicRefs(row.sourceRefs).length > 0 ? 'assigned' : 'registered'
}

const leaderboardEntities = (
  rows: ReadonlyArray<PublicTrainingRunLeaderboardRow> | undefined,
  settlements: ReadonlyArray<PublicTassadarSettlementRow>,
): ReadonlyArray<TrainingRunEntityDefinition> =>
  (rows ?? []).flatMap(row => {
    if (row.pylonRef === undefined || row.pylonRef.trim() === '') {
      return []
    }
    const rank = finiteOrZero(row.rank)
    return [
      {
        id: row.pylonRef,
        label: rank > 0 ? `P${rank}` : shortRef(row.pylonRef),
        status: leaderboardRowStatus(row, settlements),
      },
    ]
  })

const verifiedReplayEntities = (
  pairs: ReadonlyArray<PublicTrainingRunVerifiedReplayPair> | undefined,
): ReadonlyArray<TrainingRunEntityDefinition> =>
  (pairs ?? []).flatMap((pair, index) => {
    if (
      pair.workerRef === undefined ||
      pair.workerRef.trim() === '' ||
      pair.validatorRef === undefined ||
      pair.validatorRef.trim() === ''
    ) {
      return []
    }
    return [
      { id: pair.workerRef, label: `W${index + 1}`, status: 'verified' },
      { id: pair.validatorRef, label: `V${index + 1}`, status: 'verified' },
    ]
  })

const verifiedReplayBeams = (
  pairs: ReadonlyArray<PublicTrainingRunVerifiedReplayPair> | undefined,
): ReadonlyArray<TrainingRunBeamDefinition> =>
  (pairs ?? []).flatMap(pair => {
    if (
      pair.workerRef === undefined ||
      pair.workerRef.trim() === '' ||
      pair.validatorRef === undefined ||
      pair.validatorRef.trim() === '' ||
      pair.challengeRef === undefined ||
      pair.challengeRef.trim() === ''
    ) {
      return []
    }
    return [{ fromId: pair.workerRef, toId: pair.validatorRef }]
  })

const rejectedReplayEntities = (
  pairs: ReadonlyArray<PublicTrainingRunRejectedReplayPair> | undefined,
): ReadonlyArray<TrainingRunEntityDefinition> =>
  (pairs ?? []).flatMap((pair, index) => {
    const entities: TrainingRunEntityDefinition[] = []
    if (pair.workerRef !== undefined && pair.workerRef.trim() !== '') {
      entities.push({
        id: pair.workerRef,
        label: `RW${index + 1}`,
        status: 'rejected',
      })
    }
    if (pair.validatorRef !== null && pair.validatorRef !== undefined) {
      const validatorRef = pair.validatorRef.trim()
      if (validatorRef !== '') {
        entities.push({
          id: validatorRef,
          label: `RV${index + 1}`,
          status: 'rejected',
        })
      }
    }
    return entities
  })

const settlementEntities = (
  rows: ReadonlyArray<PublicTassadarSettlementRow>,
): ReadonlyArray<TrainingRunEntityDefinition> =>
  rows.flatMap(row => {
    const receiptRef = settlementRowReceiptRef(row)
    if (receiptRef === undefined) return []
    const amount =
      typeof row.amountSats === 'number' && Number.isFinite(row.amountSats)
        ? `${row.amountSats}s`
        : 'receipt'
    return [
      {
        id: receiptRef,
        label: amount,
        status: settlementRowStatus(row),
      },
    ]
  })

const corpusEntities = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<TrainingRunEntityDefinition> =>
  publicRefs(summary.corpus?.traceRefs).map((traceRef, index) => ({
    id: traceRef,
    label: `T${index + 1}`,
    status: 'accepted_trace',
  }))

const settlementBursts = (
  rows: ReadonlyArray<PublicTassadarSettlementRow>,
): ReadonlyArray<TrainingRunBurstDefinition> =>
  rows.flatMap(row =>
    row.realBitcoinMoved === true &&
    typeof row.contributorRef === 'string' &&
    row.contributorRef.trim() !== ''
      ? [{ atId: row.contributorRef }]
      : [],
  )

const contributorLifecycleState = (
  row: PublicTrainingRunLeaderboardRow,
  settlements: ReadonlyArray<PublicTassadarSettlementRow>,
): TrainingRunContributorDefinition['lifecycleState'] => {
  const settlementStatus =
    row.pylonRef === undefined
      ? undefined
      : contributorSettlementStatus(
          settlementRowsForContributor(settlements, row.pylonRef),
        )
  if (
    settlementStatus === 'real_settled' ||
    settlementStatus === 'simulation_settled'
  ) {
    return 'active'
  }
  if (finiteOrZero(row.verifiedWindowCount) > 0) {
    return 'active'
  }
  return publicRefs(row.sourceRefs).length > 0 ? 'warmup' : 'registered'
}

const contributorDefinitions = (
  rows: ReadonlyArray<PublicTrainingRunLeaderboardRow> | undefined,
  settlements: ReadonlyArray<PublicTassadarSettlementRow>,
): ReadonlyArray<TrainingRunContributorDefinition> => {
  const validRows = (rows ?? []).filter(
    row => row.pylonRef !== undefined && row.pylonRef.trim() !== '',
  )
  const divisor = Math.max(validRows.length, 1)

  return validRows.map((row, index) => ({
    id: row.pylonRef!,
    label:
      finiteOrZero(row.rank) > 0 ? `P${finiteOrZero(row.rank)}` : shortRef(row.pylonRef!),
    lifecycleState: contributorLifecycleState(row, settlements),
    phase: index / divisor,
  }))
}

const lossCurveFromPublicSummary = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<TrainingRunLossPoint> =>
  (summary.realGradient?.lossCurve ?? []).flatMap(point => {
    if (
      typeof point.step !== 'number' ||
      typeof point.validationLoss !== 'number' ||
      !Number.isFinite(point.step) ||
      !Number.isFinite(point.validationLoss)
    ) {
      return []
    }
    return [{ step: point.step, validationLoss: point.validationLoss }]
  })

export const trainingRunEntityLayerFromPublicSummary = (
  summary: TassadarRunPublicSummary,
): Pick<
  TrainingRunVisualizationOptions,
  'beams' | 'bursts' | 'contributors' | 'entities' | 'lossCurve'
> => {
  const rows = summary.realGradient?.leaderboardRows
  const pairs = summary.realGradient?.verifiedReplayPairs
  const rejectedPairs = summary.realGradient?.rejectedReplayPairs
  const settlements = Array.isArray(summary.settlementRows)
    ? summary.settlementRows
    : []
  const entities = [
    ...leaderboardEntities(rows, settlements),
    ...verifiedReplayEntities(pairs),
    ...rejectedReplayEntities(rejectedPairs),
    ...settlementEntities(settlements),
    ...corpusEntities(summary),
  ]

  return {
    beams: verifiedReplayBeams(pairs),
    bursts: settlementBursts(settlements),
    contributors: contributorDefinitions(rows, settlements),
    entities,
    lossCurve: lossCurveFromPublicSummary(summary),
  }
}

/**
 * Map a public Tassadar run summary into the visualization snapshot. Pure;
 * defensive (every field optional → honest default). Idle/just-launched runs
 * render as `planned` with zeroed counts.
 */
export const trainingRunSnapshotFromPublicSummary = (
  summary: TassadarRunPublicSummary,
): TrainingRunVisualizationSnapshot => {
  const metrics = summary.metrics ?? {}
  const gradient = summary.realGradient ?? {}
  const idle = summary.emptyState?.idle === true

  return {
    runState: summary.runState ?? (idle ? 'planned' : 'active'),
    runLabel: summary.runLabel ?? 'Tassadar executor run',
    runDetail: summary.runRef ?? 'run.tassadar.executor',
    activeWindowCount: metricValue(metrics.activeWindowCount),
    plannedWindowCount: metricValue(metrics.plannedWindowCount),
    sealedWindowCount: metricValue(metrics.sealedWindowCount),
    reconciledWindowCount: metricValue(metrics.reconciledWindowCount),
    assignedContributorCount: metricValue(metrics.assignedContributorCount),
    verifiedWorkCount: metricValue(metrics.verifiedWorkCount),
    rejectedWorkCount: metricValue(metrics.rejectedWorkCount),
    pendingPayoutCount: metricValue(metrics.pendingPayoutCount),
    receiptRefCount: metricValue(metrics.receiptRefCount),
    settledPayoutSats: metricValue(metrics.providerConfirmedSettledPayoutSats),
    deviceObserved: finiteOrZero(
      gradient.deviceRequirement?.observedDistinctContributorDevices,
    ),
    deviceRequired: finiteOrZero(
      gradient.deviceRequirement?.requiredDistinctContributorDevices,
    ),
    finalValidationLoss: lossOrNull(
      gradient.lossUnderBudget?.finalValidationLoss,
    ),
    maxValidationLoss: lossOrNull(gradient.lossUnderBudget?.maxValidationLoss),
    lossUnderBudget: gradient.lossUnderBudget?.satisfied === true,
    closeoutSatisfied: gradient.closeoutRequirement?.satisfied === true,
    freivaldsRefCount: refCount(
      gradient.closeoutRequirement?.freivaldsCommitmentRefs,
    ),
    gradientCloseoutRefCount: refCount(
      gradient.closeoutRequirement?.gradientCloseoutRefs,
    ),
    blockerRefCount: refCount(gradient.externalAsk?.blockerRefs),
  }
}

/**
 * Full chain: public run summary → resolved `trainingRunView` options. This is
 * the value a live route (#5114 public read / #5118 ship) hands to the element.
 */
export const tassadarRunVisualizationOptions = (
  summary: TassadarRunPublicSummary,
): TrainingRunVisualizationOptions => ({
  ...trainingRunVisualizationOptionsFromSnapshot(
    trainingRunSnapshotFromPublicSummary(summary),
  ),
  ...trainingRunEntityLayerFromPublicSummary(summary),
})
