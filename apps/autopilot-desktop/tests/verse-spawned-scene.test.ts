// Dev affordance (#6033 / EPIC #6017): spawn an ISOLATED scene (the Khala
// "crackling energy" effect) into the SAME live Verse world the avatar walks in,
// then walk up to it with the existing third-person character controller.
//
// Covers, with NO node, NO Region DO, NO D1, NO Worker, and NO live receipt:
//   1. the spawned-scene layer mapper (synthetic, simulated, evidence-bound):
//      a registered scene id produces a crackling_arc beam between two positioned
//      in-world endpoints at a fixed station, labelled simulated:true; the portal
//      toggle adds a gateway_portal entity; an unknown scene id produces NOTHING.
//   2. the reducer toggles: spawn/unspawn a scene id, and flip its portal — both
//      pure model state, idempotent, no-op for unknown ids.
//   3. the ⌘⇧E / ⌘⇧P keyboard intents resolve to the spawn/portal toggles in the
//      Verse explore context (and NOT in code mode / outside the Verse).
//   4. the render assertion (the headline contract): spawning places the scene
//      object(s) into the SAME world visualization read by trainingRunView, and
//      that visualization still drives the avatar with the third_person_character
//      controller — i.e. you can walk up to the spawned effect.

import { describe, expect, test } from "bun:test"

import {
  verseSpawnedSceneLayer,
  withVerseSpawnedSceneLayer,
  verseSpawnedSceneEvidenceLines,
  verseSpawnableSceneById,
  verseSpawnedSceneStationForAvatar,
  VERSE_SPAWNABLE_SCENES,
  VERSE_SPAWNED_SCENE_NODE_PREFIX,
  VERSE_SPAWNED_SCENE_SOURCE_REF,
  VERSE_SPAWNED_SCENE_STATION_POSITION,
  DEFAULT_SPAWNABLE_SCENE_ID,
} from "../src/shared/verse-spawned-scene"
import { HOTBAR_SLOTS } from "../src/ui/nav"
import { initialModel, Model } from "../src/ui/model"
import { update } from "../src/ui/update"
import { interpretKey } from "../src/ui/keyboard"
import { verseSceneVisualization } from "../src/ui/view"
import { keyboardForwardDecision } from "../src/ui/subscriptions"
import { SpawnedVerseScene, ToggledVerseScenePortal, PressedKey } from "../src/ui/message"

const CRACKLING = DEFAULT_SPAWNABLE_SCENE_ID

