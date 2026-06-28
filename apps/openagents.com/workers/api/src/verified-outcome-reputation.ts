import { Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const VerifiedOutcomeReputationEndpoint =
  '/api/public/reputation/verified-outcomes'
export const VerifiedOutcomeReputationSchemaVersion =
  'openagents.reputation.verified_outcomes.v1'
export const VerifiedOutcomeReputationMinimumGreenEdgeCount = 8
export const VerifiedOutcomeReputationDefaultDamping = 0.85
export const VerifiedOutcomeReputationDefaultIterations = 32
export const VerifiedOutcomeReputationStaleness = liveAtReadStaleness([
  'accepted_outcome_receipt_published',
  'trace_replay_verdict_published',
  'labor_escrow_release_receipt_published',
  'pylon_settlement_receipt_published',
  'product_promise_registry_updated',
])

export const VerifiedOutcomeReputationActorKind = S.Literals([
  'pylon',
  'coordinator',
  'plugin',
  'forum_agent',
])
export type VerifiedOutcomeReputationActorKind = S.Schema.Type<
  typeof VerifiedOutcomeReputationActorKind
>

export class VerifiedOutcomeReputationEdge extends S.Class<VerifiedOutcomeReputationEdge>(
  'VerifiedOutcomeReputationEdge',
)({
  edgeRef: S.String,
  fromActorRef: S.String,
  toActorRef: S.String,
  outcomeRef: S.String,
  acceptedOutcomeRef: S.String,
  verificationRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
  replayVerified: S.Boolean,
  bitcoinSettled: S.Boolean,
  settledSats: S.Int,
  observedAt: S.String,
  sourceRefs: S.Array(S.String),
}) {}

export class VerifiedOutcomeReputationActorScore extends S.Class<VerifiedOutcomeReputationActorScore>(
  'VerifiedOutcomeReputationActorScore',
)({
  actorRef: S.String,
  actorKind: VerifiedOutcomeReputationActorKind,
  traceRank: S.Number,
  score: S.Number,
  inboundVerifiedSettledSats: S.Int,
  inboundVerifiedSettledEdgeCount: S.Int,
  acceptedOutcomeRefs: S.Array(S.String),
  verificationRefs: S.Array(S.String),
  settlementReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
}) {}

export class VerifiedOutcomeReputationGate extends S.Class<VerifiedOutcomeReputationGate>(
  'VerifiedOutcomeReputationGate',
)({
  state: S.Literals(['yellow', 'green']),
  currentVerifiedSettledEdgeCount: S.Int,
  requiredVerifiedSettledEdgeCount: S.Int,
  broadRankingClaimAllowed: S.Boolean,
  sybilResistanceClaimAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
}) {}

export class VerifiedOutcomeReputationProjection extends S.Class<VerifiedOutcomeReputationProjection>(
  'VerifiedOutcomeReputationProjection',
)({
  schemaVersion: S.String,
  generatedAt: S.String,
  projectionId: S.Literal('reputation.verified_outcomes.v1'),
  definitionRef: S.String,
  promiseRef: S.String,
  staleness: PublicProjectionStalenessContract,
  status: S.Literal('instrumented_seed'),
  statusLabel: S.String,
  algorithm: S.Struct({
    family: S.Literal('tracerank_eigentrust'),
    damping: S.Number,
    iterations: S.Int,
    trustRoot: S.Literal('replay_verified_bitcoin_settled_outcomes_only'),
    edgeWeight: S.Literal('settled_sats'),
    normalization: S.Literal('outgoing_verified_settled_weight'),
    ignoredEdgeRule: S.Literal(
      'ignore_edges_without_replay_verdict_and_bitcoin_settlement_receipt',
    ),
  }),
  graph: S.Struct({
    inputEdgeCount: S.Int,
    verifiedSettledEdgeCount: S.Int,
    ignoredEdgeCount: S.Int,
    actorCount: S.Int,
    totalVerifiedSettledSats: S.Int,
  }),
  gate: VerifiedOutcomeReputationGate,
  scores: S.Array(VerifiedOutcomeReputationActorScore),
  ignoredEdgeRefs: S.Array(S.String),
  authorityBoundary: S.String,
  unsafeCopy: S.String,
  sourceRefs: S.Array(S.String),
}) {}

export type VerifiedOutcomeReputationActorInput = Readonly<{
  actorRef: string
  actorKind: VerifiedOutcomeReputationActorKind
}>

export type ProjectVerifiedOutcomeReputationInput = Readonly<{
  generatedAt?: string | undefined
  actors?: ReadonlyArray<VerifiedOutcomeReputationActorInput> | undefined
  edges?: ReadonlyArray<VerifiedOutcomeReputationEdge> | undefined
  damping?: number | undefined
  iterations?: number | undefined
}>

const round = (value: number, places = 6): number => {
  const factor = 10 ** places
  return Math.round(value * factor) / factor
}

const isVerifiedSettledEdge = (
  edge: VerifiedOutcomeReputationEdge,
): boolean =>
  edge.replayVerified &&
  edge.bitcoinSettled &&
  edge.settledSats > 0 &&
  edge.verificationRefs.length > 0 &&
  edge.settlementReceiptRefs.length > 0

const normalizePositiveInteger = (
  value: number | undefined,
  fallback: number,
): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return fallback
  }
  return Math.max(1, Math.floor(value))
}

