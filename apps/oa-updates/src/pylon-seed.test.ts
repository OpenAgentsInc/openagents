import { describe, expect, test } from "bun:test"

import { createUpdatesServer } from "./server.ts"
import { selectPylonUpdate, type PylonFeed } from "./pylon-release.ts"
import { normalizePylonReleaseSeed, seedPylonReleases } from "./pylon-seed.ts"

describe("normalizePylonReleaseSeed", () => {
  test("requires the signed fields", () => {
    expect(() => normalizePylonReleaseSeed({ channel: "rc" })).toThrow(/version/)
    expect(() =>
      normalizePylonReleaseSeed({
        channel: "rc",
        version: "1.0.0-rc.1",
        platform: "darwin-arm64",
        artifactPath: "assets/x",
        // missing signature/kid
      }),
    ).toThrow(/signature/)
  })
})

describe("seedPylonReleases end-to-end", () => {
  test("seeds the server from a dist dir and the feed selects the update", async () => {
    const server = createUpdatesServer()
    const bin = new TextEncoder().encode("fake-pylon-binary")
    const files: Record<string, Uint8Array> = {
      "pylon-releases.json": new TextEncoder().encode(
        JSON.stringify({
          releases: [
            {
              channel: "rc",
              version: "1.0.0-rc.1",
              platform: "darwin-arm64",
              artifactPath: "assets/pylon-darwin-arm64",
              signature: "sig-aa",
              kid: "2dbe811d19f67528",
            },
          ],
        }),
      ),
      "assets/pylon-darwin-arm64": bin,
    }
    const readFile = async (path: string) => {
      const key = Object.keys(files).find((k) => path.endsWith(k))
      if (!key) throw Object.assign(new Error("ENOENT"), { code: "ENOENT" })
      return files[key]
    }

    const result = await seedPylonReleases({
      server,
      distDir: "/dist",
      baseUrl: "https://updates.openagents.test",
      readFile,
    })
    expect(result.releases[0].signature).toBe("sig-aa")
    expect(result.releases[0].artifactUrl).toContain("/assets/")

    const response = await server.fetch(
      new Request("https://updates.openagents.test/pylon/rc/darwin-arm64/feed.json"),
    )
    const feed = (await response.json()) as PylonFeed
    expect(feed.releases).toHaveLength(1)
    // a client on an older version is offered the seeded release
    expect(selectPylonUpdate(feed, "0.9.0", "client-x")?.version).toBe("1.0.0-rc.1")
  })
})
