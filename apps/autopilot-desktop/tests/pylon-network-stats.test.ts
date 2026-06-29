import { describe, expect, test } from "bun:test"

import { fetchPublicPylonStats } from "../src/bun/pylon-network-stats"
import { projectPylonNetworkScene } from "../src/shared/pylon-network-scene"

const nowIso = () => "2026-06-15T00:00:00.000Z"

describe("fetchPublicPylonStats", () => {
  test("hits /api/public/pylon-stats and returns the snapshot", async () => {
    let requested = ""
    const fetchFn = (async (url: string) => {
      requested = url
      return new Response(JSON.stringify({ available: true, status: "live", pylonsOnlineNow: 4 }), {
        status: 200,
      })
    }) as unknown as typeof fetch
    const result = await fetchPublicPylonStats({ baseUrl: "https://openagents.com/", fetchFn, nowIso })
    expect(requested).toBe("https://openagents.com/api/public/pylon-stats")
    expect(result.ok).toBe(true)
    expect(result.snapshot?.pylonsOnlineNow).toBe(4)
    // feeds straight into the pure projection
    expect(projectPylonNetworkScene(result.snapshot).onlineNow).toBe(4)
  })

  test("non-200 fails soft -> dormant scene, no fake counts", async () => {
    const fetchFn = (async () => new Response("nope", { status: 503 })) as unknown as typeof fetch
    const result = await fetchPublicPylonStats({ baseUrl: "https://openagents.com", fetchFn, nowIso })
    expect(result.ok).toBe(false)
    expect(result.error).toContain("503")
    expect(projectPylonNetworkScene(result.snapshot).dormant).toBe(true)
  })

  test("network error fails soft", async () => {
    const fetchFn = (async () => {
      throw new Error("offline")
    }) as unknown as typeof fetch
    const result = await fetchPublicPylonStats({ baseUrl: "https://openagents.com", fetchFn, nowIso })
    expect(result.ok).toBe(false)
    expect(result.error).toBe("offline")
    expect(result.snapshot).toBeNull()
  })
})
