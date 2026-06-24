import { describe, expect, test } from 'vitest'

import { initGymModel, runGymFixture } from '../page/loggedOut/gym/flow'
import {
  buildGymFixtureSceneGeometry,
  encodeGymFixtureSceneLanes,
  GYM_FIXTURE_RUN_SCENE_TAG,
  registerGymFixtureRunSceneElement,
} from './gymFixtureRunSceneElement'

describe('Gym fixture run scene', () => {
  test('maps fixture lanes to deterministic arcs and honest skipped states', () => {
    const result = runGymFixture(initGymModel()).result

    if (result === null) {
      throw new Error('expected fixture result')
    }

    const geometry = buildGymFixtureSceneGeometry(result.scene)

    expect(geometry.lanes).toHaveLength(3)
    expect(geometry.costMeterFraction).toBeGreaterThan(0)
    expect(geometry.lanes.map(lane => lane.status)).toContain(
      'skipped_unavailable',
    )
    expect(
      geometry.lanes.find(lane => lane.status === 'skipped_unavailable')
        ?.verdictBeam,
    ).toBe(false)
  })

  test('encodes public-safe scene input without raw telemetry records', () => {
    const result = runGymFixture(initGymModel()).result

    if (result === null) {
      throw new Error('expected fixture result')
    }

    const encoded = encodeGymFixtureSceneLanes(result.scene)

    expect(encoded).toContain('test_passed')
    expect(encoded).toContain('skipped_unavailable')
    expect(encoded).not.toContain('prompt')
    expect(encoded).not.toContain('completion')
  })

  test('registers the custom element defensively', () => {
    registerGymFixtureRunSceneElement()

    expect(customElements.get(GYM_FIXTURE_RUN_SCENE_TAG)).toBeDefined()
  })
})
