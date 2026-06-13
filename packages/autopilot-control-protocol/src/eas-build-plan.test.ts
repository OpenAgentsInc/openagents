import { describe, expect, test } from "bun:test"

import { planEasBuild } from "./eas-build-plan"

describe("eas build plan", () => {
  test("does not build for an OTA update", () => {
    expect(planEasBuild({
      mode: "ota",
      platform: "ios",
      autoSubmit: false,
    })).toEqual({
      steps: [],
      willBuild: false,
      willSubmit: false,
      reason: "ota_no_build",
    })
  })

  test("does not submit OTA updates even when autoSubmit is enabled", () => {
    expect(planEasBuild({
      mode: "ota",
      platform: "android",
      autoSubmit: true,
    })).toEqual({
      steps: [],
      willBuild: false,
      willSubmit: false,
      reason: "ota_no_build",
    })
  })

  test("plans an iOS rebuild without submit", () => {
    expect(planEasBuild({
      mode: "rebuild",
      platform: "ios",
      autoSubmit: false,
    })).toEqual({
      steps: ["eas build --local --platform ios"],
      willBuild: true,
      willSubmit: false,
      reason: "rebuild_required",
    })
  })

  test("plans an Android rebuild without submit", () => {
    expect(planEasBuild({
      mode: "rebuild",
      platform: "android",
      autoSubmit: false,
    })).toEqual({
      steps: ["eas build --local --platform android"],
      willBuild: true,
      willSubmit: false,
      reason: "rebuild_required",
    })
  })

  test("adds submit after an iOS rebuild when autoSubmit is enabled", () => {
    expect(planEasBuild({
      mode: "rebuild",
      platform: "ios",
      autoSubmit: true,
    })).toEqual({
      steps: [
        "eas build --local --platform ios",
        "eas submit -p ios",
      ],
      willBuild: true,
      willSubmit: true,
      reason: "rebuild_required",
    })
  })

  test("adds submit after an Android rebuild when autoSubmit is enabled", () => {
    expect(planEasBuild({
      mode: "rebuild",
      platform: "android",
      autoSubmit: true,
    })).toEqual({
      steps: [
        "eas build --local --platform android",
        "eas submit -p android",
      ],
      willBuild: true,
      willSubmit: true,
      reason: "rebuild_required",
    })
  })
})
