import { createHash } from 'node:crypto'

import { Array as Arr, Schema as S } from 'effect'

export const TraceRankSettlementState = S.Literals([
  'authorized',
  'paid',
  'accepted',
  'pending_payout',
  'dispatched',
  'confirmed',
  'reconciled',
  'settled',
])
export type TraceRankSettlementState = typeof TraceRankSettlementState.Type

export const TraceRankVerificationState = S.Literals([
  'accepted_verified',
  'manual_override_verified',
  'replay_verified',
  'self_reported',
  'unverified',
])
export type TraceRankVerificationState = typeof TraceRankVerificationState.Type

export class VerifiedOutcomeTraceRankEdge extends S.Class<VerifiedOutcomeTraceRankEdge>(
  'VerifiedOutcomeTraceRankEdge',
)({
  amountMsats: S.Number,
  outcomeRef: S.String,
  settlementState: TraceRankSettlementState,
  sourceActorRef: S.String,
  targetActorRef: S.String,
  traceRef: S.String,
  verificationState: TraceRankVerificationState,
}) {}

export class TraceRankRejectedEdge extends S.Class<TraceRankRejectedEdge>(
  'TraceRankRejectedEdge',
)({
  blockerRef: S.String,
  edge: VerifiedOutcomeTraceRankEdge,
}) {}

export class TraceRankAcceptedEdge extends S.Class<TraceRankAcceptedEdge>(
  'TraceRankAcceptedEdge',
)({
  amountMsats: S.Number,
  evidenceRefs: S.Array(S.String),
  outcomeRef: S.String,
  sourceActorRef: S.String,
  targetActorRef: S.String,
}) {}

export class TraceRankScore extends S.Class<TraceRankScore>('TraceRankScore')({
  actorRef: S.String,
  caveatRefs: S.Array(S.String),
  incomingSettledMsats: S.Number,
  score: S.Number,
  scoreRef: S.String,
  settledOutcomeCount: S.Number,
}) {}

export class TraceRankProjection extends S.Class<TraceRankProjection>(
  'TraceRankProjection',
)({
  acceptedEdges: S.Array(TraceRankAcceptedEdge),
  authorityRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  damping: S.Number,
  iterationCount: S.Number,
  rejectedEdges: S.Array(TraceRankRejectedEdge),
  scores: S.Array(TraceRankScore),
}) {}

export type TraceRankOptions = Readonly<{
  damping?: number
  iterationCount?: number
  seedActorRefs?: ReadonlyArray<string>
}>

const acceptedSettlementStates = new Set<TraceRankSettlementState>([
  'confirmed',
  'reconciled',
  'settled',
])

