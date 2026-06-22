// Full-input-path harness regression (#6041 class).
//
// This is the test that would have caught the original ⌘⇧E / ⌘⇧P keybinding bug.
// It drives the REAL desktop key path through ONE reusable harness:
//
//   keyboardForwardDecision (forward gate)
//     → PressedKey (only if forwarded)
//       → interpretKey → reducer (update)
//         → verseSceneVisualization (what the renderer consumes)
//
// The original bug shipped because the spawn keys were never registered as input
// bindings, so the FORWARD GATE resolved them to zero action ids and dropped the
// keydown BEFORE the reducer ran — yet a reducer-only test was green. Here we:
//
//   • PASS on current main (the bindings are registered): the gate forwards and
//     the scene spawns + renders into the SAME world visualization.
//   • FAIL on a deliberately-broken binding map (spawn bindings removed — the
//     exact pre-#6042 state): the gate does NOT forward, no PressedKey is
//     dispatched, and nothing spawns. This proves the harness catches the bug at
//     the layer the bug lived in.
//
// Determinism: pure functions, no clock/random/rAF — running twice is identical.

import { describe, expect, test } from "bun:test"

import {
  openAgentsDefaultInputProfile,
  openAgentsInputActionMapFromProfile,
  type OpenAgentsInputActionMap,
} from "@openagentsinc/input-bindings"

import { runKeyEventThroughFullPath } from "../src/testing/full-input-path"
import { initialModel, Model } from "../src/ui/model"
import {
  DEFAULT_SPAWNABLE_SCENE_ID,
  VERSE_SPAWNED_SCENE_NODE_PREFIX,
  VERSE_SPAWNED_SCENE_SOURCE_REF,
} from "../src/shared/verse-spawned-scene"

