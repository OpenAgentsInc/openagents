import { describe, expect, test } from 'vitest'

import {
  TassadarGradientWindowQuarantineRecordFeedSchemaVersion,
  buildTassadarGradientWindowQuarantineRecordFeed,
} from './tassadar-gradient-window-quarantine-record-feed'
import { buildTassadarGradientWindowQuarantineRecord } from './tassadar-gradient-window-quarantine-record'
import type { TassadarGradientWindowCandidate } from './tassadar-gradient-window-regime'

const digest = (char: string): string => char.repeat(64)

const baseCandidate = (
  windowRef: string,
): TassadarGradientWindowCandidate => ({
  baseCheckpointDigest: digest('a'),
  compiledCoreGradientTargeted: false,
  compiledCoreRef: 'core.public.psionic.tassadar_alm_numeric_executor.v1',
  constructionReceiptRefs: [
    'receipt.public.tassadar.c1_c5.constructed_compiled_core',
  ],
  contributorRef: 'agent.public.pylon.gradient_window_author',
  curatedDataRefs: ['data.public.tassadar.curated_interface_traces.v1'],
  datasetShardDigest: digest('b'),
  frozenCoreDigestAfter: digest('c'),
  frozenCoreDigestBefore: digest('c'),
  frozenParameterScopes: ['compiled_exact_core.tassadar_alm_numeric_executor'],
  gradientsFlowThroughTrace: true,
  learnedInterfaceDigest: digest('d'),
  modelFamilyRef: 'model.public.tassadar_student.hybrid_h1',
  optimizerStateDigest: digest('e'),
  psionicH1EvidenceRefs: [
    'receipt.public.psionic.tassadar_h1.baseline_d_frozen_core',
  ],
  quarantineCheckpointDigest: digest('f'),
  randomSeedRef: 'seed.public.tassadar.h1.window_0001',
  sourceRefs: ['issue.openagents.5332'],
  trainableParameterScopes: ['learned_interface.output_routing'],
  trainingConfigDigest: digest('1'),
  updateDigest: digest('2'),
  verificationReceiptRefs: ['receipt.public.tassadar.v1_exact_replay'],
  windowRef,
})

const quarantineRecord = (windowRef: string) =>
  buildTassadarGradientWindowQuarantineRecord(baseCandidate(windowRef), {
    admittedAt: '2026-06-20T12:00:00.000Z',
  })

describe('Tassadar gradient window quarantine record feed', () => {
  test('an empty input yields an empty feed', () => {
    const feed = buildTassadarGradientWindowQuarantineRecordFeed([])

    expect(feed.schemaVersion).toBe(
      TassadarGradientWindowQuarantineRecordFeedSchemaVersion,
    )
    expect(feed.publicSafe).toBe(true)
    expect(feed.acceptedEntries).toEqual([])
    expect(feed.acceptedRecordCount).toBe(0)
    expect(feed.rejectedRecordCount).toBe(0)
    expect(feed.rejectionReasonRefs).toEqual([])
  })

  test('admits builder-emitted records ordered by record ref', () => {
    const feed = buildTassadarGradientWindowQuarantineRecordFeed([
      quarantineRecord('training_window.public.tassadar.h1.0002'),
      quarantineRecord('training_window.public.tassadar.h1.0001'),
    ])

    expect(feed.acceptedRecordCount).toBe(2)
    expect(feed.rejectedRecordCount).toBe(0)
    expect(feed.acceptedEntries.map(entry => entry.windowRef)).toEqual([
      'training_window.public.tassadar.h1.0001',
      'training_window.public.tassadar.h1.0002',
    ])
    expect(feed.acceptedEntries[0]?.pendingVerificationStages).toEqual([
      'recomputed',
      'replicated',
      'canary_passed',
      'promoted',
    ])
  })

  test('drops a duplicate record ref keeping the first', () => {
    const record = quarantineRecord('training_window.public.tassadar.h1.0001')
    const feed = buildTassadarGradientWindowQuarantineRecordFeed([
      record,
      record,
    ])

    expect(feed.acceptedRecordCount).toBe(1)
    expect(feed.rejectedRecordCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_feed.duplicate_record_ref',
    )
  })

  test('rejects an unparseable record without throwing', () => {
    const feed = buildTassadarGradientWindowQuarantineRecordFeed([
      { not: 'a quarantine record' },
      quarantineRecord('training_window.public.tassadar.h1.0001'),
    ])

    expect(feed.acceptedRecordCount).toBe(1)
    expect(feed.rejectedRecordCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_verification.quarantine_record_unparsed',
    )
  })

  test('rejects a record whose ref no longer derives from its window ref', () => {
    const tampered = {
      ...quarantineRecord('training_window.public.tassadar.h1.0001'),
      recordRef: 'quarantine.public.tassadar_gradient_window.tampered',
    }
    const feed = buildTassadarGradientWindowQuarantineRecordFeed([tampered])

    expect(feed.acceptedRecordCount).toBe(0)
    expect(feed.rejectedRecordCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain(
      'blocker.public.tassadar_gradient_window.quarantine_record_verification.record_ref_mismatch',
    )
  })
})
