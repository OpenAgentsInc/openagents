import { describe, expect, test } from "bun:test"

import { assetKeyFromBytes, createInMemoryAssetStore } from "./asset-store.ts"
import {
  buildDesktopUpdateManifest,
  normalizeDesktopReleaseSeed,
  sha256Hex,
  sortDesktopFeed,
} from "./desktop-release.ts"

const bytes = (value: string): Uint8Array => new TextEncoder().encode(value)

describe("desktop release manifests", () => {
  test("builds a full desktop artifact manifest with a sha256 verifier", async () => {
    const artifactBytes = bytes("desktop zip v2")
    const store = createInMemoryAssetStore("https://updates.openagents.test")
    const result = await buildDesktopUpdateManifest({
      version: "1.2.0",
      artifactBytes,
      baseUrl: "https://updates.openagents.test/",
      store,
      createdAt: "2026-06-14T01:00:00.000Z",
    })

    expect(result.manifest).toEqual({
      version: "1.2.0",
      artifactUrl: `https://updates.openagents.test/assets/${assetKeyFromBytes(artifactBytes)}`,
      sha256: sha256Hex(artifactBytes),
      createdAt: "2026-06-14T01:00:00.000Z",
    })
  })

  test("adds BSDIFF metadata when a previous artifact delta is provided", async () => {
    const artifactBytes = bytes("desktop zip v2")
    const bsdiffBytes = bytes("BSDIFF40 patch bytes")
    const store = createInMemoryAssetStore("https://updates.openagents.test")
    const result = await buildDesktopUpdateManifest({
      version: "1.2.0",
      artifactBytes,
      bsdiffFromVersion: "1.1.0",
      bsdiffBytes,
      baseUrl: "https://updates.openagents.test",
      store,
    })

    expect(result.manifest).toEqual({
      version: "1.2.0",
      artifactUrl: `https://updates.openagents.test/assets/${assetKeyFromBytes(artifactBytes)}`,
      sha256: sha256Hex(artifactBytes),
      bsdiffFromVersion: "1.1.0",
      bsdiffUrl: `https://updates.openagents.test/assets/${assetKeyFromBytes(bsdiffBytes)}`,
      bsdiffSha256: sha256Hex(bsdiffBytes),
    })
  })

  test("requires BSDIFF source version and patch path together", () => {
    expect(() =>
      normalizeDesktopReleaseSeed({
        channel: "stable",
        version: "1.2.0",
        artifactPath: "assets/app.zip",
        bsdiffFromVersion: "1.1.0",
      }),
    ).toThrow("bsdiffFromVersion and bsdiffPath together")
  })

  test("defaults legacy desktop seeds to Autopilot and accepts Khala product lanes", () => {
    expect(
      normalizeDesktopReleaseSeed({
        channel: "stable",
        version: "1.2.0",
        artifactPath: "assets/app.dmg",
      }).product,
    ).toBe("autopilot-desktop")

    expect(
      normalizeDesktopReleaseSeed({
        product: "khala-code-desktop",
        channel: "rc",
        version: "0.1.0-rc.1",
        artifactPath: "assets/khala-code.dmg",
      }).product,
    ).toBe("khala-code-desktop")
  })

  test("rejects prerelease versions from the stable desktop feed", () => {
    expect(() =>
      normalizeDesktopReleaseSeed({
        product: "khala-code-desktop",
        channel: "stable",
        version: "0.1.0-rc.1",
        artifactPath: "assets/khala-code.dmg",
      }),
    ).toThrow("stable channel must not contain prerelease")
  })

  test("sorts feed entries newest first", () => {
    expect(
      sortDesktopFeed([
        { version: "1.2.0", artifactUrl: "a", sha256: "a" },
        { version: "1.10.0", artifactUrl: "b", sha256: "b" },
        { version: "1.0.9", artifactUrl: "c", sha256: "c" },
      ]).map((manifest) => manifest.version),
    ).toEqual(["1.10.0", "1.2.0", "1.0.9"])
  })

  test("keeps stable releases ahead of release candidates with the same core", () => {
    expect(
      sortDesktopFeed([
        { version: "1.0.0-rc.10", artifactUrl: "rc10", sha256: "rc10" },
        { version: "1.0.0", artifactUrl: "stable", sha256: "stable" },
        { version: "1.0.0-rc.2", artifactUrl: "rc2", sha256: "rc2" },
      ]).map((manifest) => manifest.version),
    ).toEqual(["1.0.0", "1.0.0-rc.10", "1.0.0-rc.2"])
  })
})
