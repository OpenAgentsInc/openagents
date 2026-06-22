import { describe, expect, test } from 'vitest'

import {
  ARTANIS_TASSADAR_DISTILLATION_DATASET_BLOCKER,
  ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
  ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
  boundedDistillationDatasetLimit,
  projectArtanisDistillationDatasetReceipt,
  readArtanisDistillationDatasetReceipt,
  type ArtanisDistillationDatasetRow,
} from './artanis-distillation-dataset-receipt'

const nowIso = '2026-06-20T03:00:00.000Z'

const verifiedTrace = (n: number): ArtanisDistillationDatasetRow => ({
  assignment_ref: `assignment.artanis_admin.2026061508${String(n).padStart(
    4,
    '0',
  )}`,
  decision_created_at: `2026-06-15T08:${String(n).padStart(2, '0')}:10.000Z`,
  decision_id: `decision-${n}`,
  verdict_accept_state: 'accepted',
  verdict_created_at: `2026-06-15T08:${String(n).padStart(2, '0')}:20.000Z`,
  verdict_outcome: 'verified',
  verdict_trace_digest_prefix: 'f2995c4e3c959b42',
})

describe('Artanis Tassadar distillation dataset receipt', () => {
  test('projects a public-safe refs-only receipt once enough verified traces exist', () => {
    const receipt = projectArtanisDistillationDatasetReceipt(
      Array.from({ length: ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET }, (_, i) =>
        verifiedTrace(i),
      ),
      nowIso,
    )

    expect(receipt.kind).toBe('artanis_tassadar_distillation_dataset_receipt')
    expect(receipt.publicSafe).toBe(true)
    expect(receipt.staleness.composition).toBe('live_at_read')
    expect(receipt.staleness.maxStalenessSeconds).toBe(0)
    expect(receipt.receiptState).toBe('available')
    expect(receipt.receiptRef).toBe(
      ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
    )
    expect(receipt.datasetRef).toBe(
      'dataset.tassadar_distillation.artanis_admin_verified_trace_refs.v1',
    )
    expect(receipt.sourceVerifiedTraceCount).toBe(
      ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
    )
    expect(receipt.uniqueTraceDigestPrefixCount).toBe(1)
    expect(receipt.traceDigestPrefixes).toEqual(['f2995c4e3c959b42'])
    expect(receipt.clearsBlockerRefs).toEqual([
      ARTANIS_TASSADAR_DISTILLATION_DATASET_BLOCKER,
    ])
    expect(receipt.blockerRefs).toEqual([])
    expect(receipt.closeoutReceiptRefs[0]).toContain(
      'receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.',
    )
    expect(receipt.traces[0]).toEqual(
      expect.objectContaining({
        sourceKind: 'artanis_admin_executor_trace_closeout',
        verdictRef: 'verdict.artanis_closeout.verified',
      }),
    )

    const serialized = JSON.stringify(receipt)
    expect(serialized).not.toContain('secret')
    expect(serialized).not.toContain('private_key')
  })

  test('keeps the blocker when too few accepted closeouts exist', () => {
    const receipt = projectArtanisDistillationDatasetReceipt(
      Array.from({ length: 9 }, (_, i) => verifiedTrace(i)),
      nowIso,
    )

    expect(receipt.receiptState).toBe('insufficient_verified_traces')
    expect(receipt.receiptRef).toBeNull()
    expect(receipt.datasetRef).toBeNull()
    expect(receipt.clearsBlockerRefs).toEqual([])
    expect(receipt.blockerRefs).toEqual([
      ARTANIS_TASSADAR_DISTILLATION_DATASET_BLOCKER,
    ])
  })

  test('rejects unaccepted rows and smuggled public values from the dataset', () => {
    const receipt = projectArtanisDistillationDatasetReceipt(
      [
        verifiedTrace(1),
        {
          ...verifiedTrace(2),
          verdict_accept_state: 'rejected',
        },
        {
          ...verifiedTrace(3),
          assignment_ref: 'assignment.other.private',
          verdict_trace_digest_prefix:
            'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzZWNyZXQifQ.signature',
        },
        {
          ...verifiedTrace(4),
          decision_id:
            'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzZWNyZXQifQ.signature',
        },
      ],
      nowIso,
    )

    expect(receipt.sourceVerifiedTraceCount).toBe(2)
    expect(receipt.traces.map(trace => trace.assignmentRef)).toEqual([
      'assignment.artanis_admin.20260615080001',
      'assignment.artanis_admin.20260615080004',
    ])
    expect(receipt.traces[1]?.decisionRef).toBe('tick_decision.redacted')
    const serialized = JSON.stringify(receipt)
    expect(serialized).not.toContain('assignment.other.private')
    expect(serialized).not.toContain('eyJhbGci')
  })

  test('limits are bounded and defaulted', () => {
    expect(boundedDistillationDatasetLimit(null)).toBe(100)
    expect(boundedDistillationDatasetLimit('9999')).toBe(200)
    expect(boundedDistillationDatasetLimit('0')).toBe(1)
    expect(boundedDistillationDatasetLimit('not-a-number')).toBe(100)
  })

  test('the reader joins accepted tick decisions to verdicts with a bounded limit', async () => {
    const rows = [verifiedTrace(1)]
    const db = {
      prepare: (sql: string) => ({
        bind: (limit: number) => ({
          all: async () => {
            expect(sql).toContain('artanis_admin_tick_decisions')
            expect(sql).toContain('artanis_closeout_verdicts')
            expect(sql).toContain("v.accept_state = 'accepted'")
            expect(limit).toBe(200)
            return { results: rows }
          },
        }),
      }),
    } as unknown as D1Database

    const receipt = await readArtanisDistillationDatasetReceipt(db, {
      limit: 9999,
      nowIso,
    })

    expect(receipt.sourceVerifiedTraceCount).toBe(1)
    expect(receipt.blockerRefs).toEqual([
      ARTANIS_TASSADAR_DISTILLATION_DATASET_BLOCKER,
    ])
  })
})
