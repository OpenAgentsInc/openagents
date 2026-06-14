import { isRecord, parseJsonRecord, stringArrayFromUnknown } from './json-boundary'
import type { DeviceCapabilityDatasetProjection } from './training-device-capability'
import type {
  TrainingRunPublicSummary,
  TrainingRunRecord,
} from './training-run-window-authority'
import type { Cs336A5EvalDashboardProjection } from './cs336-a5-alignment-homework'
import type {
  ScalingSweepCell,
  ScalingSweepProjection,
} from './training-scaling-sweep'

export const TrainingLeaderboardLanes = [
  'a1_loss',
  'a2_throughput',
  'a3_isoflop',
  'a4_eval_delta',
  'a5_accuracy',
] as const
export type TrainingLeaderboardLane = (typeof TrainingLeaderboardLanes)[number]

export type TrainingLeaderboardRow = Readonly<{
  contributorRef: string
  lane: TrainingLeaderboardLane
  metricRef: string
  provenanceLabel: string
  rank: number
  receiptRefs: ReadonlyArray<string>
  score: number
  scoreLabel: string
  scoreSortDirection: 'asc' | 'desc'
  settledPayoutSats: number
  sourceRefs: ReadonlyArray<string>
  trainingRunRef: string
  verifiedCloseoutRefs: ReadonlyArray<string>
}>

export type TrainingLeaderboardSection = Readonly<{
  blockerRefs: ReadonlyArray<string>
  lane: TrainingLeaderboardLane
  rows: ReadonlyArray<TrainingLeaderboardRow>
  sourceRefs: ReadonlyArray<string>
  title: string
}>

export type TrainingLeaderboardsProjection = Readonly<{
  blockerRefs: ReadonlyArray<string>
  lanes: ReadonlyArray<TrainingLeaderboardSection>
  schemaVersion: 'openagents.training.leaderboards.v1'
  sourceRefs: ReadonlyArray<string>
}>

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const laneTitle = (lane: TrainingLeaderboardLane): string =>
  lane === 'a1_loss'
    ? 'A1 Loss Under Budget'
    : lane === 'a2_throughput'
      ? 'A2 Throughput'
      : lane === 'a3_isoflop'
        ? 'A3 IsoFLOP Sweep'
        : lane === 'a4_eval_delta'
          ? 'A4 Eval Delta'
          : 'A5 Accuracy'

type TrainingLeaderboardDraftRow = Omit<
  TrainingLeaderboardRow,
  'provenanceLabel' | 'rank'
>

const rowProvenanceLabel =
  'Ranked from verified closeout receipt evidence only; settledPayoutSats counts provider-confirmed settled receipts linked to this row and never counts pending, offered, claimed, or wallet-side records.'

export const settledSatsFromPaymentAuthorityReceipt = (
  record: Readonly<{ publicProjectionJson: string; receiptKind: string }>,
): number => {
  if (record.receiptKind !== 'settlement_recorded') {
    return 0
  }

  const projection = parseJsonRecord(record.publicProjectionJson)
  const amountSats = projection?.amountSats

  return projection?.state === 'settled' &&
    typeof amountSats === 'number' &&
    Number.isInteger(amountSats) &&
    amountSats > 0
    ? amountSats
    : 0
}

const sortRows = (
  rows: ReadonlyArray<TrainingLeaderboardDraftRow>,
  settledSatsByReceiptRef: ReadonlyMap<string, number>,
): ReadonlyArray<TrainingLeaderboardRow> =>
  rows
    .map(row => ({
      ...row,
      provenanceLabel: rowProvenanceLabel,
      settledPayoutSats: Math.max(
        row.settledPayoutSats,
        row.receiptRefs.reduce(
          (total, ref) => total + (settledSatsByReceiptRef.get(ref) ?? 0),
          0,
        ),
      ),
    }))
    .sort((left, right) =>
      left.scoreSortDirection === 'asc'
        ? left.score - right.score
        : right.score - left.score,
    )
    .map((row, index) => ({ ...row, rank: index + 1 }))

