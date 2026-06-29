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
  type SpatialBounds2,
  SpatialHashGrid,
  type TrainingRunBurstDefinition,
  type TrainingRunEntityDefinition,
  type TrainingRunNodeStatus,
  type TrainingRunNodeDefinition,
  type TrainingRunVector,
  type TrainingRunVisualizationOptions,
  type TrainingRunVisualizationSnapshot,
  type TrainingRunWorldItemDefinition,
  relaxMinimumDistanceLayout,
  trainingRunVisualizationOptionsFromSnapshot,
} from '@openagentsinc/three-effect/core'
import * as Three from 'three'

import { parseJsonRecord } from '../json-boundary'

interface TrainingRun {
  readonly label?: string
  readonly maxStalenessSeconds: number
  readonly runRef: string
  readonly runState?: string
  readonly sourceGeneratedAt?: string
  readonly state?: string
  readonly stalenessKind: string
}

interface RunEntity {
  readonly entityKind: string
  readonly entityRef: string
  readonly label: string
  readonly runRef: string
  readonly sourceRef?: string
  readonly status?: string
}

interface WorldEdge {
  readonly edgeKind: string
  readonly fromEntityRef: string
  readonly runRef: string
  readonly sourceRef?: string
  readonly toEntityRef: string
}

interface ProofRef {
  readonly entityRef: string
  readonly proofKind?: string
  readonly proofRef: string
  readonly runRef: string
  readonly title?: string
  readonly url: string
}

interface SettlementRef {
  readonly amountSats?: bigint | number
  readonly entityRef: string
  readonly movementMode: string
  readonly realBitcoinMoved: boolean
  readonly receiptRef: string
  readonly runRef: string
  readonly url: string
}

interface WorldEvent {
  readonly entityRef?: string
  readonly eventKind: string
  readonly eventRef: string
  readonly runRef: string
  readonly sourceGeneratedAt?: string
  readonly sourceRef?: string
  readonly summary?: string
}

interface WorldRegion {
  readonly avatarPositionMinIntervalMs: bigint | number
  readonly label?: string
  readonly localOriginX: number
  readonly localOriginY: number
  readonly localOriginZ: number
  readonly maxX: number
  readonly maxY: number
  readonly maxZ: number
  readonly minX: number
  readonly minY: number
  readonly minZ: number
  readonly proximityRadiusMeters: number
  readonly regionRef: string
  readonly roadDirectionX: number
  readonly roadDirectionY: number
  readonly roadDirectionZ: number
  readonly runRef: string
  readonly staleAvatarPositionMs: bigint | number
  readonly starterPylonSiteOffsetX: number
  readonly starterPylonSiteOffsetY: number
  readonly starterPylonSiteOffsetZ: number
  readonly streetNextRegionRef?: string
  readonly streetPrevRegionRef?: string
}

interface PylonStation {
  readonly interactionRadiusMeters: number
  readonly label?: string
  readonly positionX: number
  readonly positionY: number
  readonly positionZ: number
  readonly pylonRef: string
  readonly regionRef: string
  readonly runRef: string
  readonly sourceUrl: string
}

interface AgentAvatar {
  readonly actorKind?: string
  readonly avatarRef: string
  readonly displayName?: string
  readonly homePylonRef?: string
}

interface AvatarPosition {
  readonly avatarRef: string
  readonly movementMode?: string
  readonly pitch: number
  readonly positionX: number
  readonly positionY: number
  readonly positionZ: number
  readonly regionRef: string
  readonly yaw: number
}

interface PylonAttention {
  readonly attentionKind?: string
  readonly attentionRef: string
  readonly avatarRef: string
  readonly distanceMeters: number
  readonly pylonRef: string
  readonly sourceEntityRef?: string
}

interface LocalChatMessage {
  readonly body: string
  readonly bodyFormat?: string
  readonly channelKind?: string
  readonly messageRef: string
  readonly moderationState?: string
  readonly radiusMeters: number
  readonly regionRef: string
  readonly speakerAvatarRef: string
  readonly targetRef?: string
}

interface ChatBubble {
  readonly anchorEntityRef: string
  readonly bubbleRef: string
  readonly messageRef: string
  readonly speakerAvatarRef: string
}

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

export interface TassadarWorldVector {
  readonly x: number
  readonly y: number
  readonly z: number
}

export interface TassadarWorldPylonStation {
  readonly interactionRadiusMeters: number
  readonly label: string
  readonly position: TassadarWorldVector
  readonly pylonRef: string
  readonly regionRef: string
  readonly sourceUrl: string
}

export interface TassadarWorldRegion {
  readonly avatarPositionMinIntervalMs: number
  readonly bounds: Readonly<{
    maxX: number
    maxY: number
    maxZ: number
    minX: number
    minY: number
    minZ: number
  }>
  readonly label: string
  readonly localOrigin: TassadarWorldVector
  readonly proximityRadiusMeters: number
  readonly regionRef: string
  readonly roadDirection: TassadarWorldVector
  readonly runRef: string
  readonly staleAvatarPositionMs: number
  readonly starterPylonSiteOffset: TassadarWorldVector
  readonly streetNextRegionRef?: string
  readonly streetPrevRegionRef?: string
}

