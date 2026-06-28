import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import { openAgentsOpenApiDocument } from './openagents-openapi'
import {
  VerifiedOutcomeReputationEndpoint,
  VerifiedOutcomeReputationMinimumGreenEdgeCount,
  VerifiedOutcomeReputationProjection,
  buildVerifiedOutcomeReputationScores,
  projectVerifiedOutcomeReputation,
  seedVerifiedOutcomeReputationEdges,
  type VerifiedOutcomeReputationActorInput,
  VerifiedOutcomeReputationEdge,
} from './verified-outcome-reputation'
import { handleVerifiedOutcomeReputationApi } from './verified-outcome-reputation-routes'

type VerifiedOutcomeReputationBody = Readonly<{
  projectionId: string
  scores: ReadonlyArray<unknown>
}>

const verifiedEdge = (
  edgeRef: string,
  fromActorRef: string,
  toActorRef: string,
  settledSats: number,
): VerifiedOutcomeReputationEdge =>
  new VerifiedOutcomeReputationEdge({
    acceptedOutcomeRef: `accepted.${edgeRef}`,
    bitcoinSettled: true,
    edgeRef,
    fromActorRef,
    observedAt: '2026-06-27T12:00:00.000Z',
    outcomeRef: `outcome.${edgeRef}`,
    replayVerified: true,
    settledSats,
    settlementReceiptRefs: [`settlement.${edgeRef}`],
    sourceRefs: [`source.${edgeRef}`],
    toActorRef,
    verificationRefs: [`verification.${edgeRef}`],
  })

