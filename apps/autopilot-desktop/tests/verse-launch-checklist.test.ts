import { afterEach, describe, expect, test } from "bun:test"
import {
  openAgentsDefaultInputProfile,
  type OpenAgentsInputProfile,
} from "@openagentsinc/input-bindings"
import {
  canRetainTrainingRunVisualization,
  pointerClickPickFromGesture,
} from "@openagentsinc/three-effect/core"

import {
  agentCharacterCreationFlag,
  chatWorldBuildFlags,
  chatWorldHudFlag,
  chatWorldMultiplayerFlag,
} from "../src/shared/chat-world-flags"
import {
  pylonGrowthTier,
  type ChatWorldPylonScene,
} from "../src/shared/chat-world-scene"
import { projectOnboardingStatus } from "../src/shared/onboarding-status"
import type { NodeStateMessage } from "../src/shared/rpc"
import { VERSE_TASSADAR_BULLETIN_ITEM_ID } from "../src/shared/verse-bulletin-board"
import { initialRuntimeState } from "../src/ui/initial-state"
import {
  ChangedVerseLocalPose,
  ChangedInputProfile,
  GotChatWorldScene,
  GotNodeState,
  GotOnboardingStatus,
  GotTrainingRuns,
  TickedOnboardingStatusRefresh,
  TickedVerseTrainingProjectionRefresh,
} from "../src/ui/message"
import { update } from "../src/ui/update"
import {
  clearVerseSceneDiagnosticsForTest,
  verseSceneDiagnostics,
} from "../src/ui/verse-scene-diagnostics"
import { clearLatestVerseLocalPoseForTest } from "../src/ui/verse-local-pose"
import { verseSceneVisualization, view } from "../src/ui/view"
import { verseInputBindingProjection } from "../src/ui/verse-input-bindings"

