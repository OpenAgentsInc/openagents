import { describe, expect, test } from 'vitest'

import { CurtailmentDrillBlocker } from './training-curtailment-drill'
import {
  buildCurtailmentDrillReceipt,
  curtailmentDrillReceiptRef,
} from './training-curtailment-drill-receipt'
import {
  CurtailmentDrillReceiptFeedSchemaVersion,
  buildCurtailmentDrillReceiptFeed,
} from './training-curtailment-drill-receipt-feed'

const passingDrill = (drillRef: string) => ({
  ackLatencyMs: 1_200,
  drillRef,
  durableCheckpointSealed: true,
  haltCompleted: true,
  haltLatencyMs: 120_000,
  resumeVerified: true,
  runRef: 'run.public.psionic.marathon.alpha',
  scheduled: true,
  signalAcknowledged: true,
})

const receiptFor = (drillRef: string) =>
  buildCurtailmentDrillReceipt(passingDrill(drillRef))

describe('curtailment drill receipt feed', () => {
  test('an empty list yields an empty public-safe feed', () => {
    const feed = buildCurtailmentDrillReceiptFeed([])
    expect(feed.acceptedEntries).toEqual([])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(0)
    expect(feed.publicSafe).toBe(true)
    expect(feed.blockerRef).toBe(CurtailmentDrillBlocker)
    expect(feed.schemaVersion).toBe(CurtailmentDrillReceiptFeedSchemaVersion)
  })

  test('admits genuine receipts ordered by receipt ref', () => {
    const a = receiptFor('drill.public.marathon.curtailment.2026-06-20')
    const b = receiptFor('drill.public.marathon.curtailment.2026-07-01')
    const feed = buildCurtailmentDrillReceiptFeed([b, a])
    expect(feed.acceptedReceiptCount).toBe(2)
    expect(feed.acceptedEntries.map(e => e.receiptRef)).toEqual(
      [a.receiptRef, b.receiptRef].sort(),
    )
    expect(feed.acceptedEntries[0]?.runRef).toBe(
      'run.public.psionic.marathon.alpha',
    )
    expect(feed.rejectedReceiptCount).toBe(0)
  })

  test('drops duplicate receipt refs keeping the first', () => {
    const a = receiptFor('drill.public.marathon.curtailment.2026-06-20')
    const feed = buildCurtailmentDrillReceiptFeed([a, { ...a }])
    expect(feed.acceptedReceiptCount).toBe(1)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('duplicate_receipt_ref')
  })

  test('rejects a malformed receipt without throwing', () => {
    const feed = buildCurtailmentDrillReceiptFeed([{ not: 'a receipt' }])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('receipt_malformed')
  })

  test('rejects a decodable but unverifiable (ref-mismatched) receipt', () => {
    const tampered = {
      ...receiptFor('drill.public.marathon.curtailment.2026-06-20'),
      receiptRef: curtailmentDrillReceiptRef(
        'drill.public.marathon.curtailment.9999-99-99',
      ),
    }
    const feed = buildCurtailmentDrillReceiptFeed([tampered])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('receipt_not_verified')
  })

  test('mixes accepted and rejected receipts deterministically', () => {
    const good = receiptFor('drill.public.marathon.curtailment.2026-06-20')
    const feed = buildCurtailmentDrillReceiptFeed([
      good,
      { garbage: true },
      { ...good },
    ])
    expect(feed.acceptedReceiptCount).toBe(1)
    expect(feed.rejectedReceiptCount).toBe(2)
    expect([...feed.rejectionReasonRefs]).toEqual(
      [...feed.rejectionReasonRefs].sort(),
    )
  })
})
