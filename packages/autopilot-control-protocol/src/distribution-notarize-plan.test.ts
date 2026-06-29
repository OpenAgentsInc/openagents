import { describe, expect, test } from "bun:test"

import { planDistribution } from "./distribution-notarize-plan.js"

describe("distribution notarize plan", () => {
  test("requires notarization for desktop delta builds", () => {
    expect(planDistribution({ target: "desktop", hasPrevBuild: true })).toEqual({
      steps: [
        "sign and notarize desktop build",
        "publish full desktop artifact",
        "create bsdiff delta",
        "publish desktop update feed",
      ],
      usesBsdiff: true,
      requiresNotarize: true,
    })
  })

  test("uses a full desktop build when no previous build exists", () => {
    expect(planDistribution({ target: "desktop", hasPrevBuild: false })).toEqual({
      steps: [
        "sign and notarize desktop build",
        "publish full desktop artifact",
        "publish desktop update feed",
      ],
      usesBsdiff: false,
      requiresNotarize: true,
    })
  })

  test("does not notarize mobile distribution", () => {
    expect(planDistribution({ target: "mobile", hasPrevBuild: true }).requiresNotarize).toBe(false)
  })

  test("plans mobile store submission steps", () => {
    expect(planDistribution({ target: "mobile", hasPrevBuild: false })).toEqual({
      steps: [
        "build and upload local iOS binary to TestFlight",
        "submit owner-approved build to App Store review",
      ],
      usesBsdiff: false,
      requiresNotarize: false,
    })
  })

  test("does not use bsdiff for mobile even with a previous build", () => {
    expect(planDistribution({ target: "mobile", hasPrevBuild: true }).usesBsdiff).toBe(false)
  })

  test("plans ota publish steps", () => {
    expect(planDistribution({ target: "ota", hasPrevBuild: false })).toEqual({
      steps: ["prepare ota bundle", "publish ota update"],
      usesBsdiff: false,
      requiresNotarize: false,
    })
  })

  test("does not use bsdiff for ota even with a previous build", () => {
    expect(planDistribution({ target: "ota", hasPrevBuild: true }).usesBsdiff).toBe(false)
  })
})