describe('Verified-outcome reputation projection', () => {
  test('publishes a schema-valid seed TraceRank projection with copy gates', () => {
    const projection = projectVerifiedOutcomeReputation({
      generatedAt: '2026-06-27T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(VerifiedOutcomeReputationProjection)(projection),
    ).toEqual(projection)
    expect(projection.projectionId).toBe('reputation.verified_outcomes.v1')
    expect(projection.algorithm.family).toBe('tracerank_eigentrust')
    expect(projection.algorithm.trustRoot).toBe(
      'replay_verified_bitcoin_settled_outcomes_only',
    )
    expect(projection.graph.verifiedSettledEdgeCount).toBe(1)
    expect(projection.graph.ignoredEdgeCount).toBe(1)
    expect(projection.ignoredEdgeRefs).toContain(
      'edge.reputation.ignored.self_report_sybil',
    )
    expect(projection.gate.state).toBe('yellow')
    expect(projection.gate.broadRankingClaimAllowed).toBe(false)
    expect(projection.gate.sybilResistanceClaimAllowed).toBe(true)
    expect(projection.unsafeCopy).toContain('self-reported feedback')
    expect(projection.authorityBoundary).toContain('grants no dispatch')
  })

  test('ignores high-weight self-reported Sybil feedback without replay and settlement receipts', () => {
    const scores = buildVerifiedOutcomeReputationScores({
      actors: [
        { actorKind: 'coordinator', actorRef: 'buyer' },
        { actorKind: 'pylon', actorRef: 'honest-worker' },
        { actorKind: 'forum_agent', actorRef: 'sybil-clique' },
      ],
      edges: [
        verifiedEdge('edge.real', 'buyer', 'honest-worker', 5),
        new VerifiedOutcomeReputationEdge({
          acceptedOutcomeRef: 'accepted.fake',
          bitcoinSettled: false,
          edgeRef: 'edge.fake.high_weight',
          fromActorRef: 'sybil-clique',
          observedAt: '2026-06-27T12:00:00.000Z',
          outcomeRef: 'outcome.fake',
          replayVerified: false,
          settledSats: 999999,
          settlementReceiptRefs: [],
          sourceRefs: ['source.fake'],
          toActorRef: 'sybil-clique',
          verificationRefs: [],
        }),
      ],
    })

    expect(scores.verifiedSettledEdges.map(edge => edge.edgeRef)).toEqual([
      'edge.real',
    ])
    expect(scores.ignoredEdges.map(edge => edge.edgeRef)).toEqual([
      'edge.fake.high_weight',
    ])
    const honest = scores.scores.find(
      score => score.actorRef === 'honest-worker',
    )
    const sybil = scores.scores.find(score => score.actorRef === 'sybil-clique')
    expect(honest?.inboundVerifiedSettledSats).toBe(5)
    expect(sybil?.inboundVerifiedSettledSats).toBe(0)
    expect((honest?.traceRank ?? 0) > (sybil?.traceRank ?? 0)).toBe(true)
  })

  test('normalizes outgoing verified-settled weights and ranks larger settled outcomes higher', () => {
    const actors: ReadonlyArray<VerifiedOutcomeReputationActorInput> = [
      { actorKind: 'coordinator', actorRef: 'buyer-a' },
      { actorKind: 'coordinator', actorRef: 'buyer-b' },
      { actorKind: 'pylon', actorRef: 'worker-a' },
      { actorKind: 'pylon', actorRef: 'worker-b' },
    ]
    const result = buildVerifiedOutcomeReputationScores({
      actors,
      edges: [
        verifiedEdge('edge.a.1', 'buyer-a', 'worker-a', 2),
        verifiedEdge('edge.a.2', 'buyer-b', 'worker-a', 2),
        verifiedEdge('edge.b.1', 'buyer-a', 'worker-b', 1),
      ],
      iterations: 24,
    })

    const workerA = result.scores.find(score => score.actorRef === 'worker-a')
    const workerB = result.scores.find(score => score.actorRef === 'worker-b')
    expect(workerA?.inboundVerifiedSettledSats).toBe(4)
    expect(workerB?.inboundVerifiedSettledSats).toBe(1)
    expect((workerA?.traceRank ?? 0) > (workerB?.traceRank ?? 0)).toBe(true)
  })

  test('keeps broad ranking claims yellow until the verified-settled edge threshold is met', () => {
    const below = Array.from(
      { length: VerifiedOutcomeReputationMinimumGreenEdgeCount - 1 },
      (_, index) => verifiedEdge(`edge.${index}`, `buyer-${index}`, 'worker', 1),
    )
    const atThreshold = [
      ...below,
      verifiedEdge(
        `edge.${VerifiedOutcomeReputationMinimumGreenEdgeCount}`,
        'buyer-threshold',
        'worker',
        1,
      ),
    ]

    expect(projectVerifiedOutcomeReputation({ edges: below }).gate.state).toBe(
      'yellow',
    )
    expect(
      projectVerifiedOutcomeReputation({ edges: atThreshold }).gate.state,
    ).toBe('green')
  })

  test('serves the public route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleVerifiedOutcomeReputationApi(
        new Request(
          `https://openagents.com${VerifiedOutcomeReputationEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as VerifiedOutcomeReputationBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.projectionId).toBe('reputation.verified_outcomes.v1')
    expect(body.scores.length).toBeGreaterThan(0)
  })

  test('documents the public reputation endpoint in OpenAPI', async () => {
    const document = await Effect.runPromise(openAgentsOpenApiDocument())

    expect(
      (
        document.paths[VerifiedOutcomeReputationEndpoint] as
          | { get?: unknown }
          | undefined
      )?.get,
    ).toEqual(
      expect.objectContaining({
        operationId: 'getVerifiedOutcomeReputation',
      }),
    )
    expect(
      (document.components as { schemas: Record<string, unknown> }).schemas,
    ).toHaveProperty('VerifiedOutcomeReputationProjection')
  })

  test('seed fixture has one real edge and one ignored counterexample edge', () => {
    const edges = seedVerifiedOutcomeReputationEdges()

    expect(edges).toHaveLength(2)
    expect(edges.filter(edge => edge.replayVerified)).toHaveLength(1)
    expect(edges.filter(edge => edge.bitcoinSettled)).toHaveLength(1)
  })
})
