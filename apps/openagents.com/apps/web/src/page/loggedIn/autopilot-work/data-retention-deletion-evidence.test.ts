import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeDataRetentionDeletionInput,
  projectForgeDataRetentionDeletionEvidence,
} from './data-retention-deletion-evidence'

const baseInput = {
  generatedAt: '2026-06-18T06:00:00.000Z',
  snapshotRef: 'retention-deletion-snapshot.public.work_1',
  versionRef: 'retention-deletion-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const retainedEntry = {
  dataClass: 'memory_records' as const,
  dataClassRef: 'data-class.public.memory_records',
  exportManifestRefs: ['export-manifest.public.memory_records'],
  freshness: 'fresh' as const,
  retentionPolicyRefs: ['retention-policy.public.memory_records'],
  retentionSweepRefs: ['retention-sweep.public.memory_records'],
  status: 'retained' as const,
}

describe('Forge data retention and deletion evidence projection', () => {
  test('projects retention and deletion evidence as refs-only non-authoritative state', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [retainedEntry],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      classes: 1,
      deleteRequested: 0,
      deletedOrTombstoned: 0,
      exportable: 1,
      legalOrPaymentCaveats: 0,
      publicProjectionClasses: 0,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      cachePurgeAuthority: false,
      credentialReadAuthority: false,
      credentialRevocationAuthority: false,
      deletionExecutionAuthority: false,
      exportGenerationAuthority: false,
      exportReadAuthority: false,
      privateDataReadAuthority: false,
      projectionInvalidationAuthority: false,
      publicProjectionMutationAuthority: false,
      receiptDeletionAuthority: false,
      retentionPolicyMutationAuthority: false,
      retentionSweepAuthority: false,
      settlementAuthority: false,
      tombstoneCreationAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing retention and deletion evidence as empty', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      generatedAt: '2026-06-18T06:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks retained classes without retention policy refs', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [
        {
          ...retainedEntry,
          retentionPolicyRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:retained-data-class-policy-missing:data-class.public.memory_records',
    )
  })

  test('blocks deletion states without request plus receipt or tombstone refs', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [
        {
          dataClass: 'artifact_payloads',
          dataClassRef: 'data-class.public.artifact_payloads',
          freshness: 'fresh',
          retentionPolicyRefs: ['retention-policy.public.artifacts'],
          status: 'delete_requested',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:deletion-state-receipt-or-tombstone-missing:data-class.public.artifact_payloads',
    )
  })

  test('blocks tombstoned records that still look projected as current', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [
        {
          dataClass: 'session_summaries',
          dataClassRef: 'data-class.public.session_summary',
          deletionRequestRefs: ['deletion-request.public.session_summary'],
          projectionFreshnessRefs: ['projection-freshness.public.current'],
          status: 'tombstoned',
          tombstoneRefs: ['tombstone.public.session_summary'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:tombstoned-record-projected-current:data-class.public.session_summary',
    )
  })

  test('blocks exportable retained classes without export manifest refs', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [
        {
          ...retainedEntry,
          exportManifestRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:exportable-class-manifest-missing:data-class.public.memory_records',
    )
  })

  test('blocks public projection classes without freshness and invalidation refs', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [
        {
          caveatRefs: ['retention-caveat.public.payment_receipt_required'],
          dataClass: 'product_receipts',
          dataClassRef: 'data-class.public.product_receipts',
          exportManifestRefs: ['export-manifest.public.product_receipts'],
          retentionPolicyRefs: ['retention-policy.public.product_receipts'],
          status: 'retained',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:public-projection-invalidation-missing:data-class.public.product_receipts',
    )
  })

  test('blocks legal and payment retention without caveat refs', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [
        {
          dataClass: 'product_receipts',
          dataClassRef: 'data-class.public.product_receipts',
          exportManifestRefs: ['export-manifest.public.product_receipts'],
          projectionFreshnessRefs: ['projection-freshness.public.product_receipts'],
          projectionInvalidationRefs: ['projection-invalidation.public.product_receipts'],
          retentionPolicyRefs: ['retention-policy.public.product_receipts'],
          status: 'legal_hold',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:legal-payment-caveat-missing:data-class.public.product_receipts',
    )
  })

  test('blocks stale retention evidence', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      entries: [
        {
          ...retainedEntry,
          dataClassRef: 'data-class.public.stale_memory',
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:stale-retention-deletion-evidence:data-class.public.stale_memory',
    )
  })

  test('blocks populated retention entries without snapshot refs', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      entries: [retainedEntry],
      generatedAt: '2026-06-18T06:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.no_snapshot:missing-data-retention-deletion-snapshot-ref',
    )
  })

  test('omits unsafe private retention material before projection', () => {
    const view = projectForgeDataRetentionDeletionEvidence({
      ...baseInput,
      blockerRefs: [
        'retention-blocker.public.safe',
        'raw deleted payload /Users/christopher/deleted.json',
      ],
      entries: [
        {
          ...retainedEntry,
          blockerRefs: ['retention-entry-blocker.public.safe'],
          dataClassRef: 'data-class.public.safe_memory',
          deletionReceiptRefs: ['deletion-receipt.public.safe', 'event payload private'],
          deletionRequestRefs: ['deletion-request.public.safe'],
          exportManifestRefs: ['export-manifest.public.safe', 'export content secret'],
          projectionFreshnessRefs: ['projection-freshness.public.safe'],
          projectionInvalidationRefs: ['projection-invalidation.public.safe'],
          retentionPolicyRefs: ['retention-policy.public.safe', 'credential value password private'],
          tombstoneRefs: ['tombstone.public.safe', 'cache content /Users/christopher/cache'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.deletionReceiptRefs).toEqual([
      'deletion-receipt.public.safe',
    ])
    expect(view.entries[0]?.exportManifestRefs).toEqual([
      'export-manifest.public.safe',
    ])
    expect(view.entries[0]?.retentionPolicyRefs).toEqual([
      'retention-policy.public.safe',
    ])
    expect(view.entries[0]?.tombstoneRefs).toEqual(['tombstone.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-data-retention-deletion-blocker:work.public.work_1:unsafe-data-retention-deletion-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw deleted payload')
    expect(payload).not.toContain('event payload')
    expect(payload).not.toContain('export content')
    expect(payload).not.toContain('credential value')
    expect(payload).not.toContain('cache content')
    expect(payload).not.toContain('password')
    expect(payload).not.toContain('secret')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      dataRetentionDeletionEvidence: {
        entries: [retainedEntry],
        generatedAt: '2026-06-18T06:01:00.000Z',
        snapshotRef: 'retention-deletion-snapshot.public.work_2',
        versionRef: 'retention-deletion-version.public.v2',
      },
      generatedAt: '2026-06-18T06:00:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeDataRetentionDeletionInput(work)).toEqual({
      entries: [retainedEntry],
      generatedAt: '2026-06-18T06:01:00.000Z',
      snapshotRef: 'retention-deletion-snapshot.public.work_2',
      versionRef: 'retention-deletion-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
