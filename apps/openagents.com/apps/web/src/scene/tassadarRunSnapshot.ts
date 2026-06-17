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
  type TrainingRunEntityDefinition,
  type TrainingRunNodeDefinition,
  type TrainingRunVector,
  type TrainingRunVisualizationOptions,
  type TrainingRunVisualizationSnapshot,
  trainingRunVisualizationOptionsFromSnapshot,
} from '@openagentsinc/three-effect/core'

import type {
  ProofRef,
  RunEntity,
  SettlementRef,
  TrainingRun,
  WorldEdge,
  WorldEvent,
} from './spacetimeWorldBindings/types'

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
    readonly qualifiedContributorCount?: PublicMetric
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

const uniquePublicRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref?.trim() ?? '').filter(ref => ref !== ''))]

const lossOrNull = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null

const shortRef = (ref: string): string => {
  const pieces = ref.split('.')
  const tail = pieces[pieces.length - 1] ?? ref
  return tail.length <= 10 ? tail : `${tail.slice(0, 4)}…${tail.slice(-4)}`
}

const runRefForSummary = (summary: TassadarRunPublicSummary): string =>
  summary.runRef ?? 'run.tassadar.executor.20260615'

const liveEntityZ = 0.12

const coordinate = (value: number): number => Math.round(value * 1000) / 1000

const spread = (
  index: number,
  total: number,
  start: number,
  end: number,
): number =>
  coordinate(
    total <= 1
      ? (start + end) / 2
      : start + ((end - start) * index) / (total - 1),
  )

const pylonEntityPosition = (
  index: number,
  total: number,
): TrainingRunVector => [-2.35, spread(index, total, 1.5, -1.5), liveEntityZ]

const verifiedReplayEntityPosition = (
  index: number,
  total: number,
  role: 'worker' | 'validator',
): TrainingRunVector => [
  spread(index, total, -0.95, 0.95),
  role === 'worker' ? 2.05 : 1.48,
  liveEntityZ,
]

const rejectedReplayEntityPosition = (
  index: number,
  total: number,
  role: 'worker' | 'validator',
): TrainingRunVector => [
  spread(index, total, -0.95, 0.95),
  role === 'worker' ? -1.14 : -1.86,
  liveEntityZ,
]

const settlementEntityPosition = (
  index: number,
  total: number,
): TrainingRunVector => [spread(index, total, -1.55, -0.75), -2.2, liveEntityZ]

const corpusEntityPosition = (
  index: number,
  total: number,
): TrainingRunVector => [2.25, spread(index, total, 0.8, -0.6), liveEntityZ]

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
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const validRows = (rows ?? []).filter(
    row => row.pylonRef !== undefined && row.pylonRef.trim() !== '',
  )
  return validRows.flatMap((row, index) => {
    const pylonRef = row.pylonRef?.trim() ?? ''
    if (pylonRef === '') return []
    const rank = finiteOrZero(row.rank)
    return [
      {
        id: pylonRef,
        label: rank > 0 ? `P${rank}` : shortRef(pylonRef),
        position: pylonEntityPosition(index, validRows.length),
        status: leaderboardRowStatus(row, settlements),
      },
    ]
  })
}

const verifiedReplayEntities = (
  pairs: ReadonlyArray<PublicTrainingRunVerifiedReplayPair> | undefined,
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const validPairs = (pairs ?? []).filter(
    pair =>
      pair.workerRef !== undefined &&
      pair.workerRef.trim() !== '' &&
      pair.validatorRef !== undefined &&
      pair.validatorRef.trim() !== '',
  )
  return validPairs.flatMap((pair, index) => {
    if (
      pair.workerRef === undefined ||
      pair.workerRef.trim() === '' ||
      pair.validatorRef === undefined ||
      pair.validatorRef.trim() === ''
    ) {
      return []
    }
    return [
      {
        id: pair.workerRef,
        label: `W${index + 1}`,
        position: verifiedReplayEntityPosition(
          index,
          validPairs.length,
          'worker',
        ),
        status: 'verified',
      },
      {
        id: pair.validatorRef,
        label: `V${index + 1}`,
        position: verifiedReplayEntityPosition(
          index,
          validPairs.length,
          'validator',
        ),
        status: 'verified',
      },
    ]
  })
}