const a1Rows = (
  summaries: ReadonlyArray<TrainingRunPublicSummary>,
  settlementReceiptRefsByContributor: ReadonlyMap<
    string,
    ReadonlyArray<string>
  >,
): ReadonlyArray<TrainingLeaderboardDraftRow> =>
  summaries.flatMap(summary =>
    summary.realGradient.leaderboardRows
      .filter(
        row => row.verifiedWindowCount > 0 && row.bestValidationLoss !== null,
      )
      .map(row => {
        const verifiedCloseoutRefs = uniqueRefs([
          ...summary.realGradient.closeoutRequirement.gradientCloseoutRefs,
          ...row.sourceRefs,
        ])

        return {
          contributorRef: row.pylonRef,
          lane: 'a1_loss' as const,
          metricRef: 'metric.cs336_a1.validation_loss',
          // Provider-confirmed settlement receipts linked to this run for this
          // contributor (openagents #5009). sortRows sums their settled sats.
          receiptRefs:
            settlementReceiptRefsByContributor.get(row.pylonRef) ?? [],
          score: row.bestValidationLoss ?? Number.POSITIVE_INFINITY,
          scoreLabel: 'validation_loss',
          scoreSortDirection: 'asc' as const,
          settledPayoutSats: row.settledPayoutSats,
          sourceRefs: row.sourceRefs,
          trainingRunRef: row.trainingRunRef,
          verifiedCloseoutRefs,
        }
      }),
  )

const a2Rows = (
  projections: ReadonlyArray<DeviceCapabilityDatasetProjection>,
): ReadonlyArray<TrainingLeaderboardDraftRow> =>
  projections.flatMap(projection =>
    projection.classDistributions
      .filter(distribution => distribution.verified)
      .map(distribution => ({
        contributorRef: distribution.deviceClassRef,
        lane: 'a2_throughput' as const,
        metricRef: `metric.cs336_a2.${distribution.metric}`,
        receiptRefs: distribution.receiptRefs,
        score: distribution.p90,
        scoreLabel: `${distribution.metric}_p90_${distribution.unit}`,
        scoreSortDirection: 'desc' as const,
        settledPayoutSats: 0,
        sourceRefs: distribution.sourceRefs,
        trainingRunRef: projection.sourceRefs
          .find(ref => ref.startsWith('route:/api/training/runs/'))
          ?.replace('route:/api/training/runs/', '') ?? 'training.run.unknown',
        verifiedCloseoutRefs: distribution.verificationRefs,
      })),
  )

const a4RowsFromRun = (
  run: TrainingRunRecord,
): ReadonlyArray<TrainingLeaderboardDraftRow> => {
  const projection = parseJsonRecord(run.publicProjectionJson)
  const a4 = isRecord(projection?.a4DataRefinery)
    ? projection?.a4DataRefinery
    : undefined
  const rows = a4?.leaderboardRows

  if (!Array.isArray(rows)) {
    return []
  }

  return rows.flatMap(row => {
    if (!isRecord(row)) {
      return []
    }

    const contributorRef =
      typeof row.contributorRef === 'string' ? row.contributorRef : undefined
    const score = typeof row.evalDelta === 'number' ? row.evalDelta : undefined
    const verificationRefs = uniqueRefs(stringArrayFromUnknown(row.verificationRefs))

    if (
      contributorRef === undefined ||
      score === undefined ||
      verificationRefs.length === 0
    ) {
      return []
    }

    return [
      {
        contributorRef,
        lane: 'a4_eval_delta' as const,
        metricRef: 'metric.cs336_a4.downstream_eval_delta',
        receiptRefs: uniqueRefs(stringArrayFromUnknown(row.receiptRefs)),
        score,
        scoreLabel: 'downstream_eval_delta',
        scoreSortDirection: 'desc' as const,
        settledPayoutSats: 0,
        sourceRefs: uniqueRefs(stringArrayFromUnknown(row.sourceRefs)),
        trainingRunRef: run.trainingRunRef,
        verifiedCloseoutRefs: verificationRefs,
      },
    ]
  })
}

const a5Rows = (
  projections: ReadonlyArray<Cs336A5EvalDashboardProjection>,
): ReadonlyArray<TrainingLeaderboardDraftRow> =>
  projections.flatMap(projection =>
    projection.evalSuites
      .filter(suite => suite.verificationRefs.length > 0)
      .map(suite => ({
        contributorRef: suite.evalSuiteRef,
        lane: 'a5_accuracy' as const,
        metricRef: `metric.cs336_a5.${suite.taskSetRef}.${suite.metric}`,
        receiptRefs: suite.receiptRefs,
        score: suite.score,
        scoreLabel: suite.metric,
        scoreSortDirection: 'desc' as const,
        settledPayoutSats: 0,
        sourceRefs: suite.sourceRefs,
        trainingRunRef: projection.sourceRefs
          .find(ref => ref.startsWith('route:/api/training/runs/'))
          ?.replace('route:/api/training/runs/', '') ?? 'training.run.unknown',
        verifiedCloseoutRefs: suite.verificationRefs,
      })),
  )

