import { describe, expect, test } from 'vitest'

import {
  attachArtanisDistillationDatasetReceipt,
  type ArtanisTickDecisionRow,
  boundedTickMonitorLimit,
  projectArtanisTickMonitor,
  readArtanisTickMonitor,
} from './artanis-tick-monitor'

const nowIso = '2026-06-11T01:20:00.000Z'
const traceDigest = 'f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b'

const closedDecisionRow = (index: number): ArtanisTickDecisionRow => {
  const padded = String(index).padStart(2, '0')
  const suffix = `2026061101${padded}`
  const assignmentRef = `assignment.artanis_admin.closed_${padded}`
  return {
    accepted_work_refs_json: JSON.stringify([
      `accepted_work.artanis_admin.${suffix}`,
    ]),
    action_json: JSON.stringify({
      rationale: `Closed tick ${padded} keeps the executor loop proven.`,
    }),
    assignment_ref: assignmentRef,
    assignment_created_at: `2026-06-11T01:${padded}:00.000Z`,
    assignment_state: 'accepted_work',
    assignment_updated_at: `2026-06-11T01:${padded}:30.000Z`,
    artifact_refs_json: JSON.stringify([
      `artifact.tassadar_poc.trace_digest.${traceDigest}`,
    ]),
    closeout_refs_json: JSON.stringify([
      `closeout.artanis_admin.assignment.${suffix}`,
    ]),
    created_at: `2026-06-11T01:${padded}:00.000Z`,
    id: `decision-closed-${padded}`,
    job_kind: 'tassadar_executor_trace',
    proof_refs_json: JSON.stringify([
      `proof.tassadar_poc.trace_digest.${traceDigest.slice(0, 16)}`,
    ]),
    pylon_ref: `pylon.public.${padded}`,
    state: 'dispatched',
    verdict_accept_state: 'accepted',
    verdict_created_at: `2026-06-11T01:${padded}:45.000Z`,
    verdict_outcome: 'verified',
    verdict_trace_digest_prefix: traceDigest.slice(0, 16),
  }
}

const rows: ReadonlyArray<ArtanisTickDecisionRow> = [
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
    expect(monitor.unattendedTickStreak).toMatchObject({
      blockerRefs: [
        'blocker.product_promises.artanis_unattended_tick_streak_missing',
      ],
      currentConsecutiveClosedTicks: 1,
      kind: 'artanis_unattended_tick_streak',
      longestConsecutiveClosedTicks: 1,
      receiptRef: null,
      requiredConsecutiveClosedTicks: 10,
      satisfied: false,
    })
    expect(monitor.unattendedTickStreak.closedTickReceiptRefs).toEqual([
      'receipt.public.artanis.tetrahedron_closed_tick.assignment.artanis_admin.20260611011429',
    ])
    expect(monitor.unattendedTickStreak.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
      rebuildsOn: [
        'artanis_admin_tick_decision_recorded',
        'pylon_assignment_closeout_submitted',
        'artanis_closeout_verdict_recorded',
      ],
    })
  })

  test('emits a streak receipt only after ten consecutive closed ticks', () => {
    const monitor = projectArtanisTickMonitor(
      Array.from({ length: 10 }, (_, index) => closedDecisionRow(index + 1)),
      nowIso,
    )
    expect(monitor.unattendedTickStreak).toMatchObject({
      blockerRefs: [],
      currentConsecutiveClosedTicks: 10,
      longestConsecutiveClosedTicks: 10,
      receiptRef:
        'receipt.public.artanis.unattended_tick_streak.decision-closed-01.x10',
      requiredConsecutiveClosedTicks: 10,
      satisfied: true,
    })
    expect(monitor.unattendedTickStreak.closedTickReceiptRefs).toHaveLength(10)
    expect(monitor.unattendedTickStreak.decisionRefs).toHaveLength(10)
  })

  test('attaches a dataset_curation receipt from a satisfied verified-trace streak', async () => {
    let digestCounter = 0
    const monitor = await attachArtanisDistillationDatasetReceipt(
      projectArtanisTickMonitor(
        Array.from({ length: 10 }, (_, index) =>
          closedDecisionRow(index + 1),
        ),
        nowIso,
      ),
      async () => {
        digestCounter += 1
        return String(digestCounter).repeat(64)
      },
    )

    expect(monitor.distillationDatasetReceipt).toMatchObject({
      authorityBoundary:
        'Read-only dataset-curation receipt. Grants no training launch, model-capability, publication, payout, settlement, or promise-transition authority.',
      curationKind: 'dataset_curation',
      datasetRef:
        'dataset.public.tassadar_distillation.artanis_admin.v0_1.4444444444444444',
      encodingRef: 'encoding.tassadar_trace.compact_binary.v0_1',
      includedTraceCount: 10,
      manifestDigestRef:
        'digest.sha256.tassadar_distillation.manifest.4444444444444444',
      receiptKind: 'tassadar_distillation_dataset_curation',
      receiptRef: 'receipt.public.artanis.dataset_curation.4444444444444444',
      splitPolicyRef: 'policy.tassadar_trace.train_val_test_split.v0_1',
    })
    expect(monitor.distillationDatasetReceipt?.sourceReceiptRefs).toHaveLength(
      10,
    )
    expect(monitor.distillationDatasetReceipt?.shardRefs).toEqual([
      expect.objectContaining({
        itemCount: 8,
        shardDigestRef:
          'digest.sha256.tassadar_distillation.train.1111111111111111',
        split: 'train',
      }),
      expect.objectContaining({
        itemCount: 1,
        shardDigestRef:
          'digest.sha256.tassadar_distillation.validation.2222222222222222',
        split: 'validation',
      }),
      expect.objectContaining({
        itemCount: 1,
        shardDigestRef:
          'digest.sha256.tassadar_distillation.test.3333333333333333',
        split: 'test',
      }),
    ])
    expect(JSON.stringify(monitor.distillationDatasetReceipt)).not.toMatch(
      /dataset\.raw|private|mnemonic|preimage|secret/i,
    )
  })

  test('does not attach a dataset_curation receipt before the streak is satisfied', async () => {
    const monitor = await attachArtanisDistillationDatasetReceipt(
      projectArtanisTickMonitor(
        Array.from({ length: 9 }, (_, index) => closedDecisionRow(index + 1)),
        nowIso,
      ),
    )

    expect(monitor.distillationDatasetReceipt).toBeNull()
  })

  test('does not satisfy the streak gate across an interrupted run', () => {
    const interruptingDecision: ArtanisTickDecisionRow = {
      action_json: JSON.stringify({ reason: 'no eligible device this tick' }),
      assignment_ref: null,
      created_at: '2026-06-11T01:06:30.000Z',
      id: 'decision-interrupting-no-action',
      state: 'no_action',
    }
    const monitor = projectArtanisTickMonitor(
      [
        ...Array.from({ length: 5 }, (_, index) =>
          closedDecisionRow(index + 1),
        ),
        interruptingDecision,
        ...Array.from({ length: 6 }, (_, index) =>
          closedDecisionRow(index + 6),
        ),
      ],
      nowIso,
    )
    expect(monitor.unattendedTickStreak).toMatchObject({
      blockerRefs: [
        'blocker.product_promises.artanis_unattended_tick_streak_missing',
      ],
      currentConsecutiveClosedTicks: 5,
      longestConsecutiveClosedTicks: 6,
      receiptRef: null,
      satisfied: false,
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
