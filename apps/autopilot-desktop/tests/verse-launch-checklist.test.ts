import { afterEach, describe, expect, test } from "bun:test"

import {
  agentCharacterCreationFlag,
  chatWorldBuildFlags,
  chatWorldMultiplayerFlag,
} from "../src/shared/chat-world-flags"
import { initialRuntimeState } from "../src/ui/initial-state"
import { verseSceneVisualization, view } from "../src/ui/view"

const verseEnvKeys = [
  "VITE_DISABLE_VERSE",
  "VITE_VERSE_DISABLED",
  "VITE_VERSE_ENABLED",
  "VITE_CHAT_WORLD_SCENE",
  "VITE_CHAT_WORLD_PAYMENTS",
  "VITE_AGENT_CHARACTER_CREATION",
  "VITE_CHAT_WORLD_MULTIPLAYER",
] as const

const clearVerseEnv = (): void => {
  for (const key of verseEnvKeys) {
    delete process.env[key]
  }
}

const serializeView = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_key, value) => {
    if (typeof value === "function") return "[fn]"
    if (typeof value === "object" && value !== null) {
      if (seen.has(value)) return "[cycle]"
      seen.add(value)
    }
    return value
  })
}

describe("Verse packaged launch checklist (#5827)", () => {
  afterEach(clearVerseEnv)

  test("launch flags default the Verse, payments, character creation, and world rows on", () => {
    clearVerseEnv()

    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: true,
      CHAT_WORLD_PAYMENTS: true,
    })
    expect(agentCharacterCreationFlag()).toBe(true)
    expect(chatWorldMultiplayerFlag()).toBe(true)
  })

  test("the hard kill switch disables every first-paint Verse layer", () => {
    clearVerseEnv()
    process.env.VITE_DISABLE_VERSE = "1"
    process.env.VITE_CHAT_WORLD_SCENE = "1"
    process.env.VITE_CHAT_WORLD_PAYMENTS = "1"
    process.env.VITE_AGENT_CHARACTER_CREATION = "1"
    process.env.VITE_CHAT_WORLD_MULTIPLAYER = "1"

    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: false,
      CHAT_WORLD_PAYMENTS: false,
    })
    expect(agentCharacterCreationFlag()).toBe(false)
    expect(chatWorldMultiplayerFlag()).toBe(false)
  })

  test("fresh first paint is the packaged Verse checklist, not code chrome", () => {
    clearVerseEnv()
    const [model, commands] = initialRuntimeState()
    const tree = serializeView(view(model).body)

    expect(model.pane).toBe("chat")
    expect(model.verseEnabled).toBe(true)
    expect(commands.map(command => command.name)).toEqual([
      "LoadIdentityChoiceState",
      "LoadOnboardingStatus",
      "LoadPromiseSurfacingReadiness",
      "LoadTrainingRuns",
      "LoadTrainingPromiseGates",
      "LoadTrainingOperatorReadiness",
    ])

    expect(tree).toContain("app-shell-verse")
    expect(tree).toContain("chat-pane-world")
    expect(tree).toContain("three-effect-chat-scene")
    expect(tree).not.toContain("pylon-base-status")
    expect(tree).not.toContain("character-creation-overlay")
    expect(tree).not.toContain("The Verse")
    expect(tree).toContain("Tassadar")
    expect(tree).toContain("Pylon")
    expect(tree).not.toContain("chat-thread-shell")
    expect(tree).not.toContain("chat-message-list")
    expect(tree).toContain("Send message")
    expect(tree).toContain("Send")
    expect(tree).not.toContain("Advanced")

    expect(verseSceneVisualization(model)).toMatchObject({
      cameraMode: "perspective_walk",
      controller: "third_person_character",
    })

    expect(tree).not.toContain("sidebar")
    expect(tree).not.toContain("status-hud-overlay")
    expect(tree).not.toContain("shell-target-tabs")
    expect(tree).not.toContain("Claude Code")
    expect(tree).not.toContain("Codex")
    expect(tree).not.toContain("Spawn a session")
    expect(tree).not.toContain("Sessions")
    expect(tree).not.toContain("Swarm")
    expect(tree).not.toContain("Deploy")
  })
})
