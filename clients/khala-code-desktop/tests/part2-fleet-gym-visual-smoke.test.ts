import { describe, expect, test } from "bun:test"

import {
  assertPart2VisualGeometry,
  PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS,
  part2FleetGymVisualPlan,
} from "../scripts/part2-fleet-gym-visual-smoke"

describe("Part 2 Fleet/Gym visual smoke", () => {
  test("covers desktop and mobile Fleet to Gym proof loading", () => {
    expect(PART2_FLEET_GYM_VISUAL_SMOKE_HARNESS).toBe(
      "khala_code_part2_fleet_gym_visual_smoke",
    )
    expect(part2FleetGymVisualPlan()).toEqual([
      { name: "desktop", width: 1280, height: 800 },
      { name: "mobile", width: 390, height: 844 },
    ])
  })

  test("accepts stacked proof and parameter geometry and rejects overlap", () => {
    assertPart2VisualGeometry({
      graph: { x: 32, y: 220, width: 640, height: 240 },
      gymPanel: { x: 0, y: 0, width: 780, height: 800 },
      loadedState: { x: 24, y: 120, width: 720, height: 360 },
      parameters: { x: 24, y: 500, width: 720, height: 160 },
      viewport: { x: 0, y: 0, width: 1280, height: 800 },
    })

    expect(() =>
      assertPart2VisualGeometry({
        graph: { x: 32, y: 220, width: 640, height: 240 },
        gymPanel: { x: 0, y: 0, width: 780, height: 800 },
        loadedState: { x: 24, y: 120, width: 720, height: 360 },
        parameters: { x: 24, y: 320, width: 720, height: 160 },
        viewport: { x: 0, y: 0, width: 1280, height: 800 },
      }),
    ).toThrow("overlap")

    expect(() =>
      assertPart2VisualGeometry({
        graph: { x: 32, y: 220, width: 640, height: 240 },
        gymPanel: { x: 0, y: 0, width: 780, height: 800 },
        loadedState: { x: 24, y: 120, width: 180, height: 360 },
        parameters: { x: 24, y: 500, width: 720, height: 160 },
        viewport: { x: 0, y: 0, width: 1280, height: 800 },
      }),
    ).toThrow("too narrow")
  })
})
