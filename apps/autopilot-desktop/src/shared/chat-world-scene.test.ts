import { describe, expect, test } from "bun:test"

import {
  projectChatWorldPylonScene,
  pylonGrowthTier,
} from "./chat-world-scene.js"
import { liveChatWorldNetworkScene } from "./chat-world-visualization.js"
import { pylonNetworkVisualizationOptions } from "../ui/pylon-network-visualization.js"

describe("chat world pylon growth", () => {
  test("keeps a zero-earning Pylon at the still tier-0 crystal", () => {
    const scene = projectChatWorldPylonScene({
      available: true,
      status: "live",
      asOfLabel: "now",
      pylonsOnlineNow: 1,
      recentPylons: [
        {
          nodeLabel: "zero",
          nostrPubkeyShort: "pylon.zero",
          onlineNow: true,
          assignmentReadyNow: false,
          walletReadyNow: false,
          lastHeartbeatAgeSeconds: 8,
          cumulativeSettledSats: 0,
        },
      ],
    })

    expect(scene.growth).toEqual(pylonGrowthTier(0))
    expect(scene.nodes[0]?.growth).toEqual(pylonGrowthTier(0))
    expect(scene.nodes[0]?.pulseSpeed).toBeGreaterThan(0)

    const network = liveChatWorldNetworkScene(scene)
    const options = pylonNetworkVisualizationOptions(network!)
    const node = options.nodes?.find(item => item.id === "pylon.zero")

    expect(node).toMatchObject({
      detail: "online - tier 0 - 0 sats",
      role: "lifecycle",
      status: "queued",
    })
  })

  test("projects per-Pylon cumulative settled sats into live crystal tiering", () => {
    const scene = projectChatWorldPylonScene({
      available: true,
      status: "live",
      asOfLabel: "now",
      pylonsOnlineNow: 1,
      recentPylons: [
        {
          nodeLabel: "earned",
          nostrPubkeyShort: "pylon.earned",
          onlineNow: false,
          assignmentReadyNow: false,
          walletReadyNow: false,
          cumulativeSettledSats: 100_000,
        },
      ],
    })

    expect(scene.nodes[0]?.growth).toEqual(pylonGrowthTier(100_000))

    const network = liveChatWorldNetworkScene(scene)
    expect(network?.nodes[0]?.growth).toEqual({
      brightness: 0.6,
      facets: 12,
      scale: 1.54,
      settledSats: 100_000,
      tier: 3,
    })

    const options = pylonNetworkVisualizationOptions(network!)
    const node = options.nodes?.find(item => item.id === "pylon.earned")

    expect(node).toMatchObject({
      detail: "seen - tier 3 - 100000 sats - 12 facets",
      role: "rung",
      status: "sealed",
    })
  })
})
