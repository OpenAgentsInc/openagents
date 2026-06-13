import { describe, expect, test } from "bun:test"

import { buildLocalBuildPlan, requireRebuild } from "../src/coordinator/build-plan"

describe("TAS local build plan", () => {
  test("returns three ordered local iOS build steps with expected argv", () => {
    const plan = buildLocalBuildPlan({
      shipMode: "rebuild",
      platform: "ios",
      profile: "production",
      outPath: "dist/pylon.ipa",
    })

    expect(plan.steps).toEqual([
      {
        name: "bundle pre-check",
        argv: ["expo", "export", "--platform", "ios"],
      },
      {
        name: "local build",
        argv: [
          "eas",
          "build",
          "--platform",
          "ios",
          "--profile",
          "production",
          "--local",
          "--non-interactive",
          "--output",
          "dist/pylon.ipa",
        ],
      },
      {
        name: "submit",
        argv: [
          "eas",
          "submit",
          "--platform",
          "ios",
          "--path",
          "dist/pylon.ipa",
          "--non-interactive",
        ],
      },
    ])
  })

  test("guard rejects non-rebuild ship modes", () => {
    expect(() => requireRebuild("ota")).toThrow("requires rebuild")
    expect(() => requireRebuild("none")).toThrow("requires rebuild")
  })
})