const normalizeDamping = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return VerifiedOutcomeReputationDefaultDamping
  }
  return Math.min(0.99, Math.max(0.01, value))
}

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)]

export const buildVerifiedOutcomeReputationScores = (
  input: ProjectVerifiedOutcomeReputationInput = {},
): Readonly<{
  scores: ReadonlyArray<VerifiedOutcomeReputationActorScore>
  verifiedSettledEdges: ReadonlyArray<VerifiedOutcomeReputationEdge>
  ignoredEdges: ReadonlyArray<VerifiedOutcomeReputationEdge>
}> => {
  const edges = input.edges ?? seedVerifiedOutcomeReputationEdges()
  const verifiedSettledEdges = edges.filter(isVerifiedSettledEdge)
  const ignoredEdges = edges.filter(edge => !isVerifiedSettledEdge(edge))
  const actorKinds = new Map<string, VerifiedOutcomeReputationActorKind>()
  for (const actor of input.actors ?? seedVerifiedOutcomeReputationActors()) {
    actorKinds.set(actor.actorRef, actor.actorKind)
  }
  for (const edge of edges) {
    if (!actorKinds.has(edge.fromActorRef)) {
      actorKinds.set(edge.fromActorRef, 'coordinator')
    }
    if (!actorKinds.has(edge.toActorRef)) {
      actorKinds.set(edge.toActorRef, 'pylon')
    }
  }

  const actorRefs = [...actorKinds.keys()].sort()
  if (actorRefs.length === 0) {
    return { ignoredEdges, scores: [], verifiedSettledEdges }
  }

  const indexByActor = new Map(actorRefs.map((actorRef, index) => [actorRef, index]))
  const size = actorRefs.length
  const damping = normalizeDamping(input.damping)
  const iterations = normalizePositiveInteger(
    input.iterations,
    VerifiedOutcomeReputationDefaultIterations,
  )
  const preTrust = Array.from({ length: size }, () => 1 / size)
  let rank = [...preTrust]
  const outgoingWeight = new Map<string, number>()
  for (const edge of verifiedSettledEdges) {
    outgoingWeight.set(
      edge.fromActorRef,
      (outgoingWeight.get(edge.fromActorRef) ?? 0) + edge.settledSats,
    )
  }

  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const next = preTrust.map(value => (1 - damping) * value)
    for (const edge of verifiedSettledEdges) {
      const fromIndex = indexByActor.get(edge.fromActorRef)
      const toIndex = indexByActor.get(edge.toActorRef)
      const fromWeight = outgoingWeight.get(edge.fromActorRef) ?? 0
      if (
        fromIndex === undefined ||
        toIndex === undefined ||
        fromWeight <= 0
      ) {
        continue
      }
      next[toIndex] =
        (next[toIndex] ?? 0) +
        damping * (rank[fromIndex] ?? 0) * (edge.settledSats / fromWeight)
    }
    const total = next.reduce((sum, value) => sum + value, 0)
    rank = total > 0 ? next.map(value => value / total) : [...preTrust]
  }

  const inboundByActor = new Map<
    string,
    {
      edgeCount: number
      sats: number
      acceptedOutcomeRefs: Array<string>
      verificationRefs: Array<string>
      settlementReceiptRefs: Array<string>
    }
  >()
  for (const edge of verifiedSettledEdges) {
    const current = inboundByActor.get(edge.toActorRef) ?? {
      acceptedOutcomeRefs: [],
      edgeCount: 0,
      sats: 0,
      settlementReceiptRefs: [],
      verificationRefs: [],
    }
    current.edgeCount += 1
    current.sats += edge.settledSats
    current.acceptedOutcomeRefs.push(edge.acceptedOutcomeRef)
    current.verificationRefs.push(...edge.verificationRefs)
    current.settlementReceiptRefs.push(...edge.settlementReceiptRefs)
    inboundByActor.set(edge.toActorRef, current)
  }

  const scores = actorRefs
    .map((actorRef, index) => {
      const inbound = inboundByActor.get(actorRef) ?? {
        acceptedOutcomeRefs: [],
        edgeCount: 0,
        sats: 0,
        settlementReceiptRefs: [],
        verificationRefs: [],
      }
      const traceRank = rank[index] ?? 0
      return new VerifiedOutcomeReputationActorScore({
        acceptedOutcomeRefs: unique(inbound.acceptedOutcomeRefs),
        actorKind: actorKinds.get(actorRef) ?? 'pylon',
        actorRef,
        caveatRefs:
          inbound.edgeCount === 0
            ? ['caveat.reputation.no_verified_settled_inbound_edges']
            : ['caveat.reputation.seed_graph_not_broad_ranking'],
        inboundVerifiedSettledEdgeCount: inbound.edgeCount,
        inboundVerifiedSettledSats: inbound.sats,
        score: round(traceRank * 1000, 3),
        settlementReceiptRefs: unique(inbound.settlementReceiptRefs),
        traceRank: round(traceRank),
        verificationRefs: unique(inbound.verificationRefs),
      })
    })
    .sort((left, right) =>
      right.traceRank === left.traceRank
        ? left.actorRef.localeCompare(right.actorRef)
        : right.traceRank - left.traceRank,
    )

  return { ignoredEdges, scores, verifiedSettledEdges }
}