export interface TassadarWorldAgentAvatar {
  readonly actorKind: string
  readonly avatarRef: string
  readonly displayName: string
  readonly homePylonRef?: string
}

export interface TassadarWorldAvatarPosition {
  readonly avatarRef: string
  readonly movementMode: string
  readonly pitch: number
  readonly position: TassadarWorldVector
  readonly regionRef: string
  readonly yaw: number
}

export interface TassadarWorldPylonAttention {
  readonly attentionKind: string
  readonly attentionRef: string
  readonly avatarRef: string
  readonly distanceMeters: number
  readonly pylonRef: string
  readonly sourceEntityRef?: string
}

export interface TassadarWorldLocalChatMessage {
  readonly body: string
  readonly channelKind: string
  readonly messageRef: string
  readonly moderationState: string
  readonly radiusMeters: number
  readonly regionRef: string
  readonly speakerAvatarRef: string
  readonly targetRef?: string
}

export interface TassadarWorldChatBubble {
  readonly anchorEntityRef: string
  readonly bubbleRef: string
  readonly messageRef: string
  readonly speakerAvatarRef: string
}

export interface TassadarWorldActivityMotion {
  readonly atId: string
  readonly cursor: string
  readonly eventRef: string
  readonly expiresAt?: string
  readonly generatedAt: string
  readonly motionId: string
  readonly motionKind: string
  readonly sourceKind: string
  readonly sourceLagStatus?: string
  readonly sourceRefs: ReadonlyArray<string>
  readonly text: string
}

export interface TassadarRunBulletin {
  readonly headline?: string
  readonly latestActivity?: ReadonlyArray<{
    readonly label?: string
    readonly occurredAt?: string
    readonly sourceRefs?: ReadonlyArray<string>
    readonly text?: string
  }>
  readonly metrics?: {
    readonly acceptedTraceCount?: number
    readonly activePylonCount?: number
    readonly activeWindowCount?: number
    readonly realSettlementCount?: number
    readonly settledSats?: number
    readonly totalPylonCount?: number
    readonly verifiedWorkCount?: number
  }
  readonly onBoardLines?: ReadonlyArray<string>
  readonly schemaVersion?: string
  readonly sourceRefs?: ReadonlyArray<string>
  readonly statusLine?: string
  readonly summary?: string
  readonly title?: string
}

