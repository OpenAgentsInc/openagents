import { describe, expect, test } from "bun:test"

import { planDistribution } from "./distribution-notarize-plan"

describe("distribution notarize plan", () => {
  test("requires notarization for desktop delta builds", () => {
    expect(planDistribution({ target: "desktop", hasPrevBuild: true })).toEqual({
      steps: ["notarize desktop build", "create bsdiff delta"],
      usesBsdiff: true,
      requiresNotarize: true,
    })
  })

  test("uses a full desktop build when no previous build exists", () => {
    expect(planDistribution({ target: "desktop", hasPrevBuild: false })).toEqual({
      steps: ["notarize desktop build", "publish full desktop build"],
      usesBsdiff: false,
      requiresNotarize: true,
    })
  })

  test("does not notarize mobile distribution", () => {
    expect(planDistribution({ target: "mobile", hasPrevBuild: true }).requiresNotarize).toBe(false)
  })

  test("plans mobile store submission steps", () => {
    expect(planDistribution({ target: "mobile", hasPrevBuild: false })).toEqual({
      steps: ["prepare mobile store submission", "submit mobile build to store"],
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
