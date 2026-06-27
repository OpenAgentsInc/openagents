import { describe, expect, test } from 'vitest'

import {
  ARTANIS_DEFAULT_GOALS,
  type ArtanisAwarenessReaders,
  buildArtanisSituationalAwareness,
} from './artanis-situational-awareness'

const fixedNow = () => '2026-06-26T12:00:00.000Z'

describe('buildArtanisSituationalAwareness', () => {
  test('returns the three buckets assembled from seeded readers', async () => {
    const readers: ArtanisAwarenessReaders = {
      readActiveAssignments: async (ownerId, limit) => {
        expect(ownerId).toBe('owner-1')
        expect(limit).toBeGreaterThan(0)
        return [
          {
            assignmentRef: 'a-live',
            phase: 'proof-ready',
            startedAt: '2026-06-26T11:55:00.000Z',
            state: 'accepted',
          },
        ]
      },
      readFleetReadiness: async () => ({
        readyReplicas: 2,
        status: 'ready',
        totalReplicas: 3,
      }),
      readPublicCounter: async () => ({
        asOf: '2026-06-26T11:59:00.000Z',
        tokensServed: 123_456,
      }),
      readTokenPace: async () => ({
        behindPace: true,
        day: '2026-06-26',
        fractionOfCentralDayElapsed: 0.5,
        gapToTarget4x: 900_000,
        paceProjection: 100_000,
        target10x: 2_500_000,
        target4x: 1_000_000,
        todayTokens: 50_000,
        yesterdayTokens: 250_000,
      }),
      readRecentAssignments: async (ownerId, _limit) => {
        expect(ownerId).toBe('owner-1')
        return [
          {
            assignmentRef: 'a1',
            objective: 'Implement public issue #6363',
            state: 'closeout_submitted',
            updatedAt: '2026-06-26T11:50:00.000Z',
          },
        ]
      },
      readRecentCommits: async _limit => [
        {
          committedAt: '2026-06-26T11:40:00.000Z',
          sha: 'd092c6be4c',
          summary: 'docs(khala): operator console',
        },
      ],
      readRecentDeploys: async _limit => [
        { deployedAt: '2026-06-26T11:30:00.000Z', workerVersion: 'v123' },
      ],
      readRecentIssueChanges: async _limit => [
        {
          at: '2026-06-26T11:20:00.000Z',
          change: 'opened',
          number: 6363,
          title: 'Talk to Artanis',
        },
      ],
      readRecentTicks: async _limit => [
        {
          assignmentRef: 'a1',
          at: '2026-06-26T11:10:00.000Z',
          decisionRef: 'tick-1',
          state: 'dispatched',
        },
      ],
    }

    const awareness = await buildArtanisSituationalAwareness('owner-1', readers, {
      nowIso: fixedNow,
    })

    expect(awareness.kind).toBe('artanis_situational_awareness')
    expect(awareness.ownerOnly).toBe(true)
    expect(awareness.ownerId).toBe('owner-1')
    expect(awareness.generatedAt).toBe('2026-06-26T12:00:00.000Z')

    // recentActions bucket
    expect(awareness.recentActions.commits).toHaveLength(1)
    expect(awareness.recentActions.commits[0]!.sha).toBe('d092c6be4c')
    expect(awareness.recentActions.assignments[0]!.assignmentRef).toBe('a1')
    expect(awareness.recentActions.issueChanges[0]!.number).toBe(6363)
    expect(awareness.recentActions.ticks[0]!.state).toBe('dispatched')

    // goals bucket
    expect(awareness.goals.epics.map(e => e.number)).toContain(6359)
    expect(awareness.goals.epics.map(e => e.number)).toContain(6316)
    expect(awareness.goals.epics.map(e => e.number)).toContain(6303)

    // ongoingOps bucket
    expect(awareness.ongoingOps.activeAssignments[0]!.phase).toBe('proof-ready')
    expect(awareness.ongoingOps.recentDeploys[0]!.workerVersion).toBe('v123')
    expect(awareness.ongoingOps.fleetReadiness?.status).toBe('ready')
    expect(awareness.ongoingOps.publicCounter?.tokensServed).toBe(123_456)
    expect(awareness.ongoingOps.tokenPace?.behindPace).toBe(true)
    expect(awareness.ongoingOps.tokenPace?.paceProjection).toBe(100_000)
  })

  test('degrades to empty buckets and default goals when no readers wired', async () => {
    const awareness = await buildArtanisSituationalAwareness('owner-1', {}, {
      nowIso: fixedNow,
    })

    expect(awareness.recentActions.commits).toEqual([])
    expect(awareness.recentActions.assignments).toEqual([])
    expect(awareness.recentActions.issueChanges).toEqual([])
    expect(awareness.recentActions.ticks).toEqual([])
    expect(awareness.ongoingOps.activeAssignments).toEqual([])
    expect(awareness.ongoingOps.recentDeploys).toEqual([])
    expect(awareness.ongoingOps.fleetReadiness).toBeNull()
    expect(awareness.ongoingOps.publicCounter).toBeNull()
    expect(awareness.ongoingOps.tokenPace).toBeNull()
    // Goals fall back to the code-anchored defaults rather than going empty.
    expect(awareness.goals).toEqual(ARTANIS_DEFAULT_GOALS)
  })

  test('a failing reader degrades only its own bucket, not the whole build', async () => {
    const readers: ArtanisAwarenessReaders = {
      readFleetReadiness: async () => {
        throw new Error('GLM readiness unreachable')
      },
      readRecentCommits: async _limit => [
        {
          committedAt: '2026-06-26T11:40:00.000Z',
          sha: 'abc',
          summary: 'a commit',
        },
      ],
    }

    const awareness = await buildArtanisSituationalAwareness('owner-1', readers, {
      nowIso: fixedNow,
    })

    expect(awareness.recentActions.commits).toHaveLength(1)
    expect(awareness.ongoingOps.fleetReadiness).toBeNull()
  })

  test('bounds list sizes even when a reader over-returns', async () => {
    const big = Array.from({ length: 50 }, (_unused, i) => ({
      committedAt: `2026-06-26T10:${String(i).padStart(2, '0')}:00.000Z`,
      sha: `sha-${i}`,
      summary: `commit ${i}`,
    }))
    const readers: ArtanisAwarenessReaders = {
      readRecentCommits: async _limit => big,
    }

    const awareness = await buildArtanisSituationalAwareness(
      'owner-1',
      readers,
      { bounds: { commits: 3 }, nowIso: fixedNow },
    )
    expect(awareness.recentActions.commits).toHaveLength(3)
  })

  test('rejects an empty ownerId', async () => {
    await expect(
      buildArtanisSituationalAwareness('   '),
    ).rejects.toThrow(/ownerId must be non-empty/)
  })

  test('passes the owner scope through to owner-scoped readers', async () => {
    let seenOwner: string | null = null
    const readers: ArtanisAwarenessReaders = {
      readRecentAssignments: async (ownerId, _limit) => {
        seenOwner = ownerId
        return []
      },
    }
    await buildArtanisSituationalAwareness('owner-XYZ', readers, {
      nowIso: fixedNow,
    })
    expect(seenOwner).toBe('owner-XYZ')
  })
})
