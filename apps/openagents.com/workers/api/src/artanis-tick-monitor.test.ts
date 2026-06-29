import { describe, expect, test } from 'vitest'

import {
  boundedTickMonitorLimit,
  projectArtanisTickMonitor,
  readArtanisTickMonitor,
} from './artanis-tick-monitor'

const nowIso = '2026-06-11T01:20:00.000Z'

const rows = [
  {
    action_json: JSON.stringify({
      rationale: 'Idle eligible device; dispatch keeps the capability proven.',
    }),
    assignment_ref: 'assignment.artanis_admin.20260611011429',
    created_at: '2026-06-11T01:14:29.000Z',
    id: 'decision-1',
    state: 'dispatched',
  },
  {
    action_json: JSON.stringify({ reason: 'no useful dispatch this tick' }),
    assignment_ref: null,
    created_at: '2026-06-11T00:58:00.000Z',
    id: 'decision-2',
    state: 'no_action',
  },
  {
    action_json: JSON.stringify({
      rationale: 'leak attempt bearer abcdef0123456789 should never project',
    }),
    assignment_ref: null,
    created_at: '2026-06-10T22:00:00.000Z',
    id: 'decision-3',
    state: 'blocked',
  },
  {
    action_json: 'not-json',
    assignment_ref: null,
    created_at: '2026-06-10T21:00:00.000Z',
    id: 'decision-4',
    state: 'dispatch_failed',
  },
  {
    action_json: '{}',
    assignment_ref: null,
    created_at: '2026-06-10T20:00:00.000Z',
    id: 'decision-5',
    state: 'haunted',
  },
]

describe('artanis tick monitor projection', () => {
  test('projects decisions public-safe with counts and the daily bound', () => {
    const monitor = projectArtanisTickMonitor(rows, nowIso)
    expect(monitor.kind).toBe('artanis_admin_tick_monitor')
    expect(monitor.publicSafe).toBe(true)
    expect(monitor.dailyDispatchBound).toBeGreaterThan(0)
    expect(monitor.dispatchedToday).toBe(1)
    expect(monitor.countsByState).toEqual({
      blocked: 1,
      dispatch_failed: 1,
      dispatched: 1,
      no_action: 1,
    })
    expect(monitor.decisions).toHaveLength(4)
    expect(monitor.decisions[0]).toMatchObject({
      assignmentRef: 'assignment.artanis_admin.20260611011429',
      decisionRef: 'tick_decision.decision-1',
      state: 'dispatched',
    })
  })

  test('reasons are truncated, redaction-scanned, and never raw mind output', () => {
    const monitor = projectArtanisTickMonitor(rows, nowIso)
    const blocked = monitor.decisions.find(entry => entry.state === 'blocked')
    expect(blocked?.reason).toBe('reason.redacted')
    const failed = monitor.decisions.find(
      entry => entry.state === 'dispatch_failed',
    )
    expect(failed?.reason).toBe('reason.unparseable')
    const serialized = JSON.stringify(monitor)
    expect(serialized).not.toContain('bearer abcdef')
    expect(serialized).not.toContain('haunted')
  })

  test('limits are bounded and defaulted without raw parsing surprises', () => {
    expect(boundedTickMonitorLimit(null)).toBe(20)
    expect(boundedTickMonitorLimit('999')).toBe(50)
    expect(boundedTickMonitorLimit('0')).toBe(1)
    expect(boundedTickMonitorLimit('not-a-number')).toBe(20)
  })

  test('the reader queries the decisions table with a bounded limit', async () => {
    const db = {
      prepare: (sql: string) => ({
        bind: (limit: number) => ({
          all: async () => {
            expect(sql).toContain('artanis_admin_tick_decisions')
            expect(limit).toBe(50)
            return { results: rows }
          },
        }),
      }),
    } as unknown as D1Database
    const monitor = await readArtanisTickMonitor(db, { limit: 999, nowIso })
    expect(monitor.decisions).toHaveLength(4)
    expect(monitor.dispatchedToday).toBe(1)
  })
})
