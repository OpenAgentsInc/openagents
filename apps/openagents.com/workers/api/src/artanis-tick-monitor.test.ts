import { describe, expect, test } from 'vitest'

import {
  boundedTickMonitorLimit,
  projectArtanisTickMonitor,
  readArtanisTickMonitor,
} from './artanis-tick-monitor'

const nowIso = '2026-06-11T01:20:00.000Z'
const traceDigest = 'f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b'

const rows = [
  {
    accepted_work_refs_json: JSON.stringify([
      'accepted_work.artanis_admin.20260611011429',
    ]),
    action_json: JSON.stringify({
      rationale: 'Idle eligible device; dispatch keeps the capability proven.',
    }),
    assignment_ref: 'assignment.artanis_admin.20260611011429',
    assignment_created_at: '2026-06-11T01:14:29.000Z',
    assignment_state: 'accepted_work',
    assignment_updated_at: '2026-06-11T01:16:00.000Z',
    artifact_refs_json: JSON.stringify([
      `artifact.tassadar_poc.trace_digest.${traceDigest}`,
    ]),
    closeout_refs_json: JSON.stringify([
      'closeout.artanis_admin.assignment.20260611011429',
    ]),
    created_at: '2026-06-11T01:14:29.000Z',
    id: 'decision-1',
    job_kind: 'tassadar_executor_trace',
    proof_refs_json: JSON.stringify([
      `proof.tassadar_poc.trace_digest.${traceDigest.slice(0, 16)}`,
    ]),
    pylon_ref: 'pylon.public.alpha',
    state: 'dispatched',
    verdict_accept_state: 'accepted',
    verdict_created_at: '2026-06-11T01:17:00.000Z',
    verdict_outcome: 'verified',
    verdict_trace_digest_prefix: traceDigest.slice(0, 16),
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
      closedTickReceiptRef:
        'receipt.public.artanis.tetrahedron_closed_tick.assignment.artanis_admin.20260611011429',
      closeoutReceiptRef:
        'receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260611011429',
      closureState: 'closed_verified',
      decisionRef: 'tick_decision.decision-1',
      state: 'dispatched',
    })
    expect(monitor.closedTickReceiptRefs).toEqual([
      'receipt.public.artanis.tetrahedron_closed_tick.assignment.artanis_admin.20260611011429',
    ])
    expect(monitor.closedTickStaleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
      rebuildsOn: [
        'artanis_admin_tick_decision_recorded',
        'pylon_assignment_closeout_submitted',
        'artanis_closeout_verdict_recorded',
      ],
    })
    expect(monitor.closedTickReceipts[0]).toMatchObject({
      assignmentRef: 'assignment.artanis_admin.20260611011429',
      closeoutReceiptRef:
        'receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260611011429',
      closureContractVersion: 'tick_closure.v0.1',
      receiptKind: 'artanis_tetrahedron_closed_tick',
      verdictOutcome: 'verified',
      verdictRef: 'verdict.artanis_closeout.verified',
    })
    expect(monitor.closedTickReceipts[0]?.faceRefs).toMatchObject({
      evaluationRefs: [
        'expectation.tassadar_poc.trace_digest.f2995c4e3c959b42',
        'verdict.artanis_closeout.verified',
      ],
      stateDeltaRefs: [
        'accepted_work.artanis_admin.20260611011429',
        'receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260611011429',
      ],
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
    expect(serialized).not.toMatch(/mnemonic|preimage|secret/i)
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
            expect(sql).toContain('LEFT JOIN pylon_api_assignments')
            expect(sql).toContain('LEFT JOIN artanis_closeout_verdicts')
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
