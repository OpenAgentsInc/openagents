import { describe, expect, test } from "bun:test"

import { pylonNetworkVisualizationOptions } from "../src/ui/pylon-network-visualization"
import { projectPylonNetworkScene } from "../src/shared/pylon-network-scene"

describe("pylonNetworkVisualizationOptions (bezier graph adapter)", () => {
  test("dormant network: center planned, no pylon nodes, slow pulse", () => {
    const opts = pylonNetworkVisualizationOptions(projectPylonNetworkScene(null))
    expect(opts.backgroundColor).toBe(0x0c0f13)
    const center = opts.nodes?.find((n) => n.id === "network")
    expect(center?.status).toBe("planned")
    expect(opts.nodes).toHaveLength(1) // center only
    expect(opts.pulseSpeed).toBeCloseTo(0.55, 2)
  })

  test("active network: center active, pylons ring + connect to center, fast pulse", () => {
    const scene = projectPylonNetworkScene({
      available: true,
      status: "live",
      pylonsOnlineNow: 3,
      pylonSessionsOnlineNow: 30,
      trainingModelProgressContributors: 30,
      nip90MarketSettlementStats: {
        compute: { jobsSettled24h: 30 },
        data: { jobsSettled24h: 30 },
        labor: { jobsSettled24h: 30 },
      },
      recentPylons: [
        { nostrPubkeyShort: "aa", onlineNow: true, assignmentReadyNow: true },
        { nostrPubkeyShort: "bb", onlineNow: true, assignmentReadyNow: false },
      ],
    })
    const opts = pylonNetworkVisualizationOptions(scene)
    const center = opts.nodes?.find((n) => n.id === "network")
    expect(center?.status).toBe("active")
    // every pylon node connects to the center (bezier edge -> center)
    const pylons = opts.nodes?.filter((n) => n.id !== "network") ?? []
    expect(pylons.length).toBeGreaterThan(0)
    expect(pylons.every((n) => n.connectedTo?.includes("network"))).toBe(true)
    // working pylon -> active status; idle online -> queued
    expect(opts.nodes?.find((n) => n.id === "aa")?.status).toBe("active")
    expect(opts.nodes?.find((n) => n.id === "bb")?.status).toBe("queued")
    // busy network pulses faster than idle
    expect(opts.pulseSpeed).toBeGreaterThan(1.5)
  })

  test("online-but-idle network: center sealed (lit, not pulsing hard), pulse near base", () => {
    const scene = projectPylonNetworkScene({
      available: true,
      status: "live",
      pylonsOnlineNow: 2,
      recentPylons: [{ nostrPubkeyShort: "aa", onlineNow: true }],
    })
    const opts = pylonNetworkVisualizationOptions(scene)
    expect(opts.nodes?.find((n) => n.id === "network")?.status).toBe("sealed")
    expect(opts.pulseSpeed).toBeCloseTo(0.55, 2)
  })

  test("ring positions are deterministic and unique per node", () => {
    const scene = projectPylonNetworkScene({
      available: true,
      status: "live",
      pylonsOnlineNow: 4,
      recentPylons: [
        { nostrPubkeyShort: "a", onlineNow: true },
        { nostrPubkeyShort: "b", onlineNow: true },
        { nostrPubkeyShort: "c", onlineNow: true },
        { nostrPubkeyShort: "d", onlineNow: true },
      ],
    })
    const a = pylonNetworkVisualizationOptions(scene)
    const b = pylonNetworkVisualizationOptions(scene)
    expect(a.nodes).toEqual(b.nodes) // deterministic
    const positions = (a.nodes ?? [])
      .filter((n) => n.id !== "network")
      .map((n) => n.position.join(","))
    expect(new Set(positions).size).toBe(positions.length) // unique
  })

  test("growth tiers change pylon role, status, and detail from settled sats", () => {
    const opts = pylonNetworkVisualizationOptions({
      ...projectPylonNetworkScene(null),
      dormant: false,
      onlineNow: 2,
      nodes: [
        {
          id: "zero",
          label: "zero",
          tone: "online",
          flowing: false,
          growth: {
            tier: 0,
            scale: 1,
            facets: 6,
            brightness: 0,
            settledSats: 0,
          },
        },
        {
          id: "earned",
          label: "earned",
          tone: "online",
          flowing: false,
          growth: {
            tier: 4,
            scale: 1.72,
            facets: 14,
            brightness: 0.8,
            settledSats: 1_000_000,
          },
        },
      ],
    })
    const zero = opts.nodes?.find((node) => node.id === "zero")
    const earned = opts.nodes?.find((node) => node.id === "earned")

    expect(zero?.role).toBe("lifecycle")
    expect(zero?.status).toBe("queued")
    expect(zero?.detail).toContain("tier 0 - 0 sats")
    expect(earned?.role).toBe("run")
    expect(earned?.status).toBe("verified")
    expect(earned?.detail).toContain("tier 4 - 1000000 sats")
    expect(earned?.detail).toContain("14 facets")
  })
})
