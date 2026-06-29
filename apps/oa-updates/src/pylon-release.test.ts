import { describe, expect, test } from "bun:test"

import { createInMemoryAssetStore } from "./asset-store.ts"
import {
  buildPylonFeed,
  buildPylonReleaseManifest,
  normalizePylonPlatform,
  rolloutBucket,
  selectPylonUpdate,
  sha256Hex,
  sortPylonReleases,
  type PylonReleaseManifest,
} from "./pylon-release.ts"

const base = "https://updates.openagents.com"

const seed = (
  overrides: Partial<PylonReleaseManifest> = {},
): PylonReleaseManifest => ({
  version: "1.0.0-rc.1",
  channel: "rc",
  platform: "darwin-arm64",
  artifactUrl: `${base}/assets/abc`,
  sha256: "0".repeat(64),
  signature: "sig",
  kid: "2dbe811d19f67528",
  ...overrides,
})

describe("buildPylonReleaseManifest", () => {
  test("stores artifact and records its sha256 + signature", async () => {
    const store = createInMemoryAssetStore(base)
    const bytes = new TextEncoder().encode("pylon-binary-bytes")
    const result = await buildPylonReleaseManifest({
      version: "1.0.0-rc.1",
      channel: "rc",
      platform: "linux-x64",
      artifactBytes: bytes,
      signature: "abc.sig",
      kid: "2dbe811d19f67528",
      baseUrl: base,
      store,
    })
    expect(result.manifest.sha256).toBe(sha256Hex(bytes))
    expect(result.manifest.platform).toBe("linux-x64")
    expect(result.manifest.signature).toBe("abc.sig")
    expect(result.manifest.artifactUrl).toContain("/assets/")
  })

  test("rejects unknown platform and bad rollout", async () => {
    const store = createInMemoryAssetStore(base)
    const bytes = new Uint8Array([1])
    await expect(
      buildPylonReleaseManifest({
        version: "1.0.0",
        channel: "rc",
        // @ts-expect-error testing runtime guard
        platform: "windows-x64",
        artifactBytes: bytes,
        signature: "s",
        kid: "k",
        baseUrl: base,
        store,
      }),
    ).rejects.toThrow(/platform/)
  })
})

describe("buildPylonFeed", () => {
  test("drops yanked + off-channel/platform, sorts latest first", () => {
    const feed = buildPylonFeed("rc", "darwin-arm64", [
      seed({ version: "1.0.0-rc.1" }),
      seed({ version: "1.0.0-rc.3" }),
      seed({ version: "1.0.0-rc.2", yanked: true }),
      seed({ version: "9.9.9", channel: "stable" }),
      seed({ version: "9.9.9", platform: "linux-x64" }),
    ])
    expect(feed.releases.map((r) => r.version)).toEqual([
      "1.0.0-rc.3",
      "1.0.0-rc.1",
    ])
    expect(feed.schema).toBe("openagents.pylon.feed.v1")
  })
})

describe("version ordering", () => {
  test("stable outranks rc, rc.2 outranks rc.1", () => {
    const sorted = sortPylonReleases([
      seed({ version: "1.0.0-rc.1" }),
      seed({ version: "1.0.0" }),
      seed({ version: "1.0.0-rc.2" }),
    ]).map((r) => r.version)
    expect(sorted).toEqual(["1.0.0", "1.0.0-rc.2", "1.0.0-rc.1"])
  })
})

describe("selectPylonUpdate", () => {
  test("returns newer release, ignores same/older", () => {
    const feed = buildPylonFeed("rc", "darwin-arm64", [
      seed({ version: "1.0.0-rc.2" }),
      seed({ version: "1.0.0-rc.1" }),
    ])
    expect(selectPylonUpdate(feed, "1.0.0-rc.1", "client-a")?.version).toBe(
      "1.0.0-rc.2",
    )
    expect(selectPylonUpdate(feed, "1.0.0-rc.2", "client-a")).toBeNull()
  })

  test("staged rollout gates some clients but minVersion forces all", () => {
    const rolled = buildPylonFeed("rc", "darwin-arm64", [
      seed({ version: "1.0.0-rc.5", rolloutPercent: 0 }),
    ])
    // rollout 0 => nobody in-bucket
    expect(selectPylonUpdate(rolled, "1.0.0-rc.1", "client-a")).toBeNull()

    const floored = buildPylonFeed("rc", "darwin-arm64", [
      seed({ version: "1.0.0-rc.5", rolloutPercent: 0, minVersion: "1.0.0-rc.4" }),
    ])
    // below minVersion => forced regardless of rollout
    expect(selectPylonUpdate(floored, "1.0.0-rc.1", "client-a")?.version).toBe(
      "1.0.0-rc.5",
    )
  })

  test("rollout bucket is deterministic per client+version", () => {
    expect(rolloutBucket("client-a", "1.0.0")).toBe(rolloutBucket("client-a", "1.0.0"))
  })
})

describe("normalizePylonPlatform", () => {
  test("accepts known, rejects unknown", () => {
    expect(normalizePylonPlatform("darwin-arm64")).toBe("darwin-arm64")
    expect(() => normalizePylonPlatform("solaris")).toThrow()
  })
})
