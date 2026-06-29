// Headless harness page entry for the #6033 pixel-proof.
//
// Mounts the EXACT crackling-arc beam the desktop emits — built straight from the
// SHARED scene layer (`withVerseSpawnedSceneLayer` / `verseSpawnedSceneLayer`),
// the same module `verseSceneVisualization` calls, with the same avatar-derived
// station, the same `motionPolicy.evidence:"required"` gate, and the same
// perspective-walk / third-person camera — through three-effect's
// `mountTrainingRunVisualization`, in a real browser.
//
// NOTE on scope: we render straight from the SHARED scene layer rather than the
// desktop `view.ts`. Historically `view.ts` could not be imported here because
// it pulled in `@stylexjs/stylex`, whose `stylex.create(...)` threw at runtime
// unless compiled by its Babel plugin. #6046 removed StyleX entirely, so
// `view.ts` now mounts headless without that throw (see
// `tests/stylex-removal-headless-mount.test.ts`); this harness still uses the
// shared scene layer because that is the SAME code producing the beam in the
// real app and keeps the pixel proof focused on the scene, not the full shell.
//
// A query flag (?spawn=0) renders the identical world WITHOUT the scene so the
// capture script can prove the arc is what adds the bright pixels.

import { Effect } from "effect"

import {
  mountTrainingRunVisualization,
  type TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import { withVerseSpawnedSceneLayer } from "../../src/shared/verse-spawned-scene"

const params = new URLSearchParams(globalThis.location?.search ?? "")
const spawn = params.get("spawn") !== "0"

const mount = document.getElementById("scene")
if (mount === null) throw new Error("missing #scene mount")

// The avatar's last pose, in SCENE-WORLD (the third-person controller frame) —
// the third-person controller's default spawn. The spawned-scene layer now
// converts this scene-world pose to root-local internally (it drops the arc
// station IN FRONT of the avatar at chest height, then inverts the world `root`
// transform), so the harness avatar is a plain in-world pose, NOT a hand-tuned
// negative-lot-offset compensation as it was before the coordinate-frame fix.
const avatar = { x: 0, y: 0, z: 4.4, yaw: 0 } as const

// A minimal walkable base world, then the SHARED spawned-scene layer (the exact
// call verseSceneVisualization makes). The layer adds the crackling_arc beam +
// its endpoints and forces evidence:"required" — identical to the app.
const base: TrainingRunVisualizationOptions = {
  nodes: [],
  entities: [],
  beams: [],
  bursts: [],
  cameraMode: "perspective_walk",
  controller: "third_person_character",
  thirdPersonController: {
    character: { walkSpeed: 3.8, runSpeed: 6.7 },
    initialPosition: [avatar.x, avatar.y, avatar.z],
  },
}

const visualization = withVerseSpawnedSceneLayer(
  base,
  spawn ? [{ sceneId: "crackling-energy", avatar }] : [],
)

const arcBeamCount = (visualization.beams ?? []).filter(
  (beam) => beam.style === "crackling_arc",
).length

const handle = Effect.runSync(mountTrainingRunVisualization(mount, visualization))

;(globalThis as unknown as { __verseArcScene?: unknown }).__verseArcScene = {
  mounted: handle !== undefined,
  spawn,
  arcBeamCount,
}
