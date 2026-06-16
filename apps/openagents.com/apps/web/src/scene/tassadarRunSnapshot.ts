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
  type TrainingRunVisualizationOptions,
  type TrainingRunVisualizationSnapshot,
  trainingRunVisualizationOptionsFromSnapshot,
} from '@openagentsinc/three-effect/core'

/** One public metric value (`{ value, provenanceLabel, sourceRefs }` — we read `value`). */
export interface PublicMetric {
  readonly value?: number
}

/** Narrow structural view of the worker's `TrainingRunPublicSummary` (public-safe). */
export interface TassadarRunPublicSummary {
  readonly runRef?: string
  readonly runLabel?: string
  readonly runState?: string
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
    readonly closeoutRequirement?: {
      readonly satisfied?: boolean
      readonly freivaldsCommitmentRefs?: ReadonlyArray<string>
      readonly gradientCloseoutRefs?: ReadonlyArray<string>
    }
    readonly externalAsk?: { readonly blockerRefs?: ReadonlyArray<string> }
  }
}

const metricValue = (metric: PublicMetric | undefined): number =>
  metric !== undefined && typeof metric.value === 'number' && Number.isFinite(metric.value)
    ? metric.value
    : 0

const finiteOrZero = (value: number | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : 0

const refCount = (refs: ReadonlyArray<unknown> | undefined): number =>
  Array.isArray(refs) ? refs.length : 0

const lossOrNull = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null

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
    deviceObserved: finiteOrZero(gradient.deviceRequirement?.observedDistinctContributorDevices),
    deviceRequired: finiteOrZero(gradient.deviceRequirement?.requiredDistinctContributorDevices),
    finalValidationLoss: lossOrNull(gradient.lossUnderBudget?.finalValidationLoss),
    maxValidationLoss: lossOrNull(gradient.lossUnderBudget?.maxValidationLoss),
    lossUnderBudget: gradient.lossUnderBudget?.satisfied === true,
    closeoutSatisfied: gradient.closeoutRequirement?.satisfied === true,
    freivaldsRefCount: refCount(gradient.closeoutRequirement?.freivaldsCommitmentRefs),
    gradientCloseoutRefCount: refCount(gradient.closeoutRequirement?.gradientCloseoutRefs),
    blockerRefCount: refCount(gradient.externalAsk?.blockerRefs),
  }
}

/**
 * Full chain: public run summary → resolved `trainingRunView` options. This is
 * the value a live route (#5114 public read / #5118 ship) hands to the element.
 */
export const tassadarRunVisualizationOptions = (
  summary: TassadarRunPublicSummary,
): TrainingRunVisualizationOptions =>
  trainingRunVisualizationOptionsFromSnapshot(trainingRunSnapshotFromPublicSummary(summary))