const rejectedReplayEntities = (
  pairs: ReadonlyArray<PublicTrainingRunRejectedReplayPair> | undefined,
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const validPairs = (pairs ?? []).filter(
    pair =>
      (pair.workerRef !== undefined && pair.workerRef.trim() !== '') ||
      (pair.validatorRef !== null &&
        pair.validatorRef !== undefined &&
        pair.validatorRef.trim() !== ''),
  )
  return validPairs.flatMap((pair, index) => {
    const entities: TrainingRunEntityDefinition[] = []
    if (pair.workerRef !== undefined && pair.workerRef.trim() !== '') {
      entities.push({
        id: pair.workerRef,
        label: `RW${index + 1}`,
        position: rejectedReplayEntityPosition(
          index,
          validPairs.length,
          'worker',
        ),
        status: 'rejected',
      })
    }
    if (pair.validatorRef !== null && pair.validatorRef !== undefined) {
      const validatorRef = pair.validatorRef.trim()
      if (validatorRef !== '') {
        entities.push({
          id: validatorRef,
          label: `RV${index + 1}`,
          position: rejectedReplayEntityPosition(
            index,
            validPairs.length,
            'validator',
          ),
          status: 'rejected',
        })
      }
    }
    return entities
  })
}

const settlementEntities = (
  rows: ReadonlyArray<PublicTassadarSettlementRow>,
): ReadonlyArray<TrainingRunEntityDefinition> =>
  rows.flatMap((row, index) => {
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
        position: settlementEntityPosition(index, rows.length),
        status: settlementRowStatus(row),
      },
    ]
  })

const corpusEntities = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const refs = publicRefs(summary.corpus?.traceRefs)
  return refs.map((traceRef, index) => ({
    id: traceRef,
    label: `T${index + 1}`,
    position: corpusEntityPosition(index, refs.length),
    status: 'accepted_trace',
  }))
}

const runNodeStatus = (
  state: string | undefined,
): TrainingRunNodeDefinition['status'] =>
  state === 'active'
    ? 'active'
    : state === 'sealed' || state === 'reconciled'
      ? 'sealed'
      : state === 'blocked'
        ? 'blocked'
        : state === 'planned'
          ? 'planned'
          : 'queued'

const runNodeFromPublicSummary = (
  summary: TassadarRunPublicSummary,
): TrainingRunNodeDefinition => ({
  connectedTo: [],
  detail: summary.runRef ?? 'run.tassadar.executor.20260615',
  id: 'run',
  label: summary.runLabel ?? 'Tassadar executor run',
  position: [-0.15, 0.28, 0],
  role: 'run',
  status: runNodeStatus(summary.runState),
})

export interface TassadarSpacetimeWorldRows {
  readonly proofRefs?: ReadonlyArray<ProofRef>
  readonly runEntities?: ReadonlyArray<RunEntity>
  readonly settlementRefs?: ReadonlyArray<SettlementRef>
  readonly trainingRuns?: ReadonlyArray<TrainingRun>
  readonly worldEdges?: ReadonlyArray<WorldEdge>
  readonly worldEvents?: ReadonlyArray<WorldEvent>
}

const rowText = (value: string | undefined): string => value?.trim() ?? ''

const labelOrdinal = (label: string, prefix: string): number => {
  const match = label.match(new RegExp(`^${prefix}(\\d+)$`))
  return match?.[1] === undefined ? Number.POSITIVE_INFINITY : Number(match[1])
}

const rowSort = <Row extends { readonly label?: string; readonly entityRef: string }>(
  prefix: string,
) => (left: Row, right: Row): number => {
  const labelCompare =
    labelOrdinal(rowText(left.label), prefix) -
    labelOrdinal(rowText(right.label), prefix)
  return labelCompare === 0
    ? left.entityRef.localeCompare(right.entityRef)
    : labelCompare
}

const sortedRows = <Row extends { readonly label?: string; readonly entityRef: string }>(
  rows: ReadonlyArray<Row>,
  prefix: string,
): ReadonlyArray<Row> => [...rows].sort(rowSort(prefix))

const sourceRefsForEntity = (
  entity: RunEntity,
  proofs: ReadonlyArray<ProofRef>,
  edges: ReadonlyArray<WorldEdge>,
  events: ReadonlyArray<WorldEvent>,
): ReadonlyArray<string> =>
  uniquePublicRefs([
    entity.sourceRef,
    ...proofs
      .filter(proof => proof.entityRef === entity.entityRef)
      .flatMap(proof => [proof.proofRef, proof.url]),
    ...edges
      .filter(
        edge =>
          edge.fromEntityRef === entity.entityRef ||
          edge.toEntityRef === entity.entityRef,
      )
      .map(edge => edge.sourceRef),
    ...events
      .filter(event => event.entityRef === entity.entityRef)
      .map(event => event.sourceRef),
  ])

