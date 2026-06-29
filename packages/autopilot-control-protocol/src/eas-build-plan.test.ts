import { describe, expect, test } from "bun:test"

import { planEasBuild } from "./eas-build-plan.js"

describe("local mobile build plan", () => {
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
      steps: ["clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh --build-only"],
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
      steps: [
        "android local release build is not configured; add a signed Gradle bundle path before shipping Android",
      ],
      willBuild: false,
      willSubmit: false,
      reason: "android_local_release_not_configured",
    })
  })

  test("adds submit after an iOS rebuild when autoSubmit is enabled", () => {
    expect(planEasBuild({
      mode: "rebuild",
      platform: "ios",
      autoSubmit: true,
    })).toEqual({
      steps: ["clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh"],
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
        "android local release build is not configured; add a signed Gradle bundle path before shipping Android",
      ],
      willBuild: false,
      willSubmit: false,
      reason: "android_local_release_not_configured",
    })
  })
})
