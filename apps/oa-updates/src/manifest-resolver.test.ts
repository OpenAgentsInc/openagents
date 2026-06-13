import { describe, expect, test } from "bun:test"

import {
  parseManifestRequest,
  resolveManifest,
  type Asset,
  type Update,
} from "./manifest-resolver.ts"

const launchAsset: Asset = {
  hash: "launch-hash",
  key: "bundle",
  contentType: "application/javascript",
  fileExtension: ".js",
  url: "https://updates.openagents.test/bundle.js",
}

const makeUpdate = (overrides: Partial<Update> = {}): Update => ({
  id: "update-1",
  platform: "ios",
  branch: "production",
  runtimeVersion: "1.0.0",
  createdAt: "2026-06-13T10:00:00.000Z",
  launchAsset,
  assets: [],
  metadata: {},
  extra: {},
  ...overrides,
})

describe("manifest resolver", () => {
  test("latest-wins", () => {
    const older = makeUpdate({
      id: "older",
      createdAt: "2026-06-13T10:00:00.000Z",
    })
    const newer = makeUpdate({
      id: "newer",
      createdAt: "2026-06-13T11:00:00.000Z",
    })

    const result = resolveManifest({
      updates: [newer, older],
      channelToBranch: { production: "production" },
      request: {
        platform: "ios",
        runtimeVersion: "1.0.0",
        channelName: "production",
      },
    })

    expect(result.kind).toBe("manifest")
    if (result.kind === "manifest") {
      expect(result.manifest.id).toBe("newer")
    }
  })

  test("runtimeVersion mismatch returns noUpdateAvailable", () => {
    const result = resolveManifest({
      updates: [makeUpdate()],
      channelToBranch: { production: "production" },
      request: {
        platform: "ios",
        runtimeVersion: "2.0.0",
        channelName: "production",
      },
    })

    expect(result).toMatchObject({
      kind: "directive",
      directive: { type: "noUpdateAvailable" },
    })
  })

  test("channel->branch routing", () => {
    const production = makeUpdate({ id: "production", branch: "production" })
    const preview = makeUpdate({ id: "preview", branch: "preview" })

    const result = resolveManifest({
      updates: [production, preview],
      channelToBranch: { beta: "preview" },
      request: {
        platform: "ios",
        runtimeVersion: "1.0.0",
        channelName: "beta",
      },
    })

    expect(result.kind).toBe("manifest")
    if (result.kind === "manifest") {
      expect(result.manifest.id).toBe("preview")
    }
  })

  test("rollback directive", () => {
    const result = resolveManifest({
      updates: [makeUpdate()],
      channelToBranch: { production: "production" },
      request: {
        platform: "ios",
        runtimeVersion: "1.0.0",
        channelName: "production",
      },
      rolledBackBranches: {
        production: { commitTime: "2026-06-13T09:00:00.000Z" },
      },
    })

    expect(result).toMatchObject({
      kind: "directive",
      directive: {
        type: "rollBackToEmbedded",
        parameters: { commitTime: "2026-06-13T09:00:00.000Z" },
      },
    })
  })

  test("required response headers present", () => {
    const manifestResult = resolveManifest({
      updates: [makeUpdate()],
      channelToBranch: { production: "production" },
      request: {
        platform: "ios",
        runtimeVersion: "1.0.0",
        channelName: "production",
      },
    })
    const directiveResult = resolveManifest({
      updates: [],
      channelToBranch: { production: "production" },
      request: {
        platform: "ios",
        runtimeVersion: "1.0.0",
        channelName: "production",
      },
    })

    expect(manifestResult.responseHeaders).toMatchObject({
      "expo-protocol-version": "1",
      "expo-sfv-version": "0",
    })
    expect(directiveResult.responseHeaders).toMatchObject({
      "expo-protocol-version": "1",
      "expo-sfv-version": "0",
    })
  })

  test("parseManifestRequest extracts Expo headers case-insensitively", () => {
    expect(
      parseManifestRequest({
        "expo-platform": "android",
        "EXPO-RUNTIME-VERSION": "1.0.0",
        "Expo-Channel-Name": "production",
        "expo-current-update-id": "current-update",
      }),
    ).toEqual({
      platform: "android",
      runtimeVersion: "1.0.0",
      channelName: "production",
      currentUpdateId: "current-update",
    })
  })
})