const verseEnvKeys = [
  "VITE_DISABLE_VERSE",
  "VITE_VERSE_DISABLED",
  "VITE_VERSE_ENABLED",
  "VITE_CHAT_WORLD_SCENE",
  "VITE_CHAT_WORLD_PAYMENTS",
  "VITE_CHAT_WORLD_HUD",
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

const nodeWithBalance = (balanceSats: number | null): NodeStateMessage => ({
  ok: true,
  schema: "control.v1",
  sessions: [],
  wallet: {
    configured: true,
    daemonOnline: true,
    balanceSats,
    receiveReady: true,
    sendReady: false,
    readiness: "ready",
  },
})

const livePylonScene = (
  overrides: Partial<ChatWorldPylonScene> = {},
): ChatWorldPylonScene => ({
  empty: false,
  onlineNow: 1,
  nodes: [
    {
      id: "pylon.alpha",
      label: "Alpha Pylon",
      state: "assignment_ready",
      color: 0x4ade80,
      online: true,
      pulseSpeed: 0.4,
      products: ["labor"],
    },
  ],
  growth: pylonGrowthTier(2_100),
  asOfLabel: "moments ago",
  ...overrides,
})

const ijklInputProfile = (): OpenAgentsInputProfile => ({
  ...openAgentsDefaultInputProfile,
  profileId: "test-ijkl",
  bindings: {
    ...openAgentsDefaultInputProfile.bindings,
    "movement.forward": [{ type: "keyboard_code", code: "KeyI" }],
    "target.next": [{ type: "keyboard_code", code: "KeyE" }],
    "target.previous": [{ type: "keyboard_code", code: "KeyQ" }],
  },
})

describe("Verse packaged launch checklist (#5827)", () => {
  afterEach(() => {
    clearVerseEnv()
    clearLatestVerseLocalPoseForTest()
    clearVerseSceneDiagnosticsForTest()
  })

  test("launch flags default the Verse, payments, character creation, and world rows on", () => {
    clearVerseEnv()

    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: true,
      CHAT_WORLD_PAYMENTS: true,
    })
    expect(chatWorldHudFlag()).toBe(false)
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
    expect(chatWorldHudFlag()).toBe(false)
  })

  test("fresh first paint is the packaged Verse checklist, not code chrome", () => {
    clearVerseEnv()
    const [model, commands] = initialRuntimeState()
    const tree = serializeView(view(model).body)

    expect(model.pane).toBe("chat")
    expect(model.verseEnabled).toBe(true)
    expect(model.verseMode).toBe("explore")
    expect(commands.map(command => command.name)).toEqual([
      "LoadIdentityChoiceState",
      "LoadOnboardingStatus",
      "LoadPromiseSurfacingReadiness",
      "LoadTrainingRuns",
      "LoadTrainingPromiseGates",
      "LoadTrainingOperatorReadiness",
    ])

    expect(tree).toContain("app-shell-verse")
    expect(tree).toContain("data-verse-mode")
    expect(tree).toContain("data-verse-focus-root")
    expect(tree).toContain("explore")
    expect(tree).toContain("chat-pane-world")
    expect(tree).toContain("three-effect-chat-scene")
    expect(tree).toContain("pylon-balance-hud")
    expect(tree).toContain("Pylon Bitcoin sats")
    expect(tree).toContain("Pylon Bitcoin balance waiting for wallet state")
    expect(tree).toContain("wallet pending")
    expect(tree).toContain("\"data-pylon-balance-value\":\"wallet pending\"")
    expect(tree).not.toContain("\"data-pylon-balance-value\":\"unknown\"")
    expect(tree).not.toContain("pylon-balance-hud-label")
    expect(tree).not.toContain("\"Bitcoin\"")
    expect(tree).toContain("verse-run-hud")
    expect(tree).toContain("Tassadar run HUD")
    expect(tree).toContain("verse-presence-zone")
    expect(tree).toContain("away")
    expect(tree).not.toContain("pylon-base-status")
    expect(tree).not.toContain("character-creation-overlay")
    expect(tree).not.toContain("The Verse")
    expect(tree).toContain("Tassadar")
    expect(tree).toContain("Pylon")
    expect(tree).not.toContain("chat-thread-shell")
    expect(tree).not.toContain("chat-message-list")
    expect(tree).not.toContain("verse-bottom-hud")
    expect(tree).not.toContain("chat-composer-verse")
    expect(tree).toContain("hotbar-slot")
    expect(tree).toContain("hotbar-slot-coder")
    expect(tree).toContain("data-hotbar-icon")
    expect(tree).toContain("OpenaiLogoRegular")
    expect(tree).toContain("New Coder Session")
    expect(tree).not.toContain("Command palette")
    expect(tree).not.toContain("Send message")
    expect(tree).not.toContain("Send")
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

  test("Verse input profiles project movement and target bindings", () => {
    const projection = verseInputBindingProjection(
      ijklInputProfile(),
      "verse_explore",
    )

    expect(projection.profileId).toBe("test-ijkl")
    expect(projection.activeContext).toBe("verse_explore")
    expect(projection.movement.forward).toEqual(["KeyI"])
    expect(projection.movement.backward).toEqual(["KeyS", "ArrowDown"])
    expect(projection.keyboardTargeting.bindings?.next).toEqual([
      {
        altKey: false,
        code: "KeyE",
        ctrlKey: false,
        metaKey: false,
        shiftKey: false,
      },
    ])
    expect(projection.lastResolvedAction).toBeNull()
  })

  test("shared pointer picking keeps mouselook drags out of click selection", () => {
    expect(
      pointerClickPickFromGesture({
        buttonDown: 0,
        buttonUp: 0,
        downAtMs: 100,
        upAtMs: 170,
        downX: 320,
        downY: 240,
        upX: 327,
        upY: 244,
        pointerLocked: false,
        releasedOnCanvas: true,
      }),
    ).toBe(true)
    expect(
      pointerClickPickFromGesture({
        buttonDown: 0,
        buttonUp: 0,
        downAtMs: 100,
        upAtMs: 170,
        downX: 320,
        downY: 240,
        upX: 381,
        upY: 246,
        pointerLocked: false,
        releasedOnCanvas: true,
      }),
    ).toBe(false)
  })

  test("input profile changes update Verse bindings without rebuilding the scene", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const movedPose = {
      regionRef: "region.run.tassadar.executor.20260615",
      x: 7.25,
      y: 0,
      z: -3.5,
      yaw: 0.75,
      animation: "run" as const,
      capturedAtMs: 12_345,
    }
    const [afterMove] = update(
      initial,
      ChangedVerseLocalPose({ pose: movedPose }),
    )
    const [withScene] = update(
      afterMove,
      GotChatWorldScene({ scene: livePylonScene() }),
    )
    const before = verseSceneVisualization(withScene)
    const [withProfile] = update(
      withScene,
      ChangedInputProfile({ profile: ijklInputProfile() }),
    )
    const after = verseSceneVisualization(withProfile)

    expect(withProfile.verseSceneRestorePose).toEqual(
      withScene.verseSceneRestorePose,
    )
    expect(before.thirdPersonController?.keyboardBindings?.forward).toEqual([
      "KeyW",
      "ArrowUp",
    ])
    expect(after.thirdPersonController?.keyboardBindings?.forward).toEqual([
      "KeyI",
    ])
    expect(after.thirdPersonController?.initialPosition).toEqual([
      7.25,
      0,
      -3.5,
    ])
    expect(after.keyboardTargeting?.bindings?.next).toEqual([
      {
        altKey: false,
        code: "KeyE",
        ctrlKey: false,
        metaKey: false,
        shiftKey: false,
      },
    ])
    expect(canRetainTrainingRunVisualization(before, after)).toBe(true)
    expect(
      verseSceneDiagnostics().some(
        entry =>
          entry.event === "input.profile" &&
          entry.detail.profileId === "test-ijkl" &&
          entry.detail.activeContext === "verse_explore" &&
          entry.detail.lastResolvedAction === null,
      ),
    ).toBe(true)
  })

  test("the top-left Pylon sats HUD refreshes from live node wallet balances", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const [funded] = update(
      initial,
      GotNodeState({ node: nodeWithBalance(2_100) }),
    )
    const [refreshed] = update(
      funded,
      GotNodeState({ node: nodeWithBalance(3_456) }),
    )

    const fundedTree = serializeView(view(funded).body)
    const refreshedTree = serializeView(view(refreshed).body)

    expect(fundedTree).toContain("pylon-balance-hud")
    expect(fundedTree).toContain("2,100 sats")
    expect(fundedTree).not.toContain("pylon-balance-hud-label")
    expect(fundedTree).not.toContain("\"Bitcoin\"")
    expect(refreshedTree).toContain("pylon-balance-hud")
    expect(refreshedTree).toContain("3,456 sats")
    expect(refreshedTree).not.toContain("2,100 sats")
    expect(refreshedTree).not.toContain("pylon-balance-hud-label")
    expect(refreshedTree).not.toContain("\"Bitcoin\"")
  })

  test("the top-left Pylon sats HUD falls back to the live onboarding wallet balance", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const [withNodeButNoBalance] = update(
      initial,
      GotNodeState({ node: nodeWithBalance(null) }),
    )
    const [withOnboardingBalance] = update(
      withNodeButNoBalance,
      GotOnboardingStatus({
        projection: projectOnboardingStatus({
          fetchedAt: "2026-06-21T17:40:00.000Z",
          identityChoiceMade: true,
          identityLabel: "existing pylon.alpha",
          agentRegistered: true,
          nodeLaunchStatus: "online",
          localPylonReady: true,
          onboardingEnvConfigured: true,
          walletReceiveReady: true,
          walletBalanceSats: 4_200,
          forumTipReady: true,
          forumIntroPosted: true,
          forumWorkSearched: true,
          forumWorkOpenCount: 1,
          openAssignmentCount: 1,
        }),
      }),
    )

    const tree = serializeView(view(withOnboardingBalance).body)

    expect(tree).toContain("pylon-balance-hud")
    expect(tree).toContain("\"data-pylon-balance-hud\":\"known\"")
    expect(tree).toContain("\"data-pylon-balance-value\":\"4,200 sats\"")
    expect(tree).toContain("4,200 sats")
    expect(tree).not.toContain("pylon-balance-hud-label")
    expect(tree).not.toContain("\"Bitcoin\"")
  })

  test("the Verse training projection tick refreshes only the board data", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const [model, commands] = update(
      initial,
      TickedVerseTrainingProjectionRefresh(),
    )

    expect(model).toBe(initial)
    expect(commands.map(command => command.name)).toEqual(["LoadTrainingRuns"])
  })

  test("the onboarding status tick refreshes wallet balance data without resetting the Verse", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const [model, commands] = update(initial, TickedOnboardingStatusRefresh())

    expect(model).toBe(initial)
    expect(commands.map(command => command.name)).toEqual(["LoadOnboardingStatus"])
  })

  test("live projection refreshes do not reset the Verse controller to spawn", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const movedPose = {
      regionRef: "region.run.tassadar.executor.20260615",
      x: 7.25,
      y: 0,
      z: -3.5,
      yaw: 1.125,
      animation: "run" as const,
      capturedAtMs: 12_345,
    }
    const [afterMove] = update(
      initial,
      ChangedVerseLocalPose({ pose: movedPose }),
    )
    const [afterProjectionRefresh] = update(
      afterMove,
      GotChatWorldScene({ scene: livePylonScene() }),
    )

    expect(afterMove).toBe(initial)
    expect(verseSceneVisualization(afterProjectionRefresh).thirdPersonController)
      .toMatchObject({
        initialPosition: [7.25, 0, -3.5],
        character: {
          walkSpeed: 3.8,
          runSpeed: 6.7,
        },
      })
  })

  test("cosmetic pylon projection churn does not rerender the Verse scene", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const [withScene] = update(
      initial,
      GotChatWorldScene({ scene: livePylonScene() }),
    )
    const before = verseSceneVisualization(withScene)
    const [heartbeatOnlyRefresh] = update(
      withScene,
      GotChatWorldScene({
        scene: livePylonScene({
          asOfLabel: "4 seconds ago",
          nodes: [
            {
              ...livePylonScene().nodes[0]!,
              pulseSpeed: 1.9,
            },
          ],
        }),
      }),
    )
    const [sameAgain] = update(
      heartbeatOnlyRefresh,
      GotChatWorldScene({
        scene: livePylonScene({
          asOfLabel: "8 seconds ago",
          nodes: [
            {
              ...livePylonScene().nodes[0]!,
              pulseSpeed: 0.2,
            },
          ],
        }),
      }),
    )

    expect(heartbeatOnlyRefresh).toBe(withScene)
    expect(sameAgain).toBe(withScene)
    expect(serializeView(verseSceneVisualization(sameAgain))).toBe(
      serializeView(before),
    )
    expect(verseSceneDiagnostics().map(entry => entry.event)).toContain(
      "chat-world-scene.noop",
    )
  })

  test("the packaged Verse scene carries the Tassadar bulletin world item", () => {
    clearVerseEnv()
    const [initial] = initialRuntimeState()
    const [model] = update(
      initial,
      GotTrainingRuns({
        projection: {
          fetchedAt: "2026-06-21T17:10:00.000Z",
          ok: true,
          runs: [],
          sourceUrl: "https://openagents.test/api/training/runs",
          summaries: [],
          tassadarSummary: {
            runRef: "run.tassadar.executor.20260615",
            runState: "active",
            bulletin: {
              title: "Tassadar Run Board",
              headline: "Tassadar is active: 5 pylons, 2 active.",
              summary: "Public server-owned run summary.",
              onBoardLines: ["Status: active", "5 pylons, 2 active"],
              sourceRefs: ["run.tassadar.executor.20260615"],
            },
          },
        },
      }),
    )

    expect(verseSceneVisualization(model).worldItems?.map(item => item.id)).toContain(
      VERSE_TASSADAR_BULLETIN_ITEM_ID,
    )
  })
})