export const seedVerifiedOutcomeReputationActors =
  (): ReadonlyArray<VerifiedOutcomeReputationActorInput> => [
    { actorKind: 'coordinator', actorRef: 'agent:labor-buyer' },
    { actorKind: 'pylon', actorRef: 'pylon:labor-market-raynor' },
    { actorKind: 'forum_agent', actorRef: 'agent:forum-adjacent-sybil' },
  ]

export const seedVerifiedOutcomeReputationEdges =
  (): ReadonlyArray<VerifiedOutcomeReputationEdge> => [
    new VerifiedOutcomeReputationEdge({
      acceptedOutcomeRef:
        'closeout.public.pylon.labor_market.fe1ee748e332a9b9ff7f1e0b',
      bitcoinSettled: true,
      edgeRef: 'edge.reputation.seed.labor_4777',
      fromActorRef: 'agent:labor-buyer',
      observedAt: '2026-06-14T03:06:15.399Z',
      outcomeRef:
        'work_result.public.788b59de-8ee9-4029-9f5b-c6cf23dc668d',
      replayVerified: true,
      settledSats: 1,
      settlementReceiptRefs: [
        'receipt.labor_escrow.release.b74bb55c-849c-43a3-b8d9-9a741316b528.quote.public.pylon.labor_market.b97f312478504e9df212e333',
      ],
      sourceRefs: [
        'docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md',
        'work_request:b74bb55c-849c-43a3-b8d9-9a741316b528',
      ],
      toActorRef: 'pylon:labor-market-raynor',
      verificationRefs: ['verdict.public.pylon.labor_market.b74bb55c.bun_test.pass'],
    }),
    new VerifiedOutcomeReputationEdge({
      acceptedOutcomeRef: 'review.self_report.sybil.synthetic',
      bitcoinSettled: false,
      edgeRef: 'edge.reputation.ignored.self_report_sybil',
      fromActorRef: 'agent:forum-adjacent-sybil',
      observedAt: '2026-06-14T03:10:00.000Z',
      outcomeRef: 'review.self_report.sybil.synthetic',
      replayVerified: false,
      settledSats: 1000000,
      settlementReceiptRefs: [],
      sourceRefs: ['issue:6425.self_reported_feedback_counterexample'],
      toActorRef: 'agent:forum-adjacent-sybil',
      verificationRefs: [],
    }),
  ]

