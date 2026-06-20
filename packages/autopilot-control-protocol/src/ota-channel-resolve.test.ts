import { describe, expect, test } from "bun:test"

import { resolveOtaChannel } from "./ota-channel-resolve.js"

describe("OTA channel resolution", () => {
  test("routes production builds to the production channel and branch", () => {
    expect(resolveOtaChannel({
      releaseChannel: null,
      isProduction: true,
    })).toEqual({
      channel: "production",
      branch: "production",
      reason: "production-build",
    })
  })

  test("production builds override explicit release channels", () => {
    expect(resolveOtaChannel({
      releaseChannel: "beta",
      isProduction: true,
    })).toEqual({
      channel: "production",
      branch: "production",
      reason: "production-build",
    })
  })

  test("passes through explicit release channels for non-production builds", () => {
    expect(resolveOtaChannel({
      releaseChannel: "beta",
      isProduction: false,
    })).toEqual({
      channel: "beta",
      branch: "beta",
      reason: "explicit-release-channel",
    })
  })

  test("sanitizes explicit release channels to lowercase channel-safe names", () => {
    expect(resolveOtaChannel({
      releaseChannel: " QA/Smoke_01 ",
      isProduction: false,
    })).toEqual({
      channel: "qa-smoke-01",
      branch: "qa-smoke-01",
      reason: "explicit-release-channel",
    })
  })

  test("defaults non-production builds without a release channel to preview", () => {
    expect(resolveOtaChannel({
      releaseChannel: null,
      isProduction: false,
    })).toEqual({
      channel: "preview",
      branch: "preview",
      reason: "default-preview",
    })
  })

  test("defaults blank sanitized release channels to preview", () => {
    expect(resolveOtaChannel({
      releaseChannel: " !!! ",
      isProduction: false,
    })).toEqual({
      channel: "preview",
      branch: "preview",
      reason: "default-preview",
    })
  })
})
