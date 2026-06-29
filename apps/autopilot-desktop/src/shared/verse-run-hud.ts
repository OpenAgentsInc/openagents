import type {
  PublicTassadarRunSummary,
  TrainingPromiseGatesResponse,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
} from "./rpc.js"
import { selectedVerseTrainingSummary } from "./verse-training-visualization.js"

export type VerseRunHudSample = Readonly<{
  id: string
  label: string
  value: number
  valueText: string
  sourceRefs: readonly string[]
}>

export type VerseRunHudProjection = Readonly<{
  blockerCount: number
  fetchedAtLabel: string
  lossLabel: string
  promiseGreenCount: number
  promiseTotalCount: number
  runRef: string
  state: string
  samples: readonly VerseRunHudSample[]
}>

const boundedMetric = (value: number): number =>
  Number.isFinite(value) && value > 0 ? value : 0

const tassadarMetricValue = (
  summary: PublicTassadarRunSummary | null | undefined,
  key: string,
): number => {
  const raw = summary?.metrics?.[key]
  if (typeof raw === "number") return boundedMetric(raw)
  if (raw !== null && typeof raw === "object" && typeof raw.value === "number") {
    return boundedMetric(raw.value)
  }
  return 0
}

const metricRefs = (
  metric: { readonly sourceRefs: readonly string[] } | undefined,
): readonly string[] => metric?.sourceRefs ?? []

const shortRef = (value: string): string => {
  const trimmed = value.trim()
  if (trimmed.length <= 18) return trimmed
  const shortened = trimmed.slice(-18).replace(/^[.:/_-]+/, "")
  return shortened.length > 0 ? shortened : trimmed.slice(-18)
}

const sample = (
  id: string,
  label: string,
  value: number,
  valueText: string,
  sourceRefs: readonly string[],
): VerseRunHudSample => ({
  id,
  label,
  value: Math.max(0, Math.min(1, value)),
  valueText,
  sourceRefs,
})

const ratio = (value: number, denominator: number): number =>
  denominator <= 0 ? 0 : value / denominator

const refs = (refs: readonly string[]): readonly string[] =>
  refs.filter((ref, index) => ref.length > 0 && refs.indexOf(ref) === index)

const lossLabel = (summary: TrainingRunSummaryRow | null): string => {
  const loss = summary?.realGradient.lossUnderBudget
  if (loss?.finalValidationLoss == null || loss.maxValidationLoss == null) {
    return "loss n/a"
  }
  return `${loss.finalValidationLoss.toFixed(2)} / ${loss.maxValidationLoss.toFixed(2)}`
}

const runState = (state: string | undefined): string => {
  switch (state) {
    case "active":
    case "sealed":
    case "reconciled":
    case "planned":
      return state
    default:
      return state && state.trim().length > 0 ? state.trim() : "waiting"
  }
}

const tassadarHudProjection = (
  trainingRuns: TrainingRunsResponse,
  promiseGates: TrainingPromiseGatesResponse | null,
): VerseRunHudProjection | null => {
  const summary = trainingRuns.tassadarSummary
  if (summary === undefined || summary === null) return null

  const assigned = tassadarMetricValue(summary, "assignedContributorCount")
  const activeWindows = tassadarMetricValue(summary, "activeWindowCount")
  const acceptedTraceCount = tassadarMetricValue(summary, "acceptedTraceCount")
  const verified = tassadarMetricValue(summary, "verifiedWorkCount")
  const rejected = tassadarMetricValue(summary, "rejectedWorkCount")
  const settledSats =
    tassadarMetricValue(summary, "providerConfirmedSettledPayoutSats") ||
    tassadarMetricValue(summary, "settledSats")
  const acceptedTotal = verified + rejected
  const refs = summary.sourceRefs ?? []

  return {
    blockerCount: 0,
    fetchedAtLabel:
      trainingRuns.fetchedAt.length > 0
        ? new Date(trainingRuns.fetchedAt).toLocaleTimeString()
        : "waiting",
    lossLabel: activeWindows > 0 ? `${activeWindows.toLocaleString()} active` : "active n/a",
    promiseGreenCount: promiseGates?.stateCounts.green ?? 0,
    promiseTotalCount: promiseGates?.promises.length ?? 0,
    runRef: summary.runRef ? shortRef(summary.runRef) : "tassadar",
    state: runState(summary.runState),
    samples: [
      sample(
        "assign",
        "assign",
        ratio(assigned, Math.max(assigned, 1)),
        String(assigned),
        refs,
      ),
      sample(
        "trace",
        "trace",
        ratio(acceptedTraceCount, Math.max(acceptedTraceCount, 1)),
        String(acceptedTraceCount),
        refs,
      ),
      sample(
        "accept",
        "accept",
        ratio(verified, Math.max(acceptedTotal, 1)),
        String(verified),
        refs,
      ),
      sample(
        "reject",
        "reject",
        ratio(rejected, Math.max(acceptedTotal, 1)),
        String(rejected),
        refs,
      ),
      sample("proof", "proof", ratio(verified, Math.max(verified, 1)), String(verified), refs),
      sample(
        "settle",
        "settle",
        ratio(settledSats, Math.max(settledSats, 100_000)),
        settledSats === 0 ? "0" : `${Math.round(settledSats).toLocaleString()} sats`,
        refs,
      ),
      sample("payout", "payout", 0, "0", refs),
    ],
  }
}