const numberFromU64 = (value: SettlementRef['amountSats']): number =>
  typeof value === 'bigint'
    ? Number(value)
    : typeof value === 'number' && Number.isFinite(value)
      ? value
      : 0

const pylonRowsFromWorld = (
  entities: ReadonlyArray<RunEntity>,
  proofs: ReadonlyArray<ProofRef>,
  edges: ReadonlyArray<WorldEdge>,
  events: ReadonlyArray<WorldEvent>,
): ReadonlyArray<PublicTrainingRunLeaderboardRow> =>
  sortedRows(
    entities.filter(entity => entity.entityKind === 'pylon'),
    'P',
  ).map(entity => {
    const rank = labelOrdinal(entity.label, 'P')
    return {
      pylonRef: entity.entityRef,
      ...(Number.isFinite(rank) && rank > 0 ? { rank } : {}),
      sourceRefs: sourceRefsForEntity(entity, proofs, edges, events),
      verifiedWindowCount:
        entity.status === 'verified' ||
        entity.status === 'simulation_settled' ||
        entity.status === 'real_settled'
          ? 1
          : 0,
    }
  })

const pairRowsFromWorld = (
  entities: ReadonlyArray<RunEntity>,
  proofs: ReadonlyArray<ProofRef>,
  edges: ReadonlyArray<WorldEdge>,
  events: ReadonlyArray<WorldEvent>,
  kind: 'verified' | 'rejected',
):
  | ReadonlyArray<PublicTrainingRunVerifiedReplayPair>
  | ReadonlyArray<PublicTrainingRunRejectedReplayPair> => {
  const workerKind =
    kind === 'verified' ? 'verified_replay_worker' : 'rejected_replay_worker'
  const validatorKind =
    kind === 'verified'
      ? 'verified_replay_validator'
      : 'rejected_replay_validator'
  const workerPrefix = kind === 'verified' ? 'W' : 'RW'
  const validatorPrefix = kind === 'verified' ? 'V' : 'RV'
  const workers = sortedRows(
    entities.filter(entity => entity.entityKind === workerKind),
    workerPrefix,
  )
  const validators = sortedRows(
    entities.filter(entity => entity.entityKind === validatorKind),
    validatorPrefix,
  )

  return workers.flatMap(worker => {
    const index = labelOrdinal(worker.label, workerPrefix)
    const validator = validators.find(
      row => labelOrdinal(row.label, validatorPrefix) === index,
    )
    if (kind === 'verified' && validator === undefined) return []
    const refs = sourceRefsForEntity(worker, proofs, edges, events)
    const challengeRef = worker.sourceRef || validator?.sourceRef
    return [
      {
        ...(challengeRef === undefined || challengeRef === ''
          ? {}
          : { challengeRef }),
        sourceRefs: uniquePublicRefs([
          ...refs,
          ...(validator === undefined
            ? []
            : sourceRefsForEntity(validator, proofs, edges, events)),
        ]),
        ...(validator === undefined ? {} : { validatorRef: validator.entityRef }),
        verdictRefs: proofs
          .filter(
            proof =>
              proof.entityRef === worker.entityRef ||
              proof.entityRef === validator?.entityRef,
          )
          .map(proof => proof.proofRef),
        workerRef: worker.entityRef,
      },
    ]
  })
}

const settlementRowsFromWorld = (
  entities: ReadonlyArray<RunEntity>,
  proofs: ReadonlyArray<ProofRef>,
  edges: ReadonlyArray<WorldEdge>,
  settlements: ReadonlyArray<SettlementRef>,
  events: ReadonlyArray<WorldEvent>,
): ReadonlyArray<PublicTassadarSettlementRow> =>
  settlements.map(row => {
    const entity =
      entities.find(runEntity => runEntity.entityRef === row.entityRef) ??
      entities.find(runEntity => runEntity.entityRef === row.receiptRef)
    const sourceRefs =
      entity === undefined
        ? uniquePublicRefs([row.receiptRef])
        : sourceRefsForEntity(entity, proofs, edges, events)
    const contributorRef =
      edges.find(
        edge =>
          edge.edgeKind === 'pylon_to_settlement' &&
          edge.toEntityRef === row.entityRef,
      )?.fromEntityRef ?? null
    const settlementState =
      entity?.status === 'simulation_settled' || entity?.status === 'real_settled'
        ? 'settled'
        : entity?.status === 'failed_or_expired'
          ? 'failed'
          : 'pending'

    return {
      amountSats: numberFromU64(row.amountSats),
      apiUrl: row.url,
      contributorRef,
      movementMode: row.movementMode,
      realBitcoinMoved: row.realBitcoinMoved,
      receiptKind: 'settlement_recorded',
      receiptRef: row.receiptRef,
      sourceRefs,
      state: settlementState,
      trainingRunRef: row.runRef,
    }
  })