/** Narrow structural view of the worker's `TrainingRunPublicSummary` (public-safe). */
export interface TassadarRunPublicSummary {
  readonly bulletin?: TassadarRunBulletin
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
  readonly world?: {
    readonly activityMotions?: ReadonlyArray<TassadarWorldActivityMotion>
    readonly agentAvatars?: ReadonlyArray<TassadarWorldAgentAvatar>
    readonly avatarPositions?: ReadonlyArray<TassadarWorldAvatarPosition>
    readonly chatBubbles?: ReadonlyArray<TassadarWorldChatBubble>
    readonly localChatMessages?: ReadonlyArray<TassadarWorldLocalChatMessage>
    readonly pylonAttention?: ReadonlyArray<TassadarWorldPylonAttention>
    readonly pylonStations?: ReadonlyArray<TassadarWorldPylonStation>
    readonly worldRegions?: ReadonlyArray<TassadarWorldRegion>
  }
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

const textOrUnknown = (value: string | undefined): string => {
  const text = value?.trim()
  return text === undefined || text.length === 0 ? 'unknown' : text
}

const uniquePublicRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> => [
  ...new Set(refs.map(ref => ref?.trim() ?? '').filter(ref => ref !== '')),
]

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

const worldEntityZ = 0.2
const worldEntityLayoutBounds: SpatialBounds2 = {
  maxX: 7.6,
  maxY: 5.6,
  minX: -7.6,
  minY: -5.6,
}
const worldEntityLayoutSize = { height: 0.72, width: 0.72 } as const
const worldEntityMinimumDistance = 0.82

const stationEntityId = (pylonRef: string): string => `station.${pylonRef}`

const entityLayoutPoint = (
  entity: TrainingRunEntityDefinition,
): { x: number; y: number } | null =>
  entity.position === undefined
    ? null
    : { x: entity.position[0], y: entity.position[1] }

const entityLayoutDistance = (
  left: TrainingRunEntityDefinition,
  rightId: string,
  byId: ReadonlyMap<string, TrainingRunEntityDefinition>,
): number => {
  const right = byId.get(rightId)
  const leftPoint = entityLayoutPoint(left)
  const rightPoint = right === undefined ? null : entityLayoutPoint(right)
  if (leftPoint === null || rightPoint === null) {
    return Number.POSITIVE_INFINITY
  }
  return Math.hypot(leftPoint.x - rightPoint.x, leftPoint.y - rightPoint.y)
}

export const applyWorldEntitySpatialLayout = (
  entities: ReadonlyArray<TrainingRunEntityDefinition>,
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const positioned = entities.filter(
    (
      entity,
    ): entity is TrainingRunEntityDefinition & {
      readonly position: TrainingRunVector
    } => entity.position !== undefined,
  )
  if (positioned.length < 2) return entities

  const byId = new Map(entities.map(entity => [entity.id, entity]))
  const grid = new SpatialHashGrid<string>({
    bounds: worldEntityLayoutBounds,
    cellsX: 8,
    cellsY: 8,
  })
  for (const entity of positioned) {
    grid.insert({
      id: entity.id,
      position: { x: entity.position[0], y: entity.position[1] },
      size: worldEntityLayoutSize,
      value: entity.id,
    })
  }

  const crowded = positioned.some(entity => {
    const point = entityLayoutPoint(entity)
    if (point === null) return false
    return grid
      .findNear(point, worldEntityLayoutSize)
      .some(
        nearby =>
          nearby.id !== entity.id &&
          entityLayoutDistance(entity, nearby.id, byId) <
            worldEntityMinimumDistance,
      )
  })
  if (!crowded) return entities

  const layout = new Map(
    relaxMinimumDistanceLayout(
      positioned.map(entity => ({
        id: entity.id,
        position: new Three.Vector2(entity.position[0], entity.position[1]),
        radius: 0.18,
      })),
      {
        bounds: worldEntityLayoutBounds,
        iterations: 16,
        minDistance: worldEntityMinimumDistance,
        strength: 0.55,
      },
    ).map(result => [result.id, result.position] as const),
  )

  return entities.map(entity => {
    const position = entity.position
    const next = layout.get(entity.id)
    if (position === undefined || next === undefined) return entity
    return {
      ...entity,
      position: [coordinate(next.x), coordinate(next.y), position[2]],
    }
  })
}

const worldVectorFromRow = (row: {
  readonly positionX?: number
  readonly positionY?: number
  readonly positionZ?: number
}): TassadarWorldVector => ({
  x: coordinate(finiteOrZero(row.positionX)),
  y: coordinate(finiteOrZero(row.positionY)),
  z: coordinate(finiteOrZero(row.positionZ)),
})

const stationEntityPosition = (
  station: TassadarWorldPylonStation,
): TrainingRunVector => [
  coordinate(station.position.x - 0.32),
  coordinate(station.position.z - 0.12),
  worldEntityZ,
]

const avatarEntityPosition = (
  position: TassadarWorldAvatarPosition | undefined,
  station: TassadarWorldPylonStation | undefined,
): TrainingRunVector => [
  coordinate(position?.position.x ?? (station?.position.x ?? -1.9) + 0.45),
  coordinate(position?.position.z ?? station?.position.z ?? 0),
  worldEntityZ + 0.08,
]

const chatBubbleEntityPosition = (
  anchorPosition: TrainingRunVector | undefined,
): TrainingRunVector => [
  coordinate(anchorPosition?.[0] ?? 0),
  coordinate((anchorPosition?.[1] ?? 0) + 0.32),
  worldEntityZ + 0.18,
]

const entityPositionMap = (
  entities: ReadonlyArray<TrainingRunEntityDefinition>,
): ReadonlyMap<string, TrainingRunVector> =>
  new Map(
    entities.flatMap(entity =>
      entity.position === undefined ? [] : [[entity.id, entity.position]],
    ),
  )

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

const pylonStationEntities = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const attentionCountByPylon = new Map<string, number>()
  for (const attention of summary.world?.pylonAttention ?? []) {
    attentionCountByPylon.set(
      attention.pylonRef,
      (attentionCountByPylon.get(attention.pylonRef) ?? 0) + 1,
    )
  }
  return (summary.world?.pylonStations ?? []).map(station => {
    const attentionCount = attentionCountByPylon.get(station.pylonRef) ?? 0
    return {
      id: stationEntityId(station.pylonRef),
      label:
        attentionCount === 0
          ? `${station.label} hub`
          : `${station.label} hub +${attentionCount}`,
      position: stationEntityPosition(station),
      status: attentionCount === 0 ? 'registered' : 'nearby',
    }
  })
}

const pylonAgentAvatarEntities = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const stations = new Map(
    (summary.world?.pylonStations ?? []).map(station => [
      station.pylonRef,
      station,
    ]),
  )
  const positions = new Map(
    (summary.world?.avatarPositions ?? []).map(position => [
      position.avatarRef,
      position,
    ]),
  )
  return (summary.world?.agentAvatars ?? []).flatMap(avatar => {
    const station =
      avatar.homePylonRef === undefined
        ? undefined
        : stations.get(avatar.homePylonRef)
    const position = positions.get(avatar.avatarRef)
    if (position === undefined) return []
    return {
      id: avatar.avatarRef,
      label: avatar.displayName,
      position: avatarEntityPosition(position, station),
      status: position?.movementMode ?? 'idle',
    }
  })
}

