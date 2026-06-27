import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'vitest'

import {
  classifyFleetBurnState,
  FLEET_BURN_HEALTHY_REASON_REF,
  FLEET_BURN_IDLE_REASON_REF,
  FLEET_BURN_STALL_THRESHOLD_TOKENS,
  FLEET_BURN_STALLED_REASON_REF,
  readFleetWatchdogConfig,
  runFleetBurnStallDetectorTick,
  type FleetWatchdogStore,
} from './fleet-burn-stall-detector'

describe('classifyFleetBurnState', () => {
  test('stalled: burn below threshold WHILE active work exists -> alert', () => {
    const result = classifyFleetBurnState({
      activeCodingAssignments: 4,
      burnTokensInWindow: 0,
      queuedCodingAssignments: 0,
      stallThresholdTokens: FLEET_BURN_STALL_THRESHOLD_TOKENS,
    })

    expect(result.status).toBe('stalled')
    expect(result.reasonRef).toBe(FLEET_BURN_STALLED_REASON_REF)
    expect(result.hasActiveWork).toBe(true)
    expect(result.belowThreshold).toBe(true)
  })

  test('stalled: queued-only work also counts as work', () => {
    const result = classifyFleetBurnState({
      activeCodingAssignments: 0,
      burnTokensInWindow: 12_345,
      queuedCodingAssignments: 3,
      stallThresholdTokens: FLEET_BURN_STALL_THRESHOLD_TOKENS,
    })

    expect(result.status).toBe('stalled')
  })

  test('idle_no_work: burn below threshold but NO work -> no alert', () => {
    const result = classifyFleetBurnState({
      activeCodingAssignments: 0,
      burnTokensInWindow: 0,
      queuedCodingAssignments: 0,
      stallThresholdTokens: FLEET_BURN_STALL_THRESHOLD_TOKENS,
    })

    expect(result.status).toBe('idle_no_work')
    expect(result.reasonRef).toBe(FLEET_BURN_IDLE_REASON_REF)
    expect(result.hasActiveWork).toBe(false)
  })

  test('healthy: burn at/above threshold -> no alert even with work', () => {
    const result = classifyFleetBurnState({
      activeCodingAssignments: 8,
      burnTokensInWindow: FLEET_BURN_STALL_THRESHOLD_TOKENS + 1,
      queuedCodingAssignments: 0,
      stallThresholdTokens: FLEET_BURN_STALL_THRESHOLD_TOKENS,
    })

    expect(result.status).toBe('healthy')
    expect(result.reasonRef).toBe(FLEET_BURN_HEALTHY_REASON_REF)
    expect(result.belowThreshold).toBe(false)
  })

  test('healthy: burn exactly at threshold is not below threshold', () => {
    const result = classifyFleetBurnState({
      activeCodingAssignments: 8,
      burnTokensInWindow: FLEET_BURN_STALL_THRESHOLD_TOKENS,
      queuedCodingAssignments: 0,
      stallThresholdTokens: FLEET_BURN_STALL_THRESHOLD_TOKENS,
    })

    expect(result.status).toBe('healthy')
  })

  test('negative / fractional inputs are clamped to non-negative ints', () => {
    const result = classifyFleetBurnState({
      activeCodingAssignments: -3,
      burnTokensInWindow: -100,
      queuedCodingAssignments: 2.9,
      stallThresholdTokens: FLEET_BURN_STALL_THRESHOLD_TOKENS,
    })

    expect(result.burnTokensInWindow).toBe(0)
    expect(result.activeCodingAssignments).toBe(0)
    expect(result.queuedCodingAssignments).toBe(2)
    // burn 0 < threshold, work present (queued 2) -> stalled
    expect(result.status).toBe('stalled')
  })
})