export const verseRunHudProjection = (
  trainingRuns: TrainingRunsResponse | null,
  promiseGates: TrainingPromiseGatesResponse | null,
): VerseRunHudProjection => {
  if (trainingRuns !== null) {
    const tassadarProjection = tassadarHudProjection(trainingRuns, promiseGates)
    if (tassadarProjection !== null) return tassadarProjection
  }

  const summary = selectedVerseTrainingSummary(trainingRuns)
  const metrics = summary?.metrics
  const assigned = boundedMetric(metrics?.assignedContributorCount.value ?? 0)
  const receipts = boundedMetric(metrics?.receiptRefCount.value ?? 0)
  const verified = boundedMetric(metrics?.verifiedWorkCount.value ?? 0)
  const rejected = boundedMetric(metrics?.rejectedWorkCount.value ?? 0)
  const pendingPayouts = boundedMetric(metrics?.pendingPayoutCount.value ?? 0)
  const settledSats = boundedMetric(metrics?.providerConfirmedSettledPayoutSats.value ?? 0)
  const freivalds = summary?.realGradient.closeoutRequirement.freivaldsCommitmentRefs.length ?? 0
  const gradients = summary?.realGradient.closeoutRequirement.gradientCloseoutRefs.length ?? 0
  const blockers = refs([
    ...(promiseGates?.blockerRefs ?? []),
    ...(summary?.realGradient.externalAsk.blockerRefs ?? []),
  ])
  const promiseGreenCount = promiseGates?.stateCounts.green ?? 0
  const promiseTotalCount = promiseGates?.promises.length ?? 0
  const proofTotal = freivalds + gradients
  const acceptedTotal = verified + rejected

  return {
    blockerCount: blockers.length,
    fetchedAtLabel:
      trainingRuns?.fetchedAt && trainingRuns.fetchedAt.length > 0
        ? new Date(trainingRuns.fetchedAt).toLocaleTimeString()
        : "waiting",
    lossLabel: lossLabel(summary),
    promiseGreenCount,
    promiseTotalCount,
    runRef: summary?.run.trainingRunRef ? shortRef(summary.run.trainingRunRef) : "waiting",
    state:
      blockers.length > 0
        ? "blocked"
        : summary?.run.state ?? (trainingRuns === null ? "waiting" : "queued"),
    samples: [
      sample(
        "assign",
        "assign",
        ratio(assigned, Math.max(assigned, receipts, 1)),
        String(assigned),
        metricRefs(metrics?.assignedContributorCount),
      ),
      sample(
        "trace",
        "trace",
        ratio(receipts, Math.max(receipts, assigned, 1)),
        String(receipts),
        metricRefs(metrics?.receiptRefCount),
      ),
      sample(
        "accept",
        "accept",
        ratio(verified, Math.max(acceptedTotal, 1)),
        String(verified),
        metricRefs(metrics?.verifiedWorkCount),
      ),
      sample(
        "reject",
        "reject",
        ratio(rejected, Math.max(acceptedTotal, 1)),
        String(rejected),
        metricRefs(metrics?.rejectedWorkCount),
      ),
      sample(
        "proof",
        "proof",
        ratio(proofTotal, Math.max(proofTotal, 1)),
        String(proofTotal),
        refs([
          ...(summary?.realGradient.closeoutRequirement.freivaldsCommitmentRefs ?? []),
          ...(summary?.realGradient.closeoutRequirement.gradientCloseoutRefs ?? []),
        ]),
      ),
      sample(
        "settle",
        "settle",
        ratio(settledSats, Math.max(settledSats, 100_000)),
        settledSats === 0 ? "0" : `${Math.round(settledSats).toLocaleString()} sats`,
        metricRefs(metrics?.providerConfirmedSettledPayoutSats),
      ),
      sample(
        "payout",
        "payout",
        pendingPayouts > 0 ? 1 : 0,
        String(pendingPayouts),
        metricRefs(metrics?.pendingPayoutCount),
      ),
    ],
  }
}
