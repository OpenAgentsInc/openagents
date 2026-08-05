import { describe, expect, test } from "vite-plus/test"

import { createUpdatesServer } from "./server.ts"

describe("updates server", () => {
  test("serves signed pylon feeds by channel + platform, drops yanked", async () => {
    const server = createUpdatesServer()
    const release = (version: string, extra = {}) => ({
      version,
      channel: "rc" as const,
      platform: "darwin-arm64" as const,
      artifactUrl: `https://updates.openagents.test/assets/${version}`,
      sha256: "0".repeat(64),
      signature: "sig",
      kid: "2dbe811d19f67528",
      ...extra,
    })
    server.registerPylonUpdate(release("1.0.0-rc.1"))
    server.registerPylonUpdate(release("1.0.0-rc.2"))
    server.registerPylonUpdate(release("1.0.0-rc.3", { yanked: true }))
    // off-platform entry must not leak into the darwin feed
    server.registerPylonUpdate({ ...release("9.9.9"), platform: "linux-x64" })

    const response = await server.fetch(
      new Request(
        "https://updates.openagents.test/pylon/rc/darwin-arm64/feed.json",
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get("cache-control")).toBe("no-store")
    const feed = (await response.json()) as {
      schema: string
      platform: string
      releases: { version: string }[]
    }
    expect(feed.schema).toBe("openagents.pylon.feed.v1")
    expect(feed.platform).toBe("darwin-arm64")
    expect(feed.releases.map((r) => r.version)).toEqual([
      "1.0.0-rc.2",
      "1.0.0-rc.1",
    ])
  })

  test("rejects unknown pylon platform with 404", async () => {
    const server = createUpdatesServer()
    const response = await server.fetch(
      new Request("https://updates.openagents.test/pylon/rc/windows-x64/feed.json"),
    )
    expect(response.status).toBe(404)
  })

  test("registers and lists Pylon discovery nodes for an owner", async () => {
    const server = createUpdatesServer()

    const registered = await server.fetch(
      new Request("https://updates.openagents.test/chris/nodes", {
        method: "POST",
        body: JSON.stringify({
          nodeRef: "pylon.abc",
          updatedAt: new Date().toISOString(),
        }),
      }),
    )
    expect(registered.status).toBe(200)

    const listed = await server.fetch(
      new Request("https://updates.openagents.test/chris/nodes"),
    )
    expect(listed.status).toBe(200)
    const body = (await listed.json()) as { nodes: { nodeRef: string }[] }
    expect(body.nodes.map((node) => node.nodeRef)).toEqual(["pylon.abc"])
  })

  test("serves a registered disk asset by hash and 404s an unknown hash", async () => {
    const server = createUpdatesServer()
    server.registerDiskAsset(
      "fixture-hash",
      new URL("./server.test.ts", import.meta.url).pathname,
      "application/octet-stream",
    )

    const found = await server.fetch(
      new Request("https://updates.openagents.test/assets/fixture-hash"),
    )
    expect(found.status).toBe(200)
    expect(found.headers.get("content-type")).toBe("application/octet-stream")
    expect(found.headers.get("cache-control")).toBe(
      "public, max-age=31536000, immutable",
    )

    const missing = await server.fetch(
      new Request("https://updates.openagents.test/assets/not-a-real-hash"),
    )
    expect(missing.status).toBe(404)
  })

  // The Electron/Electrobun desktop surface was deleted with the desktop app,
  // and the Expo mobile OTA surface was retired on 2026-08-05 (#9325) after the
  // owner confirmed there are no installed mobile users. This service now
  // serves only the Pylon release feed and Pylon node discovery; every retired
  // route must be an ordinary 404, with no manifest, no directive, no lockout
  // document, no feed, and no ReleaseSet pointer left behind.
  test.each([
    // Retired Expo mobile OTA manifest routes (expo-updates protocol v1).
    "/openagents-mobile/manifest",
    "/openagents/manifest",
    "/khala-mobile/manifest",
    // Retired Electron / Electrobun desktop routes.
    "/desktop/stable/feed.json",
    "/desktop/rc/feed.json",
    "/desktop/khala-code-desktop/rc/feed.json",
    "/desktop/openagents/rc/manifest.json",
    "/desktop/openagents/rc/manifest.sig.json",
    "/desktop/openagents/rc/release.json",
    "/desktop/openagents/stable/pointer.json",
    "/desktop/openagents/stable/v2/pointer.json",
    "/desktop/openagents/stable/release-set.json",
    "/desktop/rc-darwin-arm64-update.json",
    "/metrics/release-set.json",
  ])("no longer serves the retired route %s", async (pathname) => {
    const server = createUpdatesServer()

    const response = await server.fetch(
      new Request(`https://updates.openagents.test${pathname}`),
    )

    expect(response.status).toBe(404)
  })

  // The Expo Updates client sends its channel/runtime/platform negotiation as
  // request headers. A retired manifest route must stay a flat 404 for those
  // too — never a multipart/mixed body a stranded client could act on.
  test("answers a fully-formed Expo Updates manifest request with a plain 404", async () => {
    const server = createUpdatesServer()

    const response = await server.fetch(
      new Request("https://updates.openagents.test/openagents-mobile/manifest", {
        headers: {
          "Expo-Platform": "ios",
          "Expo-Runtime-Version": "fixture-runtime",
          "Expo-Channel-Name": "openagents-production",
          "Expo-Protocol-Version": "1",
        },
      }),
    )

    expect(response.status).toBe(404)
    expect(response.headers.get("content-type")).not.toContain("multipart/mixed")
    expect(response.headers.get("expo-protocol-version")).toBeNull()
    expect(response.headers.get("expo-sfv-version")).toBeNull()
  })
})
