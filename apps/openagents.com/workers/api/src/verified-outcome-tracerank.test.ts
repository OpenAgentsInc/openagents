import { describe, expect, test } from 'vitest'

import {
  VerifiedOutcomeTraceRankEdge,
  buildVerifiedOutcomeTraceRankProjection,
} from './verified-outcome-tracerank'

const edge = (
  input: ConstructorParameters<typeof VerifiedOutcomeTraceRankEdge>[0],
): VerifiedOutcomeTraceRankEdge => new VerifiedOutcomeTraceRankEdge(input)

describe('verified outcome TraceRank', () => {
  test('ranks actors from replay-verified final settlement edges', () => {
    const projection = buildVerifiedOutcomeTraceRankProjection(
      [
        edge({
          amountMsats: 90_000,
          outcomeRef: 'outcome.public.accepted.1',
          settlementState: 'settled',
          sourceActorRef: 'actor.public.buyer',
          targetActorRef: 'actor.public.pylon-a',
          traceRef: 'trace.public.verified.1',
          verificationState: 'replay_verified',
        }),
        edge({
          amountMsats: 10_000,
          outcomeRef: 'outcome.public.accepted.2',
          settlementState: 'confirmed',
          sourceActorRef: 'actor.public.buyer',
          targetActorRef: 'actor.public.pylon-b',
          traceRef: 'trace.public.verified.2',
          verificationState: 'accepted_verified',
        }),
      ],
      {
        iterationCount: 24,
        seedActorRefs: ['actor.public.buyer'],
      },
    )

    expect(projection.authorityRefs).toContain('issue.openagents.6425')
    expect(projection.acceptedEdges).toHaveLength(2)
    expect(projection.rejectedEdges).toHaveLength(0)
    const pylonA = projection.scores.find(
      score => score.actorRef === 'actor.public.pylon-a',
    )
    const pylonB = projection.scores.find(
      score => score.actorRef === 'actor.public.pylon-b',
    )
    expect(pylonA?.incomingSettledMsats).toBe(90_000)
    expect(pylonA?.settledOutcomeCount).toBe(1)
    expect(pylonA?.score).toBeGreaterThan(pylonB?.score ?? 0)
  })

  test('rejects self-reported, non-final, non-positive, and self-payment edges', () => {
    const projection = buildVerifiedOutcomeTraceRankProjection(
      [
        edge({
          amountMsats: 100_000,
          outcomeRef: 'outcome.public.real',
          settlementState: 'settled',
          sourceActorRef: 'actor.public.buyer',
          targetActorRef: 'actor.public.worker',
          traceRef: 'trace.public.real',
          verificationState: 'replay_verified',
        }),
        edge({
          amountMsats: 9_000_000,
          outcomeRef: 'outcome.public.fake_feedback',
          settlementState: 'settled',
          sourceActorRef: 'actor.public.sybil-a',
          targetActorRef: 'actor.public.sybil-b',
          traceRef: 'trace.public.fake_feedback',
          verificationState: 'self_reported',
        }),
        edge({
          amountMsats: 9_000_000,
          outcomeRef: 'outcome.public.pending',
          settlementState: 'pending_payout',
          sourceActorRef: 'actor.public.sybil-b',
          targetActorRef: 'actor.public.sybil-a',
          traceRef: 'trace.public.pending',
          verificationState: 'replay_verified',
        }),
        edge({
          amountMsats: 9_000_000,
          outcomeRef: 'outcome.public.self_payment',
          settlementState: 'settled',
          sourceActorRef: 'actor.public.sybil-c',
          targetActorRef: 'actor.public.sybil-c',
          traceRef: 'trace.public.self_payment',
          verificationState: 'replay_verified',
        }),
        edge({
          amountMsats: 0,
          outcomeRef: 'outcome.public.zero',
          settlementState: 'settled',
          sourceActorRef: 'actor.public.sybil-d',
          targetActorRef: 'actor.public.sybil-e',
          traceRef: 'trace.public.zero',
          verificationState: 'replay_verified',
        }),
      ],
      {
        seedActorRefs: ['actor.public.buyer'],
      },
    )

    expect(projection.acceptedEdges.map(edge => edge.outcomeRef)).toEqual([
      'outcome.public.real',
    ])
    expect(projection.rejectedEdges.map(edge => edge.blockerRef).sort()).toEqual([
      'blocker.tracerank.non_positive_amount',
      'blocker.tracerank.outcome_not_verified',
      'blocker.tracerank.self_payment_edge',
      'blocker.tracerank.settlement_not_final',
    ])
    expect(projection.scores.map(score => score.actorRef).sort()).toEqual([
      'actor.public.buyer',
      'actor.public.worker',
    ])
  })

  test('is deterministic and normalizes scores to one', () => {
    const edges = [
      edge({
        amountMsats: 50_000,
        outcomeRef: 'outcome.public.1',
        settlementState: 'reconciled',
        sourceActorRef: 'actor.public.buyer-a',
        targetActorRef: 'actor.public.worker-a',
        traceRef: 'trace.public.1',
        verificationState: 'manual_override_verified',
      }),
      edge({
        amountMsats: 50_000,
        outcomeRef: 'outcome.public.2',
        settlementState: 'settled',
        sourceActorRef: 'actor.public.worker-a',
        targetActorRef: 'actor.public.plugin-a',
        traceRef: 'trace.public.2',
        verificationState: 'replay_verified',
      }),
    ]

    const first = buildVerifiedOutcomeTraceRankProjection(edges, {
      seedActorRefs: ['actor.public.buyer-a'],
    })
    const second = buildVerifiedOutcomeTraceRankProjection(edges, {
      seedActorRefs: ['actor.public.buyer-a'],
    })
    const totalScore = first.scores.reduce((sum, score) => sum + score.score, 0)

    expect(first).toEqual(second)
    expect(totalScore).toBeCloseTo(1, 12)
    expect(
      first.scores.every(score =>
        score.scoreRef.startsWith('reputation.tracerank.actor.'),
      ),
    ).toBe(true)
    expect(first.caveatRefs).toContain(
      'caveat.tracerank.accepts_only_verified_final_settlements',
    )
  })
})
