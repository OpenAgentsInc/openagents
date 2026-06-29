import { describe, expect, test } from "bun:test"

import { decideOtaPublish } from "./ota-publish-eligibility.js"

describe("OTA publish eligibility", () => {
  test("requests a rebuild when native changes are present", () => {
    expect(decideOtaPublish({
      currentFingerprint: "native.fp.0002",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: true,
    })).toEqual({
      publish: false,
      mode: "rebuild",
      reason: "native changes require a rebuild",
    })
  })

  test("native changes take precedence even when fingerprints match", () => {
    expect(decideOtaPublish({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: true,
    })).toEqual({
      publish: false,
      mode: "rebuild",
      reason: "native changes require a rebuild",
    })
  })

  test("returns noop when fingerprints match and there are no JavaScript changes", () => {
    expect(decideOtaPublish({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: false,
      hasNativeChanges: false,
    })).toEqual({
      publish: false,
      mode: "noop",
      reason: "no JavaScript changes to publish",
    })
  })

  test("publishes OTA when JavaScript changed and the native fingerprint matches", () => {
    expect(decideOtaPublish({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: false,
    })).toEqual({
      publish: true,
      mode: "ota",
      reason: "JavaScript changes are OTA-eligible for the published native fingerprint",
    })
  })

  test("requests a rebuild when JavaScript changed but the native fingerprint differs", () => {
    expect(decideOtaPublish({
      currentFingerprint: "native.fp.0002",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: true,
      hasNativeChanges: false,
    })).toEqual({
      publish: false,
      mode: "rebuild",
      reason: "current native fingerprint has not been published",
    })
  })

  test("requests a rebuild when there is no published native fingerprint", () => {
    expect(decideOtaPublish({
      currentFingerprint: "native.fp.0001",
      lastPublishedFingerprint: null,
      hasJsChanges: true,
      hasNativeChanges: false,
    })).toEqual({
      publish: false,
      mode: "rebuild",
      reason: "current native fingerprint has not been published",
    })
  })

  test("requests a rebuild when the fingerprint changed without a native-change flag", () => {
    expect(decideOtaPublish({
      currentFingerprint: "native.fp.0002",
      lastPublishedFingerprint: "native.fp.0001",
      hasJsChanges: false,
      hasNativeChanges: false,
    })).toEqual({
      publish: false,
      mode: "rebuild",
      reason: "current native fingerprint has not been published",
    })
  })
})
