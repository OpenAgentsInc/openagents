import { describe, expect, test } from "bun:test"

import {
  buildOtaPublishPlan,
  requireOtaEligible,
} from "../src/coordinator/ota-publish-plan"

describe("OTA publish plan", () => {
  test("builds a well-formed eas update argv for branch and platform", () => {
    const plan = buildOtaPublishPlan({
      shipMode: "ota",
      branch: "preview",
      platform: "ios",
      message: "Ship M6 CL-38",
    })

    expect(plan.argv).toEqual([
      "eas",
      "update",
      "--branch",
      "preview",
      "--platform",
      "ios",
      "--message",
      "Ship M6 CL-38",
      "--non-interactive",
      "--environment",
      "preview",
    ])
  })

  test("includes android platform when requested", () => {
    const plan = buildOtaPublishPlan({
      shipMode: "ota",
      branch: "production",
      platform: "android",
      message: "Ship Android OTA",
    })

    expect(plan.argv).toContain("android")
    expect(plan.argv).toContain("production")
  })

  test("guard rejects non-ota ship modes", () => {
    expect(() => requireOtaEligible("rebuild")).toThrow(
      "OTA publish requires ship mode ota; received rebuild",
    )
    expect(() => requireOtaEligible("none")).toThrow(
      "OTA publish requires ship mode ota; received none",
    )
  })

  test("returns the OTA publish receipt shape", () => {
    const plan = buildOtaPublishPlan({
      shipMode: "ota",
      branch: "preview",
      platform: "ios",
      message: "Ship M6 CL-38",
    })

    expect(plan.receipt).toEqual({
      kind: "ota_publish",
      branch: "preview",
      platform: "ios",
    })
  })
})