export const spacetimeWorldSummaryFromRows = (
  baseSummary: TassadarRunPublicSummary,
  rows: TassadarSpacetimeWorldRows,
): TassadarRunPublicSummary => {
  const runRef = runRefForSummary(baseSummary)
  const trainingRun = (rows.trainingRuns ?? []).find(row => row.runRef === runRef)
  const entities = (rows.runEntities ?? []).filter(row => row.runRef === runRef)
  const proofs = (rows.proofRefs ?? []).filter(row => row.runRef === runRef)
  const edges = (rows.worldEdges ?? []).filter(row => row.runRef === runRef)
  const settlements = (rows.settlementRefs ?? []).filter(
    row => row.runRef === runRef,
  )
  const events = (rows.worldEvents ?? []).filter(row => row.runRef === runRef)

  if (
    trainingRun === undefined &&
    entities.length === 0 &&
    proofs.length === 0 &&
    settlements.length === 0 &&
    events.length === 0
  ) {
    return baseSummary
  }

  const leaderboardRows = pylonRowsFromWorld(entities, proofs, edges, events)
  const verifiedReplayPairs = pairRowsFromWorld(
    entities,
    proofs,
    edges,
    events,
    'verified',
  ) as ReadonlyArray<PublicTrainingRunVerifiedReplayPair>
  const rejectedReplayPairs = pairRowsFromWorld(
    entities,
    proofs,
    edges,
    events,
    'rejected',
  ) as ReadonlyArray<PublicTrainingRunRejectedReplayPair>
  const settlementRows = settlementRowsFromWorld(
    entities,
    proofs,
    edges,
    settlements,
    events,
  )
  const traceRefs = sortedRows(
    entities.filter(entity => entity.entityKind === 'accepted_trace'),
    'T',
  ).map(entity => entity.entityRef)
  const receiptRefs = uniquePublicRefs([
    ...publicRefs(baseSummary.receiptRefs),
    ...settlements.map(row => row.receiptRef),
    ...proofs
      .filter(proof => proof.proofRef.startsWith('receipt.'))
      .map(proof => proof.proofRef),
  ])

  const generatedAt = trainingRun?.sourceGeneratedAt ?? baseSummary.generatedAt
  const runState = trainingRun?.runState ?? baseSummary.runState

  return {
    ...baseSummary,
    ...(traceRefs.length === 0
      ? {}
      : {
          corpus: {
            ...baseSummary.corpus,
            acceptedTraceCount: traceRefs.length,
            traceRefs,
          },
        }),
    ...(generatedAt === undefined ? {} : { generatedAt }),
    realGradient: {
      ...baseSummary.realGradient,
      ...(leaderboardRows.length > 0 ? { leaderboardRows } : {}),
      ...(rejectedReplayPairs.length > 0 ? { rejectedReplayPairs } : {}),
      ...(verifiedReplayPairs.length > 0 ? { verifiedReplayPairs } : {}),
    },
    receiptRefs,
    runRef,
    ...(runState === undefined ? {} : { runState }),
    ...(settlementRows.length === 0 ? {} : { settlementRows }),
    ...(trainingRun === undefined
      ? {}
      : {
          staleness: {
            ...baseSummary.staleness,
            composition: trainingRun.stalenessKind,
            maxStalenessSeconds: trainingRun.maxStalenessSeconds,
          },
        }),
  }
}

export const trainingRunEntityLayerFromPublicSummary = (
  summary: TassadarRunPublicSummary,
): Pick<
  TrainingRunVisualizationOptions,
  | 'beams'
  | 'bursts'
  | 'contributors'
  | 'entities'
  | 'lossCurve'
  | 'motionPolicy'
  | 'nodes'
  | 'sceneChrome'
  | 'stageNodeGlyph'
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
    beams: [],
    bursts: [],
    contributors: [],
    entities,
    lossCurve: [],
    motionPolicy: {
      ambient: 'static',
      bursts: 'once',
      evidence: 'required',
      structuralEdges: 'static',
    },
    nodes: [runNodeFromPublicSummary(summary)],
    sceneChrome: {
      contributorOrbit: 'hidden',
      lossPanel: 'hidden',
      staleRing: 'hidden',
      statusChart: 'hidden',
    },
    stageNodeGlyph: 'compact_gate',
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
