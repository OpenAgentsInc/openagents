import { describe, expect, test } from 'vitest'

import type { LandingPose } from './landingSquares'
import { POSES } from './landingSquares'

// The persistent landing-squares scene is ONE instance with named camera poses;
// navigating between routes only eases the camera to a new pose. This guards the
// pose table: every route's pose must be defined, non-degenerate, and distinct,
// including the /autopilot onboarding vantage added in #6125.
describe('landing squares camera poses', () => {
  const allPoses: ReadonlyArray<LandingPose> = [
    'landing',
    'khala',
    'tassadar',
    'autopilot',
  ]

  test('defines a distinct, non-blank pose for every route', () => {
    for (const name of allPoses) {
      const pose = POSES[name]
      expect(pose).toBeDefined()
      // Position and target must not be the zero/zero degenerate (a "blank" pose
      // would put the camera at the origin looking at the origin).
      expect(pose.pos.lengthSq() + pose.target.lengthSq()).toBeGreaterThan(0)
    }

    // No two routes may share the same camera position (each is its own vantage).
    const positions = allPoses.map(name => {
      const p = POSES[name].pos
      return `${p.x},${p.y},${p.z}`
    })
    expect(new Set(positions).size).toBe(positions.length)
  })

  test('autopilot is a fresh vantage, not an alias of an existing pose', () => {
    const autopilot = POSES.autopilot
    expect(autopilot).toBeDefined()
    expect(autopilot.pos.lengthSq()).toBeGreaterThan(0)

    for (const other of ['landing', 'khala', 'tassadar'] as const) {
      expect(autopilot.pos.equals(POSES[other].pos)).toBe(false)
    }
  })
})