describe('readFleetWatchdogConfig', () => {
  test('detection + recovery default ON; sensible window/threshold defaults', () => {
    const config = readFleetWatchdogConfig({})
    expect(config.enabled).toBe(true)
    expect(config.recoveryEnabled).toBe(true)
    expect(config.windowMinutes).toBe(5)
    expect(config.stallThresholdTokens).toBe(FLEET_BURN_STALL_THRESHOLD_TOKENS)
    expect(config.staleLeaseMinAgeMinutes).toBe(10)
    expect(config.ownerPylonRefs).toEqual([])
  })

  test('owner pylon refs parse from comma/space lists and dedupe', () => {
    const config = readFleetWatchdogConfig({
      FLEET_WATCHDOG_OWNER_PYLON_REFS: 'pylon.aaa, pylon.bbb pylon.aaa',
    })
    expect(config.ownerPylonRefs).toEqual(['pylon.aaa', 'pylon.bbb'])
  })

  test('flags can be turned off and numbers overridden', () => {
    const config = readFleetWatchdogConfig({
      FLEET_WATCHDOG_ENABLED: 'false',
      FLEET_WATCHDOG_RECOVERY_ENABLED: 'false',
      FLEET_WATCHDOG_STALL_THRESHOLD_TOKENS: '500000',
      FLEET_WATCHDOG_WINDOW_MINUTES: '3',
    })
    expect(config.enabled).toBe(false)
    expect(config.recoveryEnabled).toBe(false)
    expect(config.stallThresholdTokens).toBe(500_000)
    expect(config.windowMinutes).toBe(3)
  })
})

const baseConfig = {
  enabled: true,
  ownerPylonRefs: ['pylon.owner'] as ReadonlyArray<string>,
  recoveryEnabled: true,
  staleLeaseMinAgeMinutes: 10,
  stallThresholdTokens: FLEET_BURN_STALL_THRESHOLD_TOKENS,
  windowMinutes: 5,
}

const NOW_ISO = '2026-06-27T12:00:00.000Z'

const makeStore = (
  overrides: Partial<FleetWatchdogStore> & {
    burn: number
    active: number
    queued?: number
    flushed?: ReadonlyArray<string>
  },
): {
  store: FleetWatchdogStore
  inserted: Array<unknown>
  flushCalls: Array<{ pylonRefs: ReadonlyArray<string>; staleBeforeIso: string }>
} => {
  const inserted: Array<unknown> = []
  const flushCalls: Array<{
    pylonRefs: ReadonlyArray<string>
    staleBeforeIso: string
  }> = []
  const store: FleetWatchdogStore = {
    countActiveCodingAssignments: async () => ({
      active: overrides.active,
      queued: overrides.queued ?? 0,
    }),
    flushAbandonedLeases: async (pylonRefs, _nowIso, staleBeforeIso) => {
      flushCalls.push({ pylonRefs, staleBeforeIso })
      return overrides.flushed ?? []
    },
    insertAlert: async record => {
      inserted.push(record)
    },
    sumOwnCapacityBurnSince: async () => overrides.burn,
    ...overrides,
  }
  return { flushCalls, inserted, store }
}

