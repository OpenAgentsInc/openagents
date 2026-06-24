import { existsSync } from "node:fs"

import { describe, expect, test } from "bun:test"

import {
  findIsolatedSceneDefinition,
  isolatedSceneDefinitions,
  isolatedSceneNames,
} from "../scripts/isolated-scenes/registry"

describe("isolated scene runner registry", () => {
  test("registers the reusable scene convention with at least two scenes", () => {
    expect(isolatedSceneNames()).toEqual(["verse-arc", "pylon-network"])
    expect(isolatedSceneDefinitions).toHaveLength(2)
    expect(isolatedSceneDefinitions.every((definition) => existsSync(definition.entryModulePath))).toBe(true)
  })

  test("pins the issue refs and capture defaults for each scene", () => {
    const arc = findIsolatedSceneDefinition("verse-arc")
    const pylon = findIsolatedSceneDefinition("pylon-network")

    expect(arc?.issueRefs).toContain("github:OpenAgentsInc/openagents#6033")
    expect(pylon?.issueRefs).toContain("github:OpenAgentsInc/openagents#6033")
    expect(arc?.issueRefs).toContain("github:OpenAgentsInc/openagents#6047")
    expect(pylon?.issueRefs).toContain("github:OpenAgentsInc/openagents#6047")
    expect(arc?.defaultFrameSteps).toBeGreaterThan(pylon?.defaultFrameSteps ?? 0)
    expect(pylon?.defaultWidth).toBe(960)
    expect(arc?.renderSignature.minBrightPixels).toBeGreaterThan(0)
  })
})
