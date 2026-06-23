// Headless render entry: the SPAWNED crackling arc, through the REAL render path.
//
// This mounts the real `oa-training-run` three-effect element with the
// visualization produced by the REAL `withVerseSpawnedSceneLayer` mapper (the
// same function `verseSceneVisualization` uses for a spawned scene). It is built
// into a browser bundle and loaded by the headless-pixel harness, which then
// advances deterministic frames and screenshots.
//
// A query param `?broken=1` produces the Mode-2 failure: the beam is STILL in
// the model, but its evidence `sourceRefs` are stripped. The renderer's
// `evidence: "required"` gate then suppresses the arc, so it renders NOTHING —
// exactly the "model says it's there, screen shows nothing" bug. The regression
// asserts the fixed variant lights up bright pixels and the broken one does not.

import {
  registerTrainingRunElement,
  trainingRunTagName,
} from "@openagentsinc/three-effect/foldkit"
import {
  trainingRunVisualizationOptionsFromSnapshot,
  type TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import {
  withVerseSpawnedSceneLayer,
  DEFAULT_SPAWNABLE_SCENE_ID,
} from "../src/shared/verse-spawned-scene.js"

registerTrainingRunElement()

// A minimal, deterministic base world (mirrors the training-scene smoke seed,
// trimmed). The arc is positioned at the fixed scene station and aimed at the
// camera-facing band so it lights the upper-mid region of the frame.
const baseVisualization: TrainingRunVisualizationOptions =
  trainingRunVisualizationOptionsFromSnapshot({
    activeWindowCount: 0,
    assignedContributorCount: 0,
    blockerRefCount: 0,
    closeoutSatisfied: false,
    deviceObserved: 0,
    deviceRequired: 0,
    externalStatus: "observed",
    finalValidationLoss: 3,
    freivaldsRefCount: 0,
    gradientCloseoutRefCount: 0,
    lifecycleCounts: {
      active: 0,
      qualified: 0,
      registered: 0,
      state_synced: 0,
      sync_reentry: 0,
      warmup: 0,
    },
    maxAllowedStaleSteps: 5,
    maxValidationLoss: 3.5,
    operatorSignals: [],
    pendingPayoutCount: 0,
    plannedWindowCount: 0,
    promiseSignals: [],
    receiptRefCount: 0,
    reconciledWindowCount: 0,
    rejectedWorkCount: 0,
    runDetail: "render-harness.crackling",
    runLabel: "render-harness.crackling",
    runState: "active",
    sealInFlight: false,
    sealedWindowCount: 0,
    settledPayoutSats: 0,
    verifiedWorkCount: 0,
  })

// Render in the REAL perspective_walk / third-person frame (the frame the live
// Verse uses), with an avatar pose, so the spawned-scene layer's scene-world →
// root-local conversion is exercised exactly as it is in the app. (The avatar is
// the controller's default spawn; the layer drops the arc in front of it.)
const avatar = { x: 0, y: 0, z: 4.4, yaw: 0 } as const
const walkableBase: TrainingRunVisualizationOptions = {
  ...baseVisualization,
  cameraMode: "perspective_walk",
  controller: "third_person_character",
  thirdPersonController: {
    character: { walkSpeed: 3.8, runSpeed: 6.7 },
    initialPosition: [avatar.x, avatar.y, avatar.z],
  },
}

// The REAL spawned-scene render path: a crackling_arc beam evidence-bound to its
// synthetic source ref, with motionPolicy.evidence = "required".
const spawned = withVerseSpawnedSceneLayer(walkableBase, [
  {
    sceneId: DEFAULT_SPAWNABLE_SCENE_ID,
    avatar,
    generatedAt: "2026-06-22T00:00:00.000Z",
  },
])

const broken = new URLSearchParams(globalThis.location?.search ?? "").get(
  "broken",
)

// Broken variant: strip the beams' evidence refs so the renderer's
// evidence:required gate suppresses them. The beam is STILL in the model.
const visualization: TrainingRunVisualizationOptions =
  broken === "1"
    ? {
        ...spawned,
        beams: (spawned.beams ?? []).map((beam) => ({
          ...beam,
          sourceRefs: [],
        })),
      }
    : spawned

const host = document.createElement(trainingRunTagName) as HTMLElement & {
  visualization: TrainingRunVisualizationOptions
}
host.id = "crackling-scene"
host.style.display = "block"
host.style.width = "960px"
host.style.height = "540px"
host.style.minHeight = "540px"
host.visualization = visualization
document.getElementById("scene")?.append(host)