describe('runFleetBurnStallDetectorTick', () => {
  test('healthy fleet: no alert row, no flush', async () => {
    const { flushCalls, inserted, store } = makeStore({
      active: 8,
      burn: FLEET_BURN_STALL_THRESHOLD_TOKENS + 10,
    })
    const result = await runFleetBurnStallDetectorTick({
      config: baseConfig,
      makeId: () => 'id-1',
      nowIso: NOW_ISO,
      store,
    })
    expect(result.classification.status).toBe('healthy')
    expect(result.alertRef).toBeNull()
    expect(inserted).toHaveLength(0)
    expect(flushCalls).toHaveLength(0)
  })

  test('idle, no work: no alert row, no flush', async () => {
    const { inserted, flushCalls, store } = makeStore({ active: 0, burn: 0 })
    const result = await runFleetBurnStallDetectorTick({
      config: baseConfig,
      makeId: () => 'id-2',
      nowIso: NOW_ISO,
      store,
    })
    expect(result.classification.status).toBe('idle_no_work')
    expect(inserted).toHaveLength(0)
    expect(flushCalls).toHaveLength(0)
  })

  test('stalled with work: writes alert + flushes abandoned leases for owner pylon', async () => {
    const logs: Array<{ line: string; fields: Record<string, unknown> }> = []
    const { inserted, flushCalls, store } = makeStore({
      active: 5,
      burn: 0,
      flushed: ['assignment.a', 'assignment.b'],
    })
    const result = await runFleetBurnStallDetectorTick({
      config: baseConfig,
      log: (line, fields) => logs.push({ fields, line }),
      makeId: () => 'deadbeefcafef00d',
      nowIso: NOW_ISO,
      store,
    })

    expect(result.classification.status).toBe('stalled')
    expect(result.alertRef).not.toBeNull()
    expect(result.recoveredLeaseRefs).toEqual(['assignment.a', 'assignment.b'])
    expect(inserted).toHaveLength(1)
    expect(flushCalls).toHaveLength(1)
    expect(flushCalls[0]!.pylonRefs).toEqual(['pylon.owner'])
    // stale cutoff is 10 minutes before now
    expect(flushCalls[0]!.staleBeforeIso).toBe('2026-06-27T11:50:00.000Z')
    expect(
      result.recoveryActions.some(a =>
        a.startsWith('recovery.flushed_abandoned_leases.count=2'),
      ),
    ).toBe(true)
    expect(logs.some(l => l.line.includes('FLEET-STALL'))).toBe(true)
  })

  test('stalled but no owner pylon refs configured: alert with recovery skipped', async () => {
    const { inserted, flushCalls, store } = makeStore({ active: 3, burn: 0 })
    const result = await runFleetBurnStallDetectorTick({
      config: { ...baseConfig, ownerPylonRefs: [] },
      makeId: () => 'id-4',
      nowIso: NOW_ISO,
      store,
    })
    expect(result.classification.status).toBe('stalled')
    expect(inserted).toHaveLength(1)
    expect(flushCalls).toHaveLength(0)
    expect(result.recoveryActions).toContain(
      'recovery.skipped.no_owner_pylon_refs_configured',
    )
  })

  test('stalled but recovery disabled: alert recorded, no flush', async () => {
    const { flushCalls, store } = makeStore({ active: 3, burn: 0 })
    const result = await runFleetBurnStallDetectorTick({
      config: { ...baseConfig, recoveryEnabled: false },
      makeId: () => 'id-5',
      nowIso: NOW_ISO,
      store,
    })
    expect(flushCalls).toHaveLength(0)
    expect(result.recoveryActions).toContain('recovery.skipped.disabled')
  })

  test('disabled watchdog: returns inert, no store reads', async () => {
    let reads = 0
    const { store } = makeStore({ active: 5, burn: 0 })
    const wrapped: FleetWatchdogStore = {
      ...store,
      sumOwnCapacityBurnSince: async since => {
        reads += 1
        return store.sumOwnCapacityBurnSince(since)
      },
    }
    const result = await runFleetBurnStallDetectorTick({
      config: { ...baseConfig, enabled: false },
      makeId: () => 'id-6',
      nowIso: NOW_ISO,
      store: wrapped,
    })
    expect(result.enabled).toBe(false)
    expect(result.alertRef).toBeNull()
    expect(reads).toBe(0)
  })
})

describe('fleet_alerts migration shape', () => {
  test('migration 0247 creates fleet_alerts with indexes and columns', () => {
    const migration = readFileSync(
      new URL('../../migrations/0247_fleet_alerts.sql', import.meta.url),
      'utf8',
    )
    expect(migration).toContain('CREATE TABLE IF NOT EXISTS fleet_alerts')
    expect(migration).toContain('classification TEXT NOT NULL')
    expect(migration).toContain('reason_ref TEXT NOT NULL')
    expect(migration).toContain('recovery_actions_json TEXT NOT NULL')
    expect(migration).toContain('idx_fleet_alerts_detected_at')
    expect(migration).toContain('idx_fleet_alerts_classification_detected')
  })
})
