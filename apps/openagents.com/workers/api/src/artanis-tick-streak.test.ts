import { describe, expect, test } from 'vitest'

import {
  ARTANIS_UNATTENDED_TICK_STREAK_TARGET,
  artanisTickStreakMeetsGate,
  boundedTickStreakLimit,
  projectArtanisTickStreak,
  readArtanisTickStreak,
  type ArtanisTickStreakDecisionRow,
} from './artanis-tick-streak'

const nowIso = '2026-06-20T00:00:00.000Z'

const verifiedDispatch = (
  n: number,
  createdAt: string,
): ArtanisTickStreakDecisionRow => ({
  assignment_ref: `assignment.artanis_admin.2026062000000${n}.w0`,
  created_at: createdAt,
  id: `decision-${n}`,
  state: 'dispatched',
  verdict_accept_state: 'accepted',
  verdict_created_at: createdAt,
  verdict_outcome: 'verified',
  verdict_trace_digest_prefix: 'abc123def4567890',
})

describe('artanis unattended tick-streak projection', () => {
  test('a run of verified dispatches yields a current and longest streak', () => {
    const rows = [
      verifiedDispatch(3, '2026-06-20T00:03:00.000Z'),
      verifiedDispatch(2, '2026-06-20T00:02:00.000Z'),
      verifiedDispatch(1, '2026-06-20T00:01:00.000Z'),
    ]
    const streak = projectArtanisTickStreak(rows, nowIso)
    expect(streak.kind).toBe('artanis_unattended_tick_streak')
    expect(streak.publicSafe).toBe(true)
    expect(streak.staleness.contractVersion).toBe('projection_staleness.v1')
    expect(streak.staleness.composition).toBe('live_at_read')
    expect(streak.staleness.maxStalenessSeconds).toBe(0)
    expect(streak.currentStreak).toBe(3)
    expect(streak.longestStreak).toBe(3)
    expect(streak.verifiedTickCount).toBe(3)
    expect(streak.streakTarget).toBe(ARTANIS_UNATTENDED_TICK_STREAK_TARGET)
    expect(streak.targetReached).toBe(false)
    expect(streak.currentStreakAssignmentRefs).toHaveLength(3)
    // Each streak assignment is dereferenceable as a closeout receipt.
    expect(streak.ticks[0]?.closeoutReceiptRef).toContain(
      'receipt.nexus_pylon.artanis_admin_closeout.',
    )
  })

  test('a non-verified tick at the head breaks the current streak only', () => {
    const rows = [
      // newest: a no_action breaks the current streak
      {
        assignment_ref: null,
        created_at: '2026-06-20T00:04:00.000Z',
        id: 'decision-noop',
        state: 'no_action',
        verdict_accept_state: null,
        verdict_created_at: null,
        verdict_outcome: null,
        verdict_trace_digest_prefix: null,
      } satisfies ArtanisTickStreakDecisionRow,
      verifiedDispatch(3, '2026-06-20T00:03:00.000Z'),
      verifiedDispatch(2, '2026-06-20T00:02:00.000Z'),
      verifiedDispatch(1, '2026-06-20T00:01:00.000Z'),
    ]
    const streak = projectArtanisTickStreak(rows, nowIso)
    expect(streak.currentStreak).toBe(0)
    expect(streak.longestStreak).toBe(3)
    expect(streak.verifiedTickCount).toBe(3)
    expect(streak.currentStreakAssignmentRefs).toHaveLength(0)
  })

  test('a dispatched tick without an accepted verified verdict does not qualify', () => {
    const pendingDispatch: ArtanisTickStreakDecisionRow = {
      assignment_ref: 'assignment.artanis_admin.20260620000099.w0',
      created_at: '2026-06-20T00:05:00.000Z',
      id: 'decision-pending',
      state: 'dispatched',
      verdict_accept_state: null,
      verdict_created_at: null,
      verdict_outcome: null,
      verdict_trace_digest_prefix: null,
    }
    const rejectedDispatch: ArtanisTickStreakDecisionRow = {
      assignment_ref: 'assignment.artanis_admin.20260620000098.w0',
      created_at: '2026-06-20T00:00:30.000Z',
      id: 'decision-rejected',
      state: 'dispatched',
      verdict_accept_state: 'rejected',
      verdict_created_at: '2026-06-20T00:00:40.000Z',
      verdict_outcome: 'rejected',
      verdict_trace_digest_prefix: 'deadbeefdeadbeef',
    }
    const streak = projectArtanisTickStreak(
      [pendingDispatch, rejectedDispatch],
      nowIso,
    )
    expect(streak.currentStreak).toBe(0)
    expect(streak.longestStreak).toBe(0)
    expect(streak.verifiedTickCount).toBe(0)
    expect(streak.ticks[0]?.qualifies).toBe(false)
    expect(streak.ticks[1]?.qualifies).toBe(false)
  })

  test('targetReached flips once ten consecutive verified ticks exist', () => {
    const rows = Array.from({ length: 10 }, (_, i) =>
      verifiedDispatch(
        i,
        `2026-06-20T00:${String(10 + i).padStart(2, '0')}:00.000Z`,
      ),
    )
    const streak = projectArtanisTickStreak(rows, nowIso)
    expect(streak.currentStreak).toBe(10)
    expect(streak.longestStreak).toBe(10)
    expect(streak.targetReached).toBe(true)
    expect(artanisTickStreakMeetsGate(streak)).toBe(true)
  })

  test('longest streak is the max run even when the current streak is shorter', () => {
    const rows = [
      verifiedDispatch(9, '2026-06-20T00:09:00.000Z'),
      // break
      {
        assignment_ref: 'assignment.artanis_admin.20260620000077.w0',
        created_at: '2026-06-20T00:08:00.000Z',
        id: 'decision-fail',
        state: 'dispatch_failed',
        verdict_accept_state: null,
        verdict_created_at: null,
        verdict_outcome: null,
        verdict_trace_digest_prefix: null,
      } satisfies ArtanisTickStreakDecisionRow,
      verifiedDispatch(3, '2026-06-20T00:03:00.000Z'),
      verifiedDispatch(2, '2026-06-20T00:02:00.000Z'),
      verifiedDispatch(1, '2026-06-20T00:01:00.000Z'),
    ]
    const streak = projectArtanisTickStreak(rows, nowIso)
    expect(streak.currentStreak).toBe(1)
    expect(streak.longestStreak).toBe(3)
    expect(streak.verifiedTickCount).toBe(4)
  })

  test('unknown states and smuggled digest values never project', () => {
    const rows = [
      {
        assignment_ref: 'assignment.artanis_admin.20260620000055.w0',
        created_at: '2026-06-20T00:06:00.000Z',
        id: 'decision-haunted',
        state: 'haunted',
        verdict_accept_state: 'accepted',
        verdict_created_at: '2026-06-20T00:06:10.000Z',
        verdict_outcome: 'verified',
        verdict_trace_digest_prefix: 'bearer-leak-attempt',
      } satisfies ArtanisTickStreakDecisionRow,
      verifiedDispatch(1, '2026-06-20T00:01:00.000Z'),
    ]
    const streak = projectArtanisTickStreak(rows, nowIso)
    // haunted state row is dropped; only the verified dispatch remains
    expect(streak.ticks).toHaveLength(1)
    const serialized = JSON.stringify(streak)
    expect(serialized).not.toContain('haunted')
    expect(serialized).not.toContain('bearer-leak-attempt')
  })

  test('limits are bounded and defaulted', () => {
    expect(boundedTickStreakLimit(null)).toBe(100)
    expect(boundedTickStreakLimit('9999')).toBe(200)
    expect(boundedTickStreakLimit('0')).toBe(1)
    expect(boundedTickStreakLimit('not-a-number')).toBe(100)
  })

  test('the reader joins decisions to verdicts with a bounded limit', async () => {
    const rows = [verifiedDispatch(1, '2026-06-20T00:01:00.000Z')]
    const db = {
      prepare: (sql: string) => ({
        bind: (limit: number) => ({
          all: async () => {
            expect(sql).toContain('artanis_admin_tick_decisions')
            expect(sql).toContain('artanis_closeout_verdicts')
            expect(limit).toBe(200)
            return { results: rows }
          },
        }),
      }),
    } as unknown as D1Database
    const streak = await readArtanisTickStreak(db, { limit: 9999, nowIso })
    expect(streak.currentStreak).toBe(1)
  })
})