const CRACKLING = DEFAULT_SPAWNABLE_SCENE_ID
const fromId = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${CRACKLING}:from`
const toId = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${CRACKLING}:to`
const portalId = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${CRACKLING}:portal`

// The EXACT on-screen Verse explore state: pane "chat", verse enabled, explore
// mode — the only state where the spawn keys are live (view.ts gate).
const verseOnScreen = Model.make({
  ...initialModel,
  pane: "chat",
  verseEnabled: true,
  verseMode: "explore",
  commandPaletteOpen: false,
})

const spawnEvent = {
  key: "e",
  meta: true,
  ctrl: false,
  shift: true,
  inEditable: false,
} as const

const portalEvent = {
  key: "p",
  meta: true,
  ctrl: false,
  shift: true,
  inEditable: false,
} as const

// The deliberately-broken binding map: the real default map MINUS the two spawn
// action ids. This reproduces the pre-#6042 world where keyboardForwardDecision
// resolved ⌘⇧E / ⌘⇧P to nothing → forward=false → keydown dropped.
const brokenActionMap: OpenAgentsInputActionMap = (() => {
  const real = openAgentsInputActionMapFromProfile(openAgentsDefaultInputProfile)
  const copy: Record<string, (typeof real)[string]> = { ...real }
  delete copy["verse.spawn_scene"]
  delete copy["verse.toggle_scene_portal"]
  return copy
})()

describe("full-input-path harness: ⌘⇧E spawn (the #6041 bug layer)", () => {
  test("PASS on current bindings: gate forwards, scene spawns AND renders", () => {
    const outcome = runKeyEventThroughFullPath(verseOnScreen, spawnEvent)

    // The forward gate — the exact layer the original bug lived in.
    expect(outcome.forwarded).toBe(true)
    expect(outcome.preventDefault).toBe(true)
    expect(outcome.dispatched).toBe(true)

    // interpretKey mapping.
    expect(outcome.intent.kind).toBe("spawn-verse-scene")

    // Reducer result.
    expect(outcome.spawnedSceneIds).toEqual([CRACKLING])

    // Renders into the SAME world visualization the renderer consumes.
    expect(outcome.spawnedVizEntityIds).toContain(fromId)
    expect(outcome.spawnedVizEntityIds).toContain(toId)
    expect(outcome.cracklingArcBeamCount).toBe(1)

    // The arc carries its synthetic evidence ref so the evidence:required gate
    // in the renderer lets it animate (otherwise it would be invisible).
    const arc = (outcome.visualization.beams ?? []).find(
      (beam) => beam.fromId === fromId && beam.style === "crackling_arc",
    )
    expect(arc?.sourceRefs).toEqual([VERSE_SPAWNED_SCENE_SOURCE_REF])
    expect(outcome.visualization.motionPolicy?.evidence).toBe("required")

    // Spawning preserves the walkable third-person controller.
    expect(outcome.visualization.controller).toBe("third_person_character")
  })

  test("FAIL-on-broken: with spawn bindings removed, the gate drops the key", () => {
    const outcome = runKeyEventThroughFullPath(verseOnScreen, spawnEvent, {
      actionMap: brokenActionMap,
    })

    // This is the #6041 bug, now directly observable: the gate does not forward,
    // no PressedKey reaches the reducer, and nothing spawns or renders.
    expect(outcome.forwarded).toBe(false)
    expect(outcome.dispatched).toBe(false)
    expect(outcome.spawnedSceneIds).toEqual([])
    expect(outcome.spawnedVizEntityIds).toEqual([])
    expect(outcome.cracklingArcBeamCount).toBe(0)

    // A test that only checked the reducer would have been GREEN here too,
    // because it would never have run the gate. The harness catches it.
  })
})

describe("full-input-path harness: ⌘⇧P portal toggle", () => {
  test("PASS: portal forwards and toggles on an already-spawned scene", () => {
    const [spawned] = (() => {
      const out = runKeyEventThroughFullPath(verseOnScreen, spawnEvent)
      return [out.nextModel] as const
    })()

    const outcome = runKeyEventThroughFullPath(spawned, portalEvent)
    expect(outcome.forwarded).toBe(true)
    expect(outcome.dispatched).toBe(true)
    expect(outcome.intent.kind).toBe("toggle-verse-scene-portal")
    expect(outcome.nextModel.verseSpawnedScenes[0]?.showPortal).toBe(true)

    const portal = (outcome.visualization.entities ?? []).find(
      (entity) => entity.id === portalId,
    )
    expect(portal).toBeDefined()
    expect((portal as { visualKind?: string } | undefined)?.visualKind).toBe(
      "gateway_portal",
    )
  })

  test("FAIL-on-broken: portal binding removed → gate drops the key", () => {
    const [spawned] = (() => {
      const out = runKeyEventThroughFullPath(verseOnScreen, spawnEvent)
      return [out.nextModel] as const
    })()
    const outcome = runKeyEventThroughFullPath(spawned, portalEvent, {
      actionMap: brokenActionMap,
    })
    expect(outcome.forwarded).toBe(false)
    expect(outcome.dispatched).toBe(false)
    // Portal stays off because the key never reached the reducer.
    expect(outcome.nextModel.verseSpawnedScenes[0]?.showPortal).toBe(false)
  })
})

describe("full-input-path harness: determinism + negative contexts", () => {
  test("identical outcome when run twice (no clock/random/rAF)", () => {
    const a = runKeyEventThroughFullPath(verseOnScreen, spawnEvent)
    const b = runKeyEventThroughFullPath(verseOnScreen, spawnEvent)
    expect(b.forwarded).toBe(a.forwarded)
    expect(b.dispatched).toBe(a.dispatched)
    expect(b.intent.kind).toBe(a.intent.kind)
    expect(b.spawnedSceneIds).toEqual(a.spawnedSceneIds)
    expect(b.spawnedVizEntityIds).toEqual(a.spawnedVizEntityIds)
    expect(b.cracklingArcBeamCount).toBe(a.cracklingArcBeamCount)
  })

  test("code mode does not spawn (coding overlay owns its keys)", () => {
    const codeModel = Model.make({
      ...initialModel,
      pane: "chat",
      verseEnabled: true,
      verseMode: "code",
    })
    const outcome = runKeyEventThroughFullPath(codeModel, spawnEvent)
    // interpretKey only emits spawn in explore context, so even if forwarded the
    // reducer must not spawn here.
    expect(outcome.intent.kind).not.toBe("spawn-verse-scene")
    expect(outcome.spawnedSceneIds).toEqual([])
  })

  test("editable target: a bare letter never spawns or forwards a command", () => {
    const outcome = runKeyEventThroughFullPath(verseOnScreen, {
      key: "e",
      meta: false,
      ctrl: false,
      shift: false,
      inEditable: true,
    })
    expect(outcome.spawnedSceneIds).toEqual([])
  })
})