const fromId = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${CRACKLING}:from`
const toId = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${CRACKLING}:to`
const portalId = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${CRACKLING}:portal`

describe("verseSpawnedSceneLayer (isolated, synthetic, evidence-bound)", () => {
  test("a registered scene produces a crackling_arc between two positioned endpoints", () => {
    const layer = verseSpawnedSceneLayer({
      sceneId: CRACKLING,
      generatedAt: "2026-06-22T00:00:00.000Z",
    })
    expect(layer.beams).toHaveLength(1)
    const beam = layer.beams[0]!
    expect(beam.style).toBe("crackling_arc")
    expect(beam.fromId).toBe(fromId)
    expect(beam.toId).toBe(toId)
    // Synthetic + evidence-bound: a real public ref authorizes it, but it is
    // explicitly labelled as a developer simulation (no live receipt).
    expect(beam.sourceRefs).toEqual([VERSE_SPAWNED_SCENE_SOURCE_REF])
    expect(beam.simulated).toBe(true)
    expect(beam.motionKind).toBe("crackling_energy")
    // Two positioned endpoints sit in the live world at the fixed scene station.
    expect(layer.entities).toHaveLength(2)
    expect(layer.entities.every((e) => e.position !== undefined)).toBe(true)
    // No portal by default.
    expect(layer.entities.some((e) => e.id === portalId)).toBe(false)
  })

  test("the portal toggle adds a gateway_portal entity below the arc", () => {
    const layer = verseSpawnedSceneLayer({
      sceneId: CRACKLING,
      knobs: { showPortal: true },
    })
    const portal = layer.entities.find((e) => e.id === portalId)
    expect(portal).toBeDefined()
    expect(portal?.visualKind).toBe("gateway_portal")
    expect(portal?.gatewayLane).toBe("openrouter")
  })

  test("an unknown scene id produces nothing (never fabricate a scene)", () => {
    const layer = verseSpawnedSceneLayer({ sceneId: "does-not-exist" })
    expect(layer.entities).toHaveLength(0)
    expect(layer.beams).toHaveLength(0)
    expect(verseSpawnableSceneById("does-not-exist")).toBeNull()
  })

  test("withVerseSpawnedSceneLayer forces evidence:required and is a no-op when empty", () => {
    const base = { nodes: [], entities: [], beams: [], bursts: [] } as never
    const withScene = withVerseSpawnedSceneLayer(base, [{ sceneId: CRACKLING }])
    expect(withScene.motionPolicy?.evidence).toBe("required")
    expect((withScene.beams ?? []).length).toBe(1)

    const empty = withVerseSpawnedSceneLayer(base, [])
    expect(empty).toBe(base)
    const unknownOnly = withVerseSpawnedSceneLayer(base, [{ sceneId: "nope" }])
    expect(unknownOnly).toBe(base)
  })

  test("the evidence overlay lines mirror the standalone's contract block", () => {
    const lines = verseSpawnedSceneEvidenceLines({
      sceneId: CRACKLING,
      knobs: { showPortal: true },
    })
    expect(lines.some((l) => l.includes("motionKind:") && l.includes("crackling_energy"))).toBe(true)
    expect(lines.some((l) => l.includes("simulated:") && l.includes("true"))).toBe(true)
    expect(lines.some((l) => l.includes("evidenceMode:") && l.includes("optional"))).toBe(true)
    expect(lines.some((l) => l.includes(VERSE_SPAWNED_SCENE_SOURCE_REF))).toBe(true)
    expect(lines.some((l) => l === "portal:       on")).toBe(true)
    expect(VERSE_SPAWNABLE_SCENES.length).toBeGreaterThan(0)
  })
})

describe("spawn reducer (pure model state, #6033)", () => {
  test("SpawnedVerseScene toggles a scene id in and out of the spawn list", () => {
    const [on] = update(initialModel, SpawnedVerseScene({ sceneId: CRACKLING }))
    expect(on.verseSpawnedScenes.map((s) => s.sceneId)).toEqual([CRACKLING])
    expect(on.verseSpawnedScenes[0]!.showPortal).toBe(false)
    const [off] = update(on, SpawnedVerseScene({ sceneId: CRACKLING }))
    expect(off.verseSpawnedScenes).toHaveLength(0)
  })

  test("an unknown scene id is a no-op", () => {
    const [next] = update(initialModel, SpawnedVerseScene({ sceneId: "nope" }))
    expect(next.verseSpawnedScenes).toHaveLength(0)
  })

  test("ToggledVerseScenePortal flips the portal only for a spawned scene", () => {
    // No-op while the scene is not spawned.
    const [bare] = update(initialModel, ToggledVerseScenePortal({ sceneId: CRACKLING }))
    expect(bare.verseSpawnedScenes).toHaveLength(0)

    const [on] = update(initialModel, SpawnedVerseScene({ sceneId: CRACKLING }))
    const [withPortal] = update(on, ToggledVerseScenePortal({ sceneId: CRACKLING }))
    expect(withPortal.verseSpawnedScenes[0]!.showPortal).toBe(true)
    const [offPortal] = update(withPortal, ToggledVerseScenePortal({ sceneId: CRACKLING }))
    expect(offPortal.verseSpawnedScenes[0]!.showPortal).toBe(false)
  })
})

describe("spawn keyboard intents (#6033)", () => {
  const exploreModel = Model.make({ ...initialModel, pane: "chat", verseMode: "explore" })

  test("⌘⇧E resolves to spawn-verse-scene in the Verse explore context", () => {
    const intent = interpretKey(exploreModel, {
      key: "e",
      meta: true,
      ctrl: false,
      shift: true,
      inEditable: false,
    })
    expect(intent.kind).toBe("spawn-verse-scene")
    if (intent.kind === "spawn-verse-scene") expect(intent.sceneId).toBe(CRACKLING)
  })

  test("⌘⇧P resolves to toggle-verse-scene-portal in the Verse explore context", () => {
    const intent = interpretKey(exploreModel, {
      key: "p",
      meta: true,
      ctrl: false,
      shift: true,
      inEditable: false,
    })
    expect(intent.kind).toBe("toggle-verse-scene-portal")
  })

  test("the ⌘⇧E PressedKey path actually spawns the scene", () => {
    const [next] = update(
      exploreModel,
      PressedKey({ key: "e", meta: true, ctrl: false, shift: true, inEditable: false }),
    )
    expect(next.verseSpawnedScenes.map((s) => s.sceneId)).toEqual([CRACKLING])
  })

  test("the spawn keys do not fire in code mode (coding overlay owns its keys)", () => {
    const codeModel = Model.make({ ...initialModel, pane: "chat", verseMode: "code" })
    expect(
      interpretKey(codeModel, { key: "e", meta: true, ctrl: false, shift: true, inEditable: false }).kind,
    ).not.toBe("spawn-verse-scene")
  })
})

// #6041 REGRESSION: the FULL key path, from the keyboard subscription's forward
// decision all the way to the spawned scene appearing in the world. The original
// #6041 bug shipped because only interpretKey + the reducer were tested: the
// spawn keys were never registered as input bindings, so the subscription forward
// gate (keyboardForwardDecision) resolved them to NO action ids → forward=false →
// the keydown was DROPPED before interpretKey ever ran. These tests construct the
// EXACT model state that exists WHEN THE VERSE IS ON SCREEN — pane:"chat",
// verseEnabled:true, verseMode:"explore" (see view.ts: the explore world renders
// only at `model.pane === "chat" && verseVisible(model)`) — and drive a ⌘⇧E /
// ⌘⇧P event through ALL THREE layers. They FAIL on pre-fix main (forward===false)
// and pass after the input bindings are registered.
describe("#6041 full key path: forward gate → interpretKey → reducer (the layer the original bug lived in)", () => {
  // The real on-screen Verse explore state (view.ts gate, line ~8762).
  const verseOnScreen = Model.make({
    ...initialModel,
    pane: "chat",
    verseEnabled: true,
    verseMode: "explore",
    commandPaletteOpen: false,
  })
  // Sanity: this really is the state where the Verse world is shown — the
  // explore visualization renders the walkable third-person world.
  test("the constructed model is the actual on-screen Verse explore state", () => {
    const viz = verseSceneVisualization(verseOnScreen)
    expect(viz.controller).toBe("third_person_character")
    expect(verseOnScreen.pane).toBe("chat")
    expect(verseOnScreen.verseEnabled).toBe(true)
    expect(verseOnScreen.verseMode).toBe("explore")
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

  test("⌘⇧E: subscription FORWARDS it (forward===true) — fails before the binding fix", () => {
    const decision = keyboardForwardDecision(spawnEvent)
    // This is the assertion the original bug fails: actionIds was empty so
    // forward was false and the keydown never reached interpretKey/the reducer.
    expect(decision.forward).toBe(true)
    expect(decision.preventDefault).toBe(true)
  })

  test("⌘⇧E: interpretKey maps it to spawn-verse-scene in the on-screen Verse", () => {
    const intent = interpretKey(verseOnScreen, spawnEvent)
    expect(intent.kind).toBe("spawn-verse-scene")
    if (intent.kind === "spawn-verse-scene") expect(intent.sceneId).toBe(CRACKLING)
  })

  test("⌘⇧E: full path — forwarded PressedKey spawns the scene into the world viz", () => {
    // Gate the message exactly like the subscription does, then run the reducer.
    expect(keyboardForwardDecision(spawnEvent).forward).toBe(true)
    const [next] = update(verseOnScreen, PressedKey(spawnEvent))
    expect(next.verseSpawnedScenes.map((s) => s.sceneId)).toEqual([CRACKLING])
    // And the spawned arc is actually present in the SAME world visualization.
    const viz = verseSceneVisualization(next)
    const entityIds = (viz.entities ?? []).map((e) => e.id)
    expect(entityIds).toContain(fromId)
    expect(entityIds).toContain(toId)
    expect((viz.beams ?? []).some((b) => b.fromId === fromId && b.style === "crackling_arc")).toBe(true)
  })

  test("⌘⇧P: subscription FORWARDS it (forward===true) — fails before the binding fix", () => {
    const decision = keyboardForwardDecision(portalEvent)
    expect(decision.forward).toBe(true)
    expect(decision.preventDefault).toBe(true)
  })

  test("⌘⇧P: full path — toggles the gateway portal on an already-spawned scene", () => {
    const [spawned] = update(verseOnScreen, PressedKey(spawnEvent))
    expect(keyboardForwardDecision(portalEvent).forward).toBe(true)
    const [next] = update(spawned, PressedKey(portalEvent))
    expect(next.verseSpawnedScenes[0]!.showPortal).toBe(true)
    const viz = verseSceneVisualization(next)
    const portal = (viz.entities ?? []).find((e) => e.id === portalId)
    expect(portal).toBeDefined()
    expect((portal as { visualKind?: string } | undefined)?.visualKind).toBe("gateway_portal")
  })
})

describe("verseSceneVisualization places the spawned scene in the SAME world + keeps the avatar controller (#6033)", () => {
  // The avatar's last pose: spawning must NOT move the avatar, and the
  // third-person controller must still be present so you can walk up to the scene.
  const poseModel = Model.make({
    ...initialModel,
    pane: "chat",
    verseSceneRestorePose: {
      regionRef: "world.region.tassadar",
      x: 0,
      y: 0,
      z: 0,
      yaw: 0,
      animation: "idle",
      capturedAtMs: 1,
    },
  })

  test("before spawn: no spawned-scene objects in the world visualization", () => {
    const before = verseSceneVisualization(poseModel)
    expect((before.entities ?? []).some((e) => e.id.startsWith(VERSE_SPAWNED_SCENE_NODE_PREFIX))).toBe(false)
    // The avatar is already walkable in the live world.
    expect(before.controller).toBe("third_person_character")
    expect(before.cameraMode).toBe("perspective_walk")
    expect(before.thirdPersonController).toBeDefined()
  })

  test("after spawn: the crackling arc objects appear in the SAME world visualization", () => {
    const spawned = Model.make({
      ...poseModel,
      verseSpawnedScenes: [{ sceneId: CRACKLING, showPortal: false }],
    })
    const after = verseSceneVisualization(spawned)
    const entityIds = (after.entities ?? []).map((e) => e.id)
    expect(entityIds).toContain(fromId)
    expect(entityIds).toContain(toId)
    const arc = (after.beams ?? []).find(
      (b) => b.fromId === fromId && b.style === "crackling_arc",
    )
    expect(arc).toBeDefined()
    expect(arc?.sourceRefs).toEqual([VERSE_SPAWNED_SCENE_SOURCE_REF])
    expect(arc?.simulated).toBe(true)
    // Evidence gate: the same renderer the rest of the Verse uses requires refs.
    expect(after.motionPolicy?.evidence).toBe("required")

    // The avatar/navigation is unchanged — you can walk up to it from any angle.
    expect(after.controller).toBe("third_person_character")
    expect(after.cameraMode).toBe("perspective_walk")
    expect(after.thirdPersonController).toBeDefined()
    // Spawning did not teleport the avatar's restore pose.
    expect(spawned.verseSceneRestorePose).toEqual(poseModel.verseSceneRestorePose)
  })

  test("the portal toggle adds the gateway_portal object into the live world", () => {
    const spawned = Model.make({
      ...poseModel,
      verseSpawnedScenes: [{ sceneId: CRACKLING, showPortal: true }],
    })
    const after = verseSceneVisualization(spawned)
    const portal = (after.entities ?? []).find((e) => e.id === portalId)
    expect(portal).toBeDefined()
    expect((portal as { visualKind?: string } | undefined)?.visualKind).toBe("gateway_portal")
  })
})

// #6033 fix 2: the arc must be VISIBLE. It carries dialed-up appearance knobs and
// is dropped right in front of the avatar (not at a far fixed dark station).
describe("the spawned crackling arc is dialed UP + placed in front of the avatar (#6033 visibility)", () => {
  test("the crackling_arc beam carries bright/thick appearance knobs", () => {
    const layer = verseSpawnedSceneLayer({ sceneId: CRACKLING })
    const beam = layer.beams[0]!
    const appearance = (beam as { appearance?: Record<string, number> }).appearance
    expect(appearance).toBeDefined()
    // Well above the renderer's faint default (4 strands / 0.11 jitter / 0.72).
    expect(appearance!.strandCount).toBeGreaterThanOrEqual(8)
    expect(appearance!.opacity).toBeGreaterThanOrEqual(0.9)
    expect(appearance!.jitter).toBeGreaterThan(0.2)
  })

  test("the station is dropped in front of the avatar (not the far fixed anchor)", () => {
    // Avatar at origin, yaw 0. In perspective_walk the ground plane is X/Y and
    // HEIGHT is +Z, so "forward" is +Y and "up" is +Z. Station should land
    // forward (+Y) and lifted (+Z), distinct from the fixed fallback anchor.
    const station = verseSpawnedSceneStationForAvatar({ x: 0, y: 0, z: 0, yaw: 0 })
    expect(station[1]).toBeGreaterThan(0) // forward along the ground (+Y)
    expect(station[2]).toBeGreaterThan(0) // lifted to viewing height (+Z)
    expect(station).not.toEqual(VERSE_SPAWNED_SCENE_STATION_POSITION)
  })

  test("no avatar pose ⇒ falls back to the fixed station anchor", () => {
    expect(verseSpawnedSceneStationForAvatar(null)).toEqual(
      VERSE_SPAWNED_SCENE_STATION_POSITION,
    )
  })

  test("spawning with an avatar pose positions the arc endpoints near the avatar", () => {
    const layer = verseSpawnedSceneLayer({
      sceneId: CRACKLING,
      avatar: { x: 5, y: 0, z: 5, yaw: 0 },
    })
    const from = layer.entities.find((e) => e.id === fromId)!
    // The arc hangs near the avatar (x≈5±2, z in front of z=5), not at the
    // origin-relative far station — the endpoints track the avatar.
    expect(Math.abs((from.position?.[0] ?? 0) - 5)).toBeLessThan(2.5)
  })
})

// #6033 fix 3: hotbar slots 2 (spawn scene) and 3 (toggle portal).
describe("hotbar slots 2 & 3 are wired to spawn / portal (#6033)", () => {
  const exploreModel = Model.make({
    ...initialModel,
    pane: "chat",
    verseEnabled: true,
    verseMode: "explore",
  })

  test("digit key 2 (action_bar.slot_2) resolves to spawn-verse-scene", () => {
    const intent = interpretKey(exploreModel, {
      key: "2",
      code: "Digit2",
      meta: false,
      ctrl: false,
      shift: false,
      inEditable: false,
    })
    expect(intent.kind).toBe("spawn-verse-scene")
    if (intent.kind === "spawn-verse-scene") expect(intent.sceneId).toBe(CRACKLING)
  })

  test("digit key 3 (action_bar.slot_3) resolves to toggle-verse-scene-portal", () => {
    const intent = interpretKey(exploreModel, {
      key: "3",
      code: "Digit3",
      meta: false,
      ctrl: false,
      shift: false,
      inEditable: false,
    })
    expect(intent.kind).toBe("toggle-verse-scene-portal")
  })

  test("digit key 1 still opens a coder session (slot 1 unchanged)", () => {
    const intent = interpretKey(exploreModel, {
      key: "1",
      code: "Digit1",
      meta: false,
      ctrl: false,
      shift: false,
      inEditable: false,
    })
    expect(intent.kind).toBe("open-coder-session")
  })

  test("the digit-2 full path spawns the scene into the world viz", () => {
    expect(
      keyboardForwardDecision({
        key: "2",
        code: "Digit2",
        meta: false,
        ctrl: false,
        shift: false,
        inEditable: false,
      }).forward,
    ).toBe(true)
    const [next] = update(
      exploreModel,
      PressedKey({
        key: "2",
        code: "Digit2",
        meta: false,
        ctrl: false,
        shift: false,
        inEditable: false,
      }),
    )
    expect(next.verseSpawnedScenes.map((s) => s.sceneId)).toEqual([CRACKLING])
  })

  test("the hotbar exposes slots 2 (scene) and 3 (portal) as filled, icon-bearing slots", () => {
    const slot2 = HOTBAR_SLOTS.find((s) => s.number === 2)!
    const slot3 = HOTBAR_SLOTS.find((s) => s.number === 3)!
    expect(slot2.filled).toBe(true)
    expect(slot2.iconName).toBeDefined()
    expect(slot3.filled).toBe(true)
    expect(slot3.iconName).toBeDefined()
    // Slot 1 (coder) stays filled; slots 4..10 stay empty.
    expect(HOTBAR_SLOTS.find((s) => s.number === 1)!.filled).toBe(true)
    expect(HOTBAR_SLOTS.find((s) => s.number === 4)!.filled).toBeUndefined()
  })
})