const truncateBubbleText = (body: string): string => {
  const text = body.trim().replace(/\s+/g, ' ')
  return text.length <= 48 ? text : `${text.slice(0, 45)}...`
}

const chatBubbleEntities = (
  summary: TassadarRunPublicSummary,
  layoutPositions: ReadonlyMap<string, TrainingRunVector> = new Map(),
): ReadonlyArray<TrainingRunEntityDefinition> => {
  const stations = new Map(
    (summary.world?.pylonStations ?? []).map(station => [
      station.pylonRef,
      station,
    ]),
  )
  const positions = new Map(
    (summary.world?.avatarPositions ?? []).map(position => [
      position.avatarRef,
      position,
    ]),
  )
  const messages = new Map(
    (summary.world?.localChatMessages ?? [])
      .filter(message => message.moderationState === 'visible')
      .map(message => [message.messageRef, message]),
  )
  return (summary.world?.chatBubbles ?? []).flatMap(bubble => {
    const message = messages.get(bubble.messageRef)
    if (message === undefined) return []
    const station = stations.get(bubble.anchorEntityRef)
    const position = positions.get(bubble.anchorEntityRef)
    const speakerPosition = positions.get(message.speakerAvatarRef)
    const stationLayoutPosition =
      station === undefined
        ? undefined
        : layoutPositions.get(stationEntityId(station.pylonRef))
    const avatarLayoutPosition =
      position === undefined
        ? undefined
        : layoutPositions.get(position.avatarRef)
    const speakerLayoutPosition =
      speakerPosition === undefined
        ? undefined
        : layoutPositions.get(speakerPosition.avatarRef)
    const anchorPosition =
      station === undefined
        ? position === undefined
          ? undefined
          : (avatarLayoutPosition ?? avatarEntityPosition(position, undefined))
        : (stationLayoutPosition ?? stationEntityPosition(station))
    const anchored = {
      id: bubble.bubbleRef,
      label: truncateBubbleText(message.body),
      position: chatBubbleEntityPosition(anchorPosition),
      status: message.channelKind === 'pylon' ? 'talking_to_pylon' : 'chat',
    }
    if (
      message.channelKind !== 'pylon' ||
      speakerPosition === undefined ||
      bubble.anchorEntityRef === message.speakerAvatarRef
    ) {
      return [anchored]
    }
    return [
      {
        id: `${bubble.bubbleRef}.speaker`,
        label: truncateBubbleText(message.body),
        position: chatBubbleEntityPosition(
          speakerLayoutPosition ??
            avatarEntityPosition(speakerPosition, undefined),
        ),
        status: 'chat',
      },
      {
        ...anchored,
      },
    ]
  })
}

type TassadarActivityBurstDefinition = TrainingRunBurstDefinition & {
  readonly cursor: string
  readonly eventRef: string
  readonly sourceKind: string
  readonly sourceLagStatus?: string
}

const activityBurstsFromSummary = (
  summary: TassadarRunPublicSummary,
  entities: ReadonlyArray<TrainingRunEntityDefinition>,
): ReadonlyArray<TassadarActivityBurstDefinition> => {
  const entityIds = new Set(entities.map(entity => entity.id))
  return (summary.world?.activityMotions ?? []).flatMap(motion => {
    if (!entityIds.has(motion.atId)) return []
    if (publicRefs(motion.sourceRefs).length === 0) return []
    if (motion.generatedAt.trim() === '') return []
    if (
      (motion.expiresAt ?? '').trim() === '' &&
      (motion.sourceLagStatus ?? '').trim() === ''
    ) {
      return []
    }
    return [
      {
        atId: motion.atId,
        cursor: motion.cursor,
        eventRef: motion.eventRef,
        ...(motion.expiresAt === undefined
          ? {}
          : { expiresAt: motion.expiresAt }),
        generatedAt: motion.generatedAt,
        motionId: motion.motionId,
        motionKind: motion.motionKind,
        simulated: motion.motionKind === 'settlement_recorded',
        sourceKind: motion.sourceKind,
        ...(motion.sourceLagStatus === undefined
          ? {}
          : { sourceLagStatus: motion.sourceLagStatus }),
        sourceRefs: motion.sourceRefs,
      },
    ]
  })
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
  detail: '',
  id: 'run',
  label: 'Tassadar',
  position: [-0.15, 0.28, 0],
  role: 'run',
  status: runNodeStatus(summary.runState),
})

const bulletinStatus = (
  summary: TassadarRunPublicSummary,
): TrainingRunNodeStatus =>
  summary.runState === 'active'
    ? 'active'
    : summary.runState === 'sealed' || summary.runState === 'reconciled'
      ? 'sealed'
      : summary.runState === 'planned'
        ? 'planned'
        : 'queued'

