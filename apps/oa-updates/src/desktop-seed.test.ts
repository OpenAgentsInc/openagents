import { join } from "node:path"

import { describe, expect, test } from "bun:test"

import { assetKeyFromBytes } from "./asset-store.ts"
import { seedDesktopReleases } from "./desktop-seed.ts"
import { sha256Hex } from "./desktop-release.ts"
import { createUpdatesServer } from "./server.ts"

const bytes = (value: string): Uint8Array => new TextEncoder().encode(value)

describe("desktop release seeding", () => {
  test("loads baked desktop releases and serves the feed plus delta bytes", async () => {
    const distDir = "/tmp/khala-code-desktop-releases"
    const artifactBytes = bytes("desktop archive")
    const bsdiffBytes = bytes("BSDIFF patch")
    const files = new Map<string, Uint8Array>([
      [
        join(distDir, "releases.json"),
        bytes(
          JSON.stringify({
            releases: [
              {
                product: "khala-code-desktop",
                channel: "stable",
                version: "1.2.0",
                artifactPath: "assets/app.zip",
                artifactContentType: "application/zip",
                createdAt: "2026-06-14T01:00:00.000Z",
                bsdiffFromVersion: "1.1.0",
                bsdiffPath: "assets/app.1.1.0-1.2.0.bsdiff",
              },
            ],
          }),
        ),
      ],
      [join(distDir, "assets/app.zip"), artifactBytes],
      [join(distDir, "assets/app.1.1.0-1.2.0.bsdiff"), bsdiffBytes],
    ])
    const readFile = async (path: string): Promise<Uint8Array> => {
      const file = files.get(path)
      if (!file) throw new Error(`Missing fake file: ${path}`)

      return file
    }
    const server = createUpdatesServer({ port: 8080 })

    const seeded = await seedDesktopReleases({
      server,
      distDir,
      baseUrl: "https://updates.openagents.test",
      readFile,
    })

    expect(seeded.releases).toHaveLength(1)
    expect(seeded.releases[0]?.product).toBe("khala-code-desktop")

    const feedResponse = await server.fetch(
      new Request(
        "https://updates.openagents.test/desktop/khala-code-desktop/stable/feed.json",
      ),
    )
    const feed = await feedResponse.json() as any[]

    expect(feedResponse.status).toBe(200)
    expect(feed).toEqual([
      {
        version: "1.2.0",
        artifactUrl: `https://updates.openagents.test/assets/${assetKeyFromBytes(artifactBytes)}`,
        sha256: sha256Hex(artifactBytes),
        createdAt: "2026-06-14T01:00:00.000Z",
        bsdiffFromVersion: "1.1.0",
        bsdiffUrl: `https://updates.openagents.test/assets/${assetKeyFromBytes(bsdiffBytes)}`,
        bsdiffSha256: sha256Hex(bsdiffBytes),
      },
    ])

    const deltaResponse = await server.fetch(new Request(feed[0].bsdiffUrl))
    expect(deltaResponse.status).toBe(200)
    expect(deltaResponse.headers.get("content-type")).toBe("application/octet-stream")
    expect(new Uint8Array(await deltaResponse.arrayBuffer())).toEqual(bsdiffBytes)
  })
})