const rankableA3Cell = (
  cell: ScalingSweepCell,
): cell is ScalingSweepCell &
  Readonly<{ pylonRef: string; validationLoss: number }> =>
  cell.verified &&
  cell.verificationRefs.length > 0 &&
  cell.receiptRefs.length > 0 &&
  cell.pylonRef !== null &&
  cell.validationLoss !== null

const a3Rows = (
  projections: ReadonlyArray<ScalingSweepProjection>,
): ReadonlyArray<TrainingLeaderboardDraftRow> => {
  const bestCells = projections
    .flatMap(projection => projection.cells)
    .filter(rankableA3Cell)
    .reduce((best, cell) => {
      const key = `${cell.pylonRef}::${cell.computeBudgetFlops}`
      const current = best.get(key)

      return current !== undefined &&
        current.validationLoss <= cell.validationLoss
        ? best
        : best.set(key, cell)
    }, new Map<string, ScalingSweepCell & Readonly<{ pylonRef: string; validationLoss: number }>>())

  return [...bestCells.values()].map(cell => ({
    contributorRef: cell.pylonRef,
    lane: 'a3_isoflop' as const,
    metricRef: `metric.cs336_a3.validation_loss.c_${cell.computeBudgetFlops}`,
    receiptRefs: cell.receiptRefs,
    score: cell.validationLoss,
    scoreLabel: `validation_loss_at_${cell.computeBudgetFlops}_planned_flops`,
    scoreSortDirection: 'asc' as const,
    settledPayoutSats: cell.settledPayoutSats,
    sourceRefs: uniqueRefs([cell.cellRef, ...cell.sourceRefs]),
    trainingRunRef: cell.trainingRunRef,
    verifiedCloseoutRefs: cell.verificationRefs,
  }))
}

export const buildTrainingLeaderboardsProjection = (
  input: Readonly<{
    a2Projections: ReadonlyArray<DeviceCapabilityDatasetProjection>
    a3Projections: ReadonlyArray<ScalingSweepProjection>
    a5Projections: ReadonlyArray<Cs336A5EvalDashboardProjection>
    runs: ReadonlyArray<TrainingRunRecord>
    settledSatsByReceiptRef?: ReadonlyMap<string, number>
    settlementReceiptRefsByContributor?: ReadonlyMap<
      string,
      ReadonlyArray<string>
    >
    summaries: ReadonlyArray<TrainingRunPublicSummary>
  }>,
): TrainingLeaderboardsProjection => {
  const settledSatsByReceiptRef =
    input.settledSatsByReceiptRef ?? new Map<string, number>()
  const settlementReceiptRefsByContributor =
    input.settlementReceiptRefsByContributor ??
    new Map<string, ReadonlyArray<string>>()
  const rowsByLane: Record<
    TrainingLeaderboardLane,
    ReadonlyArray<TrainingLeaderboardDraftRow>
  > = {
    a1_loss: a1Rows(input.summaries, settlementReceiptRefsByContributor),
    a2_throughput: a2Rows(input.a2Projections),
    a3_isoflop: a3Rows(input.a3Projections),
    a4_eval_delta: input.runs.flatMap(a4RowsFromRun),
    a5_accuracy: a5Rows(input.a5Projections),
  }
  const lanes = TrainingLeaderboardLanes.map(lane => {
    const rows = sortRows(rowsByLane[lane], settledSatsByReceiptRef)

    return {
      blockerRefs:
        rows.length === 0
          ? [`blocker.training_leaderboard.${lane}.requires_verified_receipts`]
          : [],
      lane,
      rows,
      sourceRefs: uniqueRefs([
        'route:/api/training/leaderboards',
        ...rows.flatMap(row => row.sourceRefs),
      ]),
      title: laneTitle(lane),
    }
  })

  return {
    blockerRefs: lanes.flatMap(lane => lane.blockerRefs),
    lanes,
    schemaVersion: 'openagents.training.leaderboards.v1',
    sourceRefs: ['route:/api/training/leaderboards', 'route:/api/training/runs'],
  }
}