const acceptedVerificationStates = new Set<TraceRankVerificationState>([
  'accepted_verified',
  'manual_override_verified',
  'replay_verified',
])

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash('sha256').update(value).digest('hex').slice(0, 24)}`

const uniqueSorted = (values: ReadonlyArray<string>): Array<string> =>
  [...new Set(values)].sort()

const rejectEdge = (
  edge: VerifiedOutcomeTraceRankEdge,
  blockerRef: string,
): TraceRankRejectedEdge => new TraceRankRejectedEdge({ blockerRef, edge })

const acceptedEdgeFrom = (
  edge: VerifiedOutcomeTraceRankEdge,
): TraceRankAcceptedEdge =>
  new TraceRankAcceptedEdge({
    amountMsats: edge.amountMsats,
    evidenceRefs: [edge.outcomeRef, edge.traceRef],
    outcomeRef: edge.outcomeRef,
    sourceActorRef: edge.sourceActorRef,
    targetActorRef: edge.targetActorRef,
  })

const classifyEdge = (
  edge: VerifiedOutcomeTraceRankEdge,
): TraceRankAcceptedEdge | TraceRankRejectedEdge => {
  if (!Number.isFinite(edge.amountMsats) || edge.amountMsats <= 0) {
    return rejectEdge(edge, 'blocker.tracerank.non_positive_amount')
  }
  if (edge.sourceActorRef === edge.targetActorRef) {
    return rejectEdge(edge, 'blocker.tracerank.self_payment_edge')
  }
  if (!acceptedSettlementStates.has(edge.settlementState)) {
    return rejectEdge(edge, 'blocker.tracerank.settlement_not_final')
  }
  if (!acceptedVerificationStates.has(edge.verificationState)) {
    return rejectEdge(edge, 'blocker.tracerank.outcome_not_verified')
  }
  return acceptedEdgeFrom(edge)
}

const isAcceptedEdge = (
  edge: TraceRankAcceptedEdge | TraceRankRejectedEdge,
): edge is TraceRankAcceptedEdge => edge instanceof TraceRankAcceptedEdge

const actorsFrom = (
  edges: ReadonlyArray<TraceRankAcceptedEdge>,
  seedActorRefs: ReadonlyArray<string>,
): Array<string> =>
  uniqueSorted([
    ...seedActorRefs,
    ...edges.flatMap(edge => [edge.sourceActorRef, edge.targetActorRef]),
  ])

const baseRankFor = (
  actorRefs: ReadonlyArray<string>,
  seedActorRefs: ReadonlyArray<string>,
): ReadonlyMap<string, number> => {
  const seedSet = new Set(seedActorRefs.filter(ref => actorRefs.includes(ref)))
  const baseActors = seedSet.size > 0 ? [...seedSet] : actorRefs
  const baseScore = baseActors.length === 0 ? 0 : 1 / baseActors.length
  return new Map(
    actorRefs.map(ref => [ref, baseActors.includes(ref) ? baseScore : 0]),
  )
}

const outgoingTotals = (
  edges: ReadonlyArray<TraceRankAcceptedEdge>,
): ReadonlyMap<string, number> =>
  edges.reduce((totals, edge) => {
    totals.set(
      edge.sourceActorRef,
      (totals.get(edge.sourceActorRef) ?? 0) + edge.amountMsats,
    )
    return totals
  }, new Map<string, number>())

const incomingTotals = (
  edges: ReadonlyArray<TraceRankAcceptedEdge>,
  actorRefs: ReadonlyArray<string>,
): ReadonlyMap<string, Readonly<{ amountMsats: number; count: number }>> =>
  edges.reduce(
    (totals, edge) => {
      const previous = totals.get(edge.targetActorRef) ?? {
        amountMsats: 0,
        count: 0,
      }
      totals.set(edge.targetActorRef, {
        amountMsats: previous.amountMsats + edge.amountMsats,
        count: previous.count + 1,
      })
      return totals
    },
    new Map(actorRefs.map(ref => [ref, { amountMsats: 0, count: 0 }])),
  )

const iterateRank = (
  actorRefs: ReadonlyArray<string>,
  acceptedEdges: ReadonlyArray<TraceRankAcceptedEdge>,
  baseRank: ReadonlyMap<string, number>,
  totals: ReadonlyMap<string, number>,
  damping: number,
  remaining: number,
  currentRank: ReadonlyMap<string, number>,
): ReadonlyMap<string, number> => {
  if (remaining <= 0) {
    return currentRank
  }

  const nextRank = new Map(
    actorRefs.map(ref => [ref, (1 - damping) * (baseRank.get(ref) ?? 0)]),
  )

  acceptedEdges.forEach(edge => {
    const total = totals.get(edge.sourceActorRef) ?? 0
    if (total > 0) {
      nextRank.set(
        edge.targetActorRef,
        (nextRank.get(edge.targetActorRef) ?? 0) +
          damping *
            (currentRank.get(edge.sourceActorRef) ?? 0) *
            (edge.amountMsats / total),
      )
    }
  })

  actorRefs
    .filter(ref => (totals.get(ref) ?? 0) === 0)
    .forEach(ref => {
      const sinkShare = damping * (currentRank.get(ref) ?? 0)
      actorRefs.forEach(targetRef => {
        nextRank.set(
          targetRef,
          (nextRank.get(targetRef) ?? 0) +
            sinkShare * (baseRank.get(targetRef) ?? 0),
        )
      })
    })

  return iterateRank(
    actorRefs,
    acceptedEdges,
    baseRank,
    totals,
    damping,
    remaining - 1,
    nextRank,
  )
}

const normalizedScores = (
  actorRefs: ReadonlyArray<string>,
  rank: ReadonlyMap<string, number>,
): ReadonlyMap<string, number> => {
  const total = actorRefs.reduce((sum, ref) => sum + (rank.get(ref) ?? 0), 0)
  if (total <= 0) {
    return new Map(actorRefs.map(ref => [ref, 0]))
  }
  return new Map(actorRefs.map(ref => [ref, (rank.get(ref) ?? 0) / total]))
}

export const buildVerifiedOutcomeTraceRankProjection = (
  edges: ReadonlyArray<VerifiedOutcomeTraceRankEdge>,
  options: TraceRankOptions = {},
): TraceRankProjection => {
  const damping = options.damping ?? 0.85
  const iterationCount = options.iterationCount ?? 32
  const seedActorRefs = uniqueSorted(options.seedActorRefs ?? [])
  const classifiedEdges = edges.map(classifyEdge)
  const acceptedEdges = classifiedEdges.filter(isAcceptedEdge)
  const rejectedEdges = classifiedEdges.filter(
    (edge): edge is TraceRankRejectedEdge =>
      edge instanceof TraceRankRejectedEdge,
  )
  const actorRefs = actorsFrom(acceptedEdges, seedActorRefs)
  const baseRank = baseRankFor(actorRefs, seedActorRefs)
  const rank = normalizedScores(
    actorRefs,
    iterateRank(
      actorRefs,
      acceptedEdges,
      baseRank,
      outgoingTotals(acceptedEdges),
      damping,
      iterationCount,
      baseRank,
    ),
  )
  const incoming = incomingTotals(acceptedEdges, actorRefs)
  const scores = actorRefs
    .map(actorRef => {
      const totals = incoming.get(actorRef) ?? { amountMsats: 0, count: 0 }
      return new TraceRankScore({
        actorRef,
        caveatRefs: [
          'caveat.tracerank.read_only_projection',
          'caveat.tracerank.reputation_not_payment_authority',
        ],
        incomingSettledMsats: totals.amountMsats,
        score: rank.get(actorRef) ?? 0,
        scoreRef: stableRef('reputation.tracerank.actor', actorRef),
        settledOutcomeCount: totals.count,
      })
    })
    .sort(
      (left, right) =>
        right.score - left.score || left.actorRef.localeCompare(right.actorRef),
    )

  return new TraceRankProjection({
    acceptedEdges,
    authorityRefs: [
      'authority.tracerank.read_only_verified_outcome_projection',
      'issue.openagents.6425',
    ],
    caveatRefs: Arr.dedupe([
      'caveat.tracerank.accepts_only_verified_final_settlements',
      'caveat.tracerank.excludes_self_payment_edges',
      'caveat.tracerank.not_erc8004_claim',
    ]),
    damping,
    iterationCount,
    rejectedEdges,
    scores,
  })
}