export const projectVerifiedOutcomeReputation = (
  input: ProjectVerifiedOutcomeReputationInput = {},
): VerifiedOutcomeReputationProjection => {
  const edges = input.edges ?? seedVerifiedOutcomeReputationEdges()
  const { ignoredEdges, scores, verifiedSettledEdges } =
    buildVerifiedOutcomeReputationScores(input)
  const verifiedSettledEdgeCount = verifiedSettledEdges.length
  const gateState =
    verifiedSettledEdgeCount >= VerifiedOutcomeReputationMinimumGreenEdgeCount
      ? 'green'
      : 'yellow'

  return new VerifiedOutcomeReputationProjection({
    algorithm: {
      damping: normalizeDamping(input.damping),
      edgeWeight: 'settled_sats',
      family: 'tracerank_eigentrust',
      ignoredEdgeRule:
        'ignore_edges_without_replay_verdict_and_bitcoin_settlement_receipt',
      iterations: normalizePositiveInteger(
        input.iterations,
        VerifiedOutcomeReputationDefaultIterations,
      ),
      normalization: 'outgoing_verified_settled_weight',
      trustRoot: 'replay_verified_bitcoin_settled_outcomes_only',
    },
    authorityBoundary:
      'Verified-outcome reputation is a public read-only evidence projection. It grants no dispatch, marketplace ranking, assignment, payout, settlement, moderation, identity, ERC-8004 publication, or spend authority.',
    definitionRef: 'docs/research/idai/roadmap-alignment.md#1-verified-trace-reputation--sybil-proof-accounting',
    gate: new VerifiedOutcomeReputationGate({
      blockerRefs:
        gateState === 'green'
          ? []
          : [
              'blocker.reputation.seed_graph_too_small_for_broad_ranking',
              'blocker.reputation.needs_more_replay_verified_bitcoin_settled_edges',
            ],
      broadRankingClaimAllowed: gateState === 'green',
      caveatRefs: [
        'caveat.reputation.seed_projection_not_marketplace_ranking',
        'caveat.reputation.self_reported_feedback_edges_ignored',
      ],
      currentVerifiedSettledEdgeCount: verifiedSettledEdgeCount,
      requiredVerifiedSettledEdgeCount:
        VerifiedOutcomeReputationMinimumGreenEdgeCount,
      state: gateState,
      sybilResistanceClaimAllowed: verifiedSettledEdgeCount > 0,
    }),
    generatedAt: input.generatedAt ?? currentIsoTimestamp(),
    graph: {
      actorCount: scores.length,
      ignoredEdgeCount: ignoredEdges.length,
      inputEdgeCount: edges.length,
      totalVerifiedSettledSats: verifiedSettledEdges.reduce(
        (sum, edge) => sum + edge.settledSats,
        0,
      ),
      verifiedSettledEdgeCount,
    },
    ignoredEdgeRefs: ignoredEdges.map(edge => edge.edgeRef),
    projectionId: 'reputation.verified_outcomes.v1',
    promiseRef: 'promise:reputation.verified_outcomes.v1',
    schemaVersion: VerifiedOutcomeReputationSchemaVersion,
    scores,
    sourceRefs: [
      'docs/research/idai/roadmap-alignment.md',
      'docs/research/idai/sybil-resistance-for-agents.md',
      'docs/research/idai/trust-reputation-in-agentic-ai.md',
      'apps/openagents.com/workers/api/src/verified-outcome-reputation.ts',
    ],
    staleness: VerifiedOutcomeReputationStaleness,
    status: 'instrumented_seed',
    statusLabel:
      gateState === 'green'
        ? 'Verified-outcome reputation has enough replay-verified Bitcoin-settled edges for broad ranking claims.'
        : `${verifiedSettledEdgeCount} of ${VerifiedOutcomeReputationMinimumGreenEdgeCount} replay-verified Bitcoin-settled edges are present; this is a seed projection, not a broad ranking.`,
    unsafeCopy:
      'Do not describe this seed projection as a broad marketplace ranking, identity proof, moderation score, payout entitlement, ERC-8004 publication, or live dispatch signal. Only replay-verified outcomes with public-safe Bitcoin settlement receipts affect TraceRank; self-reported feedback, unverified reviews, unpaid no-spend work, and missing-receipt edges are ignored.',
  })
}
