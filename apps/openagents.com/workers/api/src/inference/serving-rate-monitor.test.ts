import { describe, expect, test } from 'vitest'

import {
  classifyServingRateMonitor,
  readServingRateMonitorConfig,
  runServingRateMonitorTick,
  SERVING_RATE_GLM_DOWN_REASON_REF,
  SERVING_RATE_LOW_REASON_REF,
  SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
  type ServingRateMonitorStore,
} from './serving-rate-monitor'

const NOW_ISO = '2026-06-29T15:00:00.000Z'

const makeStore = (
  input: Partial<ServingRateMonitorStore> &
    Readonly<{
      latestHeartbeatAt?: string | null
      recentAlert?: boolean
      tokensLastHour: number
    }>,
): Readonly<{
  inserted: Array<unknown>
  store: ServingRateMonitorStore
}> => {
  const inserted: Array<unknown> = []
  const { latestHeartbeatAt, recentAlert, tokensLastHour, ...overrides } = input
  const store: ServingRateMonitorStore = {
    hasRecentAlert: async () => recentAlert ?? false,
    insertAlert: async record => {
      inserted.push(record)
    },
    latestHealthyGlmHeartbeatAt: async () =>
      latestHeartbeatAt === undefined ? NOW_ISO : latestHeartbeatAt,
    tokensSince: async () => tokensLastHour,
    ...overrides,
  }
  return { inserted, store }
}

const config = {
  cadenceMinutes: 5,
  enabled: true,
  glmDownMinutes: 15,
  tokenFloorPerHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
}

describe('classifyServingRateMonitor', () => {
  test('healthy when hourly tokens meet floor and GLM heartbeat is fresh', () => {
    const result = classifyServingRateMonitor({
      glmDownMinutes: 15,
      latestHealthyGlmHeartbeatAt: '2026-06-29T14:59:00.000Z',
      nowIso: NOW_ISO,
      tokenFloorPerHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
      tokensLastHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
    })

    expect(result.status).toBe('healthy')
    expect(result.alertKinds).toEqual([])
    expect(result.reasonRefs).toEqual([])
  })

  test('alerts when aggregate tokens/hour drops below the configured floor', () => {
    const result = classifyServingRateMonitor({
      glmDownMinutes: 15,
      latestHealthyGlmHeartbeatAt: '2026-06-29T14:59:00.000Z',
      nowIso: NOW_ISO,
      tokenFloorPerHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
      tokensLastHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR - 1,
    })

    expect(result.status).toBe('serving_rate_low')
    expect(result.alertKinds).toEqual(['serving_rate_low'])
    expect(result.reasonRefs).toContain(SERVING_RATE_LOW_REASON_REF)
  })

  test('alerts when no healthy GLM heartbeat is inside the down window', () => {
    const result = classifyServingRateMonitor({
      glmDownMinutes: 15,
      latestHealthyGlmHeartbeatAt: '2026-06-29T14:44:59.000Z',
      nowIso: NOW_ISO,
      tokenFloorPerHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
      tokensLastHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR + 1,
    })

    expect(result.status).toBe('glm_down')
    expect(result.alertKinds).toEqual(['glm_down'])
    expect(result.reasonRefs).toContain(SERVING_RATE_GLM_DOWN_REASON_REF)
  })

  test('returns both alert kinds when serving rate and GLM health are bad', () => {
    const result = classifyServingRateMonitor({
      glmDownMinutes: 15,
      latestHealthyGlmHeartbeatAt: null,
      nowIso: NOW_ISO,
      tokenFloorPerHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
      tokensLastHour: 0,
    })

    expect(result.status).toBe('glm_down')
    expect(result.alertKinds).toEqual(['serving_rate_low', 'glm_down'])
  })
})

describe('readServingRateMonitorConfig', () => {
  test('defaults to a 5 minute Cloudflare cron cadence and 50M/hr floor', () => {
    const result = readServingRateMonitorConfig({})

    expect(result).toEqual({
      cadenceMinutes: 5,
      enabled: true,
      glmDownMinutes: 15,
      tokenFloorPerHour: 50_000_000,
    })
  })

  test('parses deployment overrides', () => {
    const result = readServingRateMonitorConfig({
      SERVING_RATE_MONITOR_CADENCE_MINUTES: '10',
      SERVING_RATE_MONITOR_ENABLED: 'false',
      SERVING_RATE_MONITOR_GLM_DOWN_MINUTES: '20',
      SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR: '123',
    })

    expect(result).toEqual({
      cadenceMinutes: 10,
      enabled: false,
      glmDownMinutes: 20,
      tokenFloorPerHour: 123,
    })
  })
})

describe('runServingRateMonitorTick', () => {
  test('healthy tick writes no alert rows', async () => {
    const { inserted, store } = makeStore({
      tokensLastHour: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR + 1,
    })

    const result = await runServingRateMonitorTick({
      config,
      makeId: () => 'alert-id-1',
      nowIso: NOW_ISO,
      store,
    })

    expect(result.classification.status).toBe('healthy')
    expect(result.alertRefs).toEqual([])
    expect(inserted).toEqual([])
  })

  test('low serving rate writes one public-safe alert row', async () => {
    const logs: Array<unknown> = []
    const { inserted, store } = makeStore({
      tokensLastHour: 10,
    })

    const result = await runServingRateMonitorTick({
      config,
      log: (line, fields) => logs.push({ fields, line }),
      makeId: () => 'feedfacecafebeef',
      nowIso: NOW_ISO,
      store,
    })

    expect(result.alertRefs).toHaveLength(1)
    expect(inserted).toHaveLength(1)
    expect(inserted[0]).toMatchObject({
      burnTokensWindow: 10,
      classification: 'serving_rate_low',
      reasonRef: SERVING_RATE_LOW_REASON_REF,
      stallThresholdTokens: SERVING_RATE_MONITOR_TOKEN_FLOOR_PER_HOUR,
      windowMinutes: 60,
    })
    expect(logs).toHaveLength(1)
  })

  test('stale GLM plus low rate writes both alert rows', async () => {
    const { inserted, store } = makeStore({
      latestHeartbeatAt: null,
      tokensLastHour: 0,
    })

    const result = await runServingRateMonitorTick({
      config,
      makeId: () => `id-${inserted.length}`,
      nowIso: NOW_ISO,
      store,
    })

    expect(result.classification.alertKinds).toEqual([
      'serving_rate_low',
      'glm_down',
    ])
    expect(result.alertRefs).toHaveLength(2)
    expect(
      inserted.map(row => (row as { classification: string }).classification),
    ).toEqual(['serving_rate_low', 'glm_down'])
  })

  test('recent same-kind alert suppresses duplicate writes', async () => {
    const { inserted, store } = makeStore({
      recentAlert: true,
      tokensLastHour: 0,
    })

    const result = await runServingRateMonitorTick({
      config,
      makeId: () => 'id-duplicate',
      nowIso: NOW_ISO,
      store,
    })

    expect(result.classification.status).toBe('serving_rate_low')
    expect(result.alertRefs).toEqual([])
    expect(inserted).toEqual([])
  })
})
