import { describe, expect, test } from "bun:test"

import {
  projectChatWorldGameLayer,
  projectHandPinch,
  projectHotbarAgentGroups,
  projectManaBudgetHud,
  reputationGlyphForScore,
  type ChatWorldGameLayerFlags,
} from "./chat-world-game-layer"

const flags: ChatWorldGameLayerFlags = {
  handTracking: true,
  hotbar: true,
  manaHud: true,
  reputation: true,
}

describe("chat world game layer (#5740)", () => {
  test("projects 1-9 hotbar groups with Ctrl+n focus commands", () => {
    const slots = projectHotbarAgentGroups({
      flags,
      agents: [
        { agentRef: "agent.1", label: "one", group: 1 },
        { agentRef: "agent.2", label: "two", group: 2 },
      ],
    })
    expect(slots).toHaveLength(9)
    expect(slots[0]?.agents.map(agent => agent.agentRef)).toEqual(["agent.1"])
    expect(slots[1]?.focusCommand).toBe("Ctrl+2")
  })

  test("maps reputation scores to distinct glyph tiers", () => {
    expect(reputationGlyphForScore({ flags, actorRef: "a", score: 0 }).glyph).toBe("dot")
    expect(reputationGlyphForScore({ flags, actorRef: "a", score: 60 }).glyph).toBe("chevron")
    expect(reputationGlyphForScore({ flags, actorRef: "a", score: 260 }).glyph).toBe("diamond")
    expect(reputationGlyphForScore({ flags, actorRef: "a", score: 1200 }).glyph).toBe("crown")
  })

  test("computes a clamped mana budget HUD", () => {
    expect(projectManaBudgetHud({ flags, available: 30, total: 120 }).ratio).toBe(0.25)
    expect(projectManaBudgetHud({ flags, available: 200, total: 120 }).available).toBe(120)
    expect(projectManaBudgetHud({ flags, available: 10, total: 0 }).ratio).toBe(0)
  })

  test("detects hand pinch only when flag and confidence are high", () => {
    expect(projectHandPinch({
      flags,
      pose: { thumbTip: [0, 0], indexTip: [0.02, 0.02], confidence: 0.9 },
    }).pinching).toBe(true)
    expect(projectHandPinch({
      flags: { ...flags, handTracking: false },
      pose: { thumbTip: [0, 0], indexTip: [0.02, 0.02], confidence: 0.9 },
    }).pinching).toBe(false)
  })

  test("keeps every subfeature independently flag-gated", () => {
    const layer = projectChatWorldGameLayer({
      flags: { handTracking: false, hotbar: true, manaHud: false, reputation: true },
      agents: [{ agentRef: "agent.1", label: "one", group: 1 }],
      reputationScores: [{ actorRef: "agent.1", score: 250 }],
      manaAvailable: 1,
      manaTotal: 2,
      handPose: { thumbTip: [0, 0], indexTip: [0.01, 0.01], confidence: 1 },
    })

    expect(layer.hotbar[0]?.enabled).toBe(true)
    expect(layer.reputation[0]?.enabled).toBe(true)
    expect(layer.mana.enabled).toBe(false)
    expect(layer.hand.enabled).toBe(false)
  })
})
