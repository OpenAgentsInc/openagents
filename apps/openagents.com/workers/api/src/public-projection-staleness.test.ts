import { describe, expect, test } from 'vitest'
import { Schema as S } from 'effect'

import {
  PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION,
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
  projectionDataAgeSeconds,
  projectionStalenessExceeded,
  rebuiltOnTransitionStaleness,
  storedSnapshotStaleness,
} from './public-projection-staleness'

describe('public projection staleness contract (epic #4751)', () => {
  test('live_at_read declares zero staleness with the rebuild transitions', () => {
    const contract = liveAtReadStaleness(['agent_owner_claim_approved'])

    expect(contract.composition).toBe('live_at_read')
    expect(contract.contractVersion).toBe(
      PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION,
    )
    expect(contract.maxStalenessSeconds).toBe(0)
    expect(contract.rebuildsOn).toEqual(['agent_owner_claim_approved'])
    expect(() =>
      S.decodeUnknownSync(PublicProjectionStalenessContract)(contract),
    ).not.toThrow()
  })

  test('rebuilt_on_transition and stored_snapshot carry their declared bound', () => {
    const rebuilt = rebuiltOnTransitionStaleness(86_400, [
      'artanis_loop_tick_closeout',
    ])
    const snapshot = storedSnapshotStaleness(3_600, [
      'pylon_capacity_funnel_snapshot_recorded',
    ])

    expect(rebuilt.composition).toBe('rebuilt_on_transition')
    expect(rebuilt.maxStalenessSeconds).toBe(86_400)
    expect(snapshot.composition).toBe('stored_snapshot')
    expect(snapshot.maxStalenessSeconds).toBe(3_600)
    for (const contract of [rebuilt, snapshot]) {
      expect(() =>
        S.decodeUnknownSync(PublicProjectionStalenessContract)(contract),
      ).not.toThrow()
    }
  })

  test('live_at_read is never flagged stale by construction', () => {
    const contract = liveAtReadStaleness(['forum_payment_event_confirmed'])

    expect(projectionStalenessExceeded(contract, 999_999)).toBe(false)
    expect(projectionStalenessExceeded(contract, null)).toBe(false)
  })

  test('declared bounds flag data older than the contract, and only that', () => {
    const contract = rebuiltOnTransitionStaleness(600, ['tick_closeout'])

    expect(projectionStalenessExceeded(contract, 599)).toBe(false)
    expect(projectionStalenessExceeded(contract, 600)).toBe(false)
    expect(projectionStalenessExceeded(contract, 601)).toBe(true)
    expect(projectionStalenessExceeded(contract, null)).toBe(false)
  })

  test('data age computes whole seconds and refuses malformed timestamps', () => {
    expect(
      projectionDataAgeSeconds(
        '2026-06-07T00:00:00.000Z',
        '2026-06-11T00:00:00.000Z',
      ),
    ).toBe(4 * 24 * 3600)
    expect(
      projectionDataAgeSeconds(
        '2026-06-11T00:00:30.000Z',
        '2026-06-11T00:00:00.000Z',
      ),
    ).toBe(0)
    expect(projectionDataAgeSeconds(null, '2026-06-11T00:00:00.000Z')).toBe(
      null,
    )
    expect(
      projectionDataAgeSeconds('not-a-timestamp', '2026-06-11T00:00:00.000Z'),
    ).toBe(null)
  })
})
