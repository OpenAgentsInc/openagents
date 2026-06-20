import { describe, expect, test } from 'vitest'

import { ArtanisLaborReceiptError } from './artanis-labor-request-receipt'
import {
  verifyArtanisLaborReceiptFeed,
  verifyArtanisLaborReceiptFeedRow,
} from './artanis-labor-receipt-feed-verify'
import {
  buildArtanisLaborReceiptFeedProjection,
  type ArtanisLaborReceiptFeedRow,
} from './artanis-labor-receipt-routes'
import {
  sealArtanisLaborUnattendedRequestReceipt,
  type ArtanisLaborSealedReceipt,
} from './artanis-labor-receipt-store'
import type {
  ArtanisLaborAcceptanceOutcome,
  ArtanisLaborRequesterOutcome,
} from './artanis-labor-requester'

const requestedOutcome: ArtanisLaborRequesterOutcome = {
  budgetMsat: 2_000_000,
  kind: 'requested',
  receipt: {
    jobEventId: 'a'.repeat(64),
    topicId: 'topic_1',
    workRequestId: 'work_request_1',
  },
  reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_1',
}

const acceptedOutcome: ArtanisLaborAcceptanceOutcome = {
  kind: 'accepted',
  releaseReceiptRef: 'receipt.labor_escrow.release.artanis_1',
}

const base = {
  artanisActorRef: 'agent:artanis',
  nowIso: '2026-06-20T12:00:00.000Z',
  tickRef: 'tick.public.artanis.2026-06-20T12:00',
}

const sealRequested = (): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({ ...base, requestOutcome: requestedOutcome })

const sealSkipped = (): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef: 'tick.public.artanis.2026-06-20T12:01',
    requestOutcome: { kind: 'skipped', reason: 'config_disabled' },
  })

const sealAccepted = (): ArtanisLaborSealedReceipt =>
  sealArtanisLaborUnattendedRequestReceipt({
    ...base,
    tickRef: 'tick.public.artanis.2026-06-20T12:02',
    requestOutcome: requestedOutcome,
    acceptanceOutcome: acceptedOutcome,
  })

const feedOf = (sealed: ReadonlyArray<ArtanisLaborSealedReceipt>) =>
  buildArtanisLaborReceiptFeedProjection({
    sealed,
    generatedAt: base.nowIso,
  })

describe('artanis labor receipt feed consumer-side verification', () => {
  test('a clean served row re-derives its served ref from public fields', () => {
    const feed = feedOf([sealRequested()])
    const row = feed.rows[0]
    expect(row).toBeDefined()
    const receipt = verifyArtanisLaborReceiptFeedRow(row as ArtanisLaborReceiptFeedRow)
    expect(receipt.terminalState).toBe('requested_pending_delivery')
    expect(receipt.workRequestId).toBe('work_request_1')
  })

  test('the derived receipt matches the sealed source receipt', () => {
    const sealed = sealAccepted()
    const feed = feedOf([sealed])
    const receipt = verifyArtanisLaborReceiptFeedRow(
      feed.rows[0] as ArtanisLaborReceiptFeedRow,
    )
    expect(receipt).toEqual(sealed.receipt)
  })

  test('a tampered served ref is rejected by the consumer', () => {
    const feed = feedOf([sealRequested()])
    const tampered: ArtanisLaborReceiptFeedRow = {
      ...(feed.rows[0] as ArtanisLaborReceiptFeedRow),
      receiptRef: 'receipt.artanis_labor.unattended_request.deadbeefdeadbeef',
    }
    expect(() => verifyArtanisLaborReceiptFeedRow(tampered)).toThrow(
      ArtanisLaborReceiptError,
    )
  })

  test('an internally inconsistent row (budget on a non-placed state) is rejected', () => {
    const feed = feedOf([sealSkipped()])
    const inconsistent: ArtanisLaborReceiptFeedRow = {
      ...(feed.rows[0] as ArtanisLaborReceiptFeedRow),
      budgetMsat: 2_000_000,
    }
    expect(() => verifyArtanisLaborReceiptFeedRow(inconsistent)).toThrow(
      ArtanisLaborReceiptError,
    )
  })

  test('a mutated public field no longer addresses the served ref', () => {
    const feed = feedOf([sealRequested()])
    const mutated: ArtanisLaborReceiptFeedRow = {
      ...(feed.rows[0] as ArtanisLaborReceiptFeedRow),
      budgetMsat: 1,
    }
    expect(() => verifyArtanisLaborReceiptFeedRow(mutated)).toThrow(
      ArtanisLaborReceiptError,
    )
  })

  test('a full clean feed verifies every row and counts them', () => {
    const feed = feedOf([sealRequested(), sealSkipped(), sealAccepted()])
    const result = verifyArtanisLaborReceiptFeed(feed)
    expect(result.verifiedRowCount).toBe(3)
    expect(result.receipts).toHaveLength(3)
  })

  test('an empty feed verifies to zero rows without throwing', () => {
    const result = verifyArtanisLaborReceiptFeed(feedOf([]))
    expect(result.verifiedRowCount).toBe(0)
    expect(result.receipts).toEqual([])
  })

  test('a feed with one tampered row throws on that row', () => {
    const feed = feedOf([sealRequested(), sealSkipped()])
    const broken = {
      ...feed,
      rows: [
        feed.rows[0] as ArtanisLaborReceiptFeedRow,
        {
          ...(feed.rows[1] as ArtanisLaborReceiptFeedRow),
          tickRef: 'tick.public.artanis.forged',
        },
      ],
    }
    expect(() => verifyArtanisLaborReceiptFeed(broken)).toThrow(
      ArtanisLaborReceiptError,
    )
  })
})
