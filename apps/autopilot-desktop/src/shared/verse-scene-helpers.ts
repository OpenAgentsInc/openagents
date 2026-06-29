import { metaverseStreetLayout } from "@openagentsinc/three-effect/core"
import type {
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunEntityDefinition,
  TrainingRunNodeDefinition,
  TrainingRunRemoteAvatarDefinition,
  TrainingRunVector,
  TrainingRunVisualizationOptions,
  TrainingRunWorldItemDefinition,
} from "@openagentsinc/three-effect/core"

export type VerseVisualizationAppend = Readonly<{
  beams?: readonly TrainingRunBeamDefinition[]
  bursts?: readonly TrainingRunBurstDefinition[]
  entities?: readonly TrainingRunEntityDefinition[]
  nodes?: readonly TrainingRunNodeDefinition[]
  remoteAvatars?: readonly TrainingRunRemoteAvatarDefinition[]
  worldItems?: readonly TrainingRunWorldItemDefinition[]
}>

export const uniqueVerseStrings = (
  values: ReadonlyArray<string | null | undefined>,
): string[] => {
  const out: string[] = []
  for (const value of values) {
    const trimmed = typeof value === "string" ? value.trim() : ""
    if (trimmed.length === 0 || out.includes(trimmed)) continue
    out.push(trimmed)
  }
  return out
}

export const compactVerseLines = (
  values: ReadonlyArray<string | null | undefined>,
): readonly string[] => uniqueVerseStrings(values)

export const finitePositiveVerseNumber = (
  value: number | null | undefined,
): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0

export const roundVerseNumber = (value: number, decimals = 3): number =>
  Number.isFinite(value) ? Number(value.toFixed(decimals)) : 0

export const roundedVerseVector = (
  vector: readonly [number, number, number],
  decimals = 3,
): TrainingRunVector => [
  roundVerseNumber(vector[0], decimals),
  roundVerseNumber(vector[1], decimals),
  roundVerseNumber(vector[2], decimals),
]

// ── The perspective_walk world `root` transform (the no-particle-effect bug) ───
//
// In perspective_walk the renderer (three-effect trainingRun.ts) builds the world
// in TWO frames: the avatar + camera live in SCENE-WORLD (the third-person
// controller's position, what `verseSceneRestorePose` captures), while entity /
// beam positions are added to a `root` group that is SCALED by `tassadarSceneScale`,
// ROTATED -90° about X, and TRANSLATED to the Tassadar street lot (`tassadarLotX/Z`).
// So an entity position is interpreted in ROOT-LOCAL space, NOT scene-world.
//
// Forward root transform (local → world):
//   wx = s·lx + lotX ; wy = s·lz ; wz = -s·ly + lotZ
// Inverse (world → local), what an in-world effect needs to place an entity at a
// given scene-world point (e.g. in front of the avatar):
//   lx = (wx - lotX)/s ; ly = -(wz - lotZ)/s ; lz = wy/s
//
// Placing a scene-world avatar pose AS IF it were root-local (the original bug)
// pushes the arc ~5 units into the air and off to the lot, outside the camera.
export const VERSE_ROOT_SCALE = metaverseStreetLayout.tassadarSceneScale
export const VERSE_ROOT_OFFSET_X = metaverseStreetLayout.tassadarLotX
export const VERSE_ROOT_OFFSET_Z = metaverseStreetLayout.tassadarLotZ

export const verseSceneWorldToRootLocal = (
  world: TrainingRunVector,
): TrainingRunVector =>
  roundedVerseVector([
    (world[0] - VERSE_ROOT_OFFSET_X) / VERSE_ROOT_SCALE,
    -(world[2] - VERSE_ROOT_OFFSET_Z) / VERSE_ROOT_SCALE,
    world[1] / VERSE_ROOT_SCALE,
  ])

// The three-effect verse host (`trainingRunView` / the foldkit `oa-training-run`
// element) RECONCILES beams, gateway portals, bursts, and their endpoint markers
// on every live `updateVisualization` (add new, remove gone, rebuild moved) WITHOUT
// a full remount, so a beam/entity/portal added AFTER mount — the hotbar-2
// "crackling energy" spawn, the slot-3 gateway portal, the local Khala arc —
// renders in place and the avatar/camera/controller are never rebuilt. The old
// motionPolicy-fingerprint remount hack (#6054) that forced a scene rebuild on
// every spawn has been removed; the consumer just hands the host the updated
// visualization.

export const appendVerseVisualization = (
  base: TrainingRunVisualizationOptions,
  append: VerseVisualizationAppend,
): TrainingRunVisualizationOptions => ({
  ...base,
  ...(append.nodes === undefined
    ? {}
    : { nodes: [...(base.nodes ?? []), ...append.nodes] }),
  ...(append.entities === undefined
    ? {}
    : { entities: [...(base.entities ?? []), ...append.entities] }),
  ...(append.worldItems === undefined
    ? {}
    : { worldItems: [...(base.worldItems ?? []), ...append.worldItems] }),
  ...(append.remoteAvatars === undefined
    ? {}
    : {
        remoteAvatars: [
          ...(base.remoteAvatars ?? []),
          ...append.remoteAvatars,
        ],
      }),
  ...(append.beams === undefined
    ? {}
    : { beams: [...(base.beams ?? []), ...append.beams] }),
  ...(append.bursts === undefined
    ? {}
    : { bursts: [...(base.bursts ?? []), ...append.bursts] }),
})
