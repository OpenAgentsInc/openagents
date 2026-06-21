import { describe, expect, test } from "bun:test"

import {
  buildNetworkNodes,
  computeActivityIntensity,
  projectPylonNetworkScene,
  type PylonStatsSnapshot,
} from "../src/shared/pylon-network-scene"

const live = (overrides: Partial<PylonStatsSnapshot> = {}): PylonStatsSnapshot => ({
  available: true,
  status: "live",
  asOfLabel: "moments ago",
  pylonsOnlineNow: 0,
  ...overrides,
})

describe("computeActivityIntensity (§2 glow)", () => {
  test("idle network => 0; work raises it; monotonic and bounded", () => {
    expect(computeActivityIntensity(live())).toBe(0)
    const someWork = computeActivityIntensity(live({ pylonSessionsOnlineNow: 2 }))
    expect(someWork).toBeGreaterThan(0)
    expect(someWork).toBeLessThan(1)
    const moreWork = computeActivityIntensity(
      live({ pylonSessionsOnlineNow: 50 }),
    )
    expect(moreWork).toBeGreaterThan(someWork)
    expect(moreWork).toBeLessThanOrEqual(1)
  })

  test("all three signals together approach full brightness", () => {
    const intensity = computeActivityIntensity(
      live({
        pylonSessionsOnlineNow: 40,
        trainingModelProgressContributors: 40,
        nip90MarketSettlementStats: {
          compute: { jobsSettled24h: 30 },
          data: { jobsSettled24h: 30 },
          labor: { jobsSettled24h: 30 },
        },
      }),
    )
    expect(intensity).toBeGreaterThan(0.8)
    expect(intensity).toBeLessThanOrEqual(1)
  })

  test("each signal contributes independently (no single signal can exceed its weight)", () => {
    const onlySessions = computeActivityIntensity(
      live({ pylonSessionsOnlineNow: 1e6 }),
    )
    // a single saturated signal approaches its 1/3 weight, not the whole bar
    expect(onlySessions).toBeGreaterThan(0.28)
    expect(onlySessions).toBeLessThan(0.4)
  })
})

describe("buildNetworkNodes (§3 graph)", () => {
  test("recent pylons map to tones without fabricating ambient pylon nodes", () => {
    const nodes = buildNetworkNodes(
      live({
        pylonsOnlineNow: 5,
        pylonsAssignmentReadyNow: 0,
        recentPylons: [
          { nostrPubkeyShort: "aa", onlineNow: true, assignmentReadyNow: true },
          { nostrPubkeyShort: "bb", onlineNow: true, assignmentReadyNow: false },
          { nostrPubkeyShort: "cc", onlineNow: false },
        ],
      }),
    )
    // Aggregate counts live on the hub; each visible node must be a network row.
    expect(nodes).toHaveLength(3)
    expect(nodes.find((node) => node.id === "aa")?.tone).toBe("working")
    expect(nodes.find((node) => node.id === "bb")?.tone).toBe("online")
    expect(nodes.find((node) => node.id === "cc")?.tone).toBe("offline")
    expect(nodes.some((node) => node.id.startsWith("ambient-"))).toBe(false)
  })

  test("uses network labels and falls back to stable pylon refs, not generic pylon text", () => {
    const nodes = buildNetworkNodes(
      live({
        pylonsOnlineNow: 3,
        recentPylons: [
          {
            nostrPubkeyShort: "pylon.public.studio",
            nodeLabel: "Studio Rig",
            onlineNow: true,
          },
          {
            nostrPubkeyShort: "pylon.public.remote",
            nodeLabel: "pylon",
            onlineNow: true,
          },
          {
            nostrPubkeyShort: "pylon.public.unnamed",
            onlineNow: false,
          },
        ],
      }),
    )

    expect(nodes.map((node) => node.label)).toEqual([
      "Studio Rig",
      "pylon.public.remote",
      "pylon.public.unnamed",
    ])
    expect(nodes.some((node) => node.label.toLowerCase() === "pylon")).toBe(false)
  })

  test("working nodes flow toward the center", () => {
    const nodes = buildNetworkNodes(
      live({
        pylonsOnlineNow: 1,
        recentPylons: [{ nostrPubkeyShort: "aa", onlineNow: true, assignmentReadyNow: true }],
      }),
    )
    expect(nodes[0]?.flowing).toBe(true)
  })
})

describe("projectPylonNetworkScene", () => {
  test("unavailable / null => dormant, no fake counts", () => {
    expect(projectPylonNetworkScene(null).dormant).toBe(true)
    expect(projectPylonNetworkScene(null).onlineNow).toBe(0)
    const unavailable = projectPylonNetworkScene({ status: "unavailable", asOfLabel: "x" })
    expect(unavailable.dormant).toBe(true)
    expect(unavailable.nodes).toHaveLength(0)
    expect(unavailable.asOfLabel).toBe("x")
  })

  test("zero online => dormant but available (be the first pylon)", () => {
    const scene = projectPylonNetworkScene(live({ pylonsOnlineNow: 0 }))
    expect(scene.dormant).toBe(true)
    expect(scene.onlineNow).toBe(0)
  })

  test("a live network with work => not dormant, glow > 0, sats summed", () => {
    const scene = projectPylonNetworkScene(
      live({
        pylonsOnlineNow: 3,
        sellablePylonsOnlineNow: 2,
        pylonsWalletReadyNow: 4,
        pylonsAssignmentReadyNow: 1,
        pylonsSeen24h: 9,
        pylonsRegisteredTotal: 12,
        pylonSessionsOnlineNow: 2,
        trainingAssignedContributors: 5,
        trainingAcceptedContributors: 4,
        trainingModelProgressContributors: 3,
        nip90MarketSettlementStats: {
          compute: { satsSettled24h: 100, satsSettledTotal: 1000 },
          labor: { satsSettled24h: 50, satsSettledTotal: 500 },
        },
      }),
    )
    expect(scene.dormant).toBe(false)
    expect(scene.activityIntensity).toBeGreaterThan(0)
    expect(scene.satsSettled24h).toBe(150)
    expect(scene.satsSettledTotal).toBe(1500)
    expect(scene.sellableOnlineNow).toBe(2)
    expect(scene.walletReadyNow).toBe(4)
    expect(scene.assignmentReadyNow).toBe(1)
    expect(scene.seen24h).toBe(9)
    expect(scene.registeredTotal).toBe(12)
    expect(scene.trainingAssignedContributors).toBe(5)
    expect(scene.trainingAcceptedContributors).toBe(4)
    expect(scene.trainingProgressContributors).toBe(3)
  })
})
