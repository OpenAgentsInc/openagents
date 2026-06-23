// Khala M4 — Pylon serving ADMISSION gate tests (EPIC #6017, #6012).
//
// Proves the supply-side admission decision: a Pylon is admitted to serve a Khala
// request ONLY when it advertises the required capability + a serving lane, has a
// fresh healthy heartbeat, and is wallet/payout ready. Every gate fails CLOSED
// (refuse + neutral blocker ref), so a missing capability / stale heartbeat /
// unready wallet degrades safely (no routing) rather than serving against an
// unready node. PURE: the freshness clock is injected, no Date.now().

import { describe, expect, it } from 'vitest'

import {
  DEFAULT_HEARTBEAT_TTL_MS,
  PYLON_ADMISSION_CAPABILITY_MISSING_REF,
  PYLON_ADMISSION_HEARTBEAT_STALE_REF,
  PYLON_ADMISSION_HEARTBEAT_UNHEALTHY_REF,
  PYLON_ADMISSION_NOT_ACTIVE_REF,
  PYLON_ADMISSION_NO_HEARTBEAT_REF,
  PYLON_ADMISSION_NO_PAYOUT_TARGET_REF,
  PYLON_ADMISSION_NO_SERVING_LANE_REF,
  PYLON_ADMISSION_POLICY_REF,
  PYLON_ADMISSION_WALLET_NOT_READY_REF,
  type PylonServingSnapshot,
  decidePylonAdmission,
} from './khala-pylon-admission'

const REQUIRED_CAP = 'capability.serving.khala_mini.v1'
const NOW_MS = Date.parse('2026-06-22T18:00:00.000Z')
const FRESH_HEARTBEAT = '2026-06-22T17:59:30.000Z' // 30s ago — fresh
const STALE_HEARTBEAT = '2026-06-22T17:55:00.000Z' // 5 min ago — stale

// A fully-ready, admissible serving snapshot. Individual tests degrade one field.
const readySnapshot = (
  overrides: Partial<PylonServingSnapshot> = {},
): PylonServingSnapshot => ({
  capabilityRefs: [REQUIRED_CAP, 'capability.other.v1'],
  latestHeartbeatAt: FRESH_HEARTBEAT,
  latestHeartbeatStatus: 'ok',
  pylonRef: 'pylon.khala.guinea_pig',
  servingLaneRefs: ['lane.nip90.serving.v1'],
  sparkPayoutTargetRef: 'payout.spark.deadbeef',
  status: 'active',
  walletReady: true,
  ...overrides,
})

const decide = (snapshot: PylonServingSnapshot) =>
  decidePylonAdmission({
    nowMs: NOW_MS,
    requiredCapabilityRef: REQUIRED_CAP,
    snapshot,
  })

