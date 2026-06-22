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
  VERSE_SPAWNABLE_SCENES,
  VERSE_SPAWNED_SCENE_NODE_PREFIX,
  VERSE_SPAWNED_SCENE_SOURCE_REF,
  DEFAULT_SPAWNABLE_SCENE_ID,
} from "../src/shared/verse-spawned-scene"
import { initialModel, Model } from "../src/ui/model"
import { update } from "../src/ui/update"
import { interpretKey } from "../src/ui/keyboard"
import { verseSceneVisualization } from "../src/ui/view"
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
