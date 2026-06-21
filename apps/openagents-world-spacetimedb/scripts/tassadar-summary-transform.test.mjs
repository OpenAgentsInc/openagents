import { describe, expect, it } from 'bun:test'

import {
  assertNoDuplicateWorldEvents,
  assertWorldEventsAreSourced,
  buildTassadarProjectionPlan,
  reducerCounts,
} from './tassadar-summary-transform.mjs'

const summary = {
  corpus: {
    traceRefs: ['trace.accepted.1'],
    verdictRefs: ['verdict.accepted.1'],
  },
  generatedAt: '2026-06-17T20:37:52.303Z',
  metrics: {
    receiptRefCount: { value: 1 },
  },
  realGradient: {
    leaderboardRows: [
      {
        pylonRef: 'pylon.public.worker1',
        rank: 1,
        sourceRefs: ['pylon.public.worker1', 'training.lease.public.1'],
        verifiedWindowCount: 1,
      },
    ],
    rejectedReplayPairs: [
      {
        challengeRef: 'challenge.rejected.1',
        sourceRefs: ['challenge.rejected.1'],
        validatorRef: 'pylon.public.validator2',
        verdictRefs: ['verdict.rejected.1'],
        workerRef: 'pylon.public.worker2',
      },
    ],
    verifiedReplayPairs: [
      {
        challengeRef: 'challenge.verified.1',
        sourceRefs: ['challenge.verified.1'],
        validatorRef: 'pylon.public.validator1',
        verdictRefs: ['verdict.verified.1'],
        workerRef: 'pylon.public.worker1',
      },
    ],
  },
  runRef: 'run.tassadar.executor.20260615',
  runState: 'active',
  settlementRows: [
    {
      amountSats: 5,
      apiUrl:
        'https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.test',
      contributorRef: 'pylon.public.worker1',
      movementMode: 'simulation',
      realBitcoinMoved: false,
      receiptRef: 'receipt.nexus.test',
      sourceRefs: ['receipt.nexus.test', 'pylon.public.worker1'],
      state: 'settled',
      verificationChallengeRef: 'challenge.verified.1',
    },
  ],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
  },
}

describe('Tassadar SpacetimeDB projection transform', () => {
  it('maps the public summary into required reducer calls', () => {
    const plan = buildTassadarProjectionPlan(summary)
    const counts = reducerCounts(plan)

    expect(plan.runRef).toBe('run.tassadar.executor.20260615')
    expect(counts.upsert_training_run).toBe(1)
    expect(counts.upsert_world_region).toBe(1)
    expect(counts.upsert_run_entity).toBeGreaterThanOrEqual(7)
    expect(counts.upsert_settlement_ref).toBe(1)
    expect(counts.upsert_pylon_station_from_projection).toBe(1)
    expect(counts.ensure_pylon_agent_avatar).toBe(1)
    expect(counts.record_projection_cursor).toBe(1)
    expect(counts.append_world_event).toBeGreaterThan(0)

    expect(
      plan.calls.find(call => call.reducer === 'upsert_world_region')?.args,
    ).toEqual([
      'region.run.tassadar.executor.20260615.main',
      'run.tassadar.executor.20260615',
      'Tassadar main run space',
      -160,
      0,
      -160,
      160,
      40,
      160,
      0,
      0,
      1,
      0,
      0,
      0,
      24,
      0,
      0,
      'region.run.tassadar.executor.20260615.street.prev',
      'region.run.tassadar.executor.20260615.street.next',
      12,
      100,
      20_000,
    ])
  })

  it('projects deterministic pylon stations and pylon-agent avatars', () => {
    const plan = buildTassadarProjectionPlan(summary)
    const stationCalls = plan.calls.filter(
      call => call.reducer === 'upsert_pylon_station_from_projection',
    )
    const avatarCalls = plan.calls.filter(
      call => call.reducer === 'ensure_pylon_agent_avatar',
    )

    expect(stationCalls).toHaveLength(summary.realGradient.leaderboardRows.length)
    expect(avatarCalls).toHaveLength(summary.realGradient.leaderboardRows.length)
    expect(stationCalls[0]?.args).toEqual([
      'pylon.public.worker1',
      'run.tassadar.executor.20260615',
      'region.run.tassadar.executor.20260615.main',
      'P1',
      'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615?focusRef=pylon.public.worker1',
      -2.35,
      0,
      0,
      0,
      2.4,
    ])
    expect(avatarCalls[0]?.args).toEqual([
      'avatar.pylon_agent.pylon.public.worker1',
      'pylon.public.worker1',
      'P1 agent',
      'region.run.tassadar.executor.20260615.main',
      -1.9,
      0,
      0,
      0,
    ])
  })

  it('is deterministic for replaying the same summary', () => {
    const first = buildTassadarProjectionPlan(summary)
    const second = buildTassadarProjectionPlan(summary)

    expect(second).toEqual(first)
    assertNoDuplicateWorldEvents(first)
  })

  it('rejects unsourced world events before apply', () => {
    const plan = buildTassadarProjectionPlan(summary)
    assertWorldEventsAreSourced(plan)

    const badPlan = {
      ...plan,
      calls: [
        ...plan.calls,
        {
          reducer: 'append_world_event',
          args: [
            'world_event.bad',
            plan.runRef,
            'bad',
            'entity.bad',
            '',
            '',
            'bad event',
          ],
        },
      ],
    }

    expect(() => assertWorldEventsAreSourced(badPlan)).toThrow(
      /unsourced world_event/,
    )
  })
})