describe('decidePylonAdmission — Khala M4 supply-side admission gate', () => {
  it('ADMITS a fully-ready Pylon (active + capability + lane + fresh heartbeat + wallet/payout ready)', () => {
    const decision = decide(readySnapshot())
    expect(decision.admitted).toBe(true)
    expect(decision.blockerRefs).toEqual([])
    expect(decision.policyRefs).toContain(PYLON_ADMISSION_POLICY_REF)
    expect(decision.pylonRef).toBe('pylon.khala.guinea_pig')
    expect(decision.schema).toBe('openagents.khala_pylon_admission.v1')
  })

  it('REFUSES a non-active registration (blocked/retired)', () => {
    for (const status of ['blocked', 'retired'] as const) {
      const decision = decide(readySnapshot({ status }))
      expect(decision.admitted).toBe(false)
      expect(decision.blockerRefs).toContain(PYLON_ADMISSION_NOT_ACTIVE_REF)
    }
  })

  it('REFUSES when the required capability is not advertised', () => {
    const decision = decide(
      readySnapshot({ capabilityRefs: ['capability.other.v1'] }),
    )
    expect(decision.admitted).toBe(false)
    expect(decision.blockerRefs).toContain(
      PYLON_ADMISSION_CAPABILITY_MISSING_REF,
    )
  })

  it('REFUSES when no serving lane is advertised', () => {
    const decision = decide(readySnapshot({ servingLaneRefs: [] }))
    expect(decision.admitted).toBe(false)
    expect(decision.blockerRefs).toContain(PYLON_ADMISSION_NO_SERVING_LANE_REF)
  })

  it('REFUSES when there is no heartbeat at all', () => {
    const decision = decide(readySnapshot({ latestHeartbeatAt: null }))
    expect(decision.admitted).toBe(false)
    expect(decision.blockerRefs).toContain(PYLON_ADMISSION_NO_HEARTBEAT_REF)
  })

  it('REFUSES when the heartbeat is stale (older than the TTL)', () => {
    const decision = decide(readySnapshot({ latestHeartbeatAt: STALE_HEARTBEAT }))
    expect(decision.admitted).toBe(false)
    expect(decision.blockerRefs).toContain(PYLON_ADMISSION_HEARTBEAT_STALE_REF)
  })

  it('REFUSES when the heartbeat status is unhealthy/unknown', () => {
    for (const status of ['draining', 'degraded', null]) {
      const decision = decide(
        readySnapshot({ latestHeartbeatStatus: status }),
      )
      expect(decision.admitted).toBe(false)
      expect(decision.blockerRefs).toContain(
        PYLON_ADMISSION_HEARTBEAT_UNHEALTHY_REF,
      )
    }
  })

  it('REFUSES when the wallet is not receive-ready', () => {
    const decision = decide(readySnapshot({ walletReady: false }))
    expect(decision.admitted).toBe(false)
    expect(decision.blockerRefs).toContain(
      PYLON_ADMISSION_WALLET_NOT_READY_REF,
    )
  })

  it('REFUSES when no Spark payout target is registered', () => {
    for (const ref of [null, '   ']) {
      const decision = decide(readySnapshot({ sparkPayoutTargetRef: ref }))
      expect(decision.admitted).toBe(false)
      expect(decision.blockerRefs).toContain(
        PYLON_ADMISSION_NO_PAYOUT_TARGET_REF,
      )
    }
  })

  it('accumulates ALL blockers for a wholly-unready node (multiple gates fail at once)', () => {
    const decision = decide({
      capabilityRefs: [],
      latestHeartbeatAt: null,
      latestHeartbeatStatus: null,
      pylonRef: 'pylon.unready',
      servingLaneRefs: [],
      sparkPayoutTargetRef: null,
      status: 'retired',
      walletReady: false,
    })
    expect(decision.admitted).toBe(false)
    expect(decision.blockerRefs).toEqual(
      expect.arrayContaining([
        PYLON_ADMISSION_NOT_ACTIVE_REF,
        PYLON_ADMISSION_CAPABILITY_MISSING_REF,
        PYLON_ADMISSION_NO_SERVING_LANE_REF,
        PYLON_ADMISSION_NO_HEARTBEAT_REF,
        PYLON_ADMISSION_WALLET_NOT_READY_REF,
        PYLON_ADMISSION_NO_PAYOUT_TARGET_REF,
      ]),
    )
  })

  it('treats a heartbeat exactly at the TTL boundary as fresh, and just past it as stale', () => {
    const atBoundary = new Date(NOW_MS - DEFAULT_HEARTBEAT_TTL_MS).toISOString()
    const pastBoundary = new Date(
      NOW_MS - DEFAULT_HEARTBEAT_TTL_MS - 1,
    ).toISOString()
    expect(decide(readySnapshot({ latestHeartbeatAt: atBoundary })).admitted).toBe(
      true,
    )
    expect(
      decide(readySnapshot({ latestHeartbeatAt: pastBoundary })).admitted,
    ).toBe(false)
  })

  it('does not treat a future-dated heartbeat (clock skew) as stale', () => {
    const future = new Date(NOW_MS + 10_000).toISOString()
    const decision = decide(readySnapshot({ latestHeartbeatAt: future }))
    expect(decision.admitted).toBe(true)
  })

  it('honors a custom heartbeat TTL', () => {
    // 5-min-old heartbeat is stale under the default but fresh under a 10-min TTL.
    const tight = decidePylonAdmission({
      nowMs: NOW_MS,
      requiredCapabilityRef: REQUIRED_CAP,
      snapshot: readySnapshot({ latestHeartbeatAt: STALE_HEARTBEAT }),
    })
    expect(tight.admitted).toBe(false)
    const loose = decidePylonAdmission({
      heartbeatTtlMs: 600_000,
      nowMs: NOW_MS,
      requiredCapabilityRef: REQUIRED_CAP,
      snapshot: readySnapshot({ latestHeartbeatAt: STALE_HEARTBEAT }),
    })
    expect(loose.admitted).toBe(true)
  })
})
