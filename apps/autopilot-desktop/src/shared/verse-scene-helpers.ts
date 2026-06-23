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

// A compact, stable fingerprint of the visualization's DYNAMICALLY-RENDERED
// EFFECT content: every beam (endpoints + style) and burst (anchor), plus the
// resolved position/status of ONLY the entities those beams/bursts reference.
//
// WHY THIS EXISTS (the no-particle-effect bug): the three-effect verse host
// (`trainingRunView` + the foldkit `oa-training-run` element) only builds the
// crackling-arc beams, payout bursts, and their endpoint markers ONCE, at mount
// time. Its live update path (`updateVisualization`) reconciles remote avatars
// and world items but NOT beams/bursts/entities, and its retained-vs-remount
// decision (`trainingRunVisualizationRetainedStructuralSignature`) deliberately
// ignores those arrays. So a beam added AFTER mount — the hotbar-2 "crackling
// energy" spawn or the local Khala arc — is treated as non-structural, the scene
// is kept, and the new beam is silently dropped: it never reaches the scene graph
// and nothing renders.
//
// `verseSceneVisualization` stamps this fingerprint onto a structural-signature
// field so any change to the effect content flips the host's structural signature
// and forces a clean REMOUNT (which rebuilds the beams). The local avatar pose is
// preserved across remount by the host, so the rebuild is seamless.
//
// SCOPE: only beams/bursts and their referenced endpoints are fingerprinted — not
// the whole base-world entity set — so unrelated base-scene churn (pylons, etc.)
// never triggers a remount. No beams/bursts ⇒ a stable empty fingerprint, so a
// Verse with no spawned/local effect is byte-identical to before (no remounts).
export const verseSceneContentFingerprint = (
  options: TrainingRunVisualizationOptions,
): string => {
  const beams = options.beams ?? []
  const bursts = options.bursts ?? []
  if (beams.length === 0 && bursts.length === 0) return ""
  const round = (value: number): number => roundVerseNumber(value, 2)
  // The endpoint ids any effect references (so moving/adding an arc endpoint, or
  // a verified turn's burst anchor, re-fingerprints; base entities do not).
  const referenced = new Set<string>()
  for (const beam of beams) {
    referenced.add(beam.fromId)
    referenced.add(beam.toId)
  }
  for (const burst of bursts) referenced.add(burst.atId)
  const endpoints = (options.entities ?? [])
    .filter((entity) => referenced.has(entity.id))
    .map((entity) => {
      const position = entity.position
      const at =
        position === undefined
          ? "ring"
          : `${round(position[0])},${round(position[1])},${round(position[2])}`
      return `${entity.id}@${at}#${entity.status}:${entity.visualKind ?? ""}`
    })
    .sort()
  const beamKeys = beams
    .map((beam) => `${beam.fromId}->${beam.toId}:${beam.style ?? "flow"}`)
    .sort()
  const burstKeys = bursts.map((burst) => burst.atId).sort()
  return JSON.stringify({ endpoints, beams: beamKeys, bursts: burstKeys })
}

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
