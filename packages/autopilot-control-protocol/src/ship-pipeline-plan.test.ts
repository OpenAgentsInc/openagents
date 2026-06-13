import { describe, expect, test } from "bun:test"

import { planShipPipeline } from "./ship-pipeline-plan"

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
      steps: ["eas update --platform ios --non-interactive"],
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
      steps: ["eas update --platform android --non-interactive"],
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
      steps: ["eas build --local --platform ios"],
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
        "eas build --local --platform android",
        "eas submit -p android",
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
      steps: [
        "eas build --local --platform ios",
        "eas submit -p ios",
      ],
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
      steps: [
        "eas build --local --platform ios",
        "eas submit -p ios",
      ],
      reason: "native changes require a rebuild",
    })
  })
})
