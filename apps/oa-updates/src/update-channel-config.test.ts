import { describe, expect, test } from "bun:test"

import {
  buildUpdatesUrl,
  parseChannelFromHeaders,
  resolveBranchForChannel,
} from "./update-channel-config.ts"

describe("update channel config", () => {
  test("buildUpdatesUrl builds the manifest URL", () => {
    expect(
      buildUpdatesUrl({
        host: "updates.openagents.test",
        owner: "autopilot",
      }),
    ).toBe("https://updates.openagents.test/autopilot/manifest")
  })

  test("buildUpdatesUrl normalizes host and owner slashes", () => {
    expect(
      buildUpdatesUrl({
        host: "https://updates.openagents.test/",
        owner: "/autopilot/",
      }),
    ).toBe("https://updates.openagents.test/autopilot/manifest")
  })

  test("buildUpdatesUrl encodes owner as a path segment", () => {
    expect(
      buildUpdatesUrl({
        host: "updates.openagents.test",
        owner: "owner with spaces",
      }),
    ).toBe("https://updates.openagents.test/owner%20with%20spaces/manifest")
  })

  test("parseChannelFromHeaders reads Expo headers case-insensitively", () => {
    expect(
      parseChannelFromHeaders({
        "Expo-Channel-Name": "production",
        "EXPO-RUNTIME-VERSION": "1.0.0",
        "expo-platform": "ios",
      }),
    ).toEqual({
      channel: "production",
      runtimeVersion: "1.0.0",
      platform: "ios",
    })
  })

  test("parseChannelFromHeaders returns nulls for missing headers", () => {
    expect(parseChannelFromHeaders({})).toEqual({
      channel: null,
      runtimeVersion: null,
      platform: null,
    })
  })

  test("parseChannelFromHeaders trims blank values to null", () => {
    expect(
      parseChannelFromHeaders({
        "expo-channel-name": " ",
        "expo-runtime-version": " 1.0.0 ",
        "expo-platform": "",
      }),
    ).toEqual({
      channel: null,
      runtimeVersion: "1.0.0",
      platform: null,
    })
  })

  test("resolveBranchForChannel maps known channels", () => {
    expect(
      resolveBranchForChannel("production", {
        production: "main",
        preview: "release-candidate",
      }),
    ).toBe("main")
  })

  test("resolveBranchForChannel defaults to channel identity", () => {
    expect(resolveBranchForChannel("staging", { production: "main" })).toBe(
      "staging",
    )
  })

  test("resolveBranchForChannel ignores inherited map keys", () => {
    const map = Object.create({ staging: "preview" }) as Record<string, string>

    expect(resolveBranchForChannel("staging", map)).toBe("staging")
  })
})