export const tassadarRunBulletinWorldItem = (
  summary: TassadarRunPublicSummary,
): TrainingRunWorldItemDefinition | null => {
  const bulletin = summary.bulletin
  if (bulletin === undefined) return null
  const title = textOrUnknown(bulletin.title)
  const headline = textOrUnknown(bulletin.headline)
  const body = textOrUnknown(bulletin.summary)
  return {
    id: 'bulletin.tassadar.run',
    kind: 'bulletin_board',
    label: title === 'unknown' ? 'Tassadar board' : title,
    title: title === 'unknown' ? 'Tassadar' : title,
    detail: body === 'unknown' ? headline : body,
    lines:
      bulletin.onBoardLines === undefined || bulletin.onBoardLines.length === 0
        ? [headline]
        : bulletin.onBoardLines,
    position: [-2.4, -2.35, 0.02],
    yaw: -0.12,
    interactionRadius: 2.8,
    status: bulletinStatus(summary),
    sourceRefs: publicRefs(bulletin.sourceRefs),
  }
}

export interface TassadarCloudflareWorldRows {
  readonly agentAvatars?: ReadonlyArray<AgentAvatar>
  readonly avatarPositions?: ReadonlyArray<AvatarPosition>
  readonly chatBubbles?: ReadonlyArray<ChatBubble>
  readonly localChatMessages?: ReadonlyArray<LocalChatMessage>
  readonly proofRefs?: ReadonlyArray<ProofRef>
  readonly pylonAttention?: ReadonlyArray<PylonAttention>
  readonly pylonStations?: ReadonlyArray<PylonStation>
  readonly runEntities?: ReadonlyArray<RunEntity>
  readonly settlementRefs?: ReadonlyArray<SettlementRef>
  readonly trainingRuns?: ReadonlyArray<TrainingRun>
  readonly worldEdges?: ReadonlyArray<WorldEdge>
  readonly worldEvents?: ReadonlyArray<WorldEvent>
  readonly worldRegions?: ReadonlyArray<WorldRegion>
}

const rowText = (value: string | null | undefined): string =>
  value?.trim() ?? ''

const ACTIVITY_WORLD_RUN_REF = 'run.public_activity_timeline'
const ACTIVITY_WORLD_EVENT_SUMMARY_SCHEMA =
  'openagents.world.public_activity_event_summary.v1'

const labelOrdinal = (label: string, prefix: string): number => {
  const match = label.match(new RegExp(`^${prefix}(\\d+)$`))
  return match?.[1] === undefined ? Number.POSITIVE_INFINITY : Number(match[1])
}

const rowSort =
  <Row extends { readonly label?: string; readonly entityRef: string }>(
    prefix: string,
  ) =>
  (left: Row, right: Row): number => {
    const labelCompare =
      labelOrdinal(rowText(left.label), prefix) -
      labelOrdinal(rowText(right.label), prefix)
    return labelCompare === 0
      ? left.entityRef.localeCompare(right.entityRef)
      : labelCompare
  }

const sortedRows = <
  Row extends { readonly label?: string; readonly entityRef: string },
