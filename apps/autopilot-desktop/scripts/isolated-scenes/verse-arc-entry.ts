// Isolated scene entry for #6033: the evidence-bound Khala crackling arc.

import {
  trainingRunVisualizationOptionsFromSnapshot,
  type TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import {
  DEFAULT_SPAWNABLE_SCENE_ID,
  withVerseSpawnedSceneLayer,
} from "../../src/shared/verse-spawned-scene.js"

import { mountTrainingRunIsolatedScene } from "./mount-training-run-scene.js"

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

const spawned = withVerseSpawnedSceneLayer(walkableBase, [
  {
    sceneId: DEFAULT_SPAWNABLE_SCENE_ID,
    avatar,
    generatedAt: "2026-06-22T00:00:00.000Z",
  },
])

const broken = new URLSearchParams(globalThis.location?.search ?? "").get("broken")

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

mountTrainingRunIsolatedScene("verse-arc", visualization)
