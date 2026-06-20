import { describe, expect, test } from 'vitest'

import { StandbyDispatchBlocker } from './training-standby-dispatch'
import {
  buildStandbyDispatchReceipt,
  standbyDispatchReceiptRef,
} from './training-standby-dispatch-receipt'
import {
  StandbyDispatchReceiptFeedSchemaVersion,
  buildStandbyDispatchReceiptFeed,
} from './training-standby-dispatch-receipt-feed'

const promotableDispatch = (standbyContributorRef: string) => ({
  bannedForRound: false,
  bootstrapSealVerified: true,
  bootstrapSealWindowRef: 'window.public.psionic.marathon.alpha.seal.42',
  lastHeartbeatAgeMs: 5_000,
  liveSealedWindowRef: 'window.public.psionic.marathon.alpha.seal.42',
  liveVacancyCount: 1,
  qualified: true,
  runRef: 'run.public.psionic.marathon.alpha',
  standbyContributorRef,
})

const receiptFor = (standbyContributorRef: string) =>
  buildStandbyDispatchReceipt(promotableDispatch(standbyContributorRef))

describe('standby dispatch receipt feed', () => {
  test('an empty list yields an empty public-safe feed', () => {
    const feed = buildStandbyDispatchReceiptFeed([])
    expect(feed.acceptedEntries).toEqual([])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(0)
    expect(feed.publicSafe).toBe(true)
    expect(feed.blockerRef).toBe(StandbyDispatchBlocker)
    expect(feed.schemaVersion).toBe(StandbyDispatchReceiptFeedSchemaVersion)
  })

  test('admits genuine receipts ordered by receipt ref', () => {
    const a = receiptFor('standby.public.pylon.aaa')
    const b = receiptFor('standby.public.pylon.bbb')
    const feed = buildStandbyDispatchReceiptFeed([b, a])
    expect(feed.acceptedReceiptCount).toBe(2)
    expect(feed.acceptedEntries.map(e => e.receiptRef)).toEqual(
      [a.receiptRef, b.receiptRef].sort(),
    )
    expect(feed.acceptedEntries[0]?.runRef).toBe(
      'run.public.psionic.marathon.alpha',
    )
    expect(feed.acceptedEntries[0]?.promotedIntoWindowRef).toBe(
      'window.public.psionic.marathon.alpha.seal.42',
    )
    expect(feed.rejectedReceiptCount).toBe(0)
  })

  test('drops duplicate receipt refs keeping the first', () => {
    const a = receiptFor('standby.public.pylon.aaa')
    const feed = buildStandbyDispatchReceiptFeed([a, { ...a }])
    expect(feed.acceptedReceiptCount).toBe(1)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('duplicate_receipt_ref')
  })

  test('rejects a malformed receipt without throwing', () => {
    const feed = buildStandbyDispatchReceiptFeed([{ not: 'a receipt' }])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('receipt_malformed')
  })

  test('rejects a decodable but unverifiable (ref-mismatched) receipt', () => {
    const tampered = {
      ...receiptFor('standby.public.pylon.aaa'),
      receiptRef: standbyDispatchReceiptRef(
        'run.public.psionic.marathon.alpha',
        'standby.public.pylon.zzz',
      ),
    }
    const feed = buildStandbyDispatchReceiptFeed([tampered])
    expect(feed.acceptedReceiptCount).toBe(0)
    expect(feed.rejectedReceiptCount).toBe(1)
    expect(feed.rejectionReasonRefs).toContain('receipt_not_verified')
  })

  test('mixes accepted and rejected receipts deterministically', () => {
    const good = receiptFor('standby.public.pylon.aaa')
    const feed = buildStandbyDispatchReceiptFeed([
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