>(
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

const unsafeActivityMotionMaterialPattern =
  /(@|\/Users\/|\/home\/|bearer\s+[A-Za-z0-9._-]+|customer[_-]?(email|name|phone|prompt)|email[_-]?(address|body|raw)|invoice[_-]?(raw|id)|lnbc|lntb|lnbcrt|mnemonic|payment[_-]?(hash|invoice|preimage|raw)|private[_-]?(key|source|trace|wallet)|provider[_-]?(payload|secret|token)|raw[_-]?(payload|prompt|trace|log)|secret|sk-[a-z0-9]|token[_-]?secret|wallet[_-]?(key|material|seed))/i

const safeActivitySummaryText = (value: string): boolean =>
  !unsafeActivityMotionMaterialPattern.test(value)

const jsonRecord = (value: string): Record<string, unknown> | null => {
  return parseJsonRecord(value) ?? null
}

const recordString = (record: Record<string, unknown>, key: string): string => {
  const value = record[key]
  return typeof value === 'string' ? value.trim() : ''
}

const recordRefs = (
  record: Record<string, unknown>,
  key: string,
): ReadonlyArray<string> => {
  const value = record[key]
  return Array.isArray(value)
    ? uniquePublicRefs(
        value.map(item => (typeof item === 'string' ? item : undefined)),
      )
    : []
}

const activityMotionFromWorldEvent = (
  row: WorldEvent,
): TassadarWorldActivityMotion | null => {
  const summaryText = rowText(row.summary)
  if (!row.eventRef.startsWith('world_event.public_activity.')) return null
  if (summaryText === '' || !safeActivitySummaryText(summaryText)) return null

  const summary = jsonRecord(summaryText)
  if (
    summary === null ||
    recordString(summary, 'schema') !== ACTIVITY_WORLD_EVENT_SUMMARY_SCHEMA
  ) {
    return null
  }

  const sourceRefs = uniquePublicRefs([
    rowText(row.sourceRef),
    ...recordRefs(summary, 'sourceRefs'),
  ])
  const generatedAt =
    recordString(summary, 'generatedAt') || rowText(row.sourceGeneratedAt)
  const expiresAt = recordString(summary, 'expiresAt')
  const sourceLagStatus = recordString(summary, 'sourceLagStatus')
  const atId = rowText(row.entityRef)

  if (
    atId === '' ||
    sourceRefs.length === 0 ||
    generatedAt === '' ||
    (expiresAt === '' && sourceLagStatus === '')
  ) {
    return null
  }

  return {
    atId,
    cursor: recordString(summary, 'cursor'),
    eventRef: recordString(summary, 'eventRef') || row.eventRef,
    ...(expiresAt === '' ? {} : { expiresAt }),
    generatedAt,
    motionId: row.eventRef,
    motionKind: recordString(summary, 'kind') || rowText(row.eventKind),
    sourceKind: recordString(summary, 'sourceKind'),
    ...(sourceLagStatus === '' ? {} : { sourceLagStatus }),
    sourceRefs,
    text: recordString(summary, 'text') || rowText(row.eventKind),
  }
}

const activityMotionsFromWorldEvents = (
  rows: ReadonlyArray<WorldEvent>,
  runRef: string,
): ReadonlyArray<TassadarWorldActivityMotion> =>
  rows
    .filter(
      row =>
        row.runRef === runRef ||
        row.runRef === ACTIVITY_WORLD_RUN_REF ||
        row.eventRef.startsWith('world_event.public_activity.'),
    )
    .map(activityMotionFromWorldEvent)
    .filter((motion): motion is TassadarWorldActivityMotion => motion !== null)
    .sort((left, right) => left.motionId.localeCompare(right.motionId))

const numberFromU64 = (value: SettlementRef['amountSats']): number =>
  typeof value === 'bigint'
    ? Number(value)
    : typeof value === 'number' && Number.isFinite(value)
      ? value
      : 0

const numberFromInteger = (value: bigint | number): number =>
  typeof value === 'bigint' ? Number(value) : Number.isFinite(value) ? value : 0

const worldRegionsFromRows = (
  rows: ReadonlyArray<WorldRegion>,
  runRef: string,
): ReadonlyArray<TassadarWorldRegion> =>
  [...rows]
    .filter(row => row.runRef === runRef && rowText(row.regionRef) !== '')
    .sort((left, right) => left.regionRef.localeCompare(right.regionRef))
    .map(row => {
      const streetPrevRegionRef = rowText(row.streetPrevRegionRef)
      const streetNextRegionRef = rowText(row.streetNextRegionRef)
      return {
        avatarPositionMinIntervalMs: numberFromInteger(
          row.avatarPositionMinIntervalMs,
        ),
        bounds: {
          maxX: finiteOrZero(row.maxX),
          maxY: finiteOrZero(row.maxY),
          maxZ: finiteOrZero(row.maxZ),
          minX: finiteOrZero(row.minX),
          minY: finiteOrZero(row.minY),
          minZ: finiteOrZero(row.minZ),
        },
        label: rowText(row.label) || shortRef(row.regionRef),
        localOrigin: {
          x: finiteOrZero(row.localOriginX),
          y: finiteOrZero(row.localOriginY),
          z: finiteOrZero(row.localOriginZ),
        },
        proximityRadiusMeters: finiteOrZero(row.proximityRadiusMeters),
        regionRef: row.regionRef,
        roadDirection: {
          x: finiteOrZero(row.roadDirectionX),
          y: finiteOrZero(row.roadDirectionY),
          z: finiteOrZero(row.roadDirectionZ),
        },
        runRef: row.runRef,
        staleAvatarPositionMs: numberFromInteger(row.staleAvatarPositionMs),
        starterPylonSiteOffset: {
          x: finiteOrZero(row.starterPylonSiteOffsetX),
          y: finiteOrZero(row.starterPylonSiteOffsetY),
          z: finiteOrZero(row.starterPylonSiteOffsetZ),
        },
        ...(streetNextRegionRef === '' ? {} : { streetNextRegionRef }),
        ...(streetPrevRegionRef === '' ? {} : { streetPrevRegionRef }),
      }
    })

const worldStationsFromRows = (
  rows: ReadonlyArray<PylonStation>,
  runRef: string,
): ReadonlyArray<TassadarWorldPylonStation> =>
  [...rows]
    .filter(row => row.runRef === runRef && rowText(row.pylonRef) !== '')
    .sort((left, right) => {
      const labelCompare =
        labelOrdinal(rowText(left.label), 'P') -
        labelOrdinal(rowText(right.label), 'P')
      return labelCompare === 0
        ? rowText(left.pylonRef).localeCompare(rowText(right.pylonRef))
        : labelCompare
    })
    .map(row => ({
      interactionRadiusMeters: finiteOrZero(row.interactionRadiusMeters),
      label: rowText(row.label) || shortRef(row.pylonRef),
      position: worldVectorFromRow(row),
      pylonRef: row.pylonRef,
      regionRef: row.regionRef,
      sourceUrl: row.sourceUrl,
    }))

const worldAvatarsFromRows = (
  rows: ReadonlyArray<AgentAvatar>,
  stationRefs: ReadonlySet<string>,
  avatarRefs: ReadonlySet<string>,
): ReadonlyArray<TassadarWorldAgentAvatar> =>
  [...rows]
    .filter(row => {
      const homePylonRef = rowText(row.homePylonRef)
      return (
        (homePylonRef !== '' && stationRefs.has(homePylonRef)) ||
        avatarRefs.has(row.avatarRef)
      )
    })
    .sort((left, right) =>
      rowText(left.displayName).localeCompare(rowText(right.displayName)),
    )
    .map(row => {
      const homePylonRef = rowText(row.homePylonRef)
      return {
        actorKind: rowText(row.actorKind) || 'pylon_agent',
        avatarRef: row.avatarRef,
        displayName: rowText(row.displayName) || shortRef(row.avatarRef),
        ...(homePylonRef === '' ? {} : { homePylonRef }),
      }
    })

const worldAvatarPositionsFromRows = (
  rows: ReadonlyArray<AvatarPosition>,
  regionRefs: ReadonlySet<string>,
): ReadonlyArray<TassadarWorldAvatarPosition> =>
  [...rows]
    .filter(row => regionRefs.has(row.regionRef))
    .sort((left, right) => left.avatarRef.localeCompare(right.avatarRef))
    .map(row => ({
      avatarRef: row.avatarRef,
      movementMode: rowText(row.movementMode) || 'idle',
      pitch: finiteOrZero(row.pitch),
      position: worldVectorFromRow(row),
      regionRef: row.regionRef,
      yaw: finiteOrZero(row.yaw),
    }))

const worldPylonAttentionFromRows = (
  rows: ReadonlyArray<PylonAttention>,
  stationRefs: ReadonlySet<string>,
  avatarRefs: ReadonlySet<string>,
): ReadonlyArray<TassadarWorldPylonAttention> =>
  [...rows]
    .filter(
      row => stationRefs.has(row.pylonRef) && avatarRefs.has(row.avatarRef),
    )
    .sort((left, right) => left.attentionRef.localeCompare(right.attentionRef))
    .map(row => ({
      attentionKind: rowText(row.attentionKind) || 'nearby',
      attentionRef: row.attentionRef,
      avatarRef: row.avatarRef,
      distanceMeters: finiteOrZero(row.distanceMeters),
      pylonRef: row.pylonRef,
      ...(rowText(row.sourceEntityRef) === ''
        ? {}
        : { sourceEntityRef: rowText(row.sourceEntityRef) }),
    }))

const worldLocalChatMessagesFromRows = (
  rows: ReadonlyArray<LocalChatMessage>,
  regionRefs: ReadonlySet<string>,
  avatarRefs: ReadonlySet<string>,
): ReadonlyArray<TassadarWorldLocalChatMessage> =>
  [...rows]
    .filter(
      row =>
        regionRefs.has(row.regionRef) &&
        avatarRefs.has(row.speakerAvatarRef) &&
        rowText(row.body) !== '' &&
        rowText(row.bodyFormat) === 'plain_text',
    )
    .sort((left, right) => left.messageRef.localeCompare(right.messageRef))
    .slice(-8)
    .map(row => ({
      body: rowText(row.body).slice(0, 280),
      channelKind: rowText(row.channelKind) || 'local',
      messageRef: row.messageRef,
      moderationState: rowText(row.moderationState) || 'visible',
      radiusMeters: finiteOrZero(row.radiusMeters),
      regionRef: row.regionRef,
      speakerAvatarRef: row.speakerAvatarRef,
      ...(rowText(row.targetRef) === ''
        ? {}
        : { targetRef: rowText(row.targetRef) }),
    }))

const worldChatBubblesFromRows = (
  rows: ReadonlyArray<ChatBubble>,
  messageRefs: ReadonlySet<string>,
  avatarRefs: ReadonlySet<string>,
  stationRefs: ReadonlySet<string>,
): ReadonlyArray<TassadarWorldChatBubble> =>
  [...rows]
    .filter(
      row =>
        messageRefs.has(row.messageRef) &&
        avatarRefs.has(row.speakerAvatarRef) &&
        (avatarRefs.has(row.anchorEntityRef) ||
          stationRefs.has(row.anchorEntityRef)),
    )
    .sort((left, right) => left.bubbleRef.localeCompare(right.bubbleRef))
    .map(row => ({
      anchorEntityRef: row.anchorEntityRef,
      bubbleRef: row.bubbleRef,
      messageRef: row.messageRef,
      speakerAvatarRef: row.speakerAvatarRef,
    }))

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
        ...(validator === undefined
          ? {}
          : { validatorRef: validator.entityRef }),
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
      entity?.status === 'simulation_settled' ||
      entity?.status === 'real_settled'
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

export const cloudflareWorldSummaryFromRows = (
  baseSummary: TassadarRunPublicSummary,
  rows: TassadarCloudflareWorldRows,
): TassadarRunPublicSummary => {
  const runRef = runRefForSummary(baseSummary)
  const trainingRun = (rows.trainingRuns ?? []).find(
    row => row.runRef === runRef,
  )
  const entities = (rows.runEntities ?? []).filter(row => row.runRef === runRef)
  const proofs = (rows.proofRefs ?? []).filter(row => row.runRef === runRef)
  const edges = (rows.worldEdges ?? []).filter(row => row.runRef === runRef)
  const settlements = (rows.settlementRefs ?? []).filter(
    row => row.runRef === runRef,
  )
  const events = (rows.worldEvents ?? []).filter(row => row.runRef === runRef)
  const activityMotions = activityMotionsFromWorldEvents(
    rows.worldEvents ?? [],
    runRef,
  )
  const worldRegions = worldRegionsFromRows(rows.worldRegions ?? [], runRef)
  const pylonStations = worldStationsFromRows(rows.pylonStations ?? [], runRef)
  const stationRefs = new Set(pylonStations.map(row => row.pylonRef))
  const regionRefs = new Set(
    [
      ...worldRegions.map(row => row.regionRef),
      ...pylonStations.map(row => row.regionRef),
    ].filter(ref => ref !== ''),
  )
  const avatarPositions = worldAvatarPositionsFromRows(
    rows.avatarPositions ?? [],
    regionRefs,
  )
  const positionedAvatarRefs = new Set(
    avatarPositions.map(row => row.avatarRef),
  )
  const agentAvatars = worldAvatarsFromRows(
    rows.agentAvatars ?? [],
    stationRefs,
    positionedAvatarRefs,
  )
  const avatarRefs = new Set(agentAvatars.map(row => row.avatarRef))
  const pylonAttention = worldPylonAttentionFromRows(
    rows.pylonAttention ?? [],
    stationRefs,
    avatarRefs,
  )
  const localChatMessages = worldLocalChatMessagesFromRows(
    rows.localChatMessages ?? [],
    regionRefs,
    avatarRefs,
  )
  const messageRefs = new Set(localChatMessages.map(row => row.messageRef))
  const chatBubbles = worldChatBubblesFromRows(
    rows.chatBubbles ?? [],
    messageRefs,
    avatarRefs,
    stationRefs,
  )

  if (
    trainingRun === undefined &&
    entities.length === 0 &&
    proofs.length === 0 &&
    settlements.length === 0 &&
    events.length === 0 &&
    activityMotions.length === 0 &&
    worldRegions.length === 0 &&
    pylonStations.length === 0 &&
    agentAvatars.length === 0 &&
    avatarPositions.length === 0 &&
    pylonAttention.length === 0 &&
    localChatMessages.length === 0 &&
    chatBubbles.length === 0
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
    ...(pylonStations.length === 0 &&
    activityMotions.length === 0 &&
    worldRegions.length === 0 &&
    agentAvatars.length === 0 &&
    avatarPositions.length === 0 &&
    pylonAttention.length === 0 &&
    localChatMessages.length === 0 &&
    chatBubbles.length === 0
      ? {}
      : {
          world: {
            ...baseSummary.world,
            ...(activityMotions.length === 0 ? {} : { activityMotions }),
            ...(agentAvatars.length === 0 ? {} : { agentAvatars }),
            ...(avatarPositions.length === 0 ? {} : { avatarPositions }),
            ...(chatBubbles.length === 0 ? {} : { chatBubbles }),
            ...(localChatMessages.length === 0 ? {} : { localChatMessages }),
            ...(pylonAttention.length === 0 ? {} : { pylonAttention }),
            ...(pylonStations.length === 0 ? {} : { pylonStations }),
            ...(worldRegions.length === 0 ? {} : { worldRegions }),
          },
        }),
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
  | 'worldItems'
> => {
  const rows = summary.realGradient?.leaderboardRows
  const pairs = summary.realGradient?.verifiedReplayPairs
  const rejectedPairs = summary.realGradient?.rejectedReplayPairs
  const settlements = Array.isArray(summary.settlementRows)
    ? summary.settlementRows
    : []
  const worldEntities = applyWorldEntitySpatialLayout([
    ...pylonStationEntities(summary),
    ...pylonAgentAvatarEntities(summary),
  ])
  const worldEntityPositions = entityPositionMap(worldEntities)
  const entities = [
    ...leaderboardEntities(rows, settlements),
    ...worldEntities,
    ...chatBubbleEntities(summary, worldEntityPositions),
    ...verifiedReplayEntities(pairs),
    ...rejectedReplayEntities(rejectedPairs),
    ...settlementEntities(settlements),
    ...corpusEntities(summary),
  ]
  const bursts = activityBurstsFromSummary(summary, entities)
  const bulletin = tassadarRunBulletinWorldItem(summary)

  return {
    beams: [],
    bursts,
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
    worldItems: bulletin === null ? [] : [bulletin],
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
