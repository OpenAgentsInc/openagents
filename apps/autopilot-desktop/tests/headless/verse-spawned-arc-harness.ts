// Headless harness page entry for the #6033 pixel-proof.
//
// Mounts the EXACT crackling-arc beam the desktop emits — built straight from the
// SHARED scene layer (`withVerseSpawnedSceneLayer` / `verseSpawnedSceneLayer`),
// the same module `verseSceneVisualization` calls, with the same avatar-derived
// station, the same `motionPolicy.evidence:"required"` gate, and the same
// perspective-walk / third-person camera — through three-effect's
// `mountTrainingRunVisualization`, in a real browser.
//
// NOTE on scope: we deliberately do NOT import the desktop `view.ts`/
// `verseSceneVisualization` here. That module pulls in `@stylexjs/stylex`, which
// throws at runtime unless compiled by its Babel plugin (verified: it errors on
// mount in a plain browser bundle). The shared scene layer is the SAME code that
// produces the beam in the real app — this harness renders that beam faithfully,
// which is exactly the sanctioned fallback in the task: prove the crackling_arc
// beam renders bright pixels through `mountTrainingRunVisualization`.
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

// The avatar's last pose. The trainingRun renderer translates the world root by
// the tassadar street-lot offset (≈ x 5.8, z 0.8) before the -90°X rotation, so
// to frame the arc dead-centre (as it is in the full app, where the avatar
// spawns on the lot) we place the harness avatar at the negative lot offset and
// pull it back a little so the station+arc lands centred and in view. The
// spawned-scene layer drops the arc station IN FRONT of this pose (+Y forward,
// +Z height), exactly like an in-world spawn the owner walks up to.
const avatar = { x: -5.8, y: -2.6, z: -0.8, yaw: 0 } as const

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
