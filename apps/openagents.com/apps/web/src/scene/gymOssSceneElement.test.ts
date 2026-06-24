import { describe, expect, test } from 'vitest'

import {
  GYM_OSS_SCENE_TAG,
  buildSceneFrame,
  mountGymOssScene,
  registerGymOssSceneElement,
  type SceneFrame,
} from './gymOssSceneElement'

describe('gym-oss scene geometry', () => {
  test('empty frame yields no bars and a zero meter (honest absence)', () => {
    const frame: SceneFrame = { requests: [], aggregateTps: null }
    const geometry = buildSceneFrame(frame, null)
    expect(geometry.bars).toHaveLength(0)
    expect(geometry.meterFraction).toBe(0)
  })

  test('failed request reads as an empty bar with the failed status', () => {
    const frame: SceneFrame = {
      requests: [{ index: 0, status: 'failed', perceivedTps: null }],
      aggregateTps: null,
    }
    const geometry = buildSceneFrame(frame, null)
    expect(geometry.bars[0]?.status).toBe('failed')
    expect(geometry.bars[0]?.fillFraction).toBe(0)
  })

  test('aggregate meter is monotonic against a running ceiling', () => {
    const frame: SceneFrame = {
      requests: [{ index: 0, status: 'ok', perceivedTps: 50 }],
      aggregateTps: 50,
    }
    // With a higher ceiling from a prior frame, the meter reads partial.
    const geometry = buildSceneFrame(frame, 100)
    expect(geometry.meterFraction).toBe(0.5)
  })
})

describe('gym-oss scene element', () => {
  test('registers the custom element under its tag', () => {
    registerGymOssSceneElement()
    expect(customElements.get(GYM_OSS_SCENE_TAG)).toBeDefined()
  })

  test('mount handle pushes frames without a 2D context without throwing', () => {
    // happy-dom canvas has no real 2D context; the handle keeps the latest frame
    // without drawing and must not throw.
    const canvas = document.createElement('canvas')
    const handle = mountGymOssScene(canvas)
    expect(() =>
      handle.push({
        requests: [{ index: 0, status: 'ok', perceivedTps: 50 }],
        aggregateTps: 50,
      }),
    ).not.toThrow()
    handle.dispose()
    // After dispose, pushes are no-ops.
    expect(() => handle.push({ requests: [], aggregateTps: null })).not.toThrow()
  })
})
