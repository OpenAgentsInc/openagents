import { describe, expect, test } from "bun:test"

import { planShipPipeline } from "./ship-pipeline-plan.js"

describe("ship pipeline plan", () => {
  test("publishes an iOS OTA update for JavaScript changes on the published fingerprint", () => {
    expect(planShipPipeline({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: false,
      platform: "ios",
      autoSubmit: false,
    })).toEqual({
      action: "ota",
      steps: ["apps/oa-updates/scripts/publish-ota.sh"],
      reason: "JavaScript changes are OTA-eligible for the published native fingerprint",
    })
  })

  test("publishes an Android OTA update without submit even when autoSubmit is enabled", () => {
    expect(planShipPipeline({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: false,
      platform: "android",
      autoSubmit: true,
    })).toEqual({
      action: "ota",
      steps: ["apps/oa-updates/scripts/publish-ota.sh"],
      reason: "JavaScript changes are OTA-eligible for the published native fingerprint",
    })
  })

  test("plans an iOS rebuild without submit when native changes are present", () => {
    expect(planShipPipeline({
      currentFingerprint: "native.fp.0002",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: true,
      platform: "ios",
      autoSubmit: false,
    })).toEqual({
      action: "rebuild",
      steps: ["clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh --build-only"],
      reason: "native changes require a rebuild",
    })
  })

  test("plans an Android rebuild and submit when the fingerprint is unpublished", () => {
    expect(planShipPipeline({
      currentFingerprint: "native.fp.0002",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: false,
      platform: "android",
      autoSubmit: true,
    })).toEqual({
      action: "rebuild",
      steps: [
        "android local release build is not configured; add a signed Gradle bundle path before shipping Android",
      ],
      reason: "current native fingerprint has not been published",
    })
  })

  test("plans a rebuild when there is no last published fingerprint", () => {
    expect(planShipPipeline({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: null,
      hasJsChanges: true,
      hasNativeChanges: false,
      platform: "ios",
      autoSubmit: true,
    })).toEqual({
      action: "rebuild",
      steps: ["clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh"],
      reason: "current native fingerprint has not been published",
    })
  })

  test("returns noop when the fingerprint is published and no JavaScript changed", () => {
    expect(planShipPipeline({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: false,
      hasNativeChanges: false,
      platform: "android",
      autoSubmit: true,
    })).toEqual({
      action: "noop",
      steps: [],
      reason: "no JavaScript changes to publish",
    })
  })

  test("native changes take precedence over a matching published fingerprint", () => {
    expect(planShipPipeline({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: true,
      platform: "ios",
      autoSubmit: true,
    })).toEqual({
      action: "rebuild",
      steps: ["clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh"],
      reason: "native changes require a rebuild",
    })
  })
})
